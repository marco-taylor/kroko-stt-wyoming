use crate::config::Config;
use serde_json::{json, Value};
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream};
use std::{sync::MutexGuard, time::Instant};

pub fn log(config: &Config, level: &str, event: Value) {
    if config.enabled(level) { eprintln!("{event}"); }
}
pub fn cpu_s() -> f64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    unsafe {
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 { return 0.0; }
        let r = usage.assume_init();
        (r.ru_utime.tv_sec + r.ru_stime.tv_sec) as f64 + (r.ru_utime.tv_usec + r.ru_stime.tv_usec) as f64 / 1e6
    }
}
pub fn rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status").ok()?.lines().find(|l| l.starts_with("VmRSS:"))?.split_whitespace().nth(1)?.parse().ok()
}
pub struct Engine { pub recognizer: OnlineRecognizer, tail_samples: usize, _model_lease: crate::model_manager::Lease }
impl Engine {
    pub fn load(config: &Config) -> Result<Self, String> {
        eprintln!("Kroko model: {}", config.model.key());
        let prepared=crate::model_manager::prepare(&config.model_root,&config.model,config.auto_download)?;
        let path = |role: &str| -> Result<String,String> {
            let f=prepared.manifest.files.iter().find(|f|f.role==role).ok_or_else(||format!("Missing model role {role}"))?;
            prepared.directory.join(&f.path).to_str().map(str::to_owned).ok_or_else(||"Model path must be UTF-8".into())
        };
        let mut c = OnlineRecognizerConfig::default();
        c.model_config.transducer.encoder = Some(path("encoder")?);
        c.model_config.transducer.decoder = Some(path("decoder")?);
        c.model_config.transducer.joiner = Some(path("joiner")?);
        c.model_config.tokens = Some(path("tokens")?);
        c.model_config.provider = Some("cpu".into()); c.model_config.num_threads = config.num_threads;
        c.feat_config.sample_rate = 16000; c.feat_config.feature_dim = 80;
        c.decoding_method = Some("greedy_search".into()); c.enable_endpoint = false;
        let start = Instant::now();
        let recognizer = OnlineRecognizer::create(&c).ok_or_else(|| "Cannot create Kroko OnlineRecognizer; see native diagnostics".to_string())?;
        log(config,"info",json!({"event":"model_loaded","load_s":start.elapsed().as_secs_f64(),"rss_kib":rss_kib(),"threads":config.num_threads,"sherpa_version":sherpa_onnx::version(),"onnxruntime_version":sherpa_onnx::onnxruntime_version(),"model":config.model.key(),"model_dir":prepared.directory,"revision":prepared.manifest.version}));
        Ok(Self { recognizer, tail_samples:prepared.manifest.tail_samples, _model_lease:prepared.lease })
    }
}
// Stream must be dropped before the guard releases exclusive recognizer access.
pub struct Session<'a> {
    stream: OnlineStream,
    engine: MutexGuard<'a,Engine>,
    pub id: u64,
    pub samples: usize,
    pub chunks: usize,
    pub decodes: usize,
    pending_byte: Option<u8>,
    previous: String,
    start: Instant,
    cpu_start: f64,
    last_chunk: Option<Instant>,
    finished: bool,
    log_aborts: bool,
    last_progress_samples: usize,
}
impl<'a> Session<'a> {
    pub fn new(engine: MutexGuard<'a,Engine>, id:u64, log_aborts:bool) -> Self {
        let stream = engine.recognizer.create_stream();
        Self { stream,engine,id,samples:0,chunks:0,decodes:0,pending_byte:None,previous:String::new(),start:Instant::now(),cpu_start:cpu_s(),last_chunk:None,finished:false,log_aborts,last_progress_samples:0 }
    }
    fn drain(&mut self, config:&Config, after_eof:bool) {
        while self.engine.recognizer.is_ready(&self.stream) {
            self.engine.recognizer.decode(&self.stream); self.decodes+=1;
            if let Some(result)=self.engine.recognizer.get_result(&self.stream) {
                if !result.text.is_empty() && result.text!=self.previous {
                    log(config,"debug",json!({"event":"partial","request_id":self.id,"wall_s":self.start.elapsed().as_secs_f64(),"audio_s":self.samples as f64/16000.0,"decode_calls":self.decodes,"after_eof":after_eof,"text":result.text}));
                    self.previous=result.text;
                }
            }
        }
    }
    pub fn expired(&self)->bool { self.start.elapsed().as_secs()>180 }
    pub fn accept(&mut self, bytes:&[u8], config:&Config)->Result<(),String> {
        if self.start.elapsed().as_secs()>180 { return Err("Request exceeds 180-second wall-time limit".into()); }
        if self.samples*2 + bytes.len() + usize::from(self.pending_byte.is_some()) > 16000*2*120 { return Err("Audio exceeds 120-second limit".into()); }
        self.last_chunk=Some(Instant::now());
        let mut samples=Vec::with_capacity((bytes.len()+1)/2); let mut offset=0;
        if let Some(low)=self.pending_byte.take() {
            if bytes.is_empty() { self.pending_byte=Some(low); }
            else { samples.push(i16::from_le_bytes([low,bytes[0]]) as f32 /32768.0); offset=1; }
        }
        let mut pairs=bytes[offset..].chunks_exact(2);
        samples.extend(pairs.by_ref().map(|b|i16::from_le_bytes([b[0],b[1]]) as f32/32768.0));
        if let Some(&low)=pairs.remainder().first() { self.pending_byte=Some(low); }
        self.samples+=samples.len(); self.chunks+=1;
        self.stream.accept_waveform(16000,&samples); self.drain(config,false);
        if self.chunks == 1 || self.samples.saturating_sub(self.last_progress_samples) >= 16000 {
            log(config,"debug",json!({"event":"audio_chunk_processed","request_id":self.id,"chunks":self.chunks,"samples":self.samples,"audio_s":self.samples as f64/16000.0,"decode_calls":self.decodes,"rss_kib":rss_kib()}));
            self.last_progress_samples = self.samples;
        }
        Ok(())
    }
    pub fn finish(&mut self, config:&Config)->Result<String,String> {
        if self.pending_byte.is_some() { return Err("Incomplete PCM16 sample at audio-stop".into()); }
        let finish=Instant::now();
        // Same context-safe zero tail as Phase 1D; no sleep or offline re-recognition.
        for block in vec![0.0f32;self.engine.tail_samples].chunks(320) { self.stream.accept_waveform(16000,block); self.drain(config,true); }
        self.stream.input_finished(); self.drain(config,true);
        let result=self.engine.recognizer.get_result(&self.stream).ok_or_else(||"No final ASR result".to_string())?;
        self.finished=true;
        let wall=self.start.elapsed().as_secs_f64(); let cpu=cpu_s()-self.cpu_start;
        log(config,"info",json!({"event":"request_final","request_id":self.id,"text":result.text,"audio_s":self.samples as f64/16000.0,"chunks":self.chunks,"decode_calls":self.decodes,"elapsed_s":wall,"cpu_s":cpu,"cpu_percent_one_core":100.0*cpu/wall,"finalize_s":finish.elapsed().as_secs_f64(),"last_chunk_to_final_s":self.last_chunk.map(|t|t.elapsed().as_secs_f64()),"rss_kib":rss_kib()}));
        Ok(result.text)
    }
}
impl Drop for Session<'_> {
    fn drop(&mut self) {
        if !self.finished && self.log_aborts { eprintln!("{}",json!({"event":"request_aborted","request_id":self.id,"samples":self.samples,"decode_calls":self.decodes})); }
    }
}
