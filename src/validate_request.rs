// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::borrow::Cow;
use std::path::{Component, Path, PathBuf}; // Used for working with filesystem paths and their individual components.

// A simple representation of the first line of an HTTP request.
// < METHOD > < PATH > < HTTP/Version >
// Example: "GET /index.html HTTP/1.1"
pub(crate) struct HttpRequest<'a> {
    pub(crate) method: &'a str,
    pub(crate) url: &'a str,
    pub(crate) version: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathError {
    BadRequest,
    NotFound,
}

// #############################################################################################
// HTTP request validation function.
// Checks if the HTTP request is valid and safely converts the URL into a filesystem path.
// Returns the resolved path if valid, otherwise a bounded request error.
// `canonical_root` is canonicalized once by the listener before requests.
// #############################################################################################
pub(crate) fn validate_and_resolve(
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
    let raw_path = &req.url[..path_end];
    let raw_path_bytes = raw_path.as_bytes();
    let mut requires_normalization = false;
    let mut previous_was_slash = false;
    for &byte in raw_path_bytes {
        if byte == 0 {
            return Err(PathError::BadRequest);
        }
        if byte == b'%' || byte == b'\\' || (byte == b'/' && previous_was_slash) {
            requires_normalization = true;
        }
        previous_was_slash = byte == b'/';
    }

    // Keep ordinary paths borrowed; only decoded or non-canonical paths need a copy.
    let url = if requires_normalization {
        let mut decoded = percent_decode(raw_path_bytes).ok_or(PathError::BadRequest)?;
        normalize_path(&mut decoded);
        Cow::Owned(String::from_utf8(decoded).map_err(|_| PathError::BadRequest)?)
    } else {
        Cow::Borrowed(raw_path)
    };
    let url = url.as_ref();

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
const fn hex_value(byte: u8) -> Option<u8> {
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
            Err(super::PathError::BadRequest)
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
                Err(super::PathError::BadRequest)
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
