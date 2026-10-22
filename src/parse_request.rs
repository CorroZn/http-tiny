// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::AsyncReadExt; // Enable Tokio reading methods on I/O types like File and TcpStream
use tokio::net::TcpStream; // Tokio TCP stream type used for network communication.
use tokio::time::{Duration, Instant, timeout, timeout_at};

use crate::MAX_REQUEST; // Used for checking if the HTTP request exceeds the size limit
use crate::validate_request::{HttpRequest, PathError, validate_and_resolve}; // Validates the HTTP request

const REQUEST_IDLE_TIMEOUT: Duration = Duration::from_secs(5);
// Bound slow clients even if they continue sending data before the idle timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
static REQUEST_READ_FAILURES: AtomicUsize = AtomicUsize::new(0);

fn report_read_failure(kind: io::ErrorKind) {
    let failures = REQUEST_READ_FAILURES.fetch_add(1, Ordering::Relaxed);
    if failures & 1023 == 0 {
        eprintln!("http-tiny: request read failed ({kind:?})");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestError {
    BadRequest,
    MethodNotAllowed,
    VersionNotSupported,
    HeaderTooLarge,
    NotFound,
}

// #############################################################################################
// HTTP request parsing and validation.
// Reads the HTTP request, parses the request line, validates it,
// and resolves the requested filesystem path.
// #############################################################################################
pub async fn http_parse_validate(
    stream: &mut TcpStream,
    docroot: &Path,
) -> Result<Option<PathBuf>, RequestError> {
    // Read raw HTTP request from socket
    let raw = match read_request(stream).await? {
        Some(raw) => raw,
        None => return Ok(None),
    };

    // Parse and validate the complete HTTP request.
    let parsed = parse_request(&raw)?;

    // Validate + convert URL into safe filesystem path
    match validate_and_resolve(&parsed, docroot) {
        Ok(path) => Ok(Some(path)),
        Err(PathError::BadRequest) => Err(RequestError::BadRequest),
        Err(PathError::NotFound) => Err(RequestError::NotFound),
    }
}

// #############################################################################################
// Parses and validates an HTTP/1.1 request and its headers.
//
// GET /index.html HTTP/1.1
// #############################################################################################
fn parse_request(request: &[u8]) -> Result<HttpRequest<'_>, RequestError> {
    if !request.ends_with(b"\r\n\r\n") {
        return Err(RequestError::BadRequest);
    }

    for (index, byte) in request.iter().enumerate() {
        if (*byte == 13 && request.get(index + 1) != Some(&10))
            || (*byte == 10 && (index == 0 || request[index - 1] != 13))
        {
            return Err(RequestError::BadRequest);
        }
    }

    // Parse the request line (method, URL, version).
    let request_line_end = request
        .windows(2)
        .position(|window| window == [13, 10])
        .ok_or(RequestError::BadRequest)?;
    let request_line = &request[..request_line_end];
    let mut parts = request_line.split(|byte| *byte == b' ');
    let method = parts.next().ok_or(RequestError::BadRequest)?;
    let url = parts.next().ok_or(RequestError::BadRequest)?;
    let version = parts.next().ok_or(RequestError::BadRequest)?;

    // Reject malformed request lines with extra tokens.
    if method.is_empty()
        || url.is_empty()
        || version.is_empty()
        || parts.next().is_some()
        || !method.iter().copied().all(is_token)
        || url.iter().any(|byte| *byte <= b' ' || *byte == 0x7f)
        || !valid_http_version(version)
    {
        return Err(RequestError::BadRequest);
    }

    let method = std::str::from_utf8(method).map_err(|_| RequestError::BadRequest)?;
    let url = std::str::from_utf8(url).map_err(|_| RequestError::BadRequest)?;
    let version = std::str::from_utf8(version).map_err(|_| RequestError::BadRequest)?;

    let header_end = request.len() - 4;
    let header_start = request_line_end + 2;
    let mut host_seen = false;
    let mut content_length_seen = false;

    if header_start < header_end {
        let headers = &request[header_start..header_end];
        let line_count = headers.iter().filter(|byte| **byte == 10).count() + 1;
        for (index, line) in headers.split(|byte| *byte == 10).enumerate() {
            let line = if index + 1 < line_count {
                line.strip_suffix(&[13]).ok_or(RequestError::BadRequest)?
            } else {
                line
            };
            if line.is_empty() || line[0] == b' ' || line[0] == 9 {
                return Err(RequestError::BadRequest);
            }

            let colon = line
                .iter()
                .position(|byte| *byte == b':')
                .ok_or(RequestError::BadRequest)?;
            let name = &line[..colon];
            let value = &line[colon + 1..];
            if name.is_empty() || !name.iter().copied().all(is_token) {
                return Err(RequestError::BadRequest);
            }
            if value
                .iter()
                .any(|byte| (*byte < b' ' && *byte != 9) || *byte == 0x7f)
            {
                return Err(RequestError::BadRequest);
            }

            let value = trim_ows(value);
            if name.eq_ignore_ascii_case(b"host") {
                if host_seen
                    || value.is_empty()
                    || value
                        .iter()
                        .any(|byte| *byte <= b' ' || *byte >= 0x7f || *byte == b',')
                {
                    return Err(RequestError::BadRequest);
                }
                host_seen = true;
            } else if name.eq_ignore_ascii_case(b"transfer-encoding") {
                return Err(RequestError::BadRequest);
            } else if name.eq_ignore_ascii_case(b"content-length") {
                if content_length_seen || value.is_empty() || !value.iter().all(u8::is_ascii_digit)
                {
                    return Err(RequestError::BadRequest);
                }
                content_length_seen = true;
                if value.iter().any(|byte| *byte != b'0') {
                    return Err(RequestError::BadRequest);
                }
            }
        }
    } else if header_start != header_end + 2 {
        return Err(RequestError::BadRequest);
    }

    if !host_seen {
        return Err(RequestError::BadRequest);
    }
    if method != "GET" {
        return Err(RequestError::MethodNotAllowed);
    }
    if version != "HTTP/1.1" {
        return Err(RequestError::VersionNotSupported);
    }

    Ok(HttpRequest {
        method,
        url,
        version,
    })
}

fn valid_http_version(version: &[u8]) -> bool {
    version.len() == 8
        && &version[..5] == b"HTTP/"
        && version[5].is_ascii_digit()
        && version[6] == b'.'
        && version[7].is_ascii_digit()
}

fn is_token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn trim_ows(mut value: &[u8]) -> &[u8] {
    while matches!(value.first(), Some(b' ')) || matches!(value.first(), Some(9)) {
        value = &value[1..];
    }
    while matches!(value.last(), Some(b' ')) || matches!(value.last(), Some(9)) {
        value = &value[..value.len() - 1];
    }
    value
}

// #############################################################################################
// Read HTTP request function
// Stops when CRLFCRLF is found (end of HTTP headers)
// OR request exceeds MAX_REQUEST.
// #############################################################################################
async fn read_request(stream: &mut TcpStream) -> Result<Option<Vec<u8>>, RequestError> {
    read_request_with_timeout(stream, REQUEST_TIMEOUT).await
}

async fn read_request_with_timeout(
    stream: &mut TcpStream,
    total_timeout: Duration,
) -> Result<Option<Vec<u8>>, RequestError> {
    let mut request = Vec::with_capacity(1024);
    let mut buf = [0u8; 8192];
    let deadline = Instant::now() + total_timeout;

    loop {
        // Reject requests that reach the size limit without ending the headers.
        let remaining = MAX_REQUEST
            .checked_sub(request.len())
            .ok_or(RequestError::HeaderTooLarge)?;
        if remaining == 0 {
            return Err(RequestError::HeaderTooLarge);
        }

        // Abort the connection if no data is received within the timeout.
        let n = match timeout_at(
            deadline,
            timeout(REQUEST_IDLE_TIMEOUT, stream.read(&mut buf[..remaining])),
        )
        .await
        {
            Ok(Ok(Ok(n))) => n,
            Ok(Ok(Err(error))) => {
                report_read_failure(error.kind());
                return Ok(None);
            }
            _ => return Ok(None),
        };

        // Connection closed
        if n == 0 {
            return Ok(None);
        }

        let scan_start = request.len().saturating_sub(3);
        request.extend_from_slice(&buf[..n]);

        // End of HTTP headers
        if let Some(end) = request[scan_start..]
            .windows(4)
            .position(|window| window == [13, 10, 13, 10])
        {
            request.truncate(scan_start + end + 4);
            return Ok(Some(request));
        }
        if let Some(end) = request[scan_start..]
            .windows(2)
            .position(|window| window == [10, 10])
        {
            request.truncate(scan_start + end + 2);
            return Ok(Some(request));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RequestError, parse_request, read_request_with_timeout};
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tokio::time::{sleep, timeout};

    #[test]
    fn rejects_requests_without_a_header_terminator() {
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n";

        assert!(matches!(
            parse_request(request),
            Err(RequestError::BadRequest)
        ));
    }

    #[test]
    fn rejects_bare_carriage_returns() {
        let request = b"GET / HTTP/1.1\rHost: localhost\r\n\r\n";

        assert!(matches!(
            parse_request(request),
            Err(RequestError::BadRequest)
        ));
    }

    #[tokio::test]
    async fn request_timeout_is_absolute() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (mut server, _) = listener.accept().await.unwrap();
        let mut request_task = tokio::spawn(async move {
            read_request_with_timeout(&mut server, Duration::from_millis(200)).await
        });
        let sender_task = tokio::spawn(async move {
            let mut client = client;
            loop {
                if client.write_all(b"G").await.is_err() {
                    break;
                }
                sleep(Duration::from_millis(25)).await;
            }
        });

        let result = timeout(Duration::from_secs(1), &mut request_task).await;
        sender_task.abort();
        if result.is_err() {
            request_task.abort();
        }

        assert!(result.is_ok(), "slow request exceeded its total deadline");
        assert!(result.unwrap().unwrap().unwrap().is_none());
    }
}
