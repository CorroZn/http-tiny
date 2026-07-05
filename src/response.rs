// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::fs::File; // File type for opening and accessing files.
use std::io::{Read, Write}; // Traits for reading from and writing to I/O streams.
use std::net::TcpStream; // TCP stream type used for network communication.
use std::path::Path; // Filesystem path type for representing file and directory paths.

use crate::mime::http_mime; // Determines the MIME type for a requested file.

// #############################################################################################
// HTTP response function
// Will send an HTTP response after successful request validation and either send the file or HTTP Error 404 Not Found
// #############################################################################################
pub fn http_response(mut stream: TcpStream, path: &Path) {
    // Attempt to open the requested file.
    let mut file = match File::open(path) {
        Ok(file) => file,

        // The requested file doesn't exist
        Err(_) => {
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\n\
                Connection: close\r\n\
                Content-Length: 0\r\n\
                \r\n",
            );
            return;
        }
    };

    // Determine the file size for the Content-Length header
    let file_size = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);

    // Determine the MIME type from the filename extension
    let mime = http_mime(path);

    // Build the HTTP response headers
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: {}; charset=UTF-8\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n\
        \r\n",
        mime, file_size
    );

    // Send the response headers
    let _ = stream.write_all(headers.as_bytes());

    // Send the file contents to the client in 8 KiB chunks
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = match file.read(&mut buffer) {
            Ok(0) => break, // End of file.
            Ok(n) => n,
            Err(_) => break,
        };

        if stream.write_all(&buffer[..bytes_read]).is_err() {
            break;
        }
    }
}
