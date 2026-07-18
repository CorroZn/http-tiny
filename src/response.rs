// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::Path; // Filesystem path type for representing file and directory paths.
use tokio::fs::File; // Tokio file type for opening and accessing files.
use tokio::io::{AsyncReadExt, AsyncWriteExt}; // Tokio async read/write traits.
use tokio::net::TcpStream; // Tokio TCP stream type used for network communication.

use crate::mime::http_mime; // Determines the MIME type for a requested file.
use crate::status::{status_ok, status_not_found}; // Send an HTTP status response.

// #############################################################################################
// HTTP response function
// Sends an HTTP response containing the requested file or a 404 response.
// #############################################################################################
pub async fn http_response(mut stream: TcpStream, path: &Path) -> std::io::Result<()> {
    let mut file = match File::open(path).await {
        Ok(file) => file,

        // The requested file doesn't exist.
        Err(_) => {
            status_not_found(&mut stream).await?;
            return Ok(());
        }
    };

    let metadata = file.metadata().await?;

    // The requested file is not a file.
    if !metadata.is_file() {
        status_not_found(&mut stream).await?;
        return Ok(());
    }

    // Determine the file size for the Content-Length header.
    let file_size = metadata.len();

    // Determine the MIME type from the filename extension.
    let mime = http_mime(path);

    // Send the HTTP 200 OK response headers.
    status_ok(&mut stream, mime, file_size).await?;

    // Buffer used to stream file data to the TCP socket.
    let mut buffer = [0u8; 8192];

    // Read bytes from file.
    loop {
        let bytes_read = file.read(&mut buffer).await?;

        // End of file reached.
        if bytes_read == 0 {
            break;
        }

        // Send file data to the client.
        stream.write_all(&buffer[..bytes_read]).await?;
    }

    Ok(())
}