use std::{path::PathBuf, process};

use anyhow::Result;
use clap::Parser;

use pics_validator::{load_json_profile, load_xlsx_profile};

#[derive(Parser)]
#[command(
    name = "pics-validator",
    about = "Load an IEEE 1815.2 PICS spreadsheet or JSON profile and export to JSON."
)]
struct Cli {
    /// Path to .xlsx or .json input file
    input: PathBuf,

    /// Output JSON path
    output: PathBuf,
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_file(true)
        .with_line_number(true)
        .init();

    if let Err(e) = run() {
        eprintln!("Error while running pics_validator: {:#}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if !cli.input.exists() {
        anyhow::bail!("Input file not found: {}", cli.input.display());
    }

    let is_json = cli.input.extension().and_then(|e| e.to_str()) == Some("json");

    tracing::info!(input = %cli.input.display(), "Loading and validating profile");

    let json = if is_json {
        let profile = load_json_profile(&cli.input)?;
        serde_json::to_string_pretty(&profile)?
    } else {
        let profile = match load_xlsx_profile(&cli.input) {
            Ok(profile) => profile,
            Err(errors) => {
                tracing::error!(input = %cli.input.display(), count = errors.len(), "Profile validation failed");
                for (index, error) in errors.errors.iter().enumerate() {
                    tracing::error!(number = index + 1, point = %error.point, "{}", error.message);
                }
                std::process::exit(1);
            }
        };
        serde_json::to_string_pretty(&profile)?
    };

    std::fs::write(&cli.output, json)?;

    println!("Written to: {}", cli.output.display());
    Ok(())
}
