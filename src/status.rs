// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, Instant, timeout_at};

pub const RESPONSE_WRITE_TIMEOUT: Duration = Duration::from_secs(30);

// #############################################################################################
// Sends an HTTP 200 OK response header.
// #############################################################################################
pub async fn status_ok(
    stream: &mut TcpStream,
    mime: &str,
    file_size: u64,
    deadline: Instant,
) -> io::Result<()> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: {}\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\
        X-Content-Type-Options: nosniff\r\n\
        \r\n",
        mime, file_size
    );

    write_all_with_timeout(stream, headers.as_bytes(), deadline).await
}

// #############################################################################################
// Sends an HTTP error response.
// #############################################################################################
pub async fn status_bad_request(stream: &mut TcpStream, deadline: Instant) -> io::Result<()> {
    write_all_with_timeout(
        stream,
        b"HTTP/1.1 400 Bad Request\r\n\
        Connection: close\r\n\
        Content-Length: 0\r\n\
        X-Content-Type-Options: nosniff\r\n\
        \r\n",
        deadline,
    )
    .await
}

pub async fn status_method_not_allowed(
    stream: &mut TcpStream,
    deadline: Instant,
) -> io::Result<()> {
    write_all_with_timeout(
        stream,
        b"HTTP/1.1 405 Method Not Allowed\r\n\
        Allow: GET\r\n\
        Connection: close\r\n\
        Content-Length: 0\r\n\
        X-Content-Type-Options: nosniff\r\n\
        \r\n",
        deadline,
    )
    .await
}

pub async fn status_header_too_large(stream: &mut TcpStream, deadline: Instant) -> io::Result<()> {
    write_all_with_timeout(
        stream,
        b"HTTP/1.1 431 Request Header Fields Too Large\r\n\
        Connection: close\r\n\
        Content-Length: 0\r\n\
        X-Content-Type-Options: nosniff\r\n\
        \r\n",
        deadline,
    )
    .await
}

pub async fn status_version_not_supported(
    stream: &mut TcpStream,
    deadline: Instant,
) -> io::Result<()> {
    write_all_with_timeout(
        stream,
        b"HTTP/1.1 505 HTTP Version Not Supported\r\n\
        Connection: close\r\n\
        Content-Length: 0\r\n\
        X-Content-Type-Options: nosniff\r\n\
        \r\n",
        deadline,
    )
    .await
}

// #############################################################################################
// Sends an HTTP 404 Not Found response.
// #############################################################################################
pub async fn status_not_found(stream: &mut TcpStream, deadline: Instant) -> io::Result<()> {
    write_all_with_timeout(
        stream,
        b"HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        deadline,
    )
    .await
}

pub async fn write_all_with_timeout<W: AsyncWrite + Unpin>(
    writer: &mut W,
    bytes: &[u8],
    deadline: Instant,
) -> io::Result<()> {
    let operation_deadline = deadline.min(Instant::now() + RESPONSE_WRITE_TIMEOUT);
    match timeout_at(operation_deadline, writer.write_all(bytes)).await {
        Ok(result) => result,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "response write timed out",
        )),
    }
}
