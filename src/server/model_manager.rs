//! Local-first model installation, validation and lifecycle. No inference backend here.
use crate::models::Model;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::{ffi::OsStrExt, fs::DirBuilderExt, io::AsRawFd},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
type Result<T> = std::result::Result<T, String>;
pub const NAMES: [&str; 4] = ["encoder.onnx", "decoder.onnx", "joiner.onnx", "tokens.txt"];
const MAX_FILE: u64 = 512 * 1024 * 1024;
const MAX_MANIFEST: u64 = 65536;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    pub role: String,
    pub path: String,
    pub sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub language: String,
    pub model_type: String,
    pub format: String,
    pub backend: String,
    pub version: String,
    pub license: String,
    pub files: Vec<ModelFile>,
    pub tail_samples: usize,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}
impl Manifest {
    pub fn compatible(&self) -> bool {
        self.backend == "sherpa-onnx-transducer"
            && self.format == "onnx-transducer"
            && matches!(self.model_type.as_str(), "zipformer" | "zipformer2")
            && matches!(self.language.as_str(), "de" | "de-DE")
    }
    fn validate(&self, id: &str) -> Result<()> {
        if self.schema_version != 1
            || self.id != id
            || self.display_name.is_empty()
            || self.version.is_empty()
            || self.license.is_empty()
            || self.language.is_empty()
            || self.backend.is_empty()
            || self.model_type.is_empty()
            || self.format.is_empty()
            || self.files.is_empty()
            || self.files.len() > 32
            || !(320..=160000).contains(&self.tail_samples)
            || !self.tail_samples.is_multiple_of(320)
        {
            return Err("Invalid model.json: schema_version=1, matching id, non-empty metadata/files and tail_samples=320..160000 (multiple of 320) required".into());
        }
        let mut paths = HashSet::new();
        let mut roles = HashSet::new();
        for f in &self.files {
            relative(&f.path)?;
            if f.path == "model.json"
                || f.path == "installation.json"
                || f.path.starts_with('.')
                || f.role.is_empty()
                || !paths.insert(&f.path)
                || !roles.insert(&f.role)
            {
                return Err("Invalid/duplicate/reserved model file path or role".into());
            }
            if let Some(h) = &f.sha256 {
                if h.len() != 64
                    || !h
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                {
                    return Err("SHA256 must be 64 lower-case hex characters".into());
                }
            }
        }
        if self.compatible()
            && (self.files.len() != 4
                || ["encoder", "decoder", "joiner", "tokens"]
                    .iter()
                    .any(|r| !roles.contains(&r.to_string())))
        {
            return Err(
                "sherpa-onnx-transducer requires exactly encoder, decoder, joiner and tokens roles"
                    .into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Package {
    name: String,
    sha256: String,
    size: u64,
    url: String,
}
fn package(model: &Model) -> Option<Package> {
    let (n, sha, size) = match model {
        Model::Community64 => (
            64,
            "0013d4b3e1216b4e0c15f18f345aa58ca8bb2cdbeff7a7d55a513f6b400862a5",
            155839333,
        ),
        Model::Community128 => (
            128,
            "8d39babf998aba69446b1c1e3ac780bf5f90887ad95b0d60eb4bd79d63998462",
            155836296,
        ),
        _ => return None,
    };
    let name = format!("Kroko-DE-Community-{n}-L-Streaming-001.data");
    Some(Package {
        url: format!(
            "https://huggingface.co/Banafo/Kroko-ASR/resolve/{}/{}",
            model.revision(),
            name
        ),
        name,
        sha256: sha.into(),
        size,
    })
}
fn built_in(model: &Model) -> Manifest {
    Manifest {
        schema_version: 1,
        id: model.key().into(),
        display_name: format!("Kroko {}", model.key()),
        language: "de".into(),
        model_type: "zipformer2".into(),
        format: "onnx-transducer".into(),
        backend: "sherpa-onnx-transducer".into(),
        version: model.revision().into(),
        license: if *model == Model::Classic {
            "Unresolved Classic model license"
        } else {
            "CC-BY-SA (version unspecified by provider)"
        }
        .into(),
        files: NAMES
            .iter()
            .zip(model.hashes())
            .map(|(n, h)| ModelFile {
                role: n.split('.').next().unwrap().into(),
                path: n.to_string(),
                sha256: Some(h.into()),
            })
            .collect(),
        tail_samples: model.tail_samples(),
        description: None,
        source: Some("https://huggingface.co/Banafo/Kroko-ASR".into()),
    }
}
fn relative(value: &str) -> Result<()> {
    let p = Path::new(value);
    if value.is_empty()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
        || (value.contains('\\') || value.contains('\0'))
    {
        return Err("Model paths must be relative, without traversal or backslashes".into());
    }
    Ok(())
}
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
fn safe_path(base: &Path, relative_path: &str) -> Result<PathBuf> {
    relative(relative_path)?;
    let mut p = base.to_path_buf();
    for c in Path::new(relative_path).components() {
        p.push(c);
        if let Ok(m) = fs::symlink_metadata(&p) {
            if m.file_type().is_symlink() {
                return Err(format!(
                    "Refusing symlink {} (use real files inside MODEL_DIR)",
                    p.display()
                ));
            }
        }
    }
    Ok(p)
}
fn locate(root: &Path, model: &Model) -> Result<PathBuf> {
    let p = safe_path(root, model.key())?;
    if exists(&p) {
        return Ok(p);
    }
    if *model == Model::Classic {
        let legacy = safe_path(root, "sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06")?;
        if exists(&legacy) {
            return Ok(legacy);
        }
        if NAMES.iter().any(|n| exists(&root.join(n))) {
            return Ok(root.to_path_buf());
        }
    }
    Ok(p)
}
pub fn selection(root: &Path, explicit: Option<&str>) -> Result<Model> {
    if let Some(value) = explicit {
        return Model::parse(value);
    }
    // Preserve local installations without an explicit environment setting.
    for model in [Model::Classic, Model::Community128, Model::Community64] {
        if exists(&locate(root, &model)?) {
            return Ok(model);
        }
    }
    Ok(Model::Community128)
}
fn manifest(dir: &Path, model: &Model) -> Result<Manifest> {
    if !matches!(model, Model::Custom(_)) {
        return Ok(built_in(model));
    }
    let path = safe_path(dir, "model.json")?;
    let bytes = read_bounded(&path, MAX_MANIFEST)?;
    let value: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid {}: {e}", path.display()))?;
    value.validate(model.key())?;
    Ok(value)
}
fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>> {
    let m = fs::metadata(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    if !m.is_file() || m.len() == 0 || m.len() > max {
        return Err(format!(
            "{} must be a nonempty regular file of at most {max} bytes",
            path.display()
        ));
    }
    let mut data = Vec::new();
    File::open(path)
        .and_then(|f| f.take(max + 1).read_to_end(&mut data))
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > max {
        return Err("File grew beyond its limit".into());
    }
    Ok(data)
}
fn hash(path: &Path) -> Result<String> {
    let mut f = File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let mut h = Sha256::new();
    let mut block = [0u8; 65536];
    loop {
        let n = f.read(&mut block).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&block[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn checksum(path: &Path, expected: &str) -> Result<()> {
    let actual = hash(path)?;
    if actual != expected {
        return Err(format!("SHA256 mismatch for {}: expected {expected}, got {actual}. Existing files were NOT replaced; move this model aside for inspection or explicitly remove/reinstall it.",path.display()));
    }
    Ok(())
}
fn varint(file: &mut File, end: u64) -> Result<u64> {
    let mut n = 0;
    for shift in (0..70).step_by(7) {
        if file.stream_position().map_err(|e| e.to_string())? >= end {
            return Err("Truncated ONNX protobuf".into());
        }
        let mut b = [0];
        file.read_exact(&mut b).map_err(|e| e.to_string())?;
        if shift == 63 && b[0] > 1 {
            return Err("ONNX varint overflow".into());
        }
        n |= ((b[0] & 127) as u64) << shift;
        if b[0] < 128 {
            return Ok(n);
        }
    }
    Err("ONNX varint overflow".into())
}
fn onnx_envelope(path: &Path) -> Result<()> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let end = f.metadata().map_err(|e| e.to_string())?.len();
    let mut graph = false;
    let mut ir = false;
    let mut opset = false;
    while f.stream_position().map_err(|e| e.to_string())? < end {
        let tag = varint(&mut f, end)?;
        if tag >> 3 == 0 {
            return Err("Invalid ONNX protobuf tag".into());
        }
        let skip = match tag & 7 {
            0 => {
                let value = varint(&mut f, end)?;
                if tag >> 3 == 1 {
                    ir = value > 0;
                }
                0
            }
            1 => 8,
            5 => 4,
            2 => {
                let n = varint(&mut f, end)?;
                if tag >> 3 == 7 {
                    graph = n > 0;
                }
                if tag >> 3 == 8 {
                    opset = n > 0;
                }
                n
            }
            _ => return Err("Unsupported ONNX protobuf wire type".into()),
        };
        let pos = f.stream_position().map_err(|e| e.to_string())?;
        if skip > end - pos {
            return Err("Truncated ONNX field".into());
        }
        f.seek(SeekFrom::Current(skip as i64))
            .map_err(|e| e.to_string())?;
    }
    if !ir || !graph || !opset {
        return Err(format!(
            "Invalid ONNX envelope {}: IR, graph and opset required; not a full ONNX graph check",
            path.display()
        ));
    }
    Ok(())
}
fn tokens(path: &Path) -> Result<()> {
    let data = read_bounded(path, 4 * 1024 * 1024)?;
    let text = std::str::from_utf8(&data).map_err(|_| "tokens must be UTF-8")?;
    let mut ids = HashSet::new();
    let mut symbols = HashSet::new();
    let mut blank = false;
    for line in text.lines() {
        let (symbol, id) = line
            .rsplit_once(char::is_whitespace)
            .ok_or("tokens.txt needs symbol + integer id on each line")?;
        let id: u32 = id.trim().parse().map_err(|_| "Invalid token id")?;
        if symbol.is_empty() || !ids.insert(id) || !symbols.insert(symbol) {
            return Err("Duplicate/empty token or token id".into());
        }
        blank |= id == 0 && symbol == "<blk>";
    }
    if !blank || ids.len() < 3 {
        return Err("tokens.txt requires <blk> 0 and at least three symbols".into());
    }
    Ok(())
}
fn verify_files(dir: &Path, m: &Manifest) -> Result<()> {
    for f in &m.files {
        let path = safe_path(dir, &f.path)?;
        let meta=fs::metadata(&path).map_err(|e|format!("Missing/unreadable required file {}: {e}. No files were changed. Supply all model files locally; no automatic repair.",path.display()))?;
        if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_FILE {
            return Err(format!(
                "Invalid model file {}: empty, not regular or larger than 512 MiB",
                path.display()
            ));
        }
        if let Some(h) = &f.sha256 {
            checksum(&path, h)?;
        }
        if m.compatible() {
            if f.role == "tokens" {
                tokens(&path)?;
            } else {
                onnx_envelope(&path)?;
            }
        }
    }
    Ok(())
}
pub struct Lease(File);
impl Lease {
    fn acquire(dir: &Path, exclusive: bool) -> Result<Self> {
        let file = File::open(dir)
            .map_err(|e| format!("Cannot open model directory {}: {e}", dir.display()))?;
        let operation = if exclusive {
            libc::LOCK_EX
        } else {
            libc::LOCK_SH
        };
        if unsafe { libc::flock(file.as_raw_fd(), operation | libc::LOCK_NB) } != 0 {
            return Err(format!("Model/storage busy: {}. Stop the process using this model before installation/removal, then retry.",dir.display()));
        }
        Ok(Self(file))
    }
    fn shared(&self) -> Result<()> {
        if unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) } != 0 {
            return Err("Cannot obtain shared model lease".into());
        }
        Ok(())
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
pub struct Resolved {
    pub directory: PathBuf,
    pub manifest: Manifest,
    pub lease: Lease,
}
fn component_dir(dir: &Path, model: &Model) -> Result<PathBuf> {
    if NAMES.iter().any(|n| exists(&dir.join(n))) || matches!(model, Model::Custom(_)) {
        return Ok(dir.to_path_buf());
    }
    if let Some(p) = package(model) {
        let cache = safe_path(dir, &format!(".runtime/{}", p.sha256))?;
        if exists(&cache) {
            return Ok(cache);
        }
    }
    Ok(dir.to_path_buf())
}
fn source(dir: &Path) -> &'static str {
    let p = dir.join("installation.json");
    if let Ok(bytes) = read_bounded(&p, MAX_MANIFEST) {
        if let Ok(v) = serde_json::from_slice::<Value>(&bytes) {
            if v["source"] == "download" {
                return "download";
            }
        }
    }
    "local"
}
pub fn status(root: &Path, model: &Model) -> Value {
    let inspect = || -> Result<Value> {
        let dir = locate(root, model)?;
        if !exists(&dir) {
            return Ok(
                json!({"model":model.key(),"present":false,"valid":false,"source":"local","directory":dir,"reason":"Model is not installed"}),
            );
        }
        let _lease = Lease::acquire(&dir, false)?;
        let m = manifest(&dir, model)?;
        let components = component_dir(&dir, model)?;
        if components == dir
            && !matches!(model, Model::Custom(_))
            && !NAMES.iter().any(|n| exists(&dir.join(n)))
        {
            if let Some(p) = package(model) {
                let path = safe_path(&dir, &p.name)?;
                if exists(&path) {
                    verify_package(&path, &p, &m, None)?;
                    return Ok(
                        json!({"model":model.key(),"present":true,"valid":true,"source":"local","format":"kroko-data","version":m.version,"sha256":p.sha256,"runtime_compatible":true,"needs_preparation":true,"directory":dir,"license":m.license}),
                    );
                }
            }
        }
        verify_files(&components, &m)?;
        Ok(
            json!({"model":model.key(),"present":true,"valid":true,"source":if source(&components)=="download" {"download"}else{source(&dir)},"format":m.format,"version":m.version,"files":m.files,"runtime_compatible":m.compatible(),"runtime_check":"declared backend + envelope/hash validation; native graph loading occurs at server startup","needs_preparation":false,"directory":components,"license":m.license}),
        )
    };
    inspect().unwrap_or_else(|error|json!({"model":model.key(),"present":true,"valid":false,"error":error,"source":"local"}))
}
fn u32_le(f: &mut File) -> Result<u64> {
    let mut b = [0; 4];
    f.read_exact(&mut b).map_err(|_| "Truncated .data length")?;
    Ok(u32::from_le_bytes(b) as u64)
}
fn verify_package(path: &Path, p: &Package, m: &Manifest, output: Option<&Path>) -> Result<()> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size != p.size {
        return Err(format!(
            "Incomplete/invalid .data {}: expected {} bytes, got {size}; no automatic replacement",
            path.display(),
            p.size
        ));
    }
    checksum(path, &p.sha256)?;
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let n = u32_le(&mut f)?;
    if n == 0 || n > 65536 || n > size - 4 {
        return Err("Invalid .data JSON length".into());
    }
    let mut header = vec![0; n as usize];
    f.read_exact(&mut header).map_err(|e| e.to_string())?;
    let h: Value = serde_json::from_slice(&header).map_err(|_| "Invalid .data JSON header")?;
    if h["free"] != true || h["type"] != "zipformer2" || h["language"]["iso"] != "DE" {
        return Err("Only unencrypted free German Zipformer2 packages are supported".into());
    }
    for spec in &m.files {
        let n = u32_le(&mut f)?;
        let pos = f.stream_position().map_err(|e| e.to_string())?;
        if n == 0 || n > MAX_FILE || n > size.saturating_sub(pos) {
            return Err("Invalid/truncated .data block".into());
        }
        let mut out = match output {
            Some(d) => Some(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(d.join(&spec.path))
                    .map_err(|e| e.to_string())?,
            ),
            None => None,
        };
        let mut hash = Sha256::new();
        let mut remaining = n;
        let mut block = [0u8; 65536];
        while remaining > 0 {
            let count = block.len().min(remaining as usize);
            f.read_exact(&mut block[..count])
                .map_err(|e| e.to_string())?;
            hash.update(&block[..count]);
            if let Some(o) = out.as_mut() {
                o.write_all(&block[..count]).map_err(|e| e.to_string())?;
            }
            remaining -= count as u64;
        }
        if Some(format!("{:x}", hash.finalize())) != spec.sha256 {
            return Err(format!(".data component hash mismatch: {}", spec.role));
        }
        if let Some(o) = out {
            o.sync_all().map_err(|e| e.to_string())?;
        }
    }
    if f.stream_position().map_err(|e| e.to_string())? != size {
        return Err("Trailing .data bytes".into());
    }
    Ok(())
}
struct EmptyDirectory(PathBuf);
impl Drop for EmptyDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}
struct Stage(PathBuf);
impl Stage {
    fn new(parent: &Path, label: &str) -> Result<Self> {
        let mut random = [0; 16];
        File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut random))
            .map_err(|e| e.to_string())?;
        let nonce = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let path = parent.join(format!(
            ".{label}-{}-{}-{nonce}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).map_err(|e|format!("Cannot create staging directory in {}: {e}. Use a writable persistent /models volume owned by the container user (99:100); do not replace existing models.",parent.display()))?;
        Ok(Self(path))
    }
}
impl Stage {
    fn publish(&self, to: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        publish(&self.0, to)
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        if exists(&self.0) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn publish(from: &Path, to: &Path) -> Result<()> {
    let a = CString::new(from.as_os_str().as_bytes()).map_err(|_| "Invalid path")?;
    let b = CString::new(to.as_os_str().as_bytes()).map_err(|_| "Invalid path")?;
    if unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            a.as_ptr(),
            libc::AT_FDCWD,
            b.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(format!(
            "Atomic installation refused for {}: {}. Nothing was overwritten.",
            to.display(),
            std::io::Error::last_os_error()
        ));
    }
    File::open(to.parent().ok_or("No parent")?)
        .and_then(|d| d.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn download(p: &Package, to: &Path) -> Result<()> {
    eprintln!(
        "{}",
        json!({"event":"model_download","source":p.url,"bytes":p.size,"sha256":p.sha256,"license":"Community CC-BY-SA; provider did not specify a version"})
    );
    let agent = ureq::AgentBuilder::new()
        .https_only(true)
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(60))
        .timeout(Duration::from_secs(1800))
        .build();
    let response = agent
        .get(&p.url)
        .call()
        .map_err(|e| format!("Official model download failed: {e}"))?;
    save_download(response.into_reader(), p, to)
}
fn save_download(mut reader: impl Read, p: &Package, to: &Path) -> Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(|e| e.to_string())?;
    let copied = std::io::copy(&mut reader.by_ref().take(p.size + 1), &mut f)
        .map_err(|e| format!("Interrupted model download: {e}"))?;
    if copied != p.size {
        return Err(format!(
            "Incomplete/oversized download: expected {}, received {copied}",
            p.size
        ));
    }
    f.sync_all().map_err(|e| e.to_string())?;
    checksum(to, &p.sha256)
}
fn missing(dir: &Path, model: &Model) -> String {
    format!(
        "Model {} is missing at {}. {} Automatic repair/overwrite is disabled.",
        model.key(),
        dir.display(),
        if let Some(p) = package(model) {
            format!("Copy {} here, or enable MODEL_AUTO_DOWNLOAD=true with writable persistent storage.",p.name)
        } else if *model == Model::Classic {
            format!("Classic is local-only (license unresolved): supply {} here; the historical named export directory and flat MODEL_DIR layout are also detected. No Classic download will be attempted.",NAMES.join(", "))
        } else {
            "Supply model.json and the files it declares. Custom downloads are never automatic."
                .into()
        }
    )
}
pub fn prepare(root: &Path, model: &Model, allow_download: bool) -> Result<Resolved> {
    prepare_with(root, model, allow_download, &download)
}
fn prepare_with(
    root: &Path,
    model: &Model,
    allow_download: bool,
    fetch: &dyn Fn(&Package, &Path) -> Result<()>,
) -> Result<Resolved> {
    // Complete local models need neither writes nor an Internet connection.
    if root.is_dir() {
        let root_read = Lease::acquire(root, false)?;
        let dir = locate(root, model)?;
        if exists(&dir) {
            let lease = Lease::acquire(&dir, false)?;
            let m = manifest(&dir, model)?;
            let components = component_dir(&dir, model)?;
            if components != dir
                || NAMES.iter().any(|n| exists(&dir.join(n)))
                || matches!(model, Model::Custom(_))
            {
                verify_files(&components, &m)?;
                if !m.compatible() {
                    return Err(format!("Model {} is locally valid but backend/format is not supported by this server; no new runtime is installed",m.id));
                }
                return Ok(Resolved {
                    directory: components,
                    manifest: m,
                    lease,
                });
            }
            drop(lease);
            drop(root_read);
            if let Some(p) = package(model) {
                let pkg = safe_path(&dir, &p.name)?;
                if exists(&pkg) {
                    let _root_write = Lease::acquire(root, true)?;
                    let lease = Lease::acquire(&dir, true)?;
                    verify_package(&pkg, &p, &m, None)?;
                    let runtime = safe_path(&dir, ".runtime")?;
                    if !exists(&runtime) {
                        fs::create_dir(&runtime).map_err(|e|format!("A local .data package needs one-time byte-exact preparation: {e}; make this model directory writable or supply the four prepared files"))?;
                    }
                    let target = runtime.join(&p.sha256);
                    if !exists(&target) {
                        let stage = Stage::new(&runtime, "prepare")?;
                        verify_package(&pkg, &p, &m, Some(&stage.0))?;
                        verify_files(&stage.0, &m)?;
                        stage.publish(&target)?;
                    }
                    verify_files(&target, &m)?;
                    lease.shared()?;
                    return Ok(Resolved {
                        directory: target,
                        manifest: m,
                        lease,
                    });
                }
            }
            // Empty model directories contain no weights to replace. Keep the
            // directory itself and publish a complete bundle under its cache.
            if fs::read_dir(&dir)
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
            {
                return Err(format!("Incomplete/unrecognized model directory {}. No download or replacement performed; {}",dir.display(),missing(&dir,model)));
            }
            if !allow_download || package(model).is_none() {
                return Err(missing(&dir, model));
            }
            let _root_write = Lease::acquire(root, true)?;
            let lease = Lease::acquire(&dir, true)?;
            if fs::read_dir(&dir)
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
            {
                return Err("Model changed concurrently; retry without replacing files".into());
            }
            let p = package(model).unwrap();
            let runtime = dir.join(".runtime");
            fs::create_dir(&runtime).map_err(|e| e.to_string())?;
            let _empty_cleanup = EmptyDirectory(runtime.clone());
            let target = runtime.join(&p.sha256);
            install_bundle(&runtime, &target, &p, &m, fetch)?;
            lease.shared()?;
            return Ok(Resolved {
                directory: target,
                manifest: m,
                lease,
            });
        }
    }
    let dir = locate(root, model)?;
    if !allow_download || package(model).is_none() {
        return Err(missing(&dir, model));
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let root_write = Lease::acquire(root, true)?;
    if exists(&dir) {
        return Err(
            "Model appeared concurrently; nothing overwritten. Retry startup to validate it."
                .into(),
        );
    }
    let p = package(model).unwrap();
    let m = built_in(model);
    install_bundle(root, &dir, &p, &m, fetch)?;
    let lease = Lease::acquire(&dir, false)?;
    drop(root_write);
    Ok(Resolved {
        directory: dir,
        manifest: m,
        lease,
    })
}
fn install_bundle(
    parent: &Path,
    target: &Path,
    p: &Package,
    m: &Manifest,
    fetch: &dyn Fn(&Package, &Path) -> Result<()>,
) -> Result<()> {
    let stage = Stage::new(parent, "install")?;
    let pkg = stage.0.join(&p.name);
    fetch(p, &pkg)?;
    verify_package(&pkg, p, m, Some(&stage.0))?;
    verify_files(&stage.0, m)?;
    let record = json!({"source":"download","url":p.url,"revision":m.version,"sha256":p.sha256,"model":m.id,"license":m.license,"preparation":"byte-exact unpacking, no graph conversion"});
    let mut record_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(stage.0.join("installation.json"))
        .map_err(|e| e.to_string())?;
    record_file
        .write_all(record.to_string().as_bytes())
        .and_then(|_| record_file.sync_all())
        .map_err(|e| e.to_string())?;
    stage.publish(target)
}

pub fn list(root: &Path) -> Vec<Value> {
    let mut models = vec![Model::Community128, Model::Community64, Model::Classic];
    if let Ok(entries) = fs::read_dir(root.join("custom")) {
        for e in entries.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if let Ok(m) = Model::parse(&format!("custom/{name}")) {
                    models.push(m);
                }
            }
        }
    }
    models.iter().map(|m| status(root, m)).collect()
}
pub fn remove(root: &Path, model: &Model) -> Result<()> {
    let _root = Lease::acquire(root, true)?;
    let dir = locate(root, model)?;
    if !exists(&dir) {
        return Err("Selected model does not exist; nothing removed".into());
    }
    if dir == root {
        return Err("Removal of a shared flat legacy MODEL_DIR is refused. Move only its four model files into a dedicated classic directory first; no Appdata cleanup attempted.".into());
    }
    let _lease = Lease::acquire(&dir, true)?;
    let mut allowed: HashSet<PathBuf> = HashSet::new();
    if matches!(model, Model::Custom(_)) {
        let m = manifest(&dir, model)?;
        allowed.insert(PathBuf::from("model.json"));
        for f in m.files {
            allowed.insert(PathBuf::from(f.path));
        }
    } else {
        allowed.extend(NAMES.iter().map(|n| PathBuf::from(*n)));
        allowed.insert(PathBuf::from("installation.json"));
        if let Some(p) = package(model) {
            allowed.insert(PathBuf::from(&p.name));
            for n in NAMES {
                allowed.insert(PathBuf::from(format!(".runtime/{}/{n}", p.sha256)));
            }
            for n in [p.name.as_str(), "installation.json"] {
                allowed.insert(PathBuf::from(format!(".runtime/{}/{n}", p.sha256)));
            }
        }
    }
    fn walk(dir: &Path, base: &Path, allowed: &HashSet<PathBuf>) -> Result<()> {
        let name = CString::new(dir.as_os_str().as_bytes()).map_err(|_| "Invalid removal path")?;
        if unsafe {
            libc::faccessat(
                libc::AT_FDCWD,
                name.as_ptr(),
                libc::W_OK | libc::X_OK,
                libc::AT_EACCESS,
            )
        } != 0
        {
            return Err(format!("Removal refused before changing files: directory {} is not writable: {}. Grant the operator access to this model only, then retry.",dir.display(),std::io::Error::last_os_error()));
        }
        for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let rel = path.strip_prefix(base).map_err(|e| e.to_string())?;
            let typ = e.file_type().map_err(|e| e.to_string())?;
            if typ.is_symlink() {
                return Err("Removal refused: model contains a symlink; no files removed".into());
            }
            if typ.is_dir() {
                if !allowed.iter().any(|p| p.starts_with(rel)) {
                    return Err(format!(
                        "Removal refused: unrelated directory {}",
                        path.display()
                    ));
                }
                walk(&path, base, allowed)?;
            } else if !typ.is_file() || !allowed.contains(rel) {
                return Err(format!("Removal refused: unowned file {}. Move unrelated files out first; no files removed.",path.display()));
            }
        }
        Ok(())
    }
    walk(&dir, &dir, &allowed)?;
    let quarantine = Stage::new(root, "remove")?;
    let target = quarantine.0.join("model");
    publish(&dir, &target)?;
    fs::remove_dir_all(&target)
        .map_err(|e| format!("Removal incomplete at {}: {e}", target.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> Stage {
        Stage::new(&std::env::temp_dir(), "model-manager-test").unwrap()
    }
    fn digest(data: &[u8]) -> String {
        format!("{:x}", Sha256::digest(data))
    }
    fn custom(dir: &Path, id: &str, backend: &str) -> Manifest {
        fs::create_dir_all(dir).unwrap();
        let mut m = built_in(&Model::Classic);
        m.id = id.into();
        m.display_name = "Local German model".into();
        m.version = "test-v1".into();
        m.license = "Test only".into();
        m.backend = backend.into();
        // Minimal protobuf envelope only; these fixtures never reach native inference.
        for spec in &mut m.files {
            let data = if spec.role == "tokens" {
                b"<blk> 0\n<unk> 1\na 2\n".to_vec()
            } else {
                vec![8, 8, 58, 2, 16, 1, 66, 2, 16, 13]
            };
            fs::write(dir.join(&spec.path), &data).unwrap();
            spec.sha256 = Some(digest(&data));
        }
        fs::write(dir.join("model.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        m
    }
    #[test]
    fn ids_and_paths_cannot_escape_storage() {
        for name in [
            "custom/../classic",
            "custom/a/b",
            "custom/.",
            "custom/",
            "classic/..",
            "/classic",
        ] {
            assert!(Model::parse(name).is_err());
        }
        for path in ["../x", "/x", "a/../b", "a\\b", ""] {
            assert!(relative(path).is_err());
        }
    }
    #[test]
    fn default_preserves_classic_and_new_installs_choose_128() {
        let r = root();
        assert_eq!(selection(&r.0, None).unwrap(), Model::Community128);
        fs::create_dir(r.0.join("classic")).unwrap();
        assert_eq!(selection(&r.0, None).unwrap(), Model::Classic);
        fs::create_dir(r.0.join("community-128")).unwrap();
        assert_eq!(selection(&r.0, None).unwrap(), Model::Classic);
        assert_eq!(selection(&r.0, Some("classic")).unwrap(), Model::Classic);
        assert_eq!(
            selection(&r.0, Some("community-128")).unwrap(),
            Model::Community128
        );
    }
    #[test]
    fn complete_local_custom_never_calls_downloader_and_survives_restart() {
        let r = root();
        custom(
            &r.0.join("custom/demo"),
            "custom/demo",
            "sherpa-onnx-transducer",
        );
        let m = Model::parse("custom/demo").unwrap();
        for _ in 0..2 {
            let p =
                prepare_with(&r.0, &m, true, &|_, _| panic!("network must not be used")).unwrap();
            assert_eq!(p.manifest.id, "custom/demo");
        }
        assert_eq!(status(&r.0, &m)["valid"], true);
    }
    #[test]
    fn unsupported_custom_can_be_verified_but_not_started() {
        let r = root();
        custom(
            &r.0.join("custom/future"),
            "custom/future",
            "future-backend",
        );
        let m = Model::parse("custom/future").unwrap();
        assert_eq!(status(&r.0, &m)["valid"], true);
        assert_eq!(status(&r.0, &m)["runtime_compatible"], false);
        assert!(prepare(&r.0, &m, false).is_err());
    }
    #[test]
    fn invalid_partial_and_hash_mismatch_never_download_or_overwrite() {
        let r = root();
        let dir = r.0.join("community-128");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("encoder.onnx"), b"keep me").unwrap();
        assert!(
            prepare_with(&r.0, &Model::Community128, true, &|_, _| panic!(
                "no download for partial local model"
            ))
            .is_err()
        );
        assert_eq!(fs::read(dir.join("encoder.onnx")).unwrap(), b"keep me");
        let dir = r.0.join("custom/test");
        custom(&dir, "custom/test", "sherpa-onnx-transducer");
        fs::write(dir.join("decoder.onnx"), b"broken").unwrap();
        assert!(prepare(&r.0, &Model::parse("custom/test").unwrap(), false).is_err());
        assert_eq!(fs::read(dir.join("decoder.onnx")).unwrap(), b"broken");
    }
    #[test]
    fn classic_missing_never_downloads() {
        let r = root();
        let error = prepare_with(&r.0, &Model::Classic, true, &|_, _| {
            panic!("Classic must remain local-only")
        })
        .err()
        .unwrap();
        assert!(error.contains("Classic is local-only") && error.contains("encoder.onnx"));
        assert!(!exists(&r.0.join("classic")));
    }
    #[test]
    fn failed_download_into_empty_profile_leaves_no_partial_installation() {
        let r = root();
        let dir = r.0.join("community-128");
        fs::create_dir(&dir).unwrap();
        assert!(prepare_with(&r.0, &Model::Community128, true, &|_, _| Err(
            "interrupted fixture".into()
        ))
        .is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        assert_eq!(status(&r.0, &Model::Community128)["valid"], false);
    }
    #[test]
    fn traversal_and_symlinks_are_rejected() {
        let r = root();
        let dir = r.0.join("custom/evil");
        let mut m = custom(&dir, "custom/evil", "sherpa-onnx-transducer");
        m.files[0].path = "../../classic/encoder.onnx".into();
        fs::write(dir.join("model.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(
            status(&r.0, &Model::parse("custom/evil").unwrap())["valid"],
            false
        );
        std::os::unix::fs::symlink("/tmp", r.0.join("community-128")).unwrap();
        assert!(prepare(&r.0, &Model::Community128, true).is_err());
    }
    #[test]
    fn remove_is_scoped_and_refuses_unknown_files_and_active_models() {
        let r = root();
        fs::write(r.0.join("config.txt"), b"keep configuration").unwrap();
        let a = r.0.join("custom/a");
        custom(&a, "custom/a", "sherpa-onnx-transducer");
        let b = r.0.join("custom/b");
        custom(&b, "custom/b", "sherpa-onnx-transducer");
        let m = Model::parse("custom/a").unwrap();
        let active = prepare(&r.0, &m, false).unwrap();
        assert!(remove(&r.0, &m).is_err());
        drop(active);
        fs::write(a.join("private-config"), b"keep").unwrap();
        assert!(remove(&r.0, &m).is_err());
        assert!(a.join("private-config").exists());
        fs::remove_file(a.join("private-config")).unwrap();
        remove(&r.0, &m).unwrap();
        assert!(!a.exists());
        assert_eq!(
            status(&r.0, &Model::parse("custom/b").unwrap())["valid"],
            true
        );
        assert_eq!(
            fs::read(r.0.join("config.txt")).unwrap(),
            b"keep configuration"
        );
    }
    fn synthetic() -> (Vec<u8>, Package, Manifest) {
        let r = root();
        let mut m = custom(&r.0, "community-128", "sherpa-onnx-transducer");
        m.id = "community-128".into();
        let h = br#"{"free":true,"type":"zipformer2","language":{"iso":"DE"}}"#;
        let mut bytes = (h.len() as u32).to_le_bytes().to_vec();
        bytes.extend(h);
        for spec in &m.files {
            let data = fs::read(r.0.join(&spec.path)).unwrap();
            bytes.extend((data.len() as u32).to_le_bytes());
            bytes.extend(data);
        }
        let p = Package {
            name: "original.data".into(),
            sha256: digest(&bytes),
            size: bytes.len() as u64,
            url: "https://huggingface.co/test-fixture-only".into(),
        };
        (bytes, p, m)
    }
    #[test]
    fn staged_download_is_atomic_and_preserves_original_bytes() {
        let r = root();
        let (bytes, p, m) = synthetic();
        let target = r.0.join("community-128");
        install_bundle(&r.0, &target, &p, &m, &|p, path| {
            save_download(bytes.as_slice(), p, path)
        })
        .unwrap();
        verify_files(&target, &m).unwrap();
        assert_eq!(fs::read(target.join(&p.name)).unwrap(), bytes);
        assert!(
            install_bundle(&r.0, &target, &p, &m, &|p, path| save_download(
                bytes.as_slice(),
                p,
                path
            ))
            .is_err()
        );
        assert_eq!(fs::read(target.join(&p.name)).unwrap(), bytes);
    }
    #[test]
    fn interrupted_and_wrong_hash_download_never_publishes_model() {
        let r = root();
        let (bytes, p, m) = synthetic();
        let target = r.0.join("community-128");
        assert!(
            install_bundle(&r.0, &target, &p, &m, &|p, path| save_download(
                &bytes[..bytes.len() / 2],
                p,
                path
            ))
            .is_err()
        );
        assert!(!target.exists());
        assert_eq!(fs::read_dir(&r.0).unwrap().count(), 0);
        let mut bad = bytes.clone();
        bad[10] ^= 1;
        assert!(
            install_bundle(&r.0, &target, &p, &m, &|p, path| save_download(
                bad.as_slice(),
                p,
                path
            ))
            .is_err()
        );
        assert!(!target.exists());
    }
    #[test]
    fn malformed_package_and_bad_onnx_are_rejected() {
        let r = root();
        let (mut bytes, mut p, m) = synthetic();
        bytes.push(0);
        p.size = bytes.len() as u64;
        p.sha256 = digest(&bytes);
        let file = r.0.join("bad.data");
        fs::write(&file, &bytes).unwrap();
        assert!(verify_package(&file, &p, &m, None).is_err());
        let file = r.0.join("bad.onnx");
        fs::write(&file, b"not an ONNX graph").unwrap();
        assert!(onnx_envelope(&file).is_err());
    }
}
