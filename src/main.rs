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
mod logging;
mod server;
mod static_files;

#[cfg(test)]
mod test;

#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::io::Result<()> {
    let (docroot, ip, port) = cli::flags_handler();
    logging::spawn();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    tokio::select! {
        result = server::tcp_listen(&docroot, &ip, port, shutdown_rx) => result,
        result = shutdown_signal(shutdown_tx) => result,
    }
}

#[cfg(unix)]
async fn shutdown_signal(shutdown_tx: tokio::sync::watch::Sender<bool>) -> std::io::Result<()> {
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }

    let _ = shutdown_tx.send(true);

    // Never resolve: the accept loop observes the flag and returns, which
    // ends the process with status 0.
    std::future::pending().await
}

#[cfg(not(unix))]
async fn shutdown_signal(_shutdown_tx: tokio::sync::watch::Sender<bool>) -> std::io::Result<()> {
    std::future::pending().await
}
