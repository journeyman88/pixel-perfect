// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! The loop of `pixel-perfect watch`, independent of the file watcher.
//!
//! The CLI feeds [`Signal`]s into a channel: [`Signal::Changed`] when the file
//! watcher reports an event for the worksheet (see [`concerns`]), and
//! [`Signal::Stop`] on Ctrl+C. [`run`] turns them into renders, one at a
//! time, so tests can drive it with a plain channel and no real watcher.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// Something the watch loop reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// The worksheet may have changed, been replaced or been removed.
    Changed,
    /// Stop watching (Ctrl+C).
    Stop,
}

/// What the watch loop does, provided by the caller.
pub trait Session {
    /// Whether the worksheet currently exists.
    fn exists(&self) -> bool;
    /// Renders the worksheet and reports the outcome.
    fn render(&mut self);
    /// Reports that the worksheet is missing.
    fn missing(&mut self);
}

/// True if an event on `paths` concerns the worksheet called `name`. Events
/// on other files of the watched directory, such as the render's own
/// `manifest.json`, are ignored.
pub fn concerns(name: &OsStr, paths: &[PathBuf]) -> bool {
    paths.iter().any(|p| p.file_name() == Some(name))
}

/// Renders once, then once more after each batch of signals, until a
/// [`Signal::Stop`] arrives or the channel is closed.
///
/// Signals already queued when the loop wakes up, including those that
/// arrived during the previous render, are drained first, so a burst gives a
/// single render. When the worksheet does not exist, [`Session::missing`] is
/// called once, and the next render happens when it exists again.
pub fn run(signals: &Receiver<Signal>, session: &mut impl Session) {
    session.render();
    let mut reported_missing = false;
    while let Ok(first) = signals.recv() {
        let mut stop = first == Signal::Stop;
        while let Ok(next) = signals.try_recv() {
            stop |= next == Signal::Stop;
        }
        if stop {
            return;
        }
        if session.exists() {
            reported_missing = false;
            session.render();
        } else if !reported_missing {
            reported_missing = true;
            session.missing();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    /// A session whose worksheet exists while `exists` is set, logging calls.
    struct Log {
        exists: Arc<AtomicBool>,
        calls: Vec<&'static str>,
    }

    impl Session for Log {
        fn exists(&self) -> bool {
            self.exists.load(Ordering::SeqCst)
        }
        fn render(&mut self) {
            self.calls.push("render");
        }
        fn missing(&mut self) {
            self.calls.push("missing");
        }
    }

    fn log() -> Log {
        Log {
            exists: Arc::new(AtomicBool::new(true)),
            calls: Vec::new(),
        }
    }

    #[test]
    fn only_events_on_the_worksheet_concern_it() {
        let name = OsStr::new("hero.json");
        assert!(concerns(name, &[PathBuf::from("/work/hero.json")]));
        assert!(concerns(
            name,
            &[
                PathBuf::from("/work/hero.json.tmp"),
                PathBuf::from("/work/hero.json")
            ]
        ));
        for other in [
            "/work/manifest.json",
            "/work/sprites/hero.png",
            "/work/hero.json.tmp",
            "/work",
        ] {
            assert!(!concerns(name, &[Path::new(other).to_owned()]), "{other}");
        }
        assert!(!concerns(name, &[]));
    }

    #[test]
    fn renders_at_start_and_stops() {
        let (tx, rx) = channel();
        tx.send(Signal::Stop).unwrap();
        let mut session = log();
        run(&rx, &mut session);
        assert_eq!(session.calls, ["render"]);
    }

    #[test]
    fn a_burst_gives_one_render() {
        let (tx, rx) = channel();
        for _ in 0..3 {
            tx.send(Signal::Changed).unwrap();
        }
        drop(tx);
        let mut session = log();
        run(&rx, &mut session);
        assert_eq!(session.calls, ["render", "render"]);
    }

    #[test]
    fn stop_wins_over_queued_changes() {
        let (tx, rx) = channel();
        tx.send(Signal::Changed).unwrap();
        tx.send(Signal::Stop).unwrap();
        tx.send(Signal::Changed).unwrap();
        let mut session = log();
        run(&rx, &mut session);
        assert_eq!(session.calls, ["render"]);
    }

    #[test]
    fn removal_is_reported_once_then_renders_when_back() {
        let (tx, rx) = channel();
        let session = log();
        let exists = Arc::clone(&session.exists);
        let worker = std::thread::spawn(move || {
            let mut session = session;
            run(&rx, &mut session);
            session.calls
        });
        // Each signal is handled before the next one, as when they are far
        // apart in time.
        let step = |signal| {
            tx.send(signal).unwrap();
            std::thread::sleep(Duration::from_millis(50));
        };
        exists.store(false, Ordering::SeqCst);
        step(Signal::Changed); // removed: reported
        step(Signal::Changed); // still missing: not reported again
        exists.store(true, Ordering::SeqCst);
        step(Signal::Changed); // back: rendered
        step(Signal::Stop);
        assert_eq!(worker.join().unwrap(), ["render", "missing", "render"]);
    }
}
