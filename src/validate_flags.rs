// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use std::fs::{self}; // Filesystem utilities (File + helper functions like read_dir, metadata, etc.)
use std::net::IpAddr; // Networking types for creating a responser and handling client connections.
use std::path::Path; // Used for working with filesystem paths and their individual components.
use std::str::FromStr; // Used for parsing strings into IP addresses (e.g. "127.0.0.1" -> IpAddr)

// #############################################################################################
// IP validation function
// Checks if the IP address the user specified can actually be bound to
// #############################################################################################
pub(crate) fn validate_ip(ip: &str) -> Result<String, String> {
    match IpAddr::from_str(ip) {
        Ok(_) => Ok(ip.to_string()),
        Err(_) => Err(format!("Invalid IP address specified: {ip}")),
    }
}

// #############################################################################################
// Port validation function
// Checks if the Port the user specified can actually be listened on
// #############################################################################################
pub(crate) fn validate_port(port: u16) -> Result<u16, String> {
    if (1..=65535).contains(&port) {
        Ok(port)
    } else {
        Err(format!("Invalid port specified: {port}"))
    }
}

// Create the document root when it does not exist yet, then validate it.
pub(crate) fn ensure_docroot(path: &str) -> Result<String, String> {
    let docroot = Path::new(path);
    if !docroot.exists() {
        fs::create_dir_all(docroot)
            .map_err(|_| format!("Could not create document root {path}"))?;
    }
    validate_docroot(path)
}

// #############################################################################################
// Document root validation function
// Checks that the document root is a readable directory. The caller creates it
// when missing, so it is expected to exist here.
// #############################################################################################
pub(crate) fn validate_docroot(path: &str) -> Result<String, String> {
    // Create a filesystem-aware view of the string path (no allocation, no copy)
    let p = Path::new(path);

    // Check if the document root is a directory
    if !p.is_dir() {
        return Err(format!("Document root is not a directory: {path}"));
    }

    // Check if the document root can be read
    match fs::read_dir(p) {
        Ok(_) => Ok(path.to_string()),
        Err(e) => Err(format!("Document root is not readable: {path} ({e})")),
    }
}
