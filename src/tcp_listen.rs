// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::sync::Arc; // Handle shared ownership.
use tokio::net::TcpListener; // TCP listener for accepting incoming client connections.

use crate::client::http_handle_client; // Handle the client connection.

// #############################################################################################
// TCP Listener function
// Lisens for connecting clients and pawns a new task that handles the client connection.
// #############################################################################################
pub async fn tcp_listen(docroot: &str, ip: &str, port: u16) -> std::io::Result<()> {
    // Start listening for connections on the given IP address and port.
    let listener = TcpListener::bind((ip, port)).await?;

    // Share the document root between all client threads.
    let docroot = Arc::new(docroot.to_owned());

    // Keep accepting new client connections.
    loop {
        // Accept incoming TCP connection asynchronously (discard client IP address for now).
        let (stream, _) = listener.accept().await?;

        // Share the ownership of document root to the new task.
        let docroot = Arc::clone(&docroot);

        // Spawn a task and let it handle the client connection.
        tokio::spawn(async move {
            http_handle_client(stream, docroot).await;
        });
    }
}
