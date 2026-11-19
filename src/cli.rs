use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "chronos-browser",
    author,
    version,
    about = "Ultra-fast, memory-efficient AI-first browser and MCP server in pure Rust"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Launch the Model Context Protocol (MCP) server over stdio (default)
    Mcp {
        /// Profile name to use
        #[arg(short, long, default_value = "default")]
        profile: String,

        /// Run browser with visible UI (headful mode)
        #[arg(long, default_value_t = false)]
        headful: bool,
    },

    /// Open a URL directly in browser, take a semantic snapshot, and print it
    Open {
        /// The URL to navigate to
        url: String,

        /// Run browser with visible UI (headful mode)
        #[arg(long, default_value_t = false)]
        headful: bool,

        /// Profile name to use
        #[arg(short, long, default_value = "default")]
        profile: String,
    },

    /// List all managed browser profiles
    Profiles,
}
