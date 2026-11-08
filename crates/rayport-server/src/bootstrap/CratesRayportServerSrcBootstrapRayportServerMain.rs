use rayport_adapters::SnapshotStore;
use rayport_application::Workspace;
use rayport_server::{
    apic::{DisabledExecutor, HttpApicExecutor, SourceExecutor},
    config::{initialize, Config},
    http::{router, AppState},
};
use std::{path::Path, sync::Arc};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Rayport: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args == ["--help"] {
        println!("Rayport 1.0.0\n  init --directory PATH\n  check --config PATH\n  serve --config PATH\nOnly serve starts a listener. init writes the first login token to a private file.");
        return Ok(());
    }
    if args == ["--version"] {
        println!("rayport-server {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.len() != 3 {
        return Err(
            "expected init --directory PATH, check --config PATH, or serve --config PATH".into(),
        );
    }
    if args[0] == "init" && args[1] == "--directory" {
        initialize(Path::new(&args[2]))?;
        println!("Initialized private configuration. Read initial-admin-token.txt locally, then configure allowed stores and web_directory. No service started.");
        return Ok(());
    }
    if args[1] != "--config" || !matches!(args[0].as_str(), "check" | "serve") {
        return Err("invalid command; use --help".into());
    }
    let config = Config::load(Path::new(&args[2]))?;
    if args[0] == "check" {
        println!(
            "Configuration valid. Source execution: {}. No service or source connection started.",
            if config.apic.is_some() {
                "configured; external integration not verified"
            } else {
                "disabled"
            }
        );
        return Ok(());
    }
    if !config.web_directory.join("index.html").is_file() {
        return Err("web_directory must contain the compiled React index.html".into());
    }
    let store = SnapshotStore::open(
        &config.data_directory,
        Workspace::new(config.tenant_id.clone()),
    )
    .map_err(|_| "cannot safely open workspace snapshot")?;
    let snapshot = store.read().map_err(|_| "cannot read workspace")?;
    if snapshot.schema_version != 1
        || snapshot.tenant_id != config.tenant_id
        || snapshot.marts.iter().any(|m| m.versions.is_empty())
    {
        return Err("workspace schema or tenant mismatch; explicit migration required".into());
    }
    store
        .transaction(|w| {
            w.recover();
            Ok(())
        })
        .map_err(|_| "cannot recover execution journal")?;
    let executor: Arc<dyn SourceExecutor> = if let Some(c) = config.apic.clone() {
        Arc::new(HttpApicExecutor::new(c)?)
    } else {
        Arc::new(DisabledExecutor)
    };
    let address = config.bind.clone();
    let state = AppState::new(config, store, executor);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|_| "cannot bind configured address")?;
    eprintln!(
        "Rayport {} listening on {address}",
        env!("CARGO_PKG_VERSION")
    );
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|_| "HTTP service failed".into())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}}
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
