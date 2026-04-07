//! MCP server binary for rs_cam.
//!
//! Usage: `rs_cam_mcp <project.toml>`
//!
//! Loads a project file, exposes CAM tools via MCP stdio transport.
//! All logging goes to stderr; stdout is reserved for MCP JSON-RPC.

mod server;

use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use rmcp::ServiceExt;
use rs_cam_core::session::ProjectSession;
use server::CamServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Logging to stderr (stdout is MCP transport)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let args: Vec<String> = std::env::args().collect();

    let session = if let Some(project_path) = args.get(1) {
        tracing::info!(path = %project_path, "Loading project");
        let s = ProjectSession::load(Path::new(project_path))
            .map_err(|e| format!("Failed to load project: {e}"))?;
        tracing::info!(
            name = %s.name(),
            toolpaths = s.toolpath_count(),
            setups = s.setup_count(),
            "Project loaded"
        );
        Some(s)
    } else {
        tracing::info!("No project file specified — use load_project tool to load one");
        None
    };

    let cam_server = CamServer::new(Arc::new(Mutex::new(session)));

    tracing::info!("Starting MCP server on stdio");
    let service = cam_server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| format!("MCP serve error: {e}"))?;

    service.waiting().await?;
    tracing::info!("MCP server shut down");
    Ok(())
}
