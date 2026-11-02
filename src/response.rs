// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::fs::File; // Tokio file type for opening and accessing files.
use tokio::io::AsyncReadExt; // Tokio async file reads.
use tokio::net::TcpStream; // Tokio TCP stream type used for network communication.
use tokio::time::{Duration, Instant, timeout_at};

use crate::mime::http_mime; // Determines the MIME type for a requested file.
use crate::status::{status_not_found, status_ok, write_all_with_timeout}; // Send an HTTP status response.

pub(crate) const RESPONSE_TIMEOUT: Duration = Duration::from_secs(300);
const RESPONSE_READ_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_BUFFER_SIZE: usize = 64 * 1024;
static FILE_OPEN_FAILURES: AtomicUsize = AtomicUsize::new(0);

fn report_open_failure(kind: io::ErrorKind) {
    let failures = FILE_OPEN_FAILURES.fetch_add(1, Ordering::Relaxed);
    if failures.trailing_zeros() >= 10 {
        let _ = writeln!(
            io::stderr().lock(),
            "http-tiny: file open failed ({kind:?})"
        );
    }
}

pub(crate) struct DocumentRoot {
    canonical_path: PathBuf,
}

impl DocumentRoot {
    #[cfg(unix)]
    pub(crate) fn open(path: &str) -> io::Result<Self> {
        let canonical_path = std::fs::canonicalize(path)?;
        open_root_directory(&canonical_path)?;
        Ok(Self { canonical_path })
    }

    #[cfg(not(unix))]
    pub(crate) fn open(_path: &str) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "document-root confinement is not supported on this platform",
        ))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.canonical_path
    }
}

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

async fn response_with_deadline<F>(deadline: Instant, future: F) -> io::Result<()>
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

// Choose flags that keep directory traversal pinned and prevent symlink following.
#[cfg(unix)]
fn directory_flags() -> rustix::fs::OFlags {
    use rustix::fs::OFlags;

    #[cfg(target_os = "linux")]
    {
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
    }
    #[cfg(not(target_os = "linux"))]
    {
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
    }
}

// Open the document root component-by-component so a swapped ancestor cannot redirect it.
#[cfg(unix)]
fn open_root_directory(path: &Path) -> io::Result<std::fs::File> {
    use rustix::fs::{Mode, open, openat};

    let mut directory = open("/", directory_flags(), Mode::empty()).map_err(io::Error::from)?;

    for component in path.components() {
        let name = match component {
            Component::RootDir => continue,
            Component::Normal(name) => name,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "document root contains an invalid component",
                ));
            }
        };
        directory =
            openat(&directory, name, directory_flags(), Mode::empty()).map_err(io::Error::from)?;
    }

    Ok(std::fs::File::from(directory))
}

// Open each requested path component relative to its pinned parent, rejecting symlinks.
#[cfg(unix)]
fn open_confined_file(docroot: &DocumentRoot, path: &Path) -> io::Result<std::fs::File> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, openat, statat};

    // Reopen the configured pathname so atomic document-root replacement is visible.
    let root_directory = open_root_directory(&docroot.canonical_path)?;
    let relative = path.strip_prefix(&docroot.canonical_path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "requested path is outside the document root",
        )
    })?;
    let component_count = relative.components().count();
    if component_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "requested path is not a file",
        ));
    }

    let mut directory = None;
    for (index, component) in relative.components().enumerate() {
        let Component::Normal(name) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "requested path contains an invalid component",
            ));
        };
        let is_last = index + 1 == component_count;

        if is_last {
            let metadata = match directory.as_ref() {
                Some(parent) => statat(parent, name, AtFlags::SYMLINK_NOFOLLOW),
                None => statat(&root_directory, name, AtFlags::SYMLINK_NOFOLLOW),
            }
            .map_err(io::Error::from)?;
            if !FileType::from_raw_mode(metadata.st_mode).is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "requested path is not a regular file",
                ));
            }
        }

        let flags = if is_last {
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK
        } else {
            directory_flags()
        };
        let opened = match directory.as_ref() {
            Some(parent) => openat(parent, name, flags, Mode::empty()),
            None => openat(&root_directory, name, flags, Mode::empty()),
        }
        .map_err(io::Error::from)?;
        let opened = std::fs::File::from(opened);
        if is_last {
            return Ok(opened);
        }
        directory = Some(opened);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "requested path has no file component",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::{DocumentRoot, http_response, response_with_deadline, write_all_with_timeout};
    use std::fs::{self, create_dir};
    use std::io;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWrite};
    use tokio::net::TcpListener;
    use tokio::time::{Instant, timeout};

    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

    struct PendingWriter;

    impl AsyncWrite for PendingWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            _buffer: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Pending
        }

        fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn enforces_a_total_response_deadline() -> Result<(), Box<dyn std::error::Error>> {
        let result = response_with_deadline(
            Instant::now() + Duration::from_millis(10),
            std::future::pending::<io::Result<()>>(),
        )
        .await;

        assert_eq!(
            result
                .err()
                .ok_or_else(|| std::io::Error::other("expected response deadline error"))?
                .kind(),
            io::ErrorKind::TimedOut
        );

        Ok(())
    }

    #[tokio::test]
    async fn times_out_a_stalled_response_write() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = PendingWriter;
        let result = timeout(
            Duration::from_millis(100),
            write_all_with_timeout(
                &mut writer,
                b"response",
                Instant::now() + Duration::from_millis(10),
            ),
        )
        .await;

        assert!(result.is_ok(), "response write did not time out");
        assert_eq!(
            result?
                .err()
                .ok_or_else(|| std::io::Error::other("expected response write timeout"))?
                .kind(),
            io::ErrorKind::TimedOut
        );

        Ok(())
    }

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> std::io::Result<Self> {
            let unique = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "http-tiny-response-{}-{unique}",
                std::process::id()
            ));
            create_dir(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    async fn response_for(
        document_root: Arc<DocumentRoot>,
        path: PathBuf,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
        let (server, _) = listener.accept().await?;
        let response_task =
            tokio::spawn(async move { http_response(server, document_root, path).await });

        let mut response = Vec::new();
        client.take(1024).read_to_end(&mut response).await?;
        response_task.await??;
        Ok(response)
    }

    #[tokio::test]
    async fn serves_in_place_and_atomic_file_updates_on_subsequent_responses()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = TestRoot::new()?;
        let docroot = fixture.0.join("www");
        create_dir(&docroot)?;
        let file_path = docroot.join("page.txt");
        fs::write(&file_path, b"original")?;
        let document_root =
            Arc::new(DocumentRoot::open(docroot.to_str().ok_or_else(|| {
                std::io::Error::other("path is not valid UTF-8")
            })?)?);
        let path = document_root.path().join("page.txt");

        let original = response_for(Arc::clone(&document_root), path.clone()).await?;
        fs::write(&file_path, b"in-place")?;
        let in_place = response_for(Arc::clone(&document_root), path.clone()).await?;
        let temporary_path = docroot.join("page.txt.tmp");
        fs::write(&temporary_path, b"atomic")?;
        fs::rename(&temporary_path, &file_path)?;
        let atomic = response_for(document_root, path).await?;

        assert!(original.ends_with(b"original"));
        assert!(in_place.ends_with(b"in-place"));
        assert!(atomic.ends_with(b"atomic"));

        Ok(())
    }

    #[tokio::test]
    async fn serves_the_document_root_currently_at_its_configured_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = TestRoot::new()?;
        let docroot = fixture.0.join("www");
        let moved_root = fixture.0.join("original");
        create_dir(&docroot)?;
        fs::write(docroot.join("page.txt"), b"original")?;
        let document_root =
            Arc::new(DocumentRoot::open(docroot.to_str().ok_or_else(|| {
                std::io::Error::other("path is not valid UTF-8")
            })?)?);
        let canonical_root = document_root.path().to_path_buf();
        let path = canonical_root.join("page.txt");

        fs::rename(&docroot, &moved_root)?;
        create_dir(&docroot)?;
        fs::write(docroot.join("page.txt"), b"replacement")?;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
        let (server, _) = listener.accept().await?;
        let response_task =
            tokio::spawn(async move { http_response(server, document_root, path).await });

        let mut response = Vec::new();
        client.take(1024).read_to_end(&mut response).await?;
        response_task.await??;

        assert!(response.ends_with(b"replacement"));

        Ok(())
    }

    #[tokio::test]
    async fn rejects_intermediate_symlink_replacement_after_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        use crate::validate_request::{HttpRequest, validate_and_resolve};

        let fixture = TestRoot::new()?;
        let docroot = fixture.0.join("www");
        let static_dir = docroot.join("static");
        let outside = fixture.0.join("outside");
        create_dir(&docroot)?;
        create_dir(&static_dir)?;
        create_dir(&outside)?;
        fs::write(static_dir.join("secret.txt"), b"inside")?;
        fs::write(outside.join("secret.txt"), b"outside-secret")?;

        let document_root =
            Arc::new(DocumentRoot::open(docroot.to_str().ok_or_else(|| {
                std::io::Error::other("path is not valid UTF-8")
            })?)?);
        let canonical_root = document_root.path().to_path_buf();
        let request = HttpRequest {
            method: "GET",
            url: "/static/secret.txt",
            version: "HTTP/1.1",
        };
        let path = validate_and_resolve(&request, &canonical_root)
            .map_err(|error| std::io::Error::other(format!("path resolution failed: {error:?}")))?;

        fs::rename(&static_dir, docroot.join("saved"))?;
        symlink(&outside, &static_dir)?;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
        let (server, _) = listener.accept().await?;
        let response_root = Arc::clone(&document_root);
        let response_task =
            tokio::spawn(async move { http_response(server, response_root, path).await });

        let mut response = Vec::new();
        client.take(1024).read_to_end(&mut response).await?;
        response_task.await??;

        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
        assert!(
            !response
                .windows(14)
                .any(|window| window == b"outside-secret")
        );

        Ok(())
    }

    #[tokio::test]
    async fn rejects_unix_sockets_without_opening_them() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let document_root =
            Arc::new(DocumentRoot::open(root.0.to_str().ok_or_else(|| {
                std::io::Error::other("path is not valid UTF-8")
            })?)?);
        let socket_path = document_root.path().join("socket");
        let _socket = std::os::unix::net::UnixListener::bind(&socket_path)?;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
        let (server, _) = listener.accept().await?;
        let response_root = Arc::clone(&document_root);
        let response_task =
            tokio::spawn(async move { http_response(server, response_root, socket_path).await });

        let mut response = Vec::new();
        client.take(1024).read_to_end(&mut response).await?;
        response_task.await??;

        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn rejects_fifo_without_blocking() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let document_root =
            Arc::new(DocumentRoot::open(root.0.to_str().ok_or_else(|| {
                std::io::Error::other("path is not valid UTF-8")
            })?)?);
        let canonical_root = document_root.path().to_path_buf();
        let fifo = canonical_root.join("stream");
        assert!(Command::new("mkfifo").arg(&fifo).status()?.success());

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
        let (server, _) = listener.accept().await?;
        let response_root = Arc::clone(&document_root);
        let response_path = fifo.clone();
        let response_task =
            tokio::spawn(async move { http_response(server, response_root, response_path).await });

        let mut response = Vec::new();
        let result = timeout(
            Duration::from_secs(1),
            client.take(1024).read_to_end(&mut response),
        )
        .await;

        if result.is_err() {
            // Unblock the pre-fix FIFO open so the test cannot strand a blocking worker.
            drop(fs::OpenOptions::new().write(true).open(&fifo)?);
        }
        response_task.await??;

        assert!(result.is_ok(), "opening a FIFO blocked the response worker");
        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
        assert!(response.windows(4).any(|window| window == b"\r\n\r\n"));

        Ok(())
    }
}
