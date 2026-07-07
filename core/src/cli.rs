use std::env::set_current_dir;
use std::path::PathBuf;

use clap::{Parser, Subcommand, builder::PossibleValue};
use eyre::{Result, eyre};

use crate::cmd;
use crate::net;

#[derive(Parser)]
#[command(
    author = "NTBBloodbath",
    version,
    disable_version_flag = true,
    about = "The monolithic Norg static site generator"
)]
struct Cli {
    /// Print version
    #[arg(short = 'v', long, action = clap::builder::ArgAction::Version)]
    version: (),

    /// Operate on the project in the given directory.
    #[arg(short = 'd', long = "dir", global = true)]
    project_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Initialize a new Norgolith site
    Init {
        #[arg(
            long,
            default_value_t = true,
            overrides_with = "_no_prompt",
            help = "Whether to prompt for site info"
        )]
        prompt: bool,

        #[arg(long = "no-prompt")]
        _no_prompt: bool,

        /// Site name
        name: String,
    },
    /// Theme management
    Theme {
        #[command(subcommand)]
        subcommand: cmd::ThemeCommands,
    },
    /// Run a site in development mode
    Dev {
        #[arg(short = 'p', long, default_value_t = 3030, help = "Port to be used")]
        port: u16,

        #[arg(
            long,
            default_value_t = true,
            overrides_with = "_no_drafts",
            help = "Whether to serve draft content"
        )]
        drafts: bool,

        #[arg(long = "no-drafts")]
        _no_drafts: bool,

        // TODO: add SocketAddr parsing if host is a String, similar to Vite
        #[arg(
            short = 'e',
            long,
            default_value_t = false,
            help = "Expose site to LAN network"
        )]
        host: bool,

        #[arg(
            short = 'o',
            long,
            default_value_t = false,
            help = "Open the development server in your browser"
        )]
        open: bool,
    },
    /// Create a new asset in the site and optionally open it using your preferred system editor.
    /// e.g. 'new -k norg post1.norg' -> 'content/post1.norg'
    New {
        #[arg(
            short = 'o',
            long,
            default_value_t = false,
            help = "Open the new file using your preferred system editor"
        )]
        open: bool,

        #[arg(
            short = 'k',
            long,
            default_value = "norg",
            help = "type of asset",
            value_parser = [
                PossibleValue::new("norg").help("New norg file"),
                PossibleValue::new("css").help("New CSS stylesheet"),
                PossibleValue::new("js").help("New JS script"),
                PossibleValue::new("post").help("New post in a configured collection")
            ]
        )]
        kind: Option<String>,

        /// Asset name, e.g. 'post1.norg' or 'hello.js'
        name: Option<String>,

        #[arg(
            short = 'c',
            long = "collection",
            help = "Target collection name (only used with --kind post)"
        )]
        collection: Option<String>,
    },
    /// Build a site for production
    Build {
        #[arg(
            short = 'm',
            long,
            default_value_t = true,
            overrides_with = "_no_minify",
            help = "Minify the produced assets"
        )]
        minify: bool,

        #[arg(long = "no-minify")]
        _no_minify: bool,
    },
    /// Plugin management
    Plugin {
        #[command(subcommand)]
        subcommand: cmd::PluginCommands,
    },
    /// Preview from build result
    Preview {
        #[arg(short = 'p', long, default_value_t = 3030, help = "Port to be used")]
        port: u16,

        // TODO: add SocketAddr parsing if host is a String, similar to Vite
        #[arg(
            short = 'e',
            long,
            default_value_t = false,
            help = "Expose site to LAN network"
        )]
        host: bool,

        #[arg(
            short = 'o',
            long,
            default_value_t = false,
            help = "Open the development server in your browser"
        )]
        open: bool,
    },
}

/// Asynchronously parse the command-line arguments and executes the corresponding subcommand
///
/// # Returns:
///   A `Result<()>` indicating success or error. On error, the context message will provide information on why the subcommand failed.
pub async fn start() -> Result<()> {
    let cli = Cli::parse();

    if let Some(dir) = cli.project_dir {
        set_current_dir(dir)?;
    }

    match cli.command {
        Commands::Init {
            name,
            prompt: _,
            _no_prompt,
        } => cmd::init(&name, !_no_prompt).await?,
        Commands::Theme { subcommand } => cmd::theme(&subcommand).await?,
        Commands::Dev {
            port,
            drafts: _,
            _no_drafts,
            host,
            open,
        } => run_dev_server(port, !_no_drafts, open, host).await?,
        Commands::Build {
            minify: _,
            _no_minify,
        } => cmd::build(!_no_minify)?,
        Commands::Plugin { subcommand } => cmd::plugin(&subcommand)?,
        Commands::New {
            kind,
            name,
            open,
            collection,
        } => {
            let kind = kind.unwrap_or_else(|| "norg".to_string());
            let name = name.ok_or_else(|| eyre!("Unable to create site asset: missing name for the asset"))?;
            cmd::new(&kind, &name, open, collection.as_ref()).await?
        }
        Commands::Preview { port, host, open } => cmd::preview(port, open, host).await?,
    }

    Ok(())
}

/// Checks port availability and starts the development server.
async fn run_dev_server(port: u16, drafts: bool, open: bool, host: bool) -> Result<()> {
    let listener = net::bind_available(port, host)?;
    cmd::dev(listener, port, drafts, open, host).await
}

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use tempfile::tempdir;

    use super::*;

    // init_site tests
    #[tokio::test]
    #[serial]
    async fn test_init_site_with_name() -> Result<()> {
        let dir = tempdir()?;

        let origin = std::env::current_dir()?;
        std::env::set_current_dir(dir.path())?;

        let test_name = String::from("my-site");
        let result = cmd::init(&test_name, false).await;
        assert!(result.is_ok());

        std::env::set_current_dir(origin)?;

        Ok(())
    }

    #[tokio::test]
    #[cfg_attr(feature = "ci", ignore)]
    #[serial]
    async fn test_check_and_serve() -> Result<()> {
        let dir = tempdir()?;

        let origin = std::env::current_dir()?;
        std::env::set_current_dir(dir.path())?;

        // Bind port
        let temp_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = temp_listener.local_addr()?.port();

        // Create temporal site
        let test_site_name = String::from("my-unavailable-site");
        cmd::init(&test_site_name, false).await.unwrap();

        // Enter the test directory
        let path = dir.path().join(&test_site_name);

        std::env::set_current_dir(path)?;

        let result = run_dev_server(port, false, false, false).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .root_cause()
                .to_string()
                .contains("Could not bind")
        );

        // Restore previous directory
        std::env::set_current_dir(origin)?;

        Ok(())
    }
}
