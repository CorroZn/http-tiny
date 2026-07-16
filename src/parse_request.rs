// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt; // Enable Tokio reading methods on I/O types like File and TcpStream
use tokio::net::TcpStream; // Tokio TCP stream type used for network communication.
use tokio::time::{Duration, timeout}; // Drop the worker after a specific timeout // Filesystem path type for working with file and directory paths

use crate::MAX_REQUEST; // Used for checking if the HTTP request exceeds the size limit
use crate::validate_request::{HttpRequest, validate_and_resolve}; // Validates the HTTP request

// #############################################################################################
// HTTP request parsing and validation.
// Reads the HTTP request, parses the request line, validates it,
// and resolves the requested filesystem path.
// #############################################################################################
pub async fn http_parse_validate(stream: &mut TcpStream, docroot: &Path) -> Option<PathBuf> {
    // Read raw HTTP request from socket
    let raw = read_request(stream).await?;

    // Convert bytes → string (lossy to avoid UTF-8 hard failure)
    let request = String::from_utf8_lossy(&raw);

    // Parse the request line (method, URL, version).
    let parsed = parse_request(&request)?;

    // Validate + convert URL into safe filesystem path
    validate_and_resolve(parsed, docroot)
}

// #############################################################################################
// Parses the HTTP request line:
//
// GET /index.html HTTP/1.1
// #############################################################################################
fn parse_request(request: &str) -> Option<HttpRequest> {
    let first_line = request.lines().next()?;
    let mut parts = first_line.split_whitespace();

    let method = parts.next()?.to_string();
    let url = parts.next()?.to_string();
    let version = parts.next()?.to_string();

    // Reject malformed request lines with extra tokens
    if parts.next().is_some() {
        return None;
    }

    Some(HttpRequest {
        method,
        url,
        version,
    })
}

// #############################################################################################
// Read HTTP request function
// Stops when:
// - "\r\n\r\n" is found (end of HTTP headers)
// - OR request exceeds MAX_REQUEST
// #############################################################################################
async fn read_request(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut request = Vec::new();

    loop {
        // Application read buffer for TCP socket data.
        let mut buf = [0u8; 8192];

        // Abort the connection if no data is received within the timeout.
        let n = match timeout(Duration::from_secs(5), stream.read(&mut buf)).await {
            Ok(Ok(n)) => n,
            _ => return None,
        };

        // Connection closed
        if n == 0 {
            return None;
        }

        request.extend_from_slice(&buf[..n]);

        // Reject requests that exceed the configured maximum size.
        if request.len() > MAX_REQUEST {
            return None;
        }

        // End of HTTP headers
        if request.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }

    Some(request)
}
