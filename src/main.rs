use clap::Parser;
use chronos_browser::cdp::launcher::{self, LaunchOptions};
use chronos_browser::cdp::target::PageTarget;
use chronos_browser::cdp::CdpClient;
use chronos_browser::cli::{Cli, Commands};
use chronos_browser::mcp::McpServer;
use chronos_browser::semantic::{take_semantic_snapshot, DOM_SETTLE_JS};
use chronos_browser::session::SessionManager;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    // Crucial: Route all tracing logs to stderr so stdout remains clean JSON-RPC for MCP
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Commands::Mcp {
        profile: "default".to_string(),
        headful: false,
    });

    match command {
        Commands::Mcp { profile, headful } => {
            let mut session_mgr = SessionManager::new();
            session_mgr.set_active_profile(profile);
            session_mgr.set_headful(headful);

            let mut server = McpServer::new(session_mgr);
            if let Err(e) = server.run_stdio().await {
                eprintln!("[chronos] MCP server error: {}", e);
                return ExitCode::FAILURE;
            }
        }

        Commands::Open {
            url,
            headful,
            profile,
        } => {
            eprintln!("[chronos] Opening {} with profile '{}' (headful: {})...", url, profile, headful);

            let mut session_mgr = SessionManager::new();
            session_mgr.set_active_profile(profile);
            session_mgr.set_headful(headful);

            let profile_dir = match session_mgr.ensure_profile_dir(session_mgr.active_profile()) {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("[chronos] Failed to create profile dir: {}", e);
                    return ExitCode::FAILURE;
                }
            };

            let options = LaunchOptions::new().headless(!headful).user_data_dir(profile_dir);
            let mut process = match launcher::launch(options).await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[chronos] Failed to launch browser: {}", e);
                    return ExitCode::FAILURE;
                }
            };

            let client = match CdpClient::connect(process.ws_url()).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[chronos] Failed to connect to CDP: {}", e);
                    let _ = process.shutdown().await;
                    return ExitCode::FAILURE;
                }
            };

            let target = match PageTarget::attach_or_create(client).await {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[chronos] Failed to attach target: {}", e);
                    let _ = process.shutdown().await;
                    return ExitCode::FAILURE;
                }
            };

            if let Err(e) = target.navigate(&url).await {
                eprintln!("[chronos] Navigation error: {}", e);
            }
            let _ = target.evaluate(DOM_SETTLE_JS).await;

            match take_semantic_snapshot(&target).await {
                Ok((_, snapshot)) => println!("{}", snapshot),
                Err(e) => eprintln!("[chronos] Snapshot error: {}", e),
            }

            let _ = process.shutdown().await;
        }

        Commands::Profiles => {
            let session_mgr = SessionManager::new();
            match session_mgr.list_profiles() {
                Ok(profiles) => {
                    println!("Profiles directory: {}", session_mgr.profiles_dir().display());
                    if profiles.is_empty() {
                        println!("No profiles created yet.");
                    } else {
                        println!("Existing profiles:");
                        for p in profiles {
                            println!("  - {}", p);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[chronos] Failed to list profiles: {}", e);
                    return ExitCode::FAILURE;
                }
            }
        }
    }

    ExitCode::SUCCESS
}
