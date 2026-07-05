// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::net::TcpListener; // TCP listener for accepting incoming client connections.
use std::sync::Arc; // Allows multiple threads to safely share ownership of the same data.
use std::time::Duration; // Represents a span of time (e.g., for sleeping or timeouts).

use crate::client::http_handle_client; // Spawns a thread to handle the client connection.

pub fn tcp_listen(docroot: &str, ip: &str, port: u16) -> std::io::Result<()> {
    // Start listening for connections on the given IP address and port.
    let listener = TcpListener::bind((ip, port))?;

    // Share the document root between all client threads.
    let docroot = Arc::new(docroot.to_owned());

    // Keep accepting new client connections.
    loop {
        let (stream, _) = listener.accept()?;

        // Stop waiting if the client takes too long to send or receive data.
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        // Hand the client connection to the handler, which will spawn a worker thread.
        http_handle_client(stream, Arc::clone(&docroot));
    }
}
