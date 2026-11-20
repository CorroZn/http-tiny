// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc; // Handle shared ownership.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::net::{TcpListener, TcpSocket, TcpStream}; // TCP listener and stream for client connections.
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch}; // Limit the number of active clients.
use tokio::task::JoinSet;
use tokio::time::sleep;

use crate::connection::http_handle_client; // Handle the client connection.
use crate::static_files::DocumentRoot;

// Keep per-connection task and buffer use bounded under load.
const MAX_CLIENTS: usize = 64;
const ACCEPT_RETRY_INITIAL: Duration = Duration::from_millis(10);
const ACCEPT_RETRY_MAX: Duration = Duration::from_secs(1);
static ACCEPT_FAILURES: AtomicUsize = AtomicUsize::new(0);

/// Upper bound for waiting on in-flight responses during shutdown.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Kernel accept-queue depth requested for the listening socket.
///
/// `TcpListener::bind` resolves to the standard library's backlog of 128, which
/// overflows as soon as more clients are waiting than `MAX_CLIENTS` admits,
/// so the backlog is raised explicitly. `TcpSocket` also drops the
/// `SO_REUSEADDR` option that `TcpListener::bind` used to set, so the socket
/// restores it before binding.
///
/// The kernel clamps the value to `net.core.somaxconn`, so the effective queue
/// depth is `min(LISTEN_BACKLOG, somaxconn)`.
const LISTEN_BACKLOG: u32 = 16384;

// #############################################################################################
// TCP listener function.
// Listens for incoming client connections and spawns a new task to
// handle each connection.
// #############################################################################################
pub(crate) async fn tcp_listen(
    docroot: &str,
    ip: &str,
    port: u16,
    mut shutdown: watch::Receiver<bool>,
) -> std::io::Result<()> {
    // Open the document root once before sharing it with client tasks.
    let docroot_path = docroot;
    let docroot = Arc::new(DocumentRoot::open(docroot)?);

    // Start listening for connections on the given IP address and port.
    let ip = ip
        .parse::<IpAddr>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid IP address"))?;
    let listener = match ip {
        IpAddr::V4(_) => {
            let socket = TcpSocket::new_v4()?;
            socket.set_reuseaddr(true)?;
            socket.bind(SocketAddr::new(ip, port))?;
            socket.listen(LISTEN_BACKLOG)?
        }
        IpAddr::V6(_) => {
            let socket = TcpSocket::new_v6()?;
            socket.set_reuseaddr(true)?;
            socket.bind(SocketAddr::new(ip, port))?;
            socket.listen(LISTEN_BACKLOG)?
        }
    };

    // Report the bound address only once the server is actually listening.
    let _ = writeln!(
        io::stdout().lock(),
        "HTTP server started: Serving {docroot_path} at {ip} on port {port}"
    );

    // Share the document root between all client tasks.
    let clients = Arc::new(Semaphore::new(MAX_CLIENTS));
    let mut connections = JoinSet::new();

    // Keep accepting new client connections until a fatal I/O error occurs.
    let mut retry_delay = ACCEPT_RETRY_INITIAL;
    loop {
        let (stream, permit) = tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
            accepted = accept_client(&listener, &clients) => match accepted {
                Ok(client) => {
                    retry_delay = ACCEPT_RETRY_INITIAL;
                    client
                }
                Err(error) if is_transient_accept_error(&error) => {
                    let failures = ACCEPT_FAILURES.fetch_add(1, Ordering::Relaxed);
                    // trailing_zeros() >= 10 is true for 0 and for every
                    // multiple of 1024: log the first transient error, then one
                    // per 1024.
                    if failures.trailing_zeros() >= 10 {
                        crate::logging::log(format!(
                            "http-tiny: transient accept error ({:?})\n",
                            error.kind()
                        ));
                    }
                    sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(ACCEPT_RETRY_MAX);
                    continue;
                }
                Err(error) => return Err(error),
            },
        };

        // Clone the shared document root for the new task.
        let docroot = Arc::clone(&docroot);

        // Spawn a task and let it handle the client connection.
        connections.spawn(async move {
            let _permit = permit;
            http_handle_client(stream, docroot).await;
        });
        // Reap finished connections: JoinSet keeps completed tasks until they are
        // joined, so leaving them unjoined grows memory without bound.
        while connections.try_join_next().is_some() {}
    }

    // Stop accepting, then give in-flight responses a bounded window to finish.
    let _ = tokio::time::timeout(SHUTDOWN_GRACE, async {
        while connections.join_next().await.is_some() {}
    })
    .await;

    Ok(())
}

pub(crate) fn is_transient_accept_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::TimedOut
    )
}

pub(crate) async fn accept_client(
    listener: &TcpListener,
    clients: &Arc<Semaphore>,
) -> io::Result<(TcpStream, OwnedSemaphorePermit)> {
    let permit = Arc::clone(clients)
        .acquire_owned()
        .await
        .map_err(|_| io::Error::other("client limit closed"))?;
    let (stream, _) = listener.accept().await?;
    // Nagle only affects latency; failing to disable it is not fatal.
    let _ = stream.set_nodelay(true);
    Ok((stream, permit))
}
