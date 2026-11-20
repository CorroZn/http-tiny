// Copyright (C) 2026 CorroZn <corrozn@proton.me>.
// SPDX-License-Identifier: GPL-2.0-only

//! Best-effort diagnostics that cannot stall request handling.
//!
//! Log lines are handed to a bounded channel, and a dedicated task owns stderr
//! and drains it. A slow stderr (a pipe with no reader, a stalled log
//! collector) then blocks only that task. Writing directly from a request
//! handler would hold the process-wide stderr lock across a blocking `write`;
//! the stalled handler keeps its client permit, and once enough permits are
//! stuck the accept loop can no longer admit a connection and the server wedges.

use std::io::{self, Write};
use std::sync::OnceLock;

use tokio::sync::mpsc;

/// Queued lines. Logging is throttled to roughly one line per 1024 events, so a
/// small queue is ample; a full queue drops new lines rather than growing
/// without bound.
const CHANNEL_CAPACITY: usize = 256;

static SENDER: OnceLock<mpsc::Sender<String>> = OnceLock::new();

/// Start the task that owns stderr. Must run inside a Tokio runtime.
pub(crate) fn spawn() {
    let (sender, receiver) = mpsc::channel(CHANNEL_CAPACITY);
    if SENDER.set(sender).is_ok() {
        tokio::spawn(drain(receiver));
    }
}

/// Queue one diagnostic line. Never blocks and never fails the caller: a full
/// queue, or a logging task that is gone, drops the line.
pub(crate) fn log(message: String) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.try_send(message);
    }
}

async fn drain(mut receiver: mpsc::Receiver<String>) {
    while let Some(message) = receiver.recv().await {
        // The lock is taken per line so no lock guard is held across an await.
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(message.as_bytes());
        let _ = stderr.flush();
    }
}
