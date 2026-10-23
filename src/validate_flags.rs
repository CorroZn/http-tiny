// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::fs::{self}; // Filesystem utilities (File + helper functions like read_dir, metadata, etc.)
use std::net::IpAddr; // Networking types for creating a responser and handling client connections.
use std::path::{Path, PathBuf}; // Used for working with filesystem paths and their individual components.
use std::str::FromStr; // Used for parsing strings into IP addresses (e.g. "127.0.0.1" -> IpAddr)

// #############################################################################################
// IP validation function
// Checks if the IP address the user specified can actually be bound to
// #############################################################################################
pub fn validate_ip(ip: &str) -> Result<String, String> {
    match IpAddr::from_str(ip) {
        Ok(_) => Ok(ip.to_string()),
        Err(_) => Err(format!("Invalid IP address specified: {}", ip)),
    }
}

// #############################################################################################
// Port validation function
// Checks if the Port the user specified can actually be listened on
// #############################################################################################
pub fn validate_port(port: u16) -> Result<u16, String> {
    if (1..=65535).contains(&port) {
        Ok(port)
    } else {
        Err(format!("Invalid port specified: {}", port))
    }
}

// Create the default document root beneath the application's working directory.
pub fn ensure_default_docroot(working_directory: &Path) -> std::io::Result<PathBuf> {
    let docroot = working_directory.join("www");
    fs::create_dir_all(&docroot)?;
    Ok(docroot)
}

// #############################################################################################
// Document root validation function
// Checks if the document root actually exists and can be read by this HTTP server
// #############################################################################################
pub fn validate_docroot(path: &str) -> Result<String, String> {
    // Create a filesystem-aware view of the string path (no allocation, no copy)
    let p = Path::new(path);

    // Check if the document root exists
    if !p.exists() {
        return Err(format!("Document root does not exist: {}", path));
    }

    // Check if the document root is a directory
    if !p.is_dir() {
        return Err(format!("Document root is not a directory: {}", path));
    }

    // Check if the document root can be read
    match fs::read_dir(p) {
        Ok(_) => Ok(path.to_string()),
        Err(e) => Err(format!("Document root is not readable: {} ({})", path, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_default_docroot;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let unique = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "http-tiny-default-docroot-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn creates_www_under_the_application_working_directory() {
        let working_directory = TestDir::new();

        let docroot = ensure_default_docroot(&working_directory.0).unwrap();

        assert_eq!(docroot, working_directory.0.join("www"));
        assert!(working_directory.0.join("www").is_dir());
    }
}
