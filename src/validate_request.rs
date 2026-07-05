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
    // Only allow GET requests for now
    if req.method != "GET" {
        return None;
    }

    // Only HTTP/1.1 for now
    if req.version != "HTTP/1.1" {
        return None;
    }

    // Normalize URL first
    let url = req.url.replace('\\', "/").to_ascii_lowercase();

    // Split off query + fragment into an owned string
    let mut url = url.split(['?', '#']).next().unwrap_or("").to_string();

    // Collapse repeated slashes like into /
    while url.contains("//") {
        url = url.replace("//", "/");
    }

    // URL must start with a slash
    if !url.starts_with('/') {
        return None;
    }

    // Allowed character check
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

    // Decide whether this is a directory request
    let mut is_dir_request = url.ends_with('/');

    // Check "/subdirectory" → if it exists as directory, treat it as a directory
    if !is_dir_request {
        // Start from the document root and build a candidate filesystem path
        let mut candidate = docroot.to_path_buf();

        // Append the requested URL path (without leading '/')
        candidate.push(url.trim_start_matches('/'));

        // If this path exists and is a directory, treat it as a directory request
        // so we can later serve its default index.html file
        if candidate.is_dir() {
            is_dir_request = true;
        }
    }

    // Build final URL path string
    let mut final_url = url.clone();

    // If URL ends with '/', serve index.html automatically
    // Example: http://example.corm/ -> http://example.corm/index.html
    // Example: http://example.corm/folder/ -> http://example.corm/folder/index.html
    if is_dir_request {
        if !final_url.ends_with('/') {
            final_url.push('/');
        }
        final_url.push_str("index.html");
    }

    // Convert into Path
    let path = Path::new(final_url.trim_start_matches('/'));

    // Convert document root into a mutable path buffer
    let mut root = docroot.to_path_buf();

    // Walk through path components safely
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                // Convert component to string for validation
                let s = name.to_string_lossy();

                // Block hidden files and directories like .env or .git
                if s.starts_with('.') {
                    return None;
                }

                // Append safe path component
                root.push(name);
            }

            Component::CurDir => {
                // Ignore "." (current directory)
            }

            // Block traversal attacks
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
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
