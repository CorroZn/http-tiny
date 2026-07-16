// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::Path; // Filesystem path type for representing file and directory paths.
use tokio::fs::File; // Tokio file type for opening and accessing files.
use tokio::io::{AsyncReadExt, AsyncWriteExt}; // Tokio async read/write traits.
use tokio::net::TcpStream; // Tokio TCP stream type used for network communication.

use crate::mime::http_mime; // Determines the MIME type for a requested file.

// #############################################################################################
// HTTP response function
// Sends an HTTP response containing the requested file or a 404 response.
// #############################################################################################
pub async fn http_response(mut stream: TcpStream, path: &Path) -> std::io::Result<()> {
    // Attempt to open the requested file.
    let mut file = match File::open(path).await {
        Ok(file) => file,

        // The requested file doesn't exist.
        Err(_) => {
            stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\n\
                Connection: close\r\n\
                Content-Length: 0\r\n\
                \r\n",
                )
                .await?;

            return Ok(());
        }
    };

    // Determine the file size for the Content-Length header.
    let file_size = file.metadata().await?.len();

    // Determine the MIME type from the filename extension.
    let mime = http_mime(path);

    // Build the HTTP response headers.
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: {}; charset=UTF-8\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\
        \r\n",
        mime, file_size
    );

    // Send the response headers.
    stream.write_all(headers.as_bytes()).await?;

    // Temporary buffer used to stream file data to the TCP socket.
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).await?;

        if bytes_read == 0 {
            break;
        }

        stream.write_all(&buffer[..bytes_read]).await?;
    }

    Ok(())
}
