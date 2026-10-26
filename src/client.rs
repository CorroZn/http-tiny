// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::io;
use std::sync::Arc; // Handle shared ownership.
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpStream; // Tokio TCP stream type used for network communication.
use tokio::time::Instant;

use crate::parse_request::{RequestError, http_parse_validate}; // Parses and validates incoming HTTP requests.
use crate::response::{DocumentRoot, http_response}; // Serves HTTP responses to connected clients.
use crate::status::{
    RESPONSE_WRITE_TIMEOUT, status_bad_request, status_header_too_large, status_method_not_allowed,
    status_not_found, status_version_not_supported,
};

static CLIENT_FAILURES: AtomicUsize = AtomicUsize::new(0);

fn report_failure(category: &str, kind: io::ErrorKind) {
    let failures = CLIENT_FAILURES.fetch_add(1, Ordering::Relaxed);
    if failures & 1023 == 0 {
        eprintln!("http-tiny: {category} failed ({kind:?})");
    }
}

fn request_error_name(error: RequestError) -> &'static str {
    match error {
        RequestError::BadRequest => "bad request",
        RequestError::MethodNotAllowed => "unsupported method",
        RequestError::VersionNotSupported => "unsupported HTTP version",
        RequestError::HeaderTooLarge => "request headers too large",
        RequestError::NotFound => "not found",
    }
}

// #############################################################################################
// HTTP client handler function.
// Handles a single client connection by parsing and validating the
// HTTP request before serving the requested resource.
// #############################################################################################
pub async fn http_handle_client(mut stream: TcpStream, docroot: Arc<DocumentRoot>) {
    // Check if the request is valid and get the requested file path.
    match http_parse_validate(&mut stream, docroot.path()).await {
        Ok(Some(path)) => {
            // Send the requested file, or a 404 page if it doesn't exist.
            if let Err(error) = http_response(stream, docroot, path).await {
                report_failure("response", error.kind());
            }
        }
        Ok(None) => {}
        Err(error) => {
            let deadline = Instant::now() + RESPONSE_WRITE_TIMEOUT;
            let category = request_error_name(error);
            let response = match error {
                RequestError::BadRequest => status_bad_request(&mut stream, deadline).await,
                RequestError::MethodNotAllowed => {
                    status_method_not_allowed(&mut stream, deadline).await
                }
                RequestError::VersionNotSupported => {
                    status_version_not_supported(&mut stream, deadline).await
                }
                RequestError::HeaderTooLarge => {
                    status_header_too_large(&mut stream, deadline).await
                }
                RequestError::NotFound => status_not_found(&mut stream, deadline).await,
            };
            report_failure(category, io::ErrorKind::InvalidData);
            if let Err(error) = response {
                report_failure("error response", error.kind());
            }
        }
    }
    // The connection is closed automatically when this task ends.
}

#[cfg(test)]
mod tests {
    use super::http_handle_client;
    use crate::response::DocumentRoot;
    use std::fs::{self, create_dir};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let unique = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("http-tiny-client-{}-{unique}", std::process::id()));
            create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    async fn send_request(docroot: PathBuf, target: &str) -> Vec<u8> {
        let request = format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n");
        send_raw_request(docroot, request.as_bytes()).await
    }

    async fn send_raw_request(docroot: PathBuf, request: &[u8]) -> Vec<u8> {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (server, _) = listener.accept().await.unwrap();
        let docroot = Arc::new(DocumentRoot::open(docroot.to_str().unwrap()).unwrap());
        let client_task = tokio::spawn(http_handle_client(server, docroot));
        let mut client = client;
        client.write_all(request).await.unwrap();

        let mut response = Vec::new();
        let _ = client.read_to_end(&mut response).await;
        client_task.await.unwrap();
        response
    }

    #[tokio::test]
    async fn rejects_requests_without_a_host_header() {
        let root = TestRoot::new();
        fs::write(root.0.join("index.html"), b"hello").unwrap();

        let response = send_raw_request(
            root.0.canonicalize().unwrap(),
            b"GET /index.html HTTP/1.1\r\n\r\n",
        )
        .await;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    }

    #[tokio::test]
    async fn rejects_duplicate_host_headers() {
        let root = TestRoot::new();
        let response = send_raw_request(
            root.0.canonicalize().unwrap(),
            b"GET / HTTP/1.1\r\nHost: one\r\nHost: two\r\n\r\n",
        )
        .await;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    }

    #[tokio::test]
    async fn rejects_bare_lf_line_endings() {
        let root = TestRoot::new();
        let response = send_raw_request(
            root.0.canonicalize().unwrap(),
            b"GET / HTTP/1.1\nHost: localhost\n\n",
        )
        .await;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    }

    #[tokio::test]
    async fn rejects_malformed_header_lines() {
        let root = TestRoot::new();
        let response = send_raw_request(
            root.0.canonicalize().unwrap(),
            b"GET / HTTP/1.1\r\nHost: localhost\r\nBad Header: value\r\n\r\n",
        )
        .await;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    }

    #[tokio::test]
    async fn rejects_extra_request_line_tokens() {
        let root = TestRoot::new();
        let response = send_raw_request(
            root.0.canonicalize().unwrap(),
            b"GET / extra HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    }

    #[tokio::test]
    async fn reports_unsupported_methods_and_versions() {
        let root = TestRoot::new();
        let root = root.0.canonicalize().unwrap();
        let method_response =
            send_raw_request(root.clone(), b"POST / HTTP/1.1\r\nHost: localhost\r\n\r\n").await;
        let version_response =
            send_raw_request(root, b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n").await;

        assert!(method_response.starts_with(b"HTTP/1.1 405 Method Not Allowed\r\n"));
        assert!(
            method_response
                .windows(10)
                .any(|window| window == b"Allow: GET")
        );
        assert!(version_response.starts_with(b"HTTP/1.1 505 HTTP Version Not Supported\r\n"));
    }

    #[tokio::test]
    async fn rejects_unsupported_request_body_framing() {
        let root = TestRoot::new();
        let root = root.0.canonicalize().unwrap();
        let transfer_encoding = send_raw_request(
            root.clone(),
            b"GET / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n",
        )
        .await;
        let content_length = send_raw_request(
            root,
            b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\n\r\n",
        )
        .await;

        assert!(transfer_encoding.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
        assert!(content_length.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    }

    #[tokio::test]
    async fn accepts_request_headers_at_the_size_limit() {
        let root = TestRoot::new();
        let mut request = b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Pad: ".to_vec();
        let padding = crate::MAX_REQUEST - request.len() - 4;
        request.resize(request.len() + padding, b'a');
        request.extend_from_slice(b"\r\n\r\n");
        assert_eq!(request.len(), crate::MAX_REQUEST);

        let response = send_raw_request(root.0.canonicalize().unwrap(), &request).await;

        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
    }

    #[tokio::test]
    async fn reports_oversized_request_headers() {
        let root = TestRoot::new();
        let mut request = b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Pad: ".to_vec();
        request.resize(8193, b'a');

        let response = send_raw_request(root.0.canonicalize().unwrap(), &request).await;

        assert!(response.starts_with(b"HTTP/1.1 431 Request Header Fields Too Large\r\n"));
    }

    #[tokio::test]
    async fn includes_nosniff_on_success_and_not_found_responses() {
        let root = TestRoot::new();
        fs::write(root.0.join("index.html"), b"hello").unwrap();
        let root = root.0.canonicalize().unwrap();
        let success = send_request(root.clone(), "/index.html").await;
        let not_found = send_request(root, "/missing.html").await;

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
    }

    #[tokio::test]
    async fn includes_nosniff_on_protocol_error_responses() {
        let root = TestRoot::new();
        let response =
            send_raw_request(root.0.canonicalize().unwrap(), b"GET / HTTP/1.1\r\n\r\n").await;

        assert!(
            response
                .windows(31)
                .any(|window| window == b"X-Content-Type-Options: nosniff")
        );
    }

    #[tokio::test]
    async fn accepts_zero_content_length() {
        let root = TestRoot::new();
        fs::write(root.0.join("index.html"), b"hello").unwrap();
        let response = send_raw_request(
            root.0.canonicalize().unwrap(),
            b"GET /index.html HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n",
        )
        .await;

        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
    }

    #[tokio::test]
    async fn returns_not_found_for_missing_files() {
        let root = TestRoot::new();
        let response = send_request(root.0.canonicalize().unwrap(), "/missing.html").await;

        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
    }

    #[tokio::test]
    async fn rejects_invalid_utf8_request_targets() {
        let root = TestRoot::new();
        let response = send_raw_request(
            root.0.canonicalize().unwrap(),
            b"GET /\xff HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    }

    #[tokio::test]
    async fn rejects_duplicate_content_length_headers() {
        let root = TestRoot::new();
        let response = send_raw_request(
            root.0.canonicalize().unwrap(),
            b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
        )
        .await;

        assert!(response.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));
    }

    #[tokio::test]
    async fn returns_not_found_for_invalid_targets() {
        let root = TestRoot::new();
        let response = send_request(root.0.canonicalize().unwrap(), "/%2e%2e/secret").await;

        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));
    }

    #[tokio::test]
    async fn serves_a_regular_file() {
        let root = TestRoot::new();
        fs::write(root.0.join("index.html"), b"hello").unwrap();

        let response = send_request(root.0.canonicalize().unwrap(), "/index.html").await;
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
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
    }

    #[tokio::test]
    async fn serves_empty_files_with_an_empty_body() {
        let root = TestRoot::new();
        fs::write(root.0.join("empty.txt"), b"").unwrap();

        let response = send_request(root.0.canonicalize().unwrap(), "/empty.txt").await;
        let header_end = response
            .windows(4)
            .position(|window| window == [13, 10, 13, 10])
            .unwrap()
            + 4;

        assert!(response.starts_with(b"HTTP/1.1 200 OK"));
        assert!(
            response[..header_end]
                .windows(17)
                .any(|window| window == b"Content-Length: 0")
        );
        assert_eq!(&response[header_end..], b"");
    }

    #[tokio::test]
    async fn streams_large_files_without_truncation() {
        let root = TestRoot::new();
        let contents = vec![b'x'; 1024 * 1024 + 17];
        fs::write(root.0.join("large.bin"), &contents).unwrap();

        let response = send_request(root.0.canonicalize().unwrap(), "/large.bin").await;
        let header_end = response
            .windows(4)
            .position(|window| window == [13, 10, 13, 10])
            .unwrap()
            + 4;

        assert!(response.starts_with(b"HTTP/1.1 200 OK"));
        assert_eq!(&response[header_end..], contents);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn serves_files_through_symlinks_inside_the_document_root() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        create_dir(root.0.join("assets")).unwrap();
        fs::write(root.0.join("assets/page.txt"), b"public").unwrap();
        symlink("assets", root.0.join("public")).unwrap();

        let response = send_request(root.0.canonicalize().unwrap(), "/public/page.txt").await;

        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with(b"public"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn does_not_serve_hidden_files_through_visible_symlinks() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        create_dir(root.0.join(".secret")).unwrap();
        fs::write(root.0.join(".secret/credentials.txt"), b"secret").unwrap();
        symlink(".secret", root.0.join("public")).unwrap();

        let response =
            send_request(root.0.canonicalize().unwrap(), "/public/credentials.txt").await;

        assert!(!response.windows(6).any(|window| window == b"secret"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn does_not_serve_files_through_symlinks_outside_the_document_root() {
        use std::os::unix::fs::symlink;

        let fixture = TestRoot::new();
        let docroot = fixture.0.join("www");
        let outside = fixture.0.join("outside");
        create_dir(&docroot).unwrap();
        create_dir(&outside).unwrap();
        fs::write(outside.join("secret.txt"), b"outside-secret").unwrap();
        symlink(&outside, docroot.join("public")).unwrap();

        let response = send_request(docroot.canonicalize().unwrap(), "/public/secret.txt").await;

        assert!(
            !response
                .windows(14)
                .any(|window| window == b"outside-secret")
        );
    }
}
