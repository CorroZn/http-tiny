// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use clap::Parser;
use std::io::{self, Write};

#[path = "validate_flags.rs"]
pub(crate) mod validation;

const SERVER_IP: &str = "127.0.0.1";
const SERVER_PORT: u16 = 8080;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct Args {
    /// Document root (Default: ./www)
    #[arg(short = 'd')]
    docroot: Option<String>,

    /// IP address to bind to (Default: 127.0.0.1)
    #[arg(short = 'i')]
    ip: Option<String>,

    /// TCP port to listen on (Default: 8080)
    #[arg(short = 'p')]
    port: Option<u16>,
}

pub(crate) fn resolve_string(
    command_line: Option<String>,
    environment: Option<String>,
    default: &str,
) -> String {
    command_line
        .or(environment)
        .unwrap_or_else(|| default.to_owned())
}

pub(crate) fn resolve_port(
    command_line: Option<u16>,
    environment: Option<&str>,
) -> Result<u16, String> {
    match command_line {
        Some(port) => Ok(port),
        None => environment.map_or(Ok(SERVER_PORT), |value| {
            value
                .parse::<u16>()
                .map_err(|_| format!("Invalid TCP port in LISTEN_PORT: {value}"))
        }),
    }
}

fn read_environment(name: &str) -> Result<Option<String>, String> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("Environment variable {name} must be valid UTF-8"))
        }
    }
}

// #############################################################################################
// CLI flag handler function
// Will use custom flags if the user specified some or fall back to the default values
// #############################################################################################
pub(crate) fn flags_handler() -> (String, String, u16) {
    let Args {
        docroot: command_line_docroot,
        ip: command_line_ip,
        port: command_line_port,
    } = Args::parse();

    let docroot = match command_line_docroot {
        Some(docroot) => docroot,
        None => match read_environment("DOC_ROOT") {
            Ok(Some(docroot)) => docroot,
            Ok(None) => {
                let working_directory = std::env::current_dir().unwrap_or_else(|error| {
                    exit_with_error(&format!(
                        "Failed to determine the working directory: {error}"
                    ));
                });
                validation::ensure_default_docroot(&working_directory)
                    .unwrap_or_else(|error| {
                        exit_with_error(&format!(
                            "Failed to create the default document root: {error}"
                        ));
                    })
                    .to_string_lossy()
                    .into_owned()
            }
            Err(error) => exit_with_error(&error),
        },
    };
    let docroot = validation::validate_docroot(&docroot).unwrap_or_else(|error| {
        exit_with_error(&error);
    });

    let environment_ip = if command_line_ip.is_none() {
        read_environment("LISTEN_ADDR").unwrap_or_else(|error| exit_with_error(&error))
    } else {
        None
    };
    let ip = resolve_string(command_line_ip, environment_ip, SERVER_IP);
    let ip = validation::validate_ip(&ip).unwrap_or_else(|error| {
        exit_with_error(&error);
    });

    let environment_port = if command_line_port.is_none() {
        read_environment("LISTEN_PORT").unwrap_or_else(|error| exit_with_error(&error))
    } else {
        None
    };
    let port = resolve_port(command_line_port, environment_port.as_deref())
        .unwrap_or_else(|error| exit_with_error(&error));
    let port = validation::validate_port(port).unwrap_or_else(|error| {
        exit_with_error(&error);
    });

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
