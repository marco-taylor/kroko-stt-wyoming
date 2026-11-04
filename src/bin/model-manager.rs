// Resolved fields are consumed by the server; this CLI only holds its lease.
#[allow(dead_code)]
#[path = "../server/model_manager.rs"]
mod model_manager;
#[path = "../server/models.rs"]
mod models;
use serde_json::json;
use std::{env, path::PathBuf};
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let root = PathBuf::from(env::var("MODEL_DIR").unwrap_or_else(|_| "models".into()));
    let command = args.first().map(String::as_str).unwrap_or("status");
    if command == "--help" || command == "help" {
        println!("model-manager [status [ID] | list | verify ID | install ID | remove ID --yes]\nMODEL_DIR=/models; IDs: community-128, community-64, classic, custom/<name>\nstatus/list/verify never download or modify files. remove refuses active models and unknown files.");
        return Ok(());
    }
    if command == "list" {
        if args.len() != 1 {
            return Err("Usage: model-manager list".into());
        }
        println!("{}", json!({"models":model_manager::list(&root)}));
        return Ok(());
    }
    if args.len() > 3 || (command != "remove" && args.len() > 2) {
        return Err("Unexpected arguments; use model-manager --help".into());
    }
    let old = env::var("KROKO_MODEL").ok();
    let alias = env::var("MODEL").ok();
    if old.is_some() && alias.is_some() && old != alias {
        return Err("KROKO_MODEL and MODEL conflict; set only one".into());
    }
    let configured = old.or(alias);
    let model = model_manager::selection(
        &root,
        args.get(1).map(String::as_str).or(configured.as_deref()),
    )?;
    match command {
        "status" | "verify" => {
            let report = model_manager::status(&root, &model);
            println!("{report}");
            if command == "verify" && report["valid"] != true {
                return Err("Model verification failed; existing files were not changed".into());
            }
        }
        "install" => {
            let _model = model_manager::prepare(&root, &model, true)?;
            println!("{}", model_manager::status(&root, &model));
        }
        "remove" => {
            if args.get(2).map(String::as_str) != Some("--yes") || args.get(1).is_none() {
                return Err("Removal requires: model-manager remove ID --yes. Only the selected dedicated model directory is eligible.".into());
            }
            model_manager::remove(&root, &model)?;
            println!("{}", json!({"model":model.key(),"removed":true}));
        }
        _ => return Err("Unknown command; use model-manager --help".into()),
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{}", json!({"event":"model_manager_error","error":e}));
        std::process::exit(1);
    }
}
