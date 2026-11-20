// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::path::{Path, PathBuf};
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

/// Files no larger than this are read in full inside the blocking task that
/// opens them. A small response then costs one blocking round trip instead of
/// three (open, metadata, read), which is where most of the per-request kernel
/// time goes.
#[cfg(unix)]
const INLINE_BODY_LIMIT: u64 = 256 * 1024;

/// A response body that is either already in memory or streamed from disk.
#[derive(Debug)]
enum ResponseBody {
    Inline(Vec<u8>),
    Stream(File),
}

/// Open the requested file, read small bodies in full, and return the byte
/// count to advertise plus the body to send.
#[cfg(unix)]
fn open_for_response(docroot: &DocumentRoot, path: &Path) -> io::Result<(u64, ResponseBody)> {
    let mut file = open_confined_file(docroot, path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "requested path is not a regular file",
        ));
    }
    let file_size = metadata.len();
    if file_size <= INLINE_BODY_LIMIT {
        let mut body = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut body)?;
        Ok((body.len() as u64, ResponseBody::Inline(body)))
    } else {
        Ok((file_size, ResponseBody::Stream(File::from_std(file))))
    }
}
// #############################################################################################
// HTTP response function
// Sends an HTTP response containing the requested file or a 404 response.
// #############################################################################################
pub(crate) async fn http_response(
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
    docroot: Arc<DocumentRoot>,
    path: PathBuf,
    deadline: Instant,
) -> io::Result<()> {
    // Determine the MIME type from the filename extension.
    let mime = http_mime(&path);

    #[cfg(unix)]
    let open_result = {
        let docroot = Arc::clone(&docroot);
        match tokio::task::spawn_blocking(move || open_for_response(&docroot, &path)).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => Err(error),
            Err(error) => Err(io::Error::other(error)),
        }
    };

    #[cfg(not(unix))]
    let open_result: io::Result<(u64, ResponseBody)> = Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "document-root confinement is not supported on this platform",
    ));

    let (file_size, body) = match open_result {
        Ok(result) => result,

        // The requested file doesn't exist.
        Err(error) => {
            report_open_failure(error.kind());
            status_not_found(stream, deadline).await?;
            return Ok(());
        }
    };

    // Send the HTTP 200 OK response headers.
    status_ok(stream, mime, file_size, deadline).await?;

    match body {
        ResponseBody::Inline(body) => {
            write_all_with_timeout(stream, &body, deadline).await?;
        }
        ResponseBody::Stream(mut file) => {
            stream_body(stream, &mut file, file_size, deadline).await?;
        }
    }

    Ok(())
}

/// Stream a body too large to buffer, in `RESPONSE_BUFFER_SIZE` chunks.
///
/// A file truncated after the metadata read ends the body early, so the client
/// receives fewer bytes than Content-Length advertises; the size is not
/// re-checked because that would require buffering the whole file.
async fn stream_body(
    stream: &mut TcpStream,
    file: &mut File,
    file_size: u64,
    deadline: Instant,
) -> io::Result<()> {
    let mut buffer = vec![0u8; RESPONSE_BUFFER_SIZE];
    let mut bytes_remaining = file_size;

    while bytes_remaining > 0 {
        let read_size = usize::try_from(bytes_remaining)
            .unwrap_or(buffer.len())
            .min(buffer.len());
        let bytes_read = read_file_with_timeout(file, &mut buffer[..read_size], deadline).await?;

        // End of file reached.
        if bytes_read == 0 {
            break;
        }

        // Send file data to the client.
        write_all_with_timeout(stream, &buffer[..bytes_read], deadline).await?;
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
