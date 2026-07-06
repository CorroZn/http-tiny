// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::Path; // Filesystem path type for working with file and directory paths.
use std::sync::Arc; // Handle shared ownership.
use tokio::net::TcpStream; // Tokio TCP stream type used for network connections.

use crate::parse_request::http_parse_validate; // Parses and validates incoming HTTP requests.
use crate::response::http_response; // Serves HTTP responses to connected clients.

// #############################################################################################
// HTTP client handler function
// Handles the client connection, parses and validates the request header
// #############################################################################################
pub async fn http_handle_client(stream: TcpStream, docroot: Arc<String>) {
    // Create a new thread to handle this client connection.
    // This stream is used to read the request and send the response.
    let mut stream = stream;

    // Check if the request is valid and get the requested file path.
    if let Some(path) = http_parse_validate(&mut stream, Path::new(docroot.as_str())).await {
        // Send the requested file, or a 404 page if it doesn't exist.
        let _ = http_response(stream, &path).await;
    }
    // The connection is closed automatically when the thread ends.
}
