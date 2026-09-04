use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use common::profile::PicsProfile;
use common::profile::validation::Validated;
use poem::Server;
use poem::listener::TcpListener;
use tracing_subscriber::prelude::*;

use web_server::models::scenario::Scenario;
use web_server::services::job_service::JobService;
use web_server::startup_checks::assert_no_symlinks;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    let log_directory = match std::env::var_os("LOG_DIR") {
        Some(path) => std::path::PathBuf::from(path),
        None => std::env::current_dir()?.join("tmp"),
    };
    std::fs::create_dir_all(&log_directory)?;
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_directory.join("web_server.log"))?;

    // Write detailed server logs to a new file and to the console.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_file(true)
                .with_line_number(true)
                .with_target(false)
                .with_writer(move || {
                    log_file
                        .try_clone()
                        .expect("cannot clone the web server log file handle")
                })
                .with_ansi(false)
                .with_filter(tracing_subscriber::filter::LevelFilter::DEBUG),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_file(true)
                .with_line_number(true)
                .with_target(false)
                .with_writer(std::io::stdout)
                .with_ansi(true)
                .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
        )
        .init();

    const DEFAULT_DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data");
    const DEFAULT_FRONTEND_DIR: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../frontend/dist");
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string());
    let frontend_dir =
        std::env::var("FRONTEND_DIR").unwrap_or_else(|_| DEFAULT_FRONTEND_DIR.to_string());
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let bind_addr = format!("{host}:{port}");

    // Reject symlinks inside the static-served directories before binding.
    // Poem's `StaticFilesEndpoint` follows symlinks; an attacker (or a
    // careless symlink during dev) could otherwise escape these roots and
    // serve arbitrary process-readable files.
    assert_no_symlinks("FRONTEND_DIR", Path::new(&frontend_dir))?;
    assert_no_symlinks("DATA_DIR", Path::new(&data_dir))?;

    // Load the base profile and generate scenarios dynamically
    let base_profile_path = std::path::Path::new(&data_dir).join("profiles/full.json");
    let base_profile: Validated<PicsProfile> = {
        let text = std::fs::read_to_string(&base_profile_path).unwrap_or_else(|e| {
            tracing::error!("Failed to read {}: {e}", base_profile_path.display());
            std::process::exit(1);
        });
        let parsed_profile: PicsProfile = serde_json::from_str(&text).unwrap_or_else(|e| {
            tracing::error!("Failed to parse {}: {e}", base_profile_path.display());
            std::process::exit(1);
        });
        PicsProfile::into_validated(parsed_profile).context("Validating full.json")?
    };
    let scenarios: Vec<Scenario> = scenario_generator::generate_scenarios(&base_profile);
    tracing::info!("Generated {} scenarios from profile", scenarios.len());

    let scenarios_arc = Arc::new(scenarios);
    let job_service = Arc::new(JobService::with_scenarios(
        data_dir,
        scenarios_arc.clone(),
        Arc::new(base_profile),
    ));
    let scenarios_for_app: Vec<Scenario> = scenarios_arc.as_ref().clone();
    let app = web_server::build_app(job_service, scenarios_for_app, frontend_dir);

    tracing::info!("MESA web server starting on {bind_addr}");

    Server::new(TcpListener::bind(&bind_addr)).run(app).await?;
    Ok(())
}
