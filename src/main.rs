#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![deny(clippy::correctness)]
#![deny(clippy::suspicious)]
#![deny(clippy::style)]
#![deny(clippy::complexity)]
#![deny(clippy::perf)]
#![deny(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::dbg_macro)]
#![allow(clippy::nursery)]
#![allow(clippy::cargo)]
// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

mod cli;
mod connection;
mod http;
mod server;
mod static_files;

#[cfg(test)]
mod test;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    let (docroot, ip, port) = cli::flags_handler();
    server::tcp_listen(&docroot, &ip, port).await
}
