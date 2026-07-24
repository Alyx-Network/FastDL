use std::net::SocketAddr;
use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::Router;

mod archive;
mod config;
mod directory;
mod handler;
mod rules;

use crate::handler::{handle_request, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_target(false).init();

    std::fs::create_dir_all("storage")?;
    std::fs::create_dir_all("auto")?;
    let storage_root = std::fs::canonicalize("storage")?;
    let auto_root = std::fs::canonicalize("auto")?;

    let config = Arc::new(ArcSwap::from_pointee(config::load()));
    let watcher = config::watch(config.clone());
    match &watcher {
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(error = %error, "Failed to start config watcher [config_watch_start_failed]")
        }
    }

    let archive_storage = storage_root.clone();
    let archive_auto = auto_root.clone();
    tokio::task::spawn_blocking(move || match archive::generate_all(&archive_storage, &archive_auto) {
        Ok(summary) => tracing::info!(files = summary.files, created = summary.created, skipped = summary.skipped, elapsed_ms = summary.elapsed_ms, "Generated archives [archives_generated]"),
        Err(error) => tracing::warn!(error = %error, "Failed to generate archives [archives_failed]"),
    });

    let archive_watcher = archive::watch(storage_root.clone(), auto_root.clone());
    match &archive_watcher {
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(error = %error, "Failed to start storage watcher [storage_watch_start_failed]")
        }
    }

    let snapshot = config.load();
    let rule_count = snapshot.rules.len();
    let port = std::env::var("PORT")
        .ok()
        .or_else(|| snapshot.global.port.map(|port| port.to_string()))
        .unwrap_or_else(|| "3000".to_string());
    let address: SocketAddr = format!("0.0.0.0:{port}").parse()?;

    let state = AppState {
        config,
        storage_root: storage_root.clone(),
        auto_root: auto_root.clone(),
    };
    let app = Router::new().fallback(handle_request).with_state(state);

    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(port = %port, storage = %storage_root.display(), rules = rule_count, "Server started [server_started]");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    drop(watcher);
    drop(archive_watcher);
    Ok(())
}
