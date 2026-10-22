// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::sync::Arc; // Handle shared ownership.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream}; // TCP listener and stream for client connections.
use tokio::sync::{OwnedSemaphorePermit, Semaphore}; // Limit the number of active clients.
use tokio::time::sleep;

use crate::client::http_handle_client; // Handle the client connection.
use crate::response::DocumentRoot;

// Keep per-connection task and buffer use bounded under load.
const MAX_CLIENTS: usize = 64;
const ACCEPT_RETRY_INITIAL: Duration = Duration::from_millis(10);
const ACCEPT_RETRY_MAX: Duration = Duration::from_secs(1);
static ACCEPT_FAILURES: AtomicUsize = AtomicUsize::new(0);

// #############################################################################################
// TCP listener function.
// Listens for incoming client connections and spawns a new task to
// handle each connection.
// #############################################################################################
pub async fn tcp_listen(docroot: &str, ip: &str, port: u16) -> std::io::Result<()> {
    // Open the document root once before sharing it with client tasks.
    let docroot = Arc::new(DocumentRoot::open(docroot)?);

    // Start listening for connections on the given IP address and port.
    let listener = TcpListener::bind((ip, port)).await?;

    // Share the document root between all client tasks.
    let clients = Arc::new(Semaphore::new(MAX_CLIENTS));

    // Keep accepting new client connections until a fatal I/O error occurs.
    let mut retry_delay = ACCEPT_RETRY_INITIAL;
    loop {
        let (stream, permit) = match accept_client(&listener, &clients).await {
            Ok(client) => {
                retry_delay = ACCEPT_RETRY_INITIAL;
                client
            }
            Err(error) if is_transient_accept_error(&error) => {
                let failures = ACCEPT_FAILURES.fetch_add(1, Ordering::Relaxed);
                if failures & 1023 == 0 {
                    eprintln!("http-tiny: transient accept error ({:?})", error.kind());
                }
                sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(ACCEPT_RETRY_MAX);
                continue;
            }
            Err(error) => return Err(error),
        };

        // Clone the shared document root for the new task.
        let docroot = Arc::clone(&docroot);

        // Spawn a task and let it handle the client connection.
        tokio::spawn(async move {
            let _permit = permit;
            http_handle_client(stream, docroot).await;
        });
    }
}

fn is_transient_accept_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::TimedOut
    )
}

async fn accept_client(
    listener: &TcpListener,
    clients: &Arc<Semaphore>,
) -> io::Result<(TcpStream, OwnedSemaphorePermit)> {
    let permit = Arc::clone(clients)
        .acquire_owned()
        .await
        .map_err(|_| io::Error::other("client limit closed"))?;
    let (stream, _) = listener.accept().await?;
    Ok((stream, permit))
}

#[cfg(test)]
mod tests {
    use super::{accept_client, is_transient_accept_error};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::Semaphore;
    use tokio::time::timeout;

    #[test]
    fn classifies_transient_accept_errors() {
        for kind in [
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::WouldBlock,
            std::io::ErrorKind::ConnectionAborted,
            std::io::ErrorKind::ConnectionReset,
            std::io::ErrorKind::TimedOut,
        ] {
            assert!(is_transient_accept_error(&std::io::Error::from(kind)));
        }
        assert!(!is_transient_accept_error(&std::io::Error::from(
            std::io::ErrorKind::PermissionDenied
        )));
    }

    #[tokio::test]
    async fn waits_for_capacity_before_accepting_another_client() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let clients = Arc::new(Semaphore::new(1));
        let held_permit = Arc::clone(&clients).acquire_owned().await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let mut accept_task = tokio::spawn(async move { accept_client(&listener, &clients).await });

        assert!(
            timeout(Duration::from_millis(50), &mut accept_task)
                .await
                .is_err()
        );

        drop(held_permit);
        let accepted = timeout(Duration::from_secs(1), &mut accept_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        drop(accepted);
        drop(client);
    }
}
