// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::time::{Duration, Instant, timeout_at};

use crate::http::mime::http_mime;
use crate::http::status::{status_not_found, status_ok, write_all_with_timeout};
#[cfg(unix)]
use crate::static_files::open_confined_file;
use crate::static_files::{DocumentRoot, report_open_failure};

pub(crate) const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);
const RESPONSE_READ_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_BUFFER_SIZE: usize = 64 * 1024;
// #############################################################################################
// HTTP response function
// Sends an HTTP response containing the requested file or a 404 response.
// #############################################################################################
pub(crate) async fn http_response(
    stream: TcpStream,
    docroot: Arc<DocumentRoot>,
    path: PathBuf,
) -> io::Result<()> {
    let deadline = Instant::now() + RESPONSE_TIMEOUT;
    response_with_deadline(
        deadline,
        http_response_inner(stream, docroot, path, deadline),
    )
    .await
}

pub(crate) async fn response_with_deadline<F>(deadline: Instant, future: F) -> io::Result<()>
where
    F: std::future::Future<Output = io::Result<()>>,
{
    match timeout_at(deadline, future).await {
        Ok(result) => result,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "response deadline exceeded",
        )),
    }
}

async fn http_response_inner(
    mut stream: TcpStream,
    docroot: Arc<DocumentRoot>,
    path: PathBuf,
    deadline: Instant,
) -> io::Result<()> {
    // Determine the MIME type from the filename extension.
    let mime = http_mime(&path);

    // Open every path component without following links that changed after validation.
    #[cfg(unix)]
    let open_result = {
        let docroot = Arc::clone(&docroot);
        match tokio::task::spawn_blocking(move || open_confined_file(&docroot, &path)).await {
            Ok(Ok(file)) => Ok(File::from_std(file)),
            Ok(Err(error)) => Err(error),
            Err(error) => Err(io::Error::other(error)),
        }
    };

    #[cfg(not(unix))]
    let open_result: io::Result<File> = Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "document-root confinement is not supported on this platform",
    ));

    let mut file = match open_result {
        Ok(file) => file,

        // The requested file doesn't exist.
        Err(error) => {
            report_open_failure(error.kind());
            status_not_found(&mut stream, deadline).await?;
            return Ok(());
        }
    };

    let metadata_deadline = deadline.min(Instant::now() + RESPONSE_READ_TIMEOUT);
    let metadata = match timeout_at(metadata_deadline, file.metadata()).await {
        Ok(result) => result?,
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "response file metadata timed out",
            ));
        }
    };

    // The requested file is not a file.
    if !metadata.is_file() {
        status_not_found(&mut stream, deadline).await?;
        return Ok(());
    }

    // Determine the file size for the Content-Length header.
    let file_size = metadata.len();

    // Send the HTTP 200 OK response headers.
    status_ok(&mut stream, mime, file_size, deadline).await?;

    // Allocate only what this response can use, up to the throughput-oriented cap.
    let buffer_size =
        usize::try_from(file_size.min(RESPONSE_BUFFER_SIZE as u64)).unwrap_or(RESPONSE_BUFFER_SIZE);
    let mut buffer = vec![0u8; buffer_size];
    let mut bytes_remaining = file_size;

    // Read bytes from file.
    while bytes_remaining > 0 {
        let read_size = usize::try_from(bytes_remaining)
            .unwrap_or(buffer.len())
            .min(buffer.len());
        let bytes_read =
            read_file_with_timeout(&mut file, &mut buffer[..read_size], deadline).await?;

        // End of file reached.
        if bytes_read == 0 {
            break;
        }

        // Send file data to the client.
        write_all_with_timeout(&mut stream, &buffer[..bytes_read], deadline).await?;
        bytes_remaining -= bytes_read as u64;
    }

    Ok(())
}

async fn read_file_with_timeout(
    file: &mut File,
    buffer: &mut [u8],
    deadline: Instant,
) -> io::Result<usize> {
    let operation_deadline = deadline.min(Instant::now() + RESPONSE_READ_TIMEOUT);
    match timeout_at(operation_deadline, file.read(buffer)).await {
        Ok(result) => result,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "response file read timed out",
        )),
    }
}
