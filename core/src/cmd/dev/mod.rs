mod handlers;
mod server;
mod watcher;

use std::net::{IpAddr, Ipv4Addr, TcpListener as StdTcpListener};
use std::sync::Arc;

use colored::Colorize;
use miette::{IntoDiagnostic, Result, bail};
use futures_util::StreamExt;
use hyper::Server;
use hyper::service::{make_service_fn, service_fn};
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tracing::{debug, error, info, instrument, warn};

use crate::fs;
use crate::shared;

use handlers::handle_server_request;
use server::setup_server_state;
use watcher::{process_debounced_events, setup_file_watcher};

#[instrument(skip(port, drafts, open, host))]
pub async fn dev(
    listener: StdTcpListener,
    port: u16,
    drafts: bool,
    open: bool,
    host: bool,
) -> Result<()> {
    println!("{} Starting development server...", "→".cyan().bold());

    let root = fs::find_config_file()?;
    let Some(root) = root else {
        bail!(
            "{}: not in a Norgolith site directory",
            "Could not initialize the development server".bold()
        );
    };

    debug!(path = %root.display(), "Found site root");

    let local_ip = local_ip_address::local_ip().unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    let routes_url = if host {
        format!("http://{}:{}", local_ip, port)
    } else {
        format!("http://localhost:{}", port)
    };
    let state = setup_server_state(root, drafts, routes_url).await?;
    let server_start = std::time::Instant::now();
    let rt = Handle::current();

    let _guard_receiver = state.reload_tx.subscribe();

    // WebSocket server
    let reload_tx = state.reload_tx.clone();
    let live_reload_port = 35729;
    tokio::spawn(async move {
        let listener = match TcpListener::bind(format!("127.0.0.1:{}", live_reload_port)).await {
            Ok(l) => l,
            Err(e) => {
                error!(
                    "LiveReload disabled: failed to bind port {}: {}",
                    live_reload_port, e
                );
                return;
            }
        };
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(handlers::handle_websocket(stream, reload_tx.clone()));
        }
    });

    // File watcher and event processing
    let (debouncer, mut debouncer_rx) = setup_file_watcher(state.clone(), rt.clone()).await?;
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        let _debouncer = debouncer;

        while let Some(result) = debouncer_rx.next().await {
            process_debounced_events(result, state_clone.clone()).await;
        }
    });

    // HTTP server
    let state_clone = Arc::clone(&state);
    let make_svc = make_service_fn(move |_| {
        let state = state_clone.clone();
        async move {
            Ok::<_, std::convert::Infallible>(service_fn(move |req| {
                handle_server_request(req, state.clone())
            }))
        }
    });
    listener.set_nonblocking(true).into_diagnostic()?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut stdin = tokio::io::stdin();
        let mut buf = [0u8; 1];
        loop {
            match stdin.read(&mut buf).await {
                Ok(0) | Err(_) => {
                    let _ = shutdown_tx.send(());
                    break;
                }
                _ => {}
            }
        }
    });

    let server = Server::from_tcp(listener).into_diagnostic()?
        .serve(make_svc)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });

    let localhost_address = format!(
        "{} {}   {}",
        "•".green(),
        "Local:".bold(),
        format!("http://localhost:{}/", port.to_string().cyan().bold()).blue()
    );
    let lan_address = if host {
        format!(
            "{} {} {}",
            "•".green(),
            "Network:".bold(),
            format!("http://{}:{}/", local_ip, port.to_string().cyan().bold()).blue()
        )
    } else {
        format!(
            "{} {} {} {} {}",
            "•".green().dimmed(),
            "Network:".bold().dimmed(),
            "use".dimmed(),
            "--host".bold(),
            "to expose".dimmed()
        )
    };
    println!(
        "Server started in {}\n{}\n{}\n\n{}\n",
        shared::get_elapsed_time(server_start),
        localhost_address,
        lan_address,
        "Press Ctrl-D to stop the server".dimmed()
    );

    if open {
        match open::that_detached(format!("http://localhost:{}/", port)) {
            Ok(()) => {
                info!("Opening the development server page using your browser ...");
            }
            Err(e) => warn!(
                "{}: {}",
                "Could not open the development server page".bold(),
                e
            ),
        };
    }

    if let Err(e) = server.await {
        bail!("{}: {}", "Server error".bold(), e);
    }

    println!("\n{} Development server stopped.", "→".cyan().bold());
    Ok(())
}
