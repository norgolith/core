use std::net::{Ipv4Addr, SocketAddr, TcpListener};

use miette::{Result, miette};

/// Bind to the given address, returning the listener if successful.
///
/// This eliminates the TOCTOU race of check-then-act: the caller
/// holds the bound listener and passes it directly to the server.
pub fn bind_available(port: u16, host: bool) -> Result<TcpListener> {
    let addr: SocketAddr = if host {
        (Ipv4Addr::UNSPECIFIED, port).into()
    } else {
        (Ipv4Addr::LOCALHOST, port).into()
    };
    tracing::debug!(%port, %host, "Binding to port");
    TcpListener::bind(addr).map_err(|e| {
        let label = if port == 3030 {
            "default Norgolith port (3030)"
        } else {
            "requested port"
        };
        miette!("Could not bind to {} ({}): {}", label, port, e)
    })
}
