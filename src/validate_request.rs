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
// HTTP request validation function.
// Checks if the HTTP request is valid and safely converts the URL into a filesystem path.
// Returns `Some(PathBuf)` if valid, otherwise `None`.
// #############################################################################################
pub fn validate_and_resolve(req: &HttpRequest, docroot: &Path) -> Option<PathBuf> {
    // Only allow GET requests.
    if req.method != "GET" {
        return None;
    }

    // Only support HTTP/1.1.
    if req.version != "HTTP/1.1" {
        return None;
    }

    // Canonicalize the document root
    let canonical_root = docroot.canonicalize().ok()?;

    // Normalize path separators.
    let url = req.url.replace('\\', "/");

    // Strip query string and fragment.
    let url = url.split(['?', '#']).next().unwrap_or("");

    // Collapse repeated slashes.
    let url = collapse_slashes(url);

    // Decode percent-encoded characters.
    let url = percent_decode(&url)?;

    // URL must start with a slash
    if !url.starts_with('/') {
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
    let mut final_url = url;

    // If URL ends with '/', serve index.html file automatically if it exists.
    // Example: "http://example.com/" serves "http://example.com/index.html"
    // Example: "http://example.com/folder/" serves "http://example.com/folder/index.html"
    if is_dir_request {
        if !final_url.ends_with('/') {
            final_url.push('/');
        }
        final_url.push_str("index.html");
    }

    // Convert the normalized URL into a relative filesystem path.
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


    let canonical = root.canonicalize().ok()?;

    // Ensure the final path is still inside docroot (security check)
    if !canonical.starts_with(&canonical_root) {
        return None;
    }

    // Return the safe resolved filesystem path
    Some(canonical)
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
// Decode percent-encoded characters in a URL path.
// Returns None if the input contains malformed percent escapes.
// #############################################################################################
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());

    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'%' {
            output.push(bytes[i] as char);
            i += 1;
            continue;
        }

        // '%' must be followed by exactly two hexadecimal digits.
        if i + 2 >= bytes.len() {
            return None;
        }

        let high = hex_value(bytes[i + 1])?;
        let low = hex_value(bytes[i + 2])?;

        let decoded = (high << 4) | low;

        // Reject embedded NUL bytes.
        if decoded == 0 {
            return None;
        }

        output.push(decoded as char);
        i += 3;
    }

    Some(output)
}

// #############################################################################################
// Collapse repeated slashes into a single slash.
// #############################################################################################
fn collapse_slashes(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut previous_was_slash = false;

    for ch in path.chars() {
        if ch == '/' {
            if !previous_was_slash {
                out.push('/');
            }
            previous_was_slash = true;
        } else {
            out.push(ch);
            previous_was_slash = false;
        }
    }

    out
}