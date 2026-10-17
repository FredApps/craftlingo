use craftlingo::{App, Catalog, Policy};
use serde::de::DeserializeOwned;
use std::io::{Read, Write};

fn load<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    const LIMIT: u64 = 8 * 1024 * 1024;
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut data = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut data)
        .map_err(|e| format!("{path}: {e}"))?;
    if data.len() as u64 > LIMIT {
        return Err(format!("{path}: input exceeds 8 MiB"));
    }
    serde_json::from_slice(&data).map_err(|e| format!("{path}: {e}"))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let usage = "usage: craftlingo check CATALOG POLICY | export CATALOG POLICY wordcraft|pdfcraft";
    let [command, catalog_path, policy_path, rest @ ..] = args.as_slice() else {
        return Err(usage.into());
    };
    let catalog: Catalog = load(catalog_path)?;
    let policy: Policy = load(policy_path)?;
    match (command.as_str(), rest) {
        ("check", []) => {
            craftlingo::validate(&catalog, &policy)?;
            println!(
                "Valid v1 catalog: {} messages; validation is not translation approval.",
                catalog.messages.len()
            );
        }
        ("export", [app]) => {
            let app = match app.as_str() {
                "wordcraft" => App::Wordcraft,
                "pdfcraft" => App::Pdfcraft,
                _ => return Err(usage.into()),
            };
            let result = craftlingo::export(&catalog, &policy, app)?;
            std::io::stdout()
                .lock()
                .write_all(result.tsv.as_bytes())
                .map_err(|e| e.to_string())?;
            eprintln!(
                "{} approved rows; {} pending. Overlay only; do not replace an app catalog.",
                result.exported,
                result.pending.len()
            );
        }
        _ => return Err(usage.into()),
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
