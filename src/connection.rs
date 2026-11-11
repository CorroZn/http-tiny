// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::io::Write;
use std::sync::Arc; // Handle shared ownership.
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream; // Tokio TCP stream type used for network communication.
use tokio::time::{Duration, Instant, timeout};

use crate::http::request::{RequestError, http_parse_validate}; // Parses and validates incoming HTTP requests.
use crate::http::response::http_response;
use crate::http::status::{
    RESPONSE_WRITE_TIMEOUT, status_bad_request, status_header_too_large, status_method_not_allowed,
    status_not_found, status_version_not_supported,
};
use crate::static_files::DocumentRoot;

static CLIENT_FAILURES: AtomicUsize = AtomicUsize::new(0);

/// Upper bound for discarding unread request bytes before the socket is
/// dropped, so the kernel sends FIN instead of RST.
const CONNECTION_DRAIN_TIMEOUT: Duration = Duration::from_millis(100);

/// Half-close the connection and discard any unread request bytes.
async fn finish_connection(stream: &mut TcpStream) {
    let _ = stream.shutdown().await;
    let mut buffer = [0u8; 1024];
    let _ = timeout(CONNECTION_DRAIN_TIMEOUT, async {
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    })
    .await;
}

fn report_failure(category: &str, kind: io::ErrorKind) {
    let failures = CLIENT_FAILURES.fetch_add(1, Ordering::Relaxed);
    if failures.trailing_zeros() >= 10 {
        let _ = writeln!(
            io::stderr().lock(),
            "http-tiny: {category} failed ({kind:?})"
        );
    }
}

const fn request_error_name(error: RequestError) -> &'static str {
    match error {
        RequestError::BadRequest => "bad request",
        RequestError::MethodNotAllowed => "unsupported method",
        RequestError::VersionNotSupported => "unsupported HTTP version",
        RequestError::HeaderTooLarge => "request headers too large",
        RequestError::NotFound => "not found",
    }
}

// #############################################################################################
// HTTP client handler function.
// Handles a single client connection by parsing and validating the
// HTTP request before serving the requested resource.
// #############################################################################################
pub(crate) async fn http_handle_client(mut stream: TcpStream, docroot: Arc<DocumentRoot>) {
    // Check if the request is valid and get the requested file path.
    match http_parse_validate(&mut stream, docroot.path()).await {
        Ok(Some(path)) => {
            // Send the requested file, or a 404 page if it doesn't exist.
            if let Err(error) = http_response(&mut stream, docroot, path).await {
                report_failure("response", error.kind());
            }
        }
        Ok(None) => {}
        Err(error) => {
            let deadline = Instant::now() + RESPONSE_WRITE_TIMEOUT;
            let category = request_error_name(error);
            let response = match error {
                RequestError::BadRequest => status_bad_request(&mut stream, deadline).await,
                RequestError::MethodNotAllowed => {
                    status_method_not_allowed(&mut stream, deadline).await
                }
                RequestError::VersionNotSupported => {
                    status_version_not_supported(&mut stream, deadline).await
                }
                RequestError::HeaderTooLarge => {
                    status_header_too_large(&mut stream, deadline).await
                }
                RequestError::NotFound => status_not_found(&mut stream, deadline).await,
            };
            report_failure(category, io::ErrorKind::InvalidData);
            if let Err(error) = response {
                report_failure("error response", error.kind());
            }
        }
    }
    // Half-close and drain so the kernel sends FIN rather than RST.
    finish_connection(&mut stream).await;
}
