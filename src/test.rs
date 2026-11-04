// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

mod connection {
    use crate::connection::http_handle_client;
    use crate::static_files::DocumentRoot;
    use std::fs::{self, create_dir};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> std::io::Result<Self> {
            let unique = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("http-tiny-client-{}-{unique}", std::process::id()));
            create_dir(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    async fn send_request(
        docroot: PathBuf,
        target: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let request = format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n");
        send_raw_request(docroot, request.as_bytes()).await
    }

    async fn send_raw_request(
        docroot: PathBuf,
        request: &[u8],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = TcpStream::connect(listener.local_addr()?).await?;
        let (server, _) = listener.accept().await?;
        let docroot =
            Arc::new(DocumentRoot::open(docroot.to_str().ok_or_else(|| {
                std::io::Error::other("path is not valid UTF-8")
            })?)?);
        let client_task = tokio::spawn(http_handle_client(server, docroot));
        let mut client = client;
        client.write_all(request).await?;

        let mut response = Vec::new();
        match client.read_to_end(&mut response).await {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
            Err(error) => return Err(error.into()),
        }
        client_task.await?;
        Ok(response)
    }

    #[tokio::test]
    async fn rejects_requests_without_a_host_header() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        fs::write(root.0.join("index.html"), b"hello")?;

        let response =
            send_raw_request(root.0.canonicalize()?, b"GET /index.html HTTP/1.1\r\n\r\n").await?;

        assert!(
            response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"),
            "response: {response:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn rejects_duplicate_host_headers() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let response = send_raw_request(
            root.0.canonicalize()?,
            b"GET / HTTP/1.1\r\nHost: one\r\nHost: two\r\n\r\n",
        )
        .await?;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn rejects_bare_lf_line_endings() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let response = send_raw_request(
            root.0.canonicalize()?,
            b"GET / HTTP/1.1\nHost: localhost\n\n",
        )
        .await?;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn rejects_malformed_header_lines() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let response = send_raw_request(
            root.0.canonicalize()?,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nBad Header: value\r\n\r\n",
        )
        .await?;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn rejects_extra_request_line_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let response = send_raw_request(
            root.0.canonicalize()?,
            b"GET / extra HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await?;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn reports_unsupported_methods_and_versions() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let root = root.0.canonicalize()?;
        let method_response =
            send_raw_request(root.clone(), b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\n").await?;
        let version_response =
            send_raw_request(root, b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n").await?;

        assert!(method_response.starts_with(b"HTTP/1.1 405 Method Not Allowed\r\n"));
        assert!(
            method_response
                .windows(10)
                .any(|window| window == b"Allow: GET")
        );
        assert!(version_response.starts_with(b"HTTP/1.1 505 HTTP Version Not Supported\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn rejects_unsupported_request_body_framing() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let root = root.0.canonicalize()?;
        let transfer_encoding = send_raw_request(
            root.clone(),
            b"GET / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n",
        )
        .await?;
        let content_length = send_raw_request(
            root,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\n\r\n",
        )
        .await?;

        assert!(transfer_encoding.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
        assert!(content_length.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn accepts_request_headers_at_the_size_limit() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let mut request = b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Pad: ".to_vec();
        let padding = crate::http::request::MAX_REQUEST - request.len() - 4;
        request.resize(request.len() + padding, b'a');
        request.extend_from_slice(b"\r\n\r\n");
        assert_eq!(request.len(), crate::http::request::MAX_REQUEST);

        let response = send_raw_request(root.0.canonicalize()?, &request).await?;

        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn reports_oversized_request_headers() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let mut request = b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Pad: ".to_vec();
        request.resize(8193, b'a');

        let response = send_raw_request(root.0.canonicalize()?, &request).await?;

        assert!(response.starts_with(b"HTTP/1.1 431 Request Header Fields Too Large\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn includes_nosniff_on_success_and_not_found_responses()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        fs::write(root.0.join("index.html"), b"hello")?;
        let root = root.0.canonicalize()?;
        let success = send_request(root.clone(), "/index.html").await?;
        let not_found = send_request(root, "/missing.html").await?;

        assert!(
            success
                .windows(31)
                .any(|window| window == b"X-Content-Type-Options: nosniff")
        );
        assert!(
            not_found
                .windows(31)
                .any(|window| window == b"X-Content-Type-Options: nosniff")
        );

        Ok(())
    }

    #[tokio::test]
    async fn includes_nosniff_on_protocol_error_responses() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = TestRoot::new()?;
        let response = send_raw_request(root.0.canonicalize()?, b"GET / HTTP/1.1\r\n\r\n").await?;

        assert!(
            response
                .windows(31)
                .any(|window| window == b"X-Content-Type-Options: nosniff")
        );

        Ok(())
    }

    #[tokio::test]
    async fn accepts_zero_content_length() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        fs::write(root.0.join("index.html"), b"hello")?;
        let response = send_raw_request(
            root.0.canonicalize()?,
            b"GET /index.html HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n",
        )
        .await?;

        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn returns_not_found_for_missing_files() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let response = send_request(root.0.canonicalize()?, "/missing.html").await?;

        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn rejects_invalid_utf8_request_targets() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let response = send_raw_request(
            root.0.canonicalize()?,
            b"GET /\xff HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await?;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn rejects_duplicate_content_length_headers() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let response = send_raw_request(
            root.0.canonicalize()?,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
        )
        .await?;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn returns_not_found_for_invalid_targets() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let response = send_request(root.0.canonicalize()?, "/%2e%2e/secret").await?;

        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

        Ok(())
    }

    #[tokio::test]
    async fn serves_a_regular_file() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        fs::write(root.0.join("index.html"), b"hello")?;

        let response = send_request(root.0.canonicalize()?, "/index.html").await?;
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| std::io::Error::other("missing response header terminator"))?
            + 4;

        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert_eq!(
            &response[..header_end],
            b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: 5\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n"
        );
        assert!(
            response[..header_end]
                .windows(17)
                .any(|window| window == b"Content-Length: 5")
        );
        assert_eq!(&response[header_end..], b"hello");

        Ok(())
    }

    #[tokio::test]
    async fn serves_empty_files_with_an_empty_body() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        fs::write(root.0.join("empty.txt"), b"")?;

        let response = send_request(root.0.canonicalize()?, "/empty.txt").await?;
        let header_end = response
            .windows(4)
            .position(|window| window == [13, 10, 13, 10])
            .ok_or_else(|| std::io::Error::other("missing response header terminator"))?
            + 4;

        assert!(response.starts_with(b"HTTP/1.1 200 OK"));
        assert!(
            response[..header_end]
                .windows(17)
                .any(|window| window == b"Content-Length: 0")
        );
        assert_eq!(&response[header_end..], b"");

        Ok(())
    }

    #[tokio::test]
    async fn streams_large_files_without_truncation() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let contents = vec![b'x'; 1024 * 1024 + 17];
        fs::write(root.0.join("large.bin"), &contents)?;

        let response = send_request(root.0.canonicalize()?, "/large.bin").await?;
        let header_end = response
            .windows(4)
            .position(|window| window == [13, 10, 13, 10])
            .ok_or_else(|| std::io::Error::other("missing response header terminator"))?
            + 4;

        assert!(response.starts_with(b"HTTP/1.1 200 OK"));
        assert_eq!(&response[header_end..], contents);

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn serves_files_through_symlinks_inside_the_document_root()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new()?;
        create_dir(root.0.join("assets"))?;
        fs::write(root.0.join("assets/page.txt"), b"public")?;
        symlink("assets", root.0.join("public"))?;

        let response = send_request(root.0.canonicalize()?, "/public/page.txt").await?;

        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with(b"public"));

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn does_not_serve_hidden_files_through_visible_symlinks()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new()?;
        create_dir(root.0.join(".secret"))?;
        fs::write(root.0.join(".secret/credentials.txt"), b"secret")?;
        symlink(".secret", root.0.join("public"))?;

        let response = send_request(root.0.canonicalize()?, "/public/credentials.txt").await?;

        assert!(!response.windows(6).any(|window| window == b"secret"));

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn does_not_serve_files_through_symlinks_outside_the_document_root()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let fixture = TestRoot::new()?;
        let docroot = fixture.0.join("www");
        let outside = fixture.0.join("outside");
        create_dir(&docroot)?;
        create_dir(&outside)?;
        fs::write(outside.join("secret.txt"), b"outside-secret")?;
        symlink(&outside, docroot.join("public"))?;

        let response = send_request(docroot.canonicalize()?, "/public/secret.txt").await?;

        assert!(
            !response
                .windows(14)
                .any(|window| window == b"outside-secret")
        );

        Ok(())
    }
}

mod server {
    use crate::server::{accept_client, is_transient_accept_error};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::Semaphore;
    use tokio::time::timeout;

    #[test]
    fn classifies_transient_accept_errors() {
        for kind in [
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::WouldBlock,
            std::io::ErrorKind::ConnectionAborted,
            std::io::ErrorKind::ConnectionReset,
            std::io::ErrorKind::TimedOut,
        ] {
            assert!(is_transient_accept_error(&std::io::Error::from(kind)));
        }
        assert!(!is_transient_accept_error(&std::io::Error::from(
            std::io::ErrorKind::PermissionDenied
        )));
    }

    #[tokio::test]
    async fn waits_for_capacity_before_accepting_another_client()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let clients = Arc::new(Semaphore::new(1));
        let held_permit = Arc::clone(&clients).acquire_owned().await?;
        let client = TcpStream::connect(listener.local_addr()?).await?;
        let mut accept_task = tokio::spawn(async move { accept_client(&listener, &clients).await });

        assert!(
            timeout(Duration::from_millis(50), &mut accept_task)
                .await
                .is_err()
        );

        drop(held_permit);
        let accepted = timeout(Duration::from_secs(1), &mut accept_task).await???;
        drop(accepted);
        drop(client);

        Ok(())
    }
}

#[cfg(unix)]
mod static_files {
    use crate::http::response::{http_response, response_with_deadline};
    use crate::http::status::write_all_with_timeout;
    use crate::static_files::DocumentRoot;
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

        use crate::http::validate_request::{HttpRequest, validate_and_resolve};

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

mod cli_validation {
    use crate::cli::validation::ensure_default_docroot;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> std::io::Result<Self> {
            let unique = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "http-tiny-default-docroot-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn creates_www_under_the_application_working_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        let working_directory = TestDir::new()?;

        let docroot = ensure_default_docroot(&working_directory.0)?;

        assert_eq!(docroot, working_directory.0.join("www"));
        assert!(working_directory.0.join("www").is_dir());

        Ok(())
    }
}

mod request {
    use crate::http::request::{RequestError, parse_request, read_request_with_timeout};
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
    async fn request_timeout_is_absolute() -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
        let (mut server, _) = listener.accept().await?;
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
        let result = result.map_err(std::io::Error::other)??;
        let request = result
            .map_err(|error| std::io::Error::other(format!("request parsing failed: {error:?}")))?;
        assert!(request.is_none());

        Ok(())
    }
}

mod validate_request {
    use crate::http::validate_request::{HttpRequest, validate_and_resolve};
    use std::fs::{self, create_dir};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> std::io::Result<Self> {
            let unique = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("http-tiny-{}-{unique}", std::process::id()));
            create_dir(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn get_request(url: &str) -> HttpRequest<'_> {
        HttpRequest {
            method: "GET",
            url,
            version: "HTTP/1.1",
        }
    }

    #[test]
    fn resolves_directory_requests_to_index_html() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        create_dir(root.0.join("assets"))?;
        fs::write(root.0.join("index.html"), b"home")?;
        fs::write(root.0.join("assets/index.html"), b"assets")?;
        let canonical_root = root.0.canonicalize()?;

        let index = validate_and_resolve(&get_request("/"), &canonical_root)
            .map_err(|error| std::io::Error::other(format!("path resolution failed: {error:?}")))?;
        let assets = validate_and_resolve(&get_request("/assets"), &canonical_root)
            .map_err(|error| std::io::Error::other(format!("path resolution failed: {error:?}")))?;

        assert_eq!(index, root.0.join("index.html").canonicalize()?);
        assert_eq!(assets, root.0.join("assets/index.html").canonicalize()?);

        Ok(())
    }

    #[test]
    fn rejects_percent_encoded_parent_traversal() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let canonical_root = root.0.canonicalize()?;

        assert!(validate_and_resolve(&get_request("/%2e%2e/secret.txt"), &canonical_root).is_err());

        Ok(())
    }

    #[test]
    fn resolves_raw_and_percent_encoded_utf8_paths() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        fs::write(root.0.join("café.txt"), b"utf8")?;
        let canonical_root = root.0.canonicalize()?;
        let expected = root.0.join("café.txt").canonicalize()?;

        assert_eq!(
            validate_and_resolve(&get_request("/café.txt"), &canonical_root),
            Ok(expected.clone())
        );
        assert_eq!(
            validate_and_resolve(&get_request("/caf%C3%A9.txt"), &canonical_root),
            Ok(expected)
        );

        Ok(())
    }

    #[test]
    fn rejects_percent_encoded_invalid_utf8() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        fs::write(root.0.join("ÿ.txt"), b"not utf8")?;
        let canonical_root = root.0.canonicalize()?;

        assert_eq!(
            validate_and_resolve(&get_request("/%FF.txt"), &canonical_root),
            Err(crate::http::validate_request::PathError::BadRequest)
        );

        Ok(())
    }

    #[test]
    fn decodes_separators_before_path_normalization() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        create_dir(root.0.join("nested"))?;
        fs::write(root.0.join("nested/file.txt"), b"file")?;
        let canonical_root = root.0.canonicalize()?;

        assert_eq!(
            validate_and_resolve(&get_request("/nested%2ffile.txt"), &canonical_root),
            Ok(root.0.join("nested/file.txt").canonicalize()?)
        );
        let expected = root.0.join("nested/file.txt").canonicalize()?;
        assert_eq!(
            validate_and_resolve(&get_request("/nested%5cfile.txt"), &canonical_root),
            Ok(expected.clone())
        );
        assert_eq!(
            validate_and_resolve(&get_request("/nested//file.txt"), &canonical_root),
            Ok(expected.clone())
        );
        assert_eq!(
            validate_and_resolve(&get_request(r"/nested\file.txt"), &canonical_root),
            Ok(expected)
        );

        Ok(())
    }

    #[test]
    fn preserves_query_fragment_and_encoded_delimiter_treatment()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        fs::write(root.0.join("page.html"), b"page")?;
        fs::write(root.0.join("hash#query?.txt"), b"encoded")?;
        let canonical_root = root.0.canonicalize()?;

        assert_eq!(
            validate_and_resolve(
                &get_request("/page.html?download=1#section"),
                &canonical_root
            ),
            Ok(root.0.join("page.html").canonicalize()?)
        );
        assert_eq!(
            validate_and_resolve(&get_request("/hash%23query%3F.txt"), &canonical_root),
            Ok(root.0.join("hash#query?.txt").canonicalize()?)
        );

        Ok(())
    }

    #[test]
    fn rejects_malformed_escapes_and_encoded_nul() -> Result<(), Box<dyn std::error::Error>> {
        let root = TestRoot::new()?;
        let canonical_root = root.0.canonicalize()?;

        for target in ["/%", "/%2", "/%GG", "/file%00.txt"] {
            assert_eq!(
                validate_and_resolve(&get_request(target), &canonical_root),
                Err(crate::http::validate_request::PathError::BadRequest)
            );
        }

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_document_root() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let fixture = TestRoot::new()?;
        let docroot = fixture.0.join("www");
        let outside = fixture.0.join("outside");
        create_dir(&docroot)?;
        create_dir(&outside)?;
        fs::write(outside.join("secret.txt"), b"secret")?;
        symlink(&outside, docroot.join("public"))?;
        let canonical_root = docroot.canonicalize()?;

        assert!(validate_and_resolve(&get_request("/public/secret.txt"), &canonical_root).is_err());

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn does_not_serve_hidden_files_through_visible_symlinks()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new()?;
        create_dir(root.0.join(".secret"))?;
        fs::write(root.0.join(".secret/credentials.txt"), b"secret")?;
        symlink(".secret", root.0.join("public"))?;

        let request = HttpRequest {
            method: "GET",
            url: "/public/credentials.txt",
            version: "HTTP/1.1",
        };

        let canonical_root = root.0.canonicalize()?;
        assert!(validate_and_resolve(&request, &canonical_root).is_err());

        Ok(())
    }
}

mod mime {
    use crate::http::mime::http_mime;
    use std::path::Path;

    #[test]
    fn recognizes_ascii_case_insensitive_extensions() {
        for (path, expected) in [
            ("index.HTML", "text/html; charset=utf-8"),
            ("app.Js", "application/javascript; charset=utf-8"),
            (
                "manifest.WEBMANIFEST",
                "application/manifest+json; charset=utf-8",
            ),
        ] {
            assert_eq!(http_mime(Path::new(path)), expected);
        }
    }

    #[test]
    fn unknown_extensions_use_the_binary_fallback() {
        assert_eq!(
            http_mime(Path::new("file.unknown")),
            "application/octet-stream"
        );
        assert_eq!(
            http_mime(Path::new("file.extension-that-is-too-long")),
            "application/octet-stream"
        );
    }
}

mod configuration_sources {
    use crate::cli::{resolve_port, resolve_string};

    #[test]
    fn command_line_string_value_precedes_environment_and_default() {
        assert_eq!(
            resolve_string(
                Some("from-cli".to_owned()),
                Some("from-env".to_owned()),
                "built-in"
            ),
            "from-cli"
        );
    }

    #[test]
    fn environment_string_value_precedes_default() {
        assert_eq!(
            resolve_string(None, Some("from-env".to_owned()), "built-in"),
            "from-env"
        );
    }

    #[test]
    fn uses_string_default_when_higher_priority_sources_are_unset() {
        assert_eq!(resolve_string(None, None, "built-in"), "built-in");
    }

    #[test]
    fn command_line_port_precedes_even_an_invalid_environment_value() {
        assert_eq!(resolve_port(Some(1234), Some("invalid")), Ok(1234));
    }

    #[test]
    fn uses_environment_port_when_command_line_is_unset() {
        assert_eq!(resolve_port(None, Some("4321")), Ok(4321));
    }

    #[test]
    fn rejects_an_invalid_environment_port() {
        assert!(resolve_port(None, Some("not-a-port")).is_err());
    }

    #[test]
    fn uses_default_port_when_no_higher_priority_value_is_set() {
        assert_eq!(resolve_port(None, None), Ok(8080));
    }
}
