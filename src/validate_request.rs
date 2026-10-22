// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::{Component, Path, PathBuf}; // Used for working with filesystem paths and their individual components.

// A simple representation of the first line of an HTTP request.
// < METHOD > < PATH > < HTTP/Version >
// Example: "GET /index.html HTTP/1.1"
pub struct HttpRequest<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub version: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathError {
    BadRequest,
    NotFound,
}

// #############################################################################################
// HTTP request validation function.
// Checks if the HTTP request is valid and safely converts the URL into a filesystem path.
// Returns the resolved path if valid, otherwise a bounded request error.
// `canonical_root` is opened once by the listener before requests are handled.
// #############################################################################################
pub fn validate_and_resolve(
    req: &HttpRequest<'_>,
    canonical_root: &Path,
) -> Result<PathBuf, PathError> {
    // Only allow GET requests.
    if req.method != "GET" {
        return Err(PathError::NotFound);
    }

    // Only support HTTP/1.1.
    if req.version != "HTTP/1.1" {
        return Err(PathError::NotFound);
    }

    // Strip the query string and fragment before decoding the path.
    let url_bytes = req.url.as_bytes();
    let path_end = url_bytes
        .iter()
        .position(|byte| matches!(byte, b'?' | b'#'))
        .unwrap_or(url_bytes.len());
    let mut url = percent_decode(&url_bytes[..path_end]).ok_or(PathError::BadRequest)?;

    // Decode escapes before normalizing separators and checking path components.
    normalize_path(&mut url);
    let url = std::str::from_utf8(&url).map_err(|_| PathError::BadRequest)?;

    // URL must start with a slash
    if !url.starts_with('/') {
        return Err(PathError::BadRequest);
    }

    // Convert the normalized URL into a safe path under the document root.
    let mut root = canonical_root.to_path_buf();
    for component in Path::new(url.trim_start_matches('/')).components() {
        match component {
            Component::Normal(name) => {
                // Block hidden files and directories like .env or .git
                if name.to_string_lossy().starts_with('.') {
                    return Err(PathError::NotFound);
                }

                root.push(name);
            }

            Component::CurDir => {
                // Ignore "." (current directory)
            }

            // Block traversal attacks
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PathError::NotFound);
            }
        }
    }

    // Serve index.html for explicit and implicit directory requests.
    if url.ends_with('/') || root.is_dir() {
        root.push("index.html");
    }

    let canonical = root.canonicalize().map_err(|_| PathError::NotFound)?;

    // Ensure the final path is still inside docroot (security check)
    let relative = canonical
        .strip_prefix(canonical_root)
        .map_err(|_| PathError::NotFound)?;

    // A symlink can point into a hidden directory, so check the resolved path too.
    if relative.components().any(|component| match component {
        Component::Normal(name) => name.to_string_lossy().starts_with('.'),
        _ => false,
    }) {
        return Err(PathError::NotFound);
    }

    // Return the safe resolved filesystem path
    Ok(canonical)
}

// #############################################################################################
// Convert a single hexadecimal digit into its numeric value.
// #############################################################################################
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// #############################################################################################
// Decode percent-encoded bytes in a URL path.
// Returns None if the input contains malformed escapes or an embedded NUL.
// #############################################################################################
fn percent_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len());
    let mut i = 0;

    while i < input.len() {
        if input[i] != b'%' {
            if input[i] == 0 {
                return None;
            }
            output.push(input[i]);
            i += 1;
            continue;
        }

        // '%' must be followed by exactly two hexadecimal digits.
        if i + 2 >= input.len() {
            return None;
        }

        let high = hex_value(input[i + 1])?;
        let low = hex_value(input[i + 2])?;

        let decoded = (high << 4) | low;

        // Reject embedded NUL bytes.
        if decoded == 0 {
            return None;
        }

        output.push(decoded);
        i += 3;
    }

    Some(output)
}

// #############################################################################################
// Normalize URL separators and collapse repeated slashes in place.
// #############################################################################################
fn normalize_path(path: &mut Vec<u8>) {
    let mut write = 0;
    let mut previous_was_slash = false;

    for read in 0..path.len() {
        let byte = if path[read] == b'\\' {
            b'/'
        } else {
            path[read]
        };

        if byte == b'/' && previous_was_slash {
            continue;
        }

        path[write] = byte;
        write += 1;
        previous_was_slash = byte == b'/';
    }

    path.truncate(write);
}

#[cfg(test)]
mod tests {
    use super::{HttpRequest, validate_and_resolve};
    use std::fs::{self, create_dir};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let unique = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("http-tiny-{}-{unique}", std::process::id()));
            create_dir(&path).unwrap();
            Self(path)
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
    fn resolves_directory_requests_to_index_html() {
        let root = TestRoot::new();
        create_dir(root.0.join("assets")).unwrap();
        fs::write(root.0.join("index.html"), b"home").unwrap();
        fs::write(root.0.join("assets/index.html"), b"assets").unwrap();
        let canonical_root = root.0.canonicalize().unwrap();

        let index = validate_and_resolve(&get_request("/"), &canonical_root).unwrap();
        let assets = validate_and_resolve(&get_request("/assets"), &canonical_root).unwrap();

        assert_eq!(index, root.0.join("index.html").canonicalize().unwrap());
        assert_eq!(
            assets,
            root.0.join("assets/index.html").canonicalize().unwrap()
        );
    }

    #[test]
    fn rejects_percent_encoded_parent_traversal() {
        let root = TestRoot::new();
        let canonical_root = root.0.canonicalize().unwrap();

        assert!(validate_and_resolve(&get_request("/%2e%2e/secret.txt"), &canonical_root).is_err());
    }

    #[test]
    fn resolves_raw_and_percent_encoded_utf8_paths() {
        let root = TestRoot::new();
        fs::write(root.0.join("café.txt"), b"utf8").unwrap();
        let canonical_root = root.0.canonicalize().unwrap();
        let expected = root.0.join("café.txt").canonicalize().unwrap();

        assert_eq!(
            validate_and_resolve(&get_request("/café.txt"), &canonical_root),
            Ok(expected.clone())
        );
        assert_eq!(
            validate_and_resolve(&get_request("/caf%C3%A9.txt"), &canonical_root),
            Ok(expected)
        );
    }

    #[test]
    fn rejects_percent_encoded_invalid_utf8() {
        let root = TestRoot::new();
        fs::write(root.0.join("ÿ.txt"), b"not utf8").unwrap();
        let canonical_root = root.0.canonicalize().unwrap();

        assert_eq!(
            validate_and_resolve(&get_request("/%FF.txt"), &canonical_root),
            Err(super::PathError::BadRequest)
        );
    }

    #[test]
    fn decodes_separators_before_path_normalization() {
        let root = TestRoot::new();
        create_dir(root.0.join("nested")).unwrap();
        fs::write(root.0.join("nested/file.txt"), b"file").unwrap();
        let canonical_root = root.0.canonicalize().unwrap();

        assert_eq!(
            validate_and_resolve(&get_request("/nested%2ffile.txt"), &canonical_root),
            Ok(root.0.join("nested/file.txt").canonicalize().unwrap())
        );
        assert_eq!(
            validate_and_resolve(&get_request("/nested%5cfile.txt"), &canonical_root),
            Ok(root.0.join("nested/file.txt").canonicalize().unwrap())
        );
    }

    #[test]
    fn preserves_query_fragment_and_encoded_delimiter_treatment() {
        let root = TestRoot::new();
        fs::write(root.0.join("page.html"), b"page").unwrap();
        fs::write(root.0.join("hash#query?.txt"), b"encoded").unwrap();
        let canonical_root = root.0.canonicalize().unwrap();

        assert_eq!(
            validate_and_resolve(
                &get_request("/page.html?download=1#section"),
                &canonical_root
            ),
            Ok(root.0.join("page.html").canonicalize().unwrap())
        );
        assert_eq!(
            validate_and_resolve(&get_request("/hash%23query%3F.txt"), &canonical_root),
            Ok(root.0.join("hash#query?.txt").canonicalize().unwrap())
        );
    }

    #[test]
    fn rejects_malformed_escapes_and_encoded_nul() {
        let root = TestRoot::new();
        let canonical_root = root.0.canonicalize().unwrap();

        for target in ["/%", "/%2", "/%GG", "/file%00.txt"] {
            assert_eq!(
                validate_and_resolve(&get_request(target), &canonical_root),
                Err(super::PathError::BadRequest)
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_document_root() {
        use std::os::unix::fs::symlink;

        let fixture = TestRoot::new();
        let docroot = fixture.0.join("www");
        let outside = fixture.0.join("outside");
        create_dir(&docroot).unwrap();
        create_dir(&outside).unwrap();
        fs::write(outside.join("secret.txt"), b"secret").unwrap();
        symlink(&outside, docroot.join("public")).unwrap();
        let canonical_root = docroot.canonicalize().unwrap();

        assert!(validate_and_resolve(&get_request("/public/secret.txt"), &canonical_root).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn does_not_serve_hidden_files_through_visible_symlinks() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        create_dir(root.0.join(".secret")).unwrap();
        fs::write(root.0.join(".secret/credentials.txt"), b"secret").unwrap();
        symlink(".secret", root.0.join("public")).unwrap();

        let request = HttpRequest {
            method: "GET",
            url: "/public/credentials.txt",
            version: "HTTP/1.1",
        };

        let canonical_root = root.0.canonicalize().unwrap();
        assert!(validate_and_resolve(&request, &canonical_root).is_err());
    }
}
