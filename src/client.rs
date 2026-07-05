// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::net::TcpStream; // TCP stream type used for network connections.
use std::path::Path; // Filesystem path type for working with file and directory paths.
use std::sync::Arc; // Allows multiple threads to safely share ownership of the document root.
use std::thread; // Used for spawning a thread to handle the client connection.

use crate::parse_request::http_parse_validate; // Parses and validates incoming HTTP requests.
use crate::response::http_response; // Serves HTTP responses to connected clients.

// #############################################################################################
// HTTP client handler
//
// Spawns a new thread to handle the client connection.
// #############################################################################################
pub fn http_handle_client(stream: TcpStream, docroot: Arc<String>) {
    // Create a new thread to handle this client connection.
    thread::spawn(move || {
        // This stream is used to read the request and send the response.
        let mut stream = stream;

        // Check if the request is valid and get the requested file path.
        if let Some(path) = http_parse_validate(&mut stream, Path::new(docroot.as_str())) {
            // Send the requested file, or a 404 page if it doesn't exist.
            http_response(stream, &path);
        }

        // The connection is closed automatically when the thread ends.
    });
}
