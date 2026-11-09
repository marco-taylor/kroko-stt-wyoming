mod models;
// The shared module also provides CLI-only commands.
#[allow(dead_code)]
mod model_manager;
mod asr;
mod decoder;
mod config;
mod wyoming;

use asr::{log,Engine,Session};
use config::{Config,MODEL_NAME,PROGRAM_NAME};
use serde_json::{json,Value};
use std::{io::{self,BufReader},net::{TcpListener,TcpStream},sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,AtomicUsize,Ordering}},time::Duration};
static SHUTDOWN:AtomicBool=AtomicBool::new(false);
extern "C" fn stop_signal(_:i32){SHUTDOWN.store(true,Ordering::Relaxed);}
static NEXT_REQUEST:AtomicU64=AtomicU64::new(1);
static CLIENTS:AtomicUsize=AtomicUsize::new(0);
struct ClientSlot;
impl Drop for ClientSlot {fn drop(&mut self){CLIENTS.fetch_sub(1,Ordering::SeqCst);}}

fn info(config:&Config)->Value {
    json!({"asr":[{"name":PROGRAM_NAME,"description":"Local streaming ASR in Rust; client audio-stop required","attribution":{"name":"Local Rust/sherpa-onnx prototype","url":"https://github.com/k2-fsa/sherpa-onnx"},"installed":true,"version":env!("CARGO_PKG_VERSION"),"supports_transcript_streaming":false,"requires_external_vad":true,"models":[{"name":if config.model.is_custom(){config.model.key()}else{MODEL_NAME},"description":format!("Streaming model ({})",config.model.identity()),"attribution":{"name":if config.model.is_custom(){"External custom model"}else{"Banafo / Kroko-ASR"},"url":if config.model.is_custom(){"https://github.com/k2-fsa/sherpa-onnx"}else{"https://huggingface.co/Banafo/Kroko-ASR"}},"installed":true,"version":config.model.revision(),"languages":if config.language=="de" {vec!["de", "de-DE"]} else {vec![config.language.as_str()]}}]}]})
}
fn accepts_language(configured:&str,requested:&str)->bool { models::normalize_language(requested).ok().as_deref()==Some(configured) }
fn reject(stream:&mut TcpStream,code:&str,text:&str)->io::Result<()> { wyoming::error(stream,code,text) }
fn handle(mut socket:TcpStream, engine:&Mutex<Engine>, config:&Config)->io::Result<()> {
    socket.set_read_timeout(Some(Duration::from_secs(15)))?;
    socket.set_write_timeout(Some(Duration::from_secs(15)))?;
    socket.set_nodelay(true)?;
    let mut reader=BufReader::new(socket.try_clone()?);
    let mut session=None;
    loop {
        if SHUTDOWN.load(Ordering::Relaxed) { return Ok(()); }
        let event=match wyoming::read_event(&mut reader) {
            Ok(Some(event))=>event, Ok(None)=>return Ok(()),
            Err(error)=>{let _=reject(&mut socket,"invalid_event",&error.to_string());return Err(error);}
        };
        if session.as_ref().is_some_and(|s: &Session<'_>|s.expired()) { reject(&mut socket,"request_timeout","Request exceeds wall-time limit")?;return Ok(()); }
        match event.kind.as_str() {
            "describe"=>wyoming::write_event(&mut socket,"info",info(config))?,
            "select-program"=>{
                if event.data.get("name").and_then(Value::as_str)!=Some(PROGRAM_NAME) {
                    reject(&mut socket,"unknown_program","Only kroko is available")?;return Ok(());
                }
            }
            "transcribe"=>{
                if session.is_some(){reject(&mut socket,"invalid_sequence","transcribe during active audio stream")?;return Ok(());}
                if let Some(name)=event.data.get("name").filter(|v|!v.is_null()) {
                    if name.as_str()!=Some("kroko") && !(config.model.is_custom() && name.as_str()==Some(config.model.key())) && !(config.language=="de" && name.as_str()==Some("sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06")){reject(&mut socket,"unknown_model","Requested model is not installed")?;return Ok(());}
                }
                if let Some(language)=event.data.get("language").filter(|v|!v.is_null()) {
                    if !language.as_str().is_some_and(|l|accepts_language(&config.language,l)){reject(&mut socket,"unsupported_language","Requested language differs from the configured model; no language fallback")?;return Ok(());}
                }
            }
            "audio-start"=>{
                if session.is_some(){reject(&mut socket,"invalid_sequence","Duplicate audio-start")?;return Ok(());}
                if let Err(error)=wyoming::audio_format(&event){reject(&mut socket,"invalid_audio",&error)?;return Ok(());}
                let guard=match engine.try_lock(){Ok(g)=>g,Err(_)=>{reject(&mut socket,"busy","One STT request is already active; retry after it completes")?;return Ok(());}};
                let id=NEXT_REQUEST.fetch_add(1,Ordering::Relaxed);
                session=Some(Session::new(guard,id,config.enabled("warn")));
                log(config,"info",json!({"event":"request_started","request_id":id,"rate":16000,"width":2,"channels":1,"pcm":"signed little-endian"}));
            }
            "audio-chunk"=>{
                let Some(active)=session.as_mut()else{reject(&mut socket,"invalid_sequence","audio-start is required before audio-chunk")?;return Ok(());};
                if let Err(error)=wyoming::audio_format(&event){reject(&mut socket,"invalid_audio",&error)?;return Ok(());}
                if let Err(error)=active.accept(&event.payload,config){reject(&mut socket,"invalid_audio",&error)?;return Ok(());}
            }
            "audio-stop"=>{
                let Some(mut active)=session.take()else{reject(&mut socket,"invalid_sequence","audio-start is required before audio-stop")?;return Ok(());};
                let result=match active.finish(config){Ok(t)=>t,Err(error)=>{reject(&mut socket,"invalid_audio",&error)?;return Ok(());}};
                // Release stream/cache and exclusive gate before waiting on the client socket.
                drop(active);
                wyoming::write_event(&mut socket,"transcript",json!({"text":result,"language":config.language}))?;
            }
            _=>{} // Official compatibility rule: ignore unrecognized events.
        }
    }
}
fn run()->Result<(),String> {
    let config=Arc::new(Config::from_env()?);
    let engine=Arc::new(Mutex::new(Engine::load(&config)?));
    let listener=TcpListener::bind((config.host.as_str(),config.port)).map_err(|e|format!("Cannot bind {}:{}: {e}",config.host,config.port))?;
    log(&config,"info",json!({"event":"listening","host":config.host,"port":config.port,"rss_kib":asr::rss_kib(),"max_clients":8,"max_active_inference":1}));
    unsafe { libc::signal(libc::SIGTERM,stop_signal as libc::sighandler_t); libc::signal(libc::SIGINT,stop_signal as libc::sighandler_t); }
    listener.set_nonblocking(true).map_err(|e|e.to_string())?;
    let mut workers:Vec<std::thread::JoinHandle<()>>=Vec::new();
    while !SHUTDOWN.load(Ordering::Relaxed) {
        workers.retain(|worker| !worker.is_finished());
        let connection = match listener.accept() { Ok((s,_))=>Ok(s), Err(e) if e.kind()==io::ErrorKind::WouldBlock=>{std::thread::sleep(Duration::from_millis(50));continue;}, Err(e)=>Err(e) };
        let mut socket=match connection{Ok(s)=>s,Err(error)=>{log(&config,"warn",json!({"event":"accept_error","error":error.to_string()}));continue;}};
        if CLIENTS.fetch_add(1,Ordering::SeqCst)>=8{
            CLIENTS.fetch_sub(1,Ordering::SeqCst);let _=socket.set_write_timeout(Some(Duration::from_secs(1)));let _=reject(&mut socket,"busy","Connection limit reached");continue;
        }
        let slot=ClientSlot;let engine=engine.clone();let config=config.clone();
        workers.push(std::thread::spawn(move||{let _slot=slot;if let Err(error)=handle(socket,&engine,&config){log(&config,"debug",json!({"event":"connection_closed","error":error.to_string()}));}}));
    }
    for worker in workers { let _=worker.join(); }
    log(&config,"info",json!({"event":"shutdown","model":config.model.key()}));
    Ok(())
}
fn main(){if let Err(error)=run(){eprintln!("{}",json!({"event":"startup_error","error":error}));std::process::exit(1);}}

#[cfg(test)]
mod identity_tests {
 use super::*;
 #[test]
 fn language_requests_do_not_fall_back() {
  assert!(accepts_language("en","en-US"));assert!(!accepts_language("en","de"));
  assert!(accepts_language("de","de-DE"));assert!(!accepts_language("de","en"));assert!(!accepts_language("de","auto"));
 }
 #[test]
 fn describe_advertises_only_selected_language() {
  let c=Config {model_root:std::path::PathBuf::new(),auto_download:false,custom_model_path:None,model:models::Model::Community128.for_language("en").unwrap(),host:"127.0.0.1".into(),port:10300,num_threads:1,language:"en".into(),log_level:"warn".into()};
  let v=info(&c);assert_eq!(v["asr"][0]["models"][0]["languages"],json!(["en"]));assert!(v["asr"][0]["models"][0]["description"].as_str().unwrap().contains("en/community-128"));
 }
}

#[cfg(test)]
mod custom_identity_tests {
 use super::*;
 #[test]
 fn custom_describe_does_not_claim_banafo_provenance() {
  let c=Config{model_root:std::path::PathBuf::new(),auto_download:false,custom_model_path:None,model:models::Model::parse("custom/german-zipformer").unwrap(),host:"127.0.0.1".into(),port:10300,num_threads:1,language:"de".into(),log_level:"warn".into()};
  let v=info(&c);assert_eq!(v["asr"][0]["models"][0]["name"],"custom/german-zipformer");assert_eq!(v["asr"][0]["models"][0]["attribution"]["name"],"External custom model");
 }
}
