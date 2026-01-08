//! opencomplete - AI-powered code completion agent for Neovim
//!
//! A local daemon that provides streaming code completions to a Neovim plugin.
//! Supports Claude (via API key) and Ollama (local models).

mod config;
mod providers;
mod server;

use crate::config::Config;
use crate::providers::claude::ClaudeProvider;
use crate::providers::ollama::OllamaProvider;
use crate::providers::{ProviderConfig, ProviderRegistry};
use crate::server::{AppState, run_server};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "opencomplete")]
#[command(about = "AI-powered code completion agent for Neovim")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Port to listen on (overrides config)
    #[arg(short, long)]
    port: Option<u16>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the completion server (default)
    Serve,

    /// Show or generate configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current config path
    Path,
    /// Print example config
    Example,
    /// Show current config
    Show,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Set up logging
    let log_level = if cli.debug { "debug" } else { "info" };
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| format!("opencomplete={}", log_level)),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => {
            serve(cli.port).await?;
        }
        Commands::Config { action } => {
            handle_config(action)?;
        }
    }

    Ok(())
}

async fn serve(port_override: Option<u16>) -> anyhow::Result<()> {
    let config = Config::load()?;
    let port = port_override.unwrap_or(config.server.port);

    tracing::info!("Loading providers...");

    let mut registry = ProviderRegistry::new();

    for provider_config in &config.providers {
        match provider_config {
            ProviderConfig::Claude(claude_config) => {
                match ClaudeProvider::new(claude_config.clone()) {
                    Ok(provider) => {
                        tracing::info!("Loaded Claude provider (model: {})", claude_config.model);
                        registry.register(Arc::new(provider));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load Claude provider: {}", e);
                    }
                }
            }

            ProviderConfig::Ollama(ollama_config) => {
                let provider = OllamaProvider::new(ollama_config.clone());
                tracing::info!("Loaded Ollama provider (model: {})", ollama_config.model);
                registry.register(Arc::new(provider));
            }
        }
    }

    if registry.list().is_empty() {
        tracing::error!("No providers loaded! Set ANTHROPIC_API_KEY or configure Ollama.");
        anyhow::bail!("No providers available");
    }

    if let Some(default) = &config.default_provider {
        registry.set_default(default);
    }

    let state = Arc::new(AppState {
        providers: RwLock::new(registry),
    });

    run_server(state, port).await?;

    Ok(())
}

fn handle_config(action: ConfigAction) -> anyhow::Result<()> {
    match action {
        ConfigAction::Path => {
            let home = std::env::var("HOME")
                .map_err(|_| anyhow::anyhow!("Could not determine home directory"))?;
            println!("{}/.config/opencomplete/config.toml", home);
        }
        ConfigAction::Example => {
            println!("{}", config::example_config());
        }
        ConfigAction::Show => {
            let config = Config::load()?;
            println!("{}", toml::to_string_pretty(&config)?);
        }
    }

    Ok(())
}
