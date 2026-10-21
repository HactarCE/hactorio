//! Builds a registry cache from Factorio's dump output.
//!
//! Usage: postprocess <data-raw-dump.json> <locale-dir> <out.json>

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use hactorio_data::{Locale, postprocess};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!("usage: postprocess <data-raw-dump.json> <locale-dir> <out.json>");
        return ExitCode::FAILURE;
    }
    let dump = PathBuf::from(&args[0]);
    let locale_dir = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);

    let raw = match std::fs::read_to_string(&dump) {
        Ok(raw) => raw,
        Err(error) => {
            eprintln!("failed to read {}: {error}", dump.display());
            return ExitCode::FAILURE;
        }
    };

    let registry = match postprocess(&raw, &Locale::load(&locale_dir)) {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("post-processing failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = registry.save(&out) {
        eprintln!("failed to write {}: {error}", out.display());
        return ExitCode::FAILURE;
    }

    let machines = registry
        .entries()
        .iter()
        .filter(|entry| entry.machine().is_some())
        .count();
    let modules = registry
        .entries()
        .iter()
        .filter(|entry| entry.module.is_some())
        .count();
    let synthetic = registry
        .recipes()
        .iter()
        .filter(|recipe| recipe.synthetic)
        .count();
    println!(
        "{} entries, {} recipes ({synthetic} synthetic), {machines} machines, {modules} modules",
        registry.entries().len(),
        registry.recipes().len()
    );

    match hactorio_data::Registry::load(&out) {
        Ok(loaded) if loaded.entries().len() == registry.entries().len() => {
            println!("wrote and reloaded {}", out.display());
            ExitCode::SUCCESS
        }
        Ok(loaded) => {
            eprintln!(
                "reloaded cache has {} entries, expected {}",
                loaded.entries().len(),
                registry.entries().len()
            );
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("failed to reload {}: {error}", out.display());
            ExitCode::FAILURE
        }
    }
}
