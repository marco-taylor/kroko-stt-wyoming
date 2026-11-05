//! Local-first model installation, validation and lifecycle. No inference backend here.
use crate::models::Model;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
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
    #[serde(default = "default_feature_dim")]
    pub feature_dim: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_language: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}
fn default_feature_dim() -> i32 { 80 }
impl Manifest {
    pub fn compatible(&self) -> bool {
        self.backend == "sherpa-onnx-transducer"
            && self.format == "onnx-transducer"
            && matches!(self.model_type.as_str(), "zipformer" | "zipformer2" | "nemo_transducer")
            && crate::models::normalize_language(&self.language).is_ok()
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
        if self.model_type == "nemo_transducer" {
            let language = self.stream_language.as_deref().ok_or("NeMo requires explicit stream_language, for example de-DE")?;
            if self.feature_dim != 128 || crate::models::normalize_language(language)? != crate::models::normalize_language(&self.language)? {
                return Err("NeMo feature_dim must be 128 and stream_language must match manifest language".into());
            }
        } else if self.feature_dim != 80 || self.stream_language.is_some() {
            return Err("Existing Zipformer profiles require feature_dim=80 and no NeMo stream_language".into());
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
    let entry = model.catalog_entry()?;
    Some(Package {
        name: entry.filename.clone(),
        sha256: entry.sha256.clone(),
        size: entry.bytes,
        url: format!(
            "https://huggingface.co/Banafo/Kroko-ASR/resolve/{}/{}",
            model.revision(),
            entry.filename
        ),
    })
}

fn built_in(model: &Model) -> Manifest {
    Manifest {
        schema_version: 1,
        id: model.key().into(),
        display_name: format!("Kroko {}", model.key()),
        language: model.language().into(),
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
                sha256: if h.is_empty() { None } else { Some(h.into()) },
            })
            .collect(),
        tail_samples: model.tail_samples(),
        feature_dim: 80,
        stream_language: None,
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
    let p = safe_path(root, &model.identity())?;
    if exists(&p) {
        return Ok(p);
    }
    if model.language() != "de" {
        return Ok(p);
    }
    let old = safe_path(root, model.key())?;
    if exists(&old) {
        return Ok(old);
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
    selection_for(root, "de", explicit)
}
pub fn selection_for(root: &Path, language: &str, explicit: Option<&str>) -> Result<Model> {
    if let Some(value) = explicit {
        return Model::parse(value)?.for_language(language);
    }
    for base in [Model::Classic, Model::Community128, Model::Community64] {
        if let Ok(model) = base.for_language(language) {
            if exists(&locate(root, &model)?) {
                return Ok(model);
            }
        }
    }
    Model::Community128.for_language(language)
}

fn manifest(dir: &Path, model: &Model) -> Result<Manifest> {
    let mut path = safe_path(dir, "model.json")?;
    if !model.is_custom() && !exists(&path) {
        if let Some(p) = package(model) {
            let cached = safe_path(dir, &format!(".runtime/{}/model.json", p.sha256))?;
            if exists(&cached) {
                path = cached;
            }
        }
    }
    if !model.is_custom() && !exists(&path) {
        let m = built_in(model);
        if model.language() != "de" && NAMES.iter().any(|n| exists(&dir.join(n))) {
            return Err("Prepared non-legacy model requires verified installation manifest; use the official package or an explicit custom manifest".into());
        }
        return Ok(m);
    }
    let bytes = read_bounded(&path, MAX_MANIFEST)?;
    let mut value: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid {}: {e}", path.display()))?;
    value.validate(model.key())?;
    value.language = crate::models::normalize_language(&value.language)?;
    if value.language != model.language() {
        return Err("Manifest language does not match LANGUAGE; no cross-language fallback".into());
    }
    if !model.is_custom() {
        let expected = built_in(model);
        if value.backend != expected.backend
            || value.format != expected.format
            || value.model_type != expected.model_type
            || value.version != expected.version
        {
            return Err(
                "Built-in installation manifest is incompatible with pinned catalogue".into(),
            );
        }
        for f in &expected.files {
            let v = value
                .files
                .iter()
                .find(|v| v.role == f.role)
                .ok_or("Missing built-in model role")?;
            if v.path != f.path
                || v.sha256.is_none()
                || (f.sha256.is_some() && v.sha256 != f.sha256)
            {
                return Err(
                    "Built-in component identity/hash differs from verified installation".into(),
                );
            }
        }
        if let Some(p) = package(model) {
            let record: Value = serde_json::from_slice(&read_bounded(
                &path.parent().unwrap().join("installation.json"),
                MAX_MANIFEST,
            )?)
            .map_err(|_| "Invalid installation record")?;
            // v0.2.0 wrote German-only installation records without a language
            // field. Their pinned package SHA and component hashes still bind
            // them to German. Never accept a missing language for other models.
            let language_matches = record["language"] == model.language()
                || (model.language() == "de" && record.get("language").is_none());
            if record["sha256"] != p.sha256
                || !language_matches
                || record["model"] != model.key()
            {
                return Err(
                    "Installation record does not match LANGUAGE + MODEL + pinned package".into(),
                );
            }
            let encoder = value
                .files
                .iter()
                .find(|s| s.role == "encoder")
                .ok_or("Missing encoder")?;
            let encoder = safe_path(path.parent().unwrap(), &encoder.path)?;
            let meta = fs::metadata(&encoder).map_err(|e| e.to_string())?;
            if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_FILE {
                return Err("Invalid encoder file".into());
            }
            let mut f = File::open(encoder).map_err(|e| e.to_string())?;
            let size = f.metadata().map_err(|e| e.to_string())?.len();
            if value.tail_samples != streaming_tail(&mut f, size)? {
                return Err("Installed tail differs from actual encoder metadata".into());
            }
        }
    }
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
fn varint(file: &mut (impl Read + Seek), end: u64) -> Result<u64> {
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
fn onnx_metadata(f: &mut File, end: u64) -> Result<HashMap<String,String>> {
    // Read only model-level metadata; seek over the graph and tensor payloads.
    let mut metadata = HashMap::new();
    while f.stream_position().map_err(|e| e.to_string())? < end {
        let tag = varint(f, end)?;
        let n = match tag & 7 {
            0 => {
                varint(f, end)?;
                continue;
            }
            1 => 8,
            5 => 4,
            2 => varint(f, end)?,
            _ => return Err("Invalid ONNX metadata wire type".into()),
        };
        let pos = f.stream_position().map_err(|e| e.to_string())?;
        if n > end.saturating_sub(pos) {
            return Err("Truncated ONNX metadata".into());
        }
        if tag == (14 << 3 | 2) {
            if n > MAX_MANIFEST {
                return Err("Oversized ONNX metadata entry".into());
            }
            let mut bytes = vec![0; n as usize];
            f.read_exact(&mut bytes).map_err(|e| e.to_string())?;
            let mut c = std::io::Cursor::new(bytes);
            let mut entry = HashMap::new();
            while c.position() < n {
                let t = varint(&mut c, n)?;
                if !matches!(t, 10 | 18) {
                    return Err("Invalid ONNX metadata entry".into());
                }
                let size = varint(&mut c, n)?;
                if size > n.saturating_sub(c.position()) {
                    return Err("Truncated metadata string".into());
                }
                let mut value = vec![0; size as usize];
                c.read_exact(&mut value).map_err(|e| e.to_string())?;
                if entry
                    .insert(
                        t,
                        String::from_utf8(value).map_err(|_| "Non-UTF8 ONNX metadata")?,
                    )
                    .is_some()
                {
                    return Err("Duplicate metadata field".into());
                }
            }
            let key = entry.remove(&10).ok_or("Missing ONNX metadata key")?;
            let value = entry.remove(&18).ok_or("Missing ONNX metadata value")?;
            if metadata.insert(key, value).is_some() {
                return Err("Duplicate ONNX metadata key".into());
            }
        } else {
            f.seek(SeekFrom::Current(n as i64))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(metadata)
}
fn streaming_tail(f: &mut File, end: u64) -> Result<usize> {
    let metadata = onnx_metadata(f,end)?;
    let number = |name: &str| -> Result<usize> {
        metadata
            .get(name)
            .ok_or_else(|| format!("Missing streaming metadata {name}"))?
            .parse()
            .map_err(|_| format!("Invalid streaming metadata {name}"))
    };
    let step = number("decode_chunk_len")?;
    let context = number("T")?;
    if metadata.get("model_type").map(String::as_str) != Some("zipformer2")
        || metadata.get("version").map(String::as_str) != Some("1")
        || !matches!(step, 128 | 256)
        || context != step + 13
    {
        return Err("Unsupported streaming export metadata: supply an explicit custom manifest, no parameters are inferred from filenames".into());
    }
    // Existing validated tail policy for these specific export shapes.
    Ok((step + 16) * 160)
}
fn package_tail(path: &Path, language: &str) -> Result<usize> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let size = f.metadata().map_err(|e| e.to_string())?.len();
    free_package_header(&mut f, size, language)?;
    let length = u32_le(&mut f)?;
    let start = f.stream_position().map_err(|e| e.to_string())?;
    if length == 0 || length > MAX_FILE || length > size.saturating_sub(start) {
        return Err("Invalid encoder block".into());
    }
    streaming_tail(&mut f, start + length)
}
fn verified_manifest(dir: &Path, m: &Manifest, hashes: Vec<String>) -> Result<Manifest> {
    let mut result = m.clone();
    for (spec, actual) in result.files.iter_mut().zip(hashes) {
        spec.sha256 = Some(actual);
    }
    let encoder = result
        .files
        .iter()
        .find(|s| s.role == "encoder")
        .ok_or("Missing encoder")?;
    let mut f = File::open(dir.join(&encoder.path)).map_err(|e| e.to_string())?;
    let size = f.metadata().map_err(|e| e.to_string())?.len();
    result.tail_samples = streaming_tail(&mut f, size)?;
    verify_files(dir, &result)?;
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("model.json"))
        .map_err(|e| e.to_string())?;
    out.write_all(&serde_json::to_vec(&result).map_err(|e| e.to_string())?)
        .and_then(|_| out.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(result)
}
fn tokens(path: &Path, expected_blank: Option<u32>) -> Result<usize> {
    let data = read_bounded(path, 4 * 1024 * 1024)?;
    let text = std::str::from_utf8(&data).map_err(|_| "tokens must be UTF-8")?;
    let mut ids = HashSet::new();
    let mut symbols = HashSet::new();
    let mut blank = None;
    for line in text.lines() {
        let (symbol, id) = line
            .rsplit_once(char::is_whitespace)
            .ok_or("tokens.txt needs symbol + integer id on each line")?;
        let id: u32 = id.trim().parse().map_err(|_| "Invalid token id")?;
        if symbol.is_empty() || !ids.insert(id) || !symbols.insert(symbol) {
            return Err("Duplicate/empty token or token id".into());
        }
        if symbol == "<blk>" { blank=Some(id); }
    }
    let required=expected_blank.unwrap_or(ids.len().saturating_sub(1) as u32);
    if blank != Some(required) || ids.len() < 3 || (expected_blank.is_none() && ids.iter().any(|i|*i as usize>=ids.len())) {
        return Err("tokens.txt requires the model-family blank ID and at least three unique symbols (NeMo: contiguous IDs, blank last; Zipformer: blank 0)".into());
    }
    Ok(ids.len())
}
fn verify_files(dir: &Path, m: &Manifest) -> Result<()> {
    for f in &m.files {
        let path = safe_path(dir, &f.path)?;
        let meta=fs::metadata(&path).map_err(|e|format!("Missing/unreadable required file {}: {e}. No files were changed. Supply all model files locally; no automatic repair.",path.display()))?;
        let limit = if m.model_type == "nemo_transducer" { 1024 * 1024 * 1024 } else { MAX_FILE };
        if !meta.is_file() || meta.len() == 0 || meta.len() > limit {
            return Err(format!(
                "Invalid model file {}: empty, not regular or larger than the family limit",
                path.display()
            ));
        }
        if let Some(h) = &f.sha256 {
            checksum(&path, h)?;
        }
        if m.compatible() {
            if f.role == "tokens" {
                tokens(&path, if m.model_type=="nemo_transducer" {None} else {Some(0)})?;
            } else {
                onnx_envelope(&path)?;
            }
        }
    }
    if m.model_type == "nemo_transducer" {
        let encoder=m.files.iter().find(|f|f.role=="encoder").ok_or("Missing encoder")?;
        let mut f=File::open(safe_path(dir,&encoder.path)?).map_err(|e|e.to_string())?;
        let size=f.metadata().map_err(|e|e.to_string())?.len();
        let meta=onnx_metadata(&mut f,size)?;
        validate_nemo_metadata(&meta,m)?;
        let vocab=meta.get("vocab_size").and_then(|v|v.parse::<u32>().ok()).ok_or("Missing/invalid NeMo vocabulary metadata")?;
        let token_file=m.files.iter().find(|s|s.role=="tokens").ok_or("Missing tokens")?;
        let count=tokens(&safe_path(dir,&token_file.path)?,Some(vocab))?;
        if count as u64 != vocab as u64+1 {return Err("NeMo tokens count differs from encoder vocabulary metadata".into());}
    }
    Ok(())
}
fn validate_nemo_metadata(meta:&HashMap<String,String>,m:&Manifest)->Result<()> {
    if meta.get("model_type").map(String::as_str)!=Some("EncDecRNNTBPEModelWithPrompt") || meta.get("feat_dim").map(String::as_str)!=Some("128") || meta.get("version").map(String::as_str)!=Some("1") {
        return Err("Unsupported NeMo encoder metadata; expected the verified multilingual Nemotron export, feature_dim=128".into());
    }
    let prompts:HashMap<String,i64>=serde_json::from_str(meta.get("prompt_dictionary").ok_or("Missing NeMo language prompts")?).map_err(|_|"Invalid NeMo prompt dictionary")?;
    let language=m.stream_language.as_deref().ok_or("Missing NeMo stream_language")?;
    if !prompts.iter().any(|(key,id)|key.eq_ignore_ascii_case(language) && *id>=0) {
        return Err("NeMo stream_language is not present in encoder prompt_dictionary; refusing automatic language fallback".into());
    }
    for name in ["window_size","chunk_shift","subsampling_factor"] {
        if meta.get(name).and_then(|v|v.parse::<u64>().ok()).filter(|v|*v>0).is_none() {return Err(format!("Invalid NeMo metadata {name}"));}
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
    if NAMES.iter().any(|n| exists(&dir.join(n))) || model.is_custom() {
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
                json!({"model":model.key(),"language":model.language(),"identity":model.identity(),"present":false,"valid":false,"source":"local","directory":dir,"reason":"Model is not installed"}),
            );
        }
        let _lease = Lease::acquire(&dir, false)?;
        let m = manifest(&dir, model)?;
        let components = component_dir(&dir, model)?;
        if components == dir && !model.is_custom() && !NAMES.iter().any(|n| exists(&dir.join(n))) {
            if let Some(p) = package(model) {
                let path = safe_path(&dir, &p.name)?;
                if exists(&path) {
                    verify_package(&path, &p, &m, None)?;
                    return Ok(
                        json!({"model":model.key(),"language":model.language(),"identity":model.identity(),"present":true,"valid":true,"source":"local","format":"kroko-data","version":m.version,"sha256":p.sha256,"runtime_compatible":true,"needs_preparation":true,"directory":dir,"license":m.license}),
                    );
                }
            }
        }
        verify_files(&components, &m)?;
        Ok(
            json!({"model":model.key(),"language":model.language(),"identity":model.identity(),"present":true,"valid":true,"source":if source(&components)=="download" {"download"}else{source(&dir)},"format":m.format,"version":m.version,"files":m.files,"runtime_compatible":m.compatible(),"runtime_check":"declared backend + envelope/hash validation; native graph loading occurs at server startup","needs_preparation":false,"directory":components,"license":m.license}),
        )
    };
    inspect().unwrap_or_else(|error|json!({"model":model.key(),"language":model.language(),"identity":model.identity(),"present":true,"valid":false,"error":error,"source":"local"}))
}
fn u32_le(f: &mut File) -> Result<u64> {
    let mut b = [0; 4];
    f.read_exact(&mut b).map_err(|_| "Truncated .data length")?;
    Ok(u32::from_le_bytes(b) as u64)
}
fn free_package_header(f: &mut File, size: u64, language: &str) -> Result<()> {
    let n = u32_le(f)?;
    if n == 0 || n > MAX_MANIFEST || n > size.saturating_sub(4) {
        return Err("Invalid .data JSON length".into());
    }
    let mut header = vec![0; n as usize];
    f.read_exact(&mut header)
        .map_err(|_| "Truncated .data JSON header")?;
    let h: Value = serde_json::from_slice(&header).map_err(|_| "Invalid .data JSON header")?;
    if h["free"] != true {
        return Err("Unsupported package: .data preparation requires an explicit boolean free=true header. No files were changed.".into());
    }
    if h["type"] != "zipformer2"
        || h["language"]["iso"]
            .as_str()
            .and_then(|v| crate::models::normalize_language(v).ok())
            .as_deref()
            != Some(language)
    {
        return Err(
            "Only free Zipformer2 packages matching the selected LANGUAGE are supported".into(),
        );
    }
    Ok(())
}
fn verify_package(
    path: &Path,
    p: &Package,
    m: &Manifest,
    output: Option<&Path>,
) -> Result<Vec<String>> {
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
    free_package_header(&mut f, size, &m.language)?;
    let mut hashes = Vec::new();
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
        let actual = format!("{:x}", hash.finalize());
        if let Some(expected) = &spec.sha256 {
            if &actual != expected {
                return Err(format!(".data component hash mismatch: {}", spec.role));
            }
        }
        hashes.push(actual);
        if let Some(o) = out {
            o.sync_all().map_err(|e| e.to_string())?;
        }
    }
    if f.stream_position().map_err(|e| e.to_string())? != size {
        return Err("Trailing .data bytes".into());
    }
    Ok(hashes)
}
struct EmptyDirectory(PathBuf);
impl Drop for EmptyDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}
struct Stage(PathBuf, bool);
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
        Ok(Self(path, true))
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
        if self.1 && exists(&self.0) {
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

// An explicit local package is separate from built-in installation and never downloads.
// Its source stays untouched; only a dedicated, hash-addressed custom cache is created.
fn data_context(
    root: &Path,
    model: &Model,
    input: &Path,
) -> Result<(PathBuf, PathBuf, Package, Manifest)> {
    if !model.is_custom() {
        return Err(
            "CUSTOM_MODEL_PATH requires custom/<name>; built-in models are unchanged".into(),
        );
    }
    let absolute_root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(root)
    };
    let relative_input = if input.is_absolute() {
        input
            .strip_prefix(&absolute_root)
            .map_err(|_| "CUSTOM_MODEL_PATH must be inside MODEL_DIR")?
    } else {
        input
    };
    let source = safe_path(
        root,
        relative_input.to_str().ok_or("Custom path must be UTF-8")?,
    )?;
    if source.extension().and_then(|e| e.to_str()) != Some("data") {
        return Err(
            "CUSTOM_MODEL_PATH must select one exact local .data file inside MODEL_DIR".into(),
        );
    }
    let meta = fs::metadata(&source).map_err(|e| {
        format!(
            "Cannot read selected custom .data {}: {e}",
            source.display()
        )
    })?;
    if !meta.is_file() || meta.len() < 4 || meta.len() > 4 * MAX_FILE + MAX_MANIFEST + 20 {
        return Err("Invalid custom .data: bounded regular file required".into());
    }
    // Validate the supported free-package format before creating directories.
    let mut f = File::open(&source).map_err(|e| e.to_string())?;
    free_package_header(&mut f, meta.len(), model.language())?;
    let sha256 = hash(&source)?;
    let dir = locate(root, model)?;
    if NAMES.iter().any(|n| exists(&dir.join(n))) {
        return Err("Selected custom directory already contains ONNX/token files. Omit CUSTOM_MODEL_PATH to use its manifest, or select a new custom/<name>; no existing files were changed".into());
    }
    let manifest_path = safe_path(&dir, "model.json")?;
    let mut m = if exists(&manifest_path) {
        manifest(&dir, model)?
    } else {
        let known = [Model::Community64, Model::Community128].into_iter().filter_map(|m|m.for_language(model.language()).ok())
            .find(|known| package(known).is_some_and(|p| p.sha256 == sha256 && p.size == meta.len()))
            .ok_or("Unknown free .data export: supply model.json in the selected custom/<name> directory with its actual backend, four component roles and tail_samples. No streaming settings are guessed from the filename; no files were changed")?;
        let mut m = built_in(&known);
        m.id = model.key().into();
        m
    };
    if !m.compatible() || m.model_type != "zipformer2" {
        return Err(
            "Custom .data preparation requires a matching-language Zipformer2 sherpa-onnx-transducer manifest"
                .into(),
        );
    }
    // Package order is fixed; manifest file order is not.
    m.files.sort_by_key(|f| {
        ["encoder", "decoder", "joiner", "tokens"]
            .iter()
            .position(|r| *r == f.role)
            .unwrap_or(4)
    });
    if crate::models::normalize_language(&m.language)? != model.language() {
        return Err("Custom manifest language differs from LANGUAGE".into());
    }
    m.validate(model.key())?;
    let p = Package {
        name: source
            .file_name()
            .ok_or("Missing package filename")?
            .to_str()
            .ok_or("Package filename must be UTF-8")?
            .into(),
        sha256,
        size: meta.len(),
        url: String::new(),
    };
    // Even when an unknown export has no provider hashes, bind every cached file
    // to the bytes in the selected original. This does not authenticate its origin.
    let hashes = verify_package(&source, &p, &m, None)?;
    for (spec, actual) in m.files.iter_mut().zip(hashes) {
        spec.sha256 = Some(actual);
    }
    if !exists(&manifest_path) {
        m.tail_samples = package_tail(&source, model.language())?;
    }
    Ok((dir, source, p, m))
}

pub fn status_data(root: &Path, model: &Model, input: &Path) -> Value {
    let inspect = || -> Result<Value> {
        let _root = Lease::acquire(root, false)?;
        let (dir, source, p, m) = data_context(root, model, input)?;
        let _lease = if exists(&dir) {
            Some(Lease::acquire(&dir, false)?)
        } else {
            None
        };
        verify_package(&source, &p, &m, None)?;
        let cache = safe_path(&dir, &format!(".runtime/{}", p.sha256))?;
        let ready = exists(&cache);
        if ready {
            verify_files(&cache, &m)?;
        }
        Ok(
            json!({"model":model.key(),"language":model.language(),"identity":model.identity(),"present":true,"valid":true,"source":"local","format":"kroko-data","package":source,"sha256":p.sha256,"directory":cache,"runtime_compatible":true,"needs_preparation":!ready,"license":m.license,"version":m.version,"runtime_check":"package/header/hash validation; native ONNX compatibility is checked at server startup"}),
        )
    };
    inspect().unwrap_or_else(
        |error| json!({"model":model.key(),"language":model.language(),"identity":model.identity(),"valid":false,"source":"local","error":error}),
    )
}

pub fn prepare_data(root: &Path, model: &Model, input: &Path) -> Result<Resolved> {
    let root_read = Lease::acquire(root, false)?;
    let (dir, source, p, m) = data_context(root, model, input)?;
    let cache = safe_path(&dir, &format!(".runtime/{}", p.sha256))?;
    if exists(&cache) {
        let lease = Lease::acquire(&dir, false)?;
        verify_package(&source, &p, &m, None)?;
        verify_files(&cache, &m)?;
        return Ok(Resolved {
            directory: cache,
            manifest: m,
            lease,
        });
    }
    // Fully validate the selected source before any writes; no repairs or downloads.
    verify_package(&source, &p, &m, None)?;
    drop(root_read);
    let _root_write = Lease::acquire(root, true)?;
    let parent = dir
        .parent()
        .ok_or("Custom directory has no parent")?
        .to_path_buf();
    if !exists(&parent) {
        fs::create_dir_all(&parent)
            .map_err(|e| format!("Custom preparation needs writable persistent MODEL_DIR: {e}"))?;
    }
    if !exists(&dir) {
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
    }
    let lease = Lease::acquire(&dir, true)?;
    let runtime = safe_path(&dir, ".runtime")?;
    if !exists(&runtime) {
        fs::create_dir(&runtime).map_err(|e| {
            format!("First custom .data preparation requires writable model storage: {e}")
        })?;
    }
    let target = safe_path(&runtime, &p.sha256)?;
    if !exists(&target) {
        let stage = Stage::new(&runtime, "prepare")?;
        verify_package(&source, &p, &m, Some(&stage.0))?;
        verify_files(&stage.0, &m)?;
        stage.publish(&target)?;
        eprintln!(
            "{}",
            json!({"event":"custom_model_prepared","model":model.key(),"language":model.language(),"identity":model.identity(),"sha256":p.sha256,"preparation":"byte-exact free package components; original retained"})
        );
    }
    verify_files(&target, &m)?;
    lease.shared()?;
    Ok(Resolved {
        directory: target,
        manifest: m,
        lease,
    })
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
            if components != dir || NAMES.iter().any(|n| exists(&dir.join(n))) || model.is_custom()
            {
                verify_files(&components, &m)?;
                if !m.compatible() {
                    return Err(format!("Model {} is locally valid but backend/format is not supported by this server; no new runtime is installed",m.id));
                }
                let lease = compact_free_copy(&dir, &components, model, &m, lease)?;
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
                        let hashes = verify_package(&pkg, &p, &m, Some(&stage.0))?;
                        let verified = verified_manifest(&stage.0, &m, hashes)?;
                        write_installation(&stage.0, &p, &verified, "local")?;
                        stage.publish(&target)?;
                    }
                    let prepared = manifest(&dir, model)?;
                    verify_files(&target, &prepared)?;
                    // Only the pinned free Community source can be discarded.
                    discard_free_package(&pkg, &p, &prepared, &target)?;
                    lease.shared()?;
                    return Ok(Resolved {
                        directory: target,
                        manifest: prepared,
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
            let prepared = manifest(&dir, model)?;
            lease.shared()?;
            return Ok(Resolved {
                directory: target,
                manifest: prepared,
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
    let parent = safe_path(root, model.language())?;
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    install_bundle(&parent, &dir, &p, &m, fetch)?;
    let prepared = manifest(&dir, model)?;
    let lease = Lease::acquire(&dir, false)?;
    drop(root_write);
    Ok(Resolved {
        directory: dir,
        manifest: prepared,
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
    let mut stage = Stage::new(parent, "install")?;
    let pkg = stage.0.join(&p.name);
    fetch(p, &pkg)?;
    let result = (|| -> Result<()> {
        let hashes = verify_package(&pkg, p, m, Some(&stage.0))?;
        let verified = verified_manifest(&stage.0, m, hashes)?;
        write_installation(&stage.0, p, &verified, "download")?;
        stage.publish(target)?;
        discard_free_package(&target.join(&p.name), p, &verified, target)
    })();
    if let Err(error) = result {
        // A completed source package survives failed preparation. Partial outputs
        // stay private and never count as an installed model.
        stage.1 = false;
        return Err(format!("{error}; completed package retained for inspection at {} or {}. No existing model replaced",pkg.display(),target.join(&p.name).display()));
    }
    Ok(())
}
fn write_installation(dir: &Path, p: &Package, m: &Manifest, source: &str) -> Result<()> {
    let record = json!({"source":source,"url":p.url,"revision":m.version,"sha256":p.sha256,"language":m.language,"model":m.id,"license":m.license,"preparation":"verified byte-exact components; free downloaded package removed after publication"});
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("installation.json"))
        .map_err(|e| e.to_string())?;
    f.write_all(record.to_string().as_bytes())
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())
}
fn discard_free_package(path: &Path, p: &Package, m: &Manifest, components: &Path) -> Result<()> {
    verify_files(components, m)?;
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() != p.size {
        return Err("Free source is not a complete regular package; retained".into());
    }
    checksum(path, &p.sha256)?;
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let size = f.metadata().map_err(|e| e.to_string())?.len();
    free_package_header(&mut f, size, &m.language)?;
    drop(f);
    fs::remove_file(path)
        .map_err(|e| format!("Verified components kept; could not remove free package: {e}"))?;
    File::open(path.parent().ok_or("Missing package parent")?)
        .and_then(|d| d.sync_all())
        .map_err(|e| e.to_string())
}
fn compact_free_copy(
    dir: &Path,
    components: &Path,
    model: &Model,
    m: &Manifest,
    lease: Lease,
) -> Result<Lease> {
    let Some(p) = package(model) else {
        return Ok(lease);
    };
    let source = match safe_path(dir, &p.name) {
        Ok(p) => p,
        Err(_) => return Ok(lease),
    };
    if !exists(&source) {
        return Ok(lease);
    }
    let c = CString::new(dir.as_os_str().as_bytes()).map_err(|_| "Invalid directory")?;
    if unsafe {
        libc::faccessat(
            libc::AT_FDCWD,
            c.as_ptr(),
            libc::W_OK | libc::X_OK,
            libc::AT_EACCESS,
        )
    } != 0
    {
        return Ok(lease);
    }
    // Caller still holds the shared storage lease, so coordinated removers are
    // excluded while we change the per-model lease. Busy/read-only models stay usable.
    drop(lease);
    if let Ok(exclusive) = Lease::acquire(dir, true) {
        if let Err(error) = discard_free_package(&source, &p, m, components) {
            eprintln!(
                "{}",
                json!({"event":"community_package_retained","identity":model.identity(),"reason":error})
            );
        }
        exclusive.shared()?;
        Ok(exclusive)
    } else {
        Lease::acquire(dir, false)
    }
}

pub fn list(root: &Path) -> Vec<Value> {
    list_for(root, "de")
}
pub fn list_for(root: &Path, language: &str) -> Vec<Value> {
    let mut models = Vec::new();
    for base in [Model::Community128, Model::Community64, Model::Classic] {
        if let Ok(model) = base.for_language(language) {
            models.push(model);
        }
    }
    let canonical = crate::models::normalize_language(language).unwrap_or_default();
    let mut directories = vec![root.join(&canonical).join("custom")];
    if canonical == "de" {
        directories.push(root.join("custom"));
    }
    for directory in directories {
        if let Ok(entries) = fs::read_dir(directory) {
            for e in entries.flatten() {
                if let Some(name) = e.file_name().to_str() {
                    if let Ok(model) = Model::parse(&format!("custom/{name}"))
                        .and_then(|m| m.for_language(language))
                    {
                        if !models.contains(&model) {
                            models.push(model);
                        }
                    }
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
    if model.is_custom() {
        let m = manifest(&dir, model)?;
        allowed.insert(PathBuf::from("model.json"));
        for f in m.files {
            allowed.insert(PathBuf::from(f.path));
        }
    } else {
        allowed.extend(NAMES.iter().map(|n| PathBuf::from(*n)));
        allowed.insert(PathBuf::from("installation.json"));
        allowed.insert(PathBuf::from("model.json"));
        if let Some(p) = package(model) {
            allowed.insert(PathBuf::from(&p.name));
            for n in NAMES {
                allowed.insert(PathBuf::from(format!(".runtime/{}/{n}", p.sha256)));
            }
            for n in [p.name.as_str(), "installation.json", "model.json"] {
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
            let mut data = fs::read(r.0.join(&spec.path)).unwrap();
            if spec.role == "encoder" {
                for (key, value) in [
                    ("model_type", "zipformer2"),
                    ("version", "1"),
                    ("decode_chunk_len", "128"),
                    ("T", "141"),
                ] {
                    let mut entry = vec![10, key.len() as u8];
                    entry.extend(key.as_bytes());
                    entry.extend([18, value.len() as u8]);
                    entry.extend(value.as_bytes());
                    data.extend([114, entry.len() as u8]);
                    data.extend(entry);
                }
            }
            bytes.extend((data.len() as u32).to_le_bytes());
            bytes.extend(data);
        }
        let mut cursor = 4 + h.len();
        for spec in &mut m.files {
            let length = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            spec.sha256 = Some(digest(&bytes[cursor..cursor + length]));
            cursor += length;
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
        assert!(!target.join(&p.name).exists());
        assert!(
            install_bundle(&r.0, &target, &p, &m, &|p, path| save_download(
                bytes.as_slice(),
                p,
                path
            ))
            .is_err()
        );
        assert!(!target.join(&p.name).exists());
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

    fn local_data_fixture(root: &Path) -> (Model, PathBuf, Manifest) {
        let (bytes, _, mut m) = synthetic();
        let model = Model::parse("custom/package-test").unwrap();
        m.id = model.key().into();
        let dir = root.join(model.key());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("model.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        let path = root.join("selected.data");
        fs::write(&path, bytes).unwrap();
        (model, path, m)
    }
    #[test]
    fn custom_data_prepares_once_preserves_source_and_reuses_cache() {
        let r = root();
        let (model, path, m) = local_data_fixture(&r.0);
        let source_hash = hash(&path).unwrap();
        assert_eq!(status_data(&r.0, &model, &path)["needs_preparation"], true);
        assert!(!r.0.join(model.key()).join(".runtime").exists());
        let prepared = prepare_data(&r.0, &model, &path).unwrap();
        assert_eq!(prepared.manifest.tail_samples, m.tail_samples);
        verify_files(&prepared.directory, &m).unwrap();
        let cache = prepared.directory.clone();
        let before = fs::metadata(cache.join("encoder.onnx"))
            .unwrap()
            .modified()
            .unwrap();
        drop(prepared);
        let again = prepare_data(&r.0, &model, &path).unwrap();
        assert_eq!(again.directory, cache);
        assert_eq!(
            fs::metadata(cache.join("encoder.onnx"))
                .unwrap()
                .modified()
                .unwrap(),
            before
        );
        assert_eq!(hash(&path).unwrap(), source_hash);
        assert_eq!(status_data(&r.0, &model, &path)["needs_preparation"], false);
    }
    #[test]
    fn custom_data_requires_explicit_free_flag_before_writes() {
        for flag in [json!(false), json!("true"), Value::Null] {
            let r = root();
            let h = serde_json::to_vec(
                &json!({"free":flag,"type":"zipformer2","language":{"iso":"DE"}}),
            )
            .unwrap();
            let mut bytes = (h.len() as u32).to_le_bytes().to_vec();
            bytes.extend(h);
            let path = r.0.join("unsupported.data");
            fs::write(&path, &bytes).unwrap();
            let model = Model::parse("custom/unsupported").unwrap();
            let err = prepare_data(&r.0, &model, &path).err().unwrap();
            assert!(err.contains("requires an explicit boolean free=true"));
            assert_eq!(fs::read_dir(&r.0).unwrap().count(), 1);
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert_eq!(status_data(&r.0, &model, &path)["valid"], false);
        }
    }
    #[test]
    fn unknown_free_data_requires_explicit_metadata_without_guessing() {
        let r = root();
        let (bytes, _, _) = synthetic();
        let path = r.0.join("Kroko-DE-Community-128-L-Streaming-001.data");
        fs::write(&path, &bytes).unwrap();
        let err = prepare_data(&r.0, &Model::parse("custom/unknown").unwrap(), &path)
            .err()
            .unwrap();
        assert!(err.contains("Unknown free .data export"));
        assert!(!r.0.join("custom").exists());
    }
    #[test]
    fn custom_data_uses_roles_not_manifest_order_and_rejects_partial_cache() {
        let r = root();
        let (model, path, mut m) = local_data_fixture(&r.0);
        m.files.reverse();
        fs::write(
            r.0.join(model.key()).join("model.json"),
            serde_json::to_vec(&m).unwrap(),
        )
        .unwrap();
        let prepared = prepare_data(&r.0, &model, &path).unwrap();
        let cache = prepared.directory.clone();
        drop(prepared);
        fs::write(cache.join("decoder.onnx"), b"keep broken cache").unwrap();
        assert!(prepare_data(&r.0, &model, &path).is_err());
        assert_eq!(
            fs::read(cache.join("decoder.onnx")).unwrap(),
            b"keep broken cache"
        );
    }
    #[test]
    fn custom_data_path_is_explicit_confined_and_not_a_builtin_override() {
        let r = root();
        let (model, path, _) = local_data_fixture(&r.0);
        for p in [
            PathBuf::from("../outside.data"),
            PathBuf::from("/outside.data"),
            PathBuf::from("missing.data"),
        ] {
            assert!(prepare_data(&r.0, &model, &p).is_err());
        }
        std::os::unix::fs::symlink(&path, r.0.join("link.data")).unwrap();
        assert!(prepare_data(&r.0, &model, Path::new("link.data")).is_err());
        assert!(prepare_data(&r.0, &Model::Community128, &path).is_err());
        // A second unselected file is never chosen, renamed or overwritten.
        fs::write(r.0.join("other.data"), b"untouched").unwrap();
        prepare_data(&r.0, &model, Path::new("selected.data")).unwrap();
        assert_eq!(fs::read(r.0.join("other.data")).unwrap(), b"untouched");
    }
    #[test]
    fn custom_data_rejects_truncation_and_preserves_existing_components() {
        let r = root();
        let (model, path, _) = local_data_fixture(&r.0);
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
        assert!(prepare_data(&r.0, &model, &path).is_err());
        assert!(!r.0.join(model.key()).join(".runtime").exists());
        fs::write(&path, bytes).unwrap();
        let existing = r.0.join(model.key()).join("encoder.onnx");
        fs::write(&existing, b"keep existing components").unwrap();
        assert!(prepare_data(&r.0, &model, &path).is_err());
        assert_eq!(fs::read(existing).unwrap(), b"keep existing components");
    }
    #[test]
    fn custom_data_without_provider_hashes_still_binds_cache_to_original() {
        let r = root();
        let (model, path, mut m) = local_data_fixture(&r.0);
        for spec in &mut m.files {
            spec.sha256 = None;
        }
        fs::write(
            r.0.join(model.key()).join("model.json"),
            serde_json::to_vec(&m).unwrap(),
        )
        .unwrap();
        let prepared = prepare_data(&r.0, &model, &path).unwrap();
        assert!(prepared.manifest.files.iter().all(|s| s.sha256.is_some()));
        let encoder = prepared.directory.join("encoder.onnx");
        drop(prepared);
        // Still a valid envelope, but not the original encoder bytes.
        fs::write(&encoder, [8, 8, 58, 2, 16, 1, 66, 2, 16, 12]).unwrap();
        assert!(onnx_envelope(&encoder).is_ok());
        assert!(prepare_data(&r.0, &model, &path).is_err());
    }
    #[test]
    fn languages_have_separate_paths_and_legacy_is_de_only() {
        let r = root();
        fs::create_dir(r.0.join("community-128")).unwrap();
        fs::create_dir(r.0.join("classic")).unwrap();
        let de = Model::Community128;
        let en = de.for_language("en").unwrap();
        assert_eq!(locate(&r.0, &de).unwrap(), r.0.join("community-128"));
        assert_eq!(locate(&r.0, &en).unwrap(), r.0.join("en/community-128"));
        assert_eq!(selection_for(&r.0, "en", None).unwrap(), en);
        assert!(selection_for(&r.0, "en", Some("classic")).is_err());
        fs::create_dir_all(r.0.join("de/community-128")).unwrap();
        assert_eq!(locate(&r.0, &de).unwrap(), r.0.join("de/community-128"));
    }
    #[test]
    fn lazy_download_requests_only_the_selected_language_and_variant() {
        use std::cell::RefCell;
        for lang in ["de", "en"] {
            for key in ["community-64", "community-128"] {
                let r = root();
                let model = selection_for(&r.0, lang, Some(key)).unwrap();
                let calls = RefCell::new(Vec::new());
                assert!(prepare_with(&r.0, &model, true, &|p, _| {
                    calls
                        .borrow_mut()
                        .push((p.name.clone(), p.sha256.clone(), p.url.clone()));
                    Err("injected stop before network".into())
                })
                .is_err());
                let calls = calls.into_inner();
                assert_eq!(calls.len(), 1);
                let entry = model.catalog_entry().unwrap();
                assert_eq!(calls[0].0, entry.filename);
                assert_eq!(calls[0].1, entry.sha256);
                assert!(calls[0].2.ends_with(&entry.filename));
                let other = if key == "community-64" {
                    "community-128"
                } else {
                    "community-64"
                };
                assert!(!r.0.join(lang).join(other).exists());
                assert!(!r.0.join(if lang == "de" { "en" } else { "de" }).exists());
            }
        }
    }
    #[test]
    fn custom_manifest_is_language_bound_and_local_restart_never_fetches() {
        let r = root();
        let de = Model::parse("custom/shared").unwrap();
        let en = de.for_language("en").unwrap();
        custom(
            &r.0.join("custom/shared"),
            de.key(),
            "sherpa-onnx-transducer",
        );
        assert!(prepare_with(&r.0, &en, true, &|_, _| panic!(
            "custom must never download"
        ))
        .is_err());
        let dir = r.0.join("en/custom/shared");
        let mut m = custom(&dir, en.key(), "sherpa-onnx-transducer");
        assert!(prepare(&r.0, &en, false).is_err());
        m.language = "en".into();
        fs::write(dir.join("model.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        for _ in 0..2 {
            let p =
                prepare_with(&r.0, &en, true, &|_, _| panic!("no fetch on local restart")).unwrap();
            assert_eq!(p.directory, dir);
        }
        assert_ne!(prepare(&r.0, &de, false).unwrap().directory, dir);
    }
    #[test]
    fn free_cleanup_requires_complete_components_and_valid_header() {
        let r = root();
        let (bytes, p, m) = synthetic();
        let original = r.0.join(&p.name);
        fs::write(&original, &bytes).unwrap();
        let components = r.0.join("prepared");
        fs::create_dir(&components).unwrap();
        verify_package(&original, &p, &m, Some(&components)).unwrap();
        fs::write(components.join("decoder.onnx"), b"invalid").unwrap();
        assert!(discard_free_package(&original, &p, &m, &components).is_err());
        assert_eq!(fs::read(&original).unwrap(), bytes);
        fs::remove_dir_all(&components).unwrap();
        fs::create_dir(&components).unwrap();
        verify_package(&original, &p, &m, Some(&components)).unwrap();
        let h = br#"{"free":false,"type":"zipformer2","language":{"iso":"DE"}}"#;
        let mut invalid_package = (h.len() as u32).to_le_bytes().to_vec();
        invalid_package.extend(h);
        invalid_package.extend([0; 32]);
        fs::write(&original, &invalid_package).unwrap();
        let mut protected = p.clone();
        protected.sha256 = digest(&invalid_package);
        protected.size = invalid_package.len() as u64;
        assert!(discard_free_package(&original, &protected, &m, &components).is_err());
        assert_eq!(fs::read(&original).unwrap(), invalid_package);
    }
    #[test]
    fn complete_download_survives_failed_preparation_without_publishing() {
        let r = root();
        let (mut bytes, mut p, mut m) = synthetic();
        let at = bytes.windows(3).position(|s| s == b"141").unwrap();
        bytes[at + 2] = b'0';
        p.sha256 = digest(&bytes);
        let offset = 4 + u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        let n = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        m.files[0].sha256 = Some(digest(&bytes[offset + 4..offset + 4 + n]));
        let target = r.0.join("not-installed");
        assert!(
            install_bundle(&r.0, &target, &p, &m, &|_, path| fs::write(path, &bytes)
                .map_err(|e| e.to_string()))
            .is_err()
        );
        assert!(!target.exists());
        let retained = fs::read_dir(&r.0)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join(&p.name);
        assert_eq!(fs::read(retained).unwrap(), bytes);
    }
    #[test]
    fn nemo_manifest_requires_explicit_language_and_preserves_zipformer_defaults() {
        let mut m=built_in(&Model::Classic);
        let old=serde_json::to_value(&m).unwrap();
        let mut legacy=old.clone();legacy.as_object_mut().unwrap().remove("feature_dim");legacy.as_object_mut().unwrap().remove("stream_language");
        let legacy:Manifest=serde_json::from_value(legacy).unwrap();assert_eq!(legacy.feature_dim,80);assert!(legacy.stream_language.is_none());
        m.id="custom/nemo".into();m.model_type="nemo_transducer".into();m.feature_dim=128;
        assert!(m.validate("custom/nemo").is_err());m.stream_language=Some("en-US".into());assert!(m.validate("custom/nemo").is_err());
        m.stream_language=Some("de-DE".into());assert!(m.validate("custom/nemo").is_ok());assert!(m.compatible());
        let mut meta=HashMap::from([("model_type".into(),"EncDecRNNTBPEModelWithPrompt".into()),("feat_dim".into(),"128".into()),("version".into(),"1".into()),("window_size".into(),"65".into()),("chunk_shift".into(),"56".into()),("subsampling_factor".into(),"8".into()),("prompt_dictionary".into(),r#"{"de-DE":9,"auto":101}"#.into())]);
        assert!(validate_nemo_metadata(&meta,&m).is_ok());meta.insert("prompt_dictionary".into(),r#"{"en-US":0,"auto":101}"#.into());assert!(validate_nemo_metadata(&meta,&m).is_err());
        meta.insert("model_type".into(),"zipformer2".into());assert!(validate_nemo_metadata(&meta,&m).is_err());
    }

    #[test]
    fn nemo_blank_last_and_zipformer_blank_zero_are_not_interchangeable() {
        let r=root();let path=r.0.join("tokens.txt");
        fs::write(&path,b"<unk> 0\na 1\n<blk> 2\n").unwrap();
        assert_eq!(tokens(&path,None).unwrap(),3);assert!(tokens(&path,Some(0)).is_err());
        fs::write(&path,b"<blk> 0\n<unk> 1\na 2\n").unwrap();
        assert!(tokens(&path,Some(0)).is_ok());assert!(tokens(&path,None).is_err());
    }

}
