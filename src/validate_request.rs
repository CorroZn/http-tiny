// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::path::{Component, Path, PathBuf}; // Used for working with filesystem paths and their individual components.

// A simple representation of the first line of an HTTP request.
// < METHOD > < PATH > < HTTP/Version >
// Example: "GET /index.html HTTP/1.1"
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub version: String,
}

// #############################################################################################
// HTTP requesr validation function
// Checks if the HTTP request is valid and safely converts the URL into a filesystem path.
// Returns `Some(PathBuf)` if valid, otherwise `None`.
// #############################################################################################
pub fn validate_and_resolve(req: HttpRequest, docroot: &Path) -> Option<PathBuf> {
    // Only allow GET requests
    if req.method != "GET" {
        return None;
    }

    // Only HTTP/1.1 for now
    if req.version != "HTTP/1.1" {
        return None;
    }

    // Normalize slashes and lowercase the URL
    let mut url = req.url.replace('\\', "/").to_ascii_lowercase();

    // Collapse repeated slashes like // into /
    while url.contains("//") {
        url = url.replace("//", "/");
    }

    // URL must start with a slash
    if !url.starts_with('/') {
        return None;
    }

    // allowed character check
    if !url.bytes().all(|b| {
        matches!(
            b,
            b'a'..=b'z'
        | b'0'..=b'9'
        | b'-'
        | b'_'
        | b'.'
        | b'/'
        | b'?'
        | b'='
        | b'%'
        | b'#'
        | b'&'
        | b'$'
        | b'@'
        )
    }) {
        return None;
    }

    // Convert document root into a mutable path buffer
    let mut root = docroot.to_path_buf();

    // If URL ends with '/', serve index.html automatically
    // Example: http://example.corm/ -> http://example.corm/index.html
    // Example: http://example.corm/folder/ -> http://example.corm/folder/index.html
    if url.ends_with('/') {
        url.push_str("index.html");
    }

    // Remove leading slash and convert into a Path
    let path = Path::new(url.trim_start_matches('/'));

    // Walk through path components safely
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                // Convert component to string for validation
                let s = name.to_string_lossy();

                // Block hidden files like .env or .git
                if s.starts_with('.') {
                    return None;
                }

                // Append safe path component
                root.push(name);
            }

            Component::CurDir => {
                // Ignore "." (current directory)
            }

            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                // Block path traversal like ../ or absolute paths
                return None;
            }
        }
    }

    // Convert to absolute canonical path (resolves symlinks etc.)
    let canonical = root.canonicalize().ok()?;

    // Ensure the final path is still inside docroot (security check)
    if !canonical.starts_with(docroot) {
        return None;
    }

    // Return the safe resolved filesystem path
    Some(canonical)
}
