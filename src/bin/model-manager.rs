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
        println!("model-manager [status [ID] | list | verify ID | install ID | remove ID --yes]\nMODEL_DIR=/models; LANGUAGE=de; identity=LANGUAGE/ID; IDs: community-128, community-64, classic, custom/<name>\nCUSTOM_MODEL_PATH selects one local free .data for an explicit custom/<name>.\nstatus/list/verify never download or modify files. remove refuses active models and unknown files.");
        return Ok(());
    }
    let old = env::var("KROKO_MODEL").ok();
    let alias = env::var("MODEL").ok();
    if old.is_some() && alias.is_some() && old != alias {
        return Err("KROKO_MODEL and MODEL conflict; set only one".into());
    }
    let configured = old.or(alias);
    let language =
        models::normalize_language(&env::var("LANGUAGE").unwrap_or_else(|_| "de".into()))?;
    let custom_path = env::var("CUSTOM_MODEL_PATH")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    if command == "list" {
        if args.len() != 1 {
            return Err("Usage: model-manager list".into());
        }
        let mut reports = model_manager::list_for(&root, &language);
        if let Some(path) = &custom_path {
            let model = model_manager::selection_for(&root, &language, configured.as_deref())?;
            if !model.is_custom() {
                return Err("CUSTOM_MODEL_PATH requires custom/<name>".into());
            }
            reports.retain(|r| r["identity"] != model.identity());
            reports.push(model_manager::status_data(&root, &model, path));
        }
        println!("{}", json!({"models":reports}));
        return Ok(());
    }
    if args.len() > 3 || (command != "remove" && args.len() > 2) {
        return Err("Unexpected arguments; use model-manager --help".into());
    }
    let model = model_manager::selection_for(
        &root,
        &language,
        args.get(1).map(String::as_str).or(configured.as_deref()),
    )?;
    if custom_path.is_some() && !model.is_custom() {
        return Err("Clear CUSTOM_MODEL_PATH when inspecting/selecting a built-in model".into());
    }
    match command {
        "status" | "verify" => {
            let report = if let Some(path) = &custom_path {
                model_manager::status_data(&root, &model, path)
            } else {
                model_manager::status(&root, &model)
            };
            println!("{report}");
            if command == "verify" && report["valid"] != true {
                return Err("Model verification failed; existing files were not changed".into());
            }
        }
        "install" => {
            if let Some(path) = &custom_path {
                let _model = model_manager::prepare_data(&root, &model, path)?;
                println!("{}", model_manager::status_data(&root, &model, path));
            } else {
                let _model = model_manager::prepare(&root, &model, true)?;
                println!("{}", model_manager::status(&root, &model));
            }
        }
        "remove" => {
            if custom_path.is_some() {
                return Err("Removal of a selected custom package/cache is not automatic. Original .data and existing models are preserved; clear CUSTOM_MODEL_PATH before using scoped manifest-based removal".into());
            }
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
