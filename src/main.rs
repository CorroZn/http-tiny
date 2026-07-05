// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

use clap::Parser; // Used for command-line argument parsing. (clap crate)

mod client; // Import module client.rs
mod flags; // Import module flags.rs
mod mime; // Import module mime.rs
mod parse_request; // Import module parse_request.rs
mod response; // Import module response.rs
mod tcp_listen; // Import module tcp_listen.rs
mod validate_flags; // Import module validate_flags.rs
mod validate_request; // Import module validate_request.rs

// Directory that will be responsed over HTTP.
const DOCROOT: &str = "/var/www";

// Limit the maximum request size to 8 KB.
// Prevents clients from sending infinite amounts of data and stalling the responser.
const MAX_REQUEST: usize = 8192;

// IP address the HTTP server will listen at, 0.0.0.0 will listen everywhere.
const SERVER_IP: &str = "0.0.0.0";

// TCP port the HTTP server will listen at.
const SERVER_PORT: u16 = 8080;

// #############################################################################################
// HTTP server main function
// Will start the function chain for HTTP serving operation:
//
// << main.rs >>
//    │
//    ├── << flags.rs >>
//    │      └── Parse command-line arguments
//    │          └── << validate_flags.rs >>
//    │                 ├── validate_docroot()
//    │                 ├── validate_ip()
//    │                 └── validate_port()
//    │
//    └── << tcp_listen.rs >>
//           ├── Bind socket
//           └── Accept connections
//               └── << client.rs >>
//                      ├── Spawn new thread per client and handle the client connection
//                      │   └── << parse_request.rs >>
//                      │          └── Parse HTTP request
//                      │              └── << validate_request.rs >>
//                      │                     └── Validate HTTP request
//                      └── << response.rs >>
//                             └── Build and send HTTP responses
//                                 └── << mime.rs >>
//                                        └── Determine Content-Type based on file extension and append it to the response header
//
// #############################################################################################
fn main() -> std::io::Result<()> {
    // Use custom flags if the user specified some, otherwise use the defaults
    let (docroot, ip, port) = flags::flags_handler();

    // Listen for incoming TCP connections
    tcp_listen::tcp_listen(&docroot, &ip, port)?;

    Ok(())
}

// Specify the flags the user can use to run this HTTP server
// Currently available:
// -d - Document root the HTTP server will response, Default: /var/www
// -i - IP address the HTTP server will bind to, Default: 0.0.0.0
// -p - Port the HTTP server will bind to, Default: 8080
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Document root (Default: /var/www)
    #[arg(short = 'd')]
    docroot: Option<String>,

    /// IP address to bind to (Default: 0.0.0.0)
    #[arg(short = 'i')]
    ip: Option<String>,

    /// TCP port to listen on (Default: 8080)
    #[arg(short = 'p')]
    port: Option<u16>,
}
