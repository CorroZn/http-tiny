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
