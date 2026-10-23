// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use clap::Parser; // Used for command-line argument parsing. (clap crate)

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
pub fn flags_handler() -> (String, String, u16) {
    let args = Args::parse();

    // Use the supplied document root, or create www beneath the working directory.
    let docroot = args.docroot.unwrap_or_else(|| {
        let working_directory = std::env::current_dir().unwrap_or_else(|e| {
            eprintln!("Failed to determine the working directory: {}", e);
            std::process::exit(1);
        });
        ensure_default_docroot(&working_directory)
            .unwrap_or_else(|e| {
                eprintln!("Failed to create the default document root: {}", e);
                std::process::exit(1);
            })
            .to_string_lossy()
            .into_owned()
    });
    let docroot = validate_docroot(&docroot).unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    });

    // Call the validate_ip() function
    let ip = validate_ip(&args.ip.unwrap_or_else(|| SERVER_IP.to_string())).unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    });

    // Call the validate_port() function
    let port = validate_port(args.port.unwrap_or(SERVER_PORT)).unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    });

    // On successful flag validation, print to CLI that the HTTP server is running and serving.
    println!(
        "HTTP server started: Serving {} at {} on port {}",
        docroot, ip, port
    );

    (docroot, ip, port)
}
