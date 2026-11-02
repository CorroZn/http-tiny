// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use clap::Parser; // Used for command-line argument parsing. (clap crate)
use std::io::{self, Write};

use crate::Args; // Import the `Args` defined with clap for command-line arguments
use crate::validate_flags::ensure_default_docroot;
use crate::validate_flags::validate_docroot; // Import from validate_flags.rs - Used for validating user specified document root
use crate::validate_flags::validate_ip; // Import from validate_flags.rs - Used for validating user specified IP address
use crate::validate_flags::validate_port; // Import from validate_flags.rs - Used for validating user specified listening port
use crate::{SERVER_IP, SERVER_PORT}; // Import default configuration values from main.rs that we can fallback to if the user provides no values

// #############################################################################################
// CLI flag handler function
// Will use custom flags if the user specified some or fall back to the default values
// #############################################################################################
pub(crate) fn flags_handler() -> (String, String, u16) {
    let args = Args::parse();

    // Use the supplied document root, or create www beneath the working directory.
    let docroot = args.docroot.unwrap_or_else(|| {
        let working_directory = std::env::current_dir().unwrap_or_else(|e| {
            exit_with_error(&format!("Failed to determine the working directory: {e}"));
        });
        ensure_default_docroot(&working_directory)
            .unwrap_or_else(|e| {
                exit_with_error(&format!("Failed to create the default document root: {e}"));
            })
            .to_string_lossy()
            .into_owned()
    });
    let docroot = validate_docroot(&docroot).unwrap_or_else(|e| {
        exit_with_error(&e);
    });

    // Call the validate_ip() function
    let ip = validate_ip(&args.ip.unwrap_or_else(|| SERVER_IP.to_string())).unwrap_or_else(|e| {
        exit_with_error(&e);
    });

    // Call the validate_port() function
    let port = validate_port(args.port.unwrap_or(SERVER_PORT)).unwrap_or_else(|e| {
        exit_with_error(&e);
    });

    // On successful flag validation, print to CLI that the HTTP server is running and serving.
    if let Err(error) = writeln!(
        io::stdout().lock(),
        "HTTP server started: Serving {docroot} at {ip} on port {port}"
    ) {
        exit_with_error(&format!("Failed to write startup message: {error}"));
    }

    (docroot, ip, port)
}

// Report a startup failure before exiting, even if diagnostics cannot be written.
fn exit_with_error(message: &str) -> ! {
    let _ = writeln!(io::stderr().lock(), "{message}");
    std::process::exit(1);
}
