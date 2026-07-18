// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

// #############################################################################################
// Sends an HTTP 200 OK response header.
// #############################################################################################
pub async fn status_ok(stream: &mut TcpStream, mime: &str, file_size: u64) -> std::io::Result<()> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: {}\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\
        \r\n",
        mime, file_size
    );

    stream.write_all(headers.as_bytes()).await?;

    Ok(())
}

// #############################################################################################
// Sends an HTTP 404 Not Found response.
// #############################################################################################
pub async fn status_not_found(stream: &mut TcpStream) -> std::io::Result<()> {
    stream
    .write_all(
        b"HTTP/1.1 404 Not Found\r\n\
        Connection: close\r\n\
        Content-Length: 0\r\n\
        \r\n",
    )
    .await?;

    Ok(())
}