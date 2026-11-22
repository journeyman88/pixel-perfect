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
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

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

/// Renders once, then once more after each burst of changes, until a
/// [`Signal::Stop`] arrives or the channel is closed.
///
/// After a [`Signal::Changed`], the loop waits until no further change
/// arrives for `quiet`, then renders once: a burst gives a single render,
/// after its last change. Every change restarts the quiet interval but not
/// the deadline, `max_wait` after the first change of the burst, so changes
/// that never pause still give a render at least every `max_wait`. Changes
/// that arrive during a render stay queued and start the next burst. A
/// [`Signal::Stop`] while waiting returns at once, without rendering.
///
/// When the worksheet does not exist, [`Session::missing`] is called once,
/// and the next render happens when it exists again.
pub fn run(
    signals: &Receiver<Signal>,
    quiet: Duration,
    max_wait: Duration,
    session: &mut impl Session,
) {
    session.render();
    let mut reported_missing = false;
    while let Ok(first) = signals.recv() {
        if first == Signal::Stop {
            return;
        }
        let deadline = Instant::now() + max_wait;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match signals.recv_timeout(quiet.min(left)) {
                Ok(Signal::Stop) => return,
                Ok(Signal::Changed) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                // The senders are gone: render what changed, then the outer
                // `recv` ends the loop.
                Err(RecvTimeoutError::Disconnected) => break,
            }
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
    use std::sync::mpsc::{Sender, channel};

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

    /// Intervals for the tests that do not depend on timing: changes sent
    /// together are always within `SHORT`, and `LONG` is never reached.
    const SHORT: Duration = Duration::from_millis(5);
    const LONG: Duration = Duration::from_secs(10);

    // Timing tests: a 50 ms quiet interval and a 300 ms maximum wait, with
    // signals spaced so that every assertion has a wide margin.
    const QUIET: Duration = Duration::from_millis(50);
    const MAX_WAIT: Duration = Duration::from_millis(300);

    /// A session that records when each render starts, and can make the
    /// renders after the first one take `render_time`.
    struct Timed {
        start: Instant,
        renders: Vec<Duration>,
        render_time: Duration,
    }

    impl Session for Timed {
        fn exists(&self) -> bool {
            true
        }
        fn render(&mut self) {
            self.renders.push(self.start.elapsed());
            if self.renders.len() > 1 {
                std::thread::sleep(self.render_time);
            }
        }
        fn missing(&mut self) {}
    }

    /// Runs the loop with [`QUIET`] and [`MAX_WAIT`] while `send` feeds it
    /// from another thread; returns when each render started.
    fn timed(
        render_time: Duration,
        send: impl FnOnce(Sender<Signal>) + Send + 'static,
    ) -> Vec<Duration> {
        let (tx, rx) = channel();
        let mut session = Timed {
            start: Instant::now(),
            renders: Vec::new(),
            render_time,
        };
        let sender = std::thread::spawn(move || send(tx));
        run(&rx, QUIET, MAX_WAIT, &mut session);
        sender.join().unwrap();
        session.renders
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_burst_spread_over_time_gives_one_render_after_it() {
        let renders = timed(Duration::ZERO, |tx| {
            for _ in 0..5 {
                tx.send(Signal::Changed).unwrap();
                std::thread::sleep(ms(10));
            }
            std::thread::sleep(ms(250));
            tx.send(Signal::Stop).unwrap();
        });
        assert_eq!(
            renders.len(),
            2,
            "one at start, one for the burst: {renders:?}"
        );
        // The last change is at about 40 ms: the render follows it by the quiet interval.
        assert!(renders[1] >= ms(40) + QUIET, "{renders:?}");
    }

    #[test]
    fn changes_that_never_pause_still_render_within_the_maximum_wait() {
        let renders = timed(Duration::ZERO, |tx| {
            for _ in 0..50 {
                tx.send(Signal::Changed).unwrap();
                std::thread::sleep(ms(20));
            }
            std::thread::sleep(ms(250));
            tx.send(Signal::Stop).unwrap();
        });
        // About 1 s of changes 20 ms apart: renders about every 300 ms while
        // they go on, then one after the last.
        let during: Vec<_> = renders[1..].iter().filter(|&&t| t < ms(1000)).collect();
        assert!(during.len() >= 2, "renders during the stream: {renders:?}");
        for pair in renders[1..].windows(2) {
            assert!(
                pair[1] - pair[0] <= MAX_WAIT + ms(100),
                "gap too long: {renders:?}"
            );
        }
        assert!(
            *renders.last().unwrap() >= ms(1000),
            "a render after the last change: {renders:?}"
        );
    }

    #[test]
    fn stop_while_waiting_does_not_render() {
        let renders = timed(Duration::ZERO, |tx| {
            tx.send(Signal::Changed).unwrap();
            std::thread::sleep(ms(10));
            tx.send(Signal::Stop).unwrap();
        });
        assert_eq!(renders.len(), 1, "only the render at start: {renders:?}");
    }

    #[test]
    fn a_change_during_a_render_gives_one_more_render() {
        // The second render starts at about 50 ms and lasts 150 ms; the change
        // at 100 ms arrives during it.
        let renders = timed(ms(150), |tx| {
            tx.send(Signal::Changed).unwrap();
            std::thread::sleep(ms(100));
            tx.send(Signal::Changed).unwrap();
            std::thread::sleep(ms(500));
            tx.send(Signal::Stop).unwrap();
        });
        assert_eq!(
            renders.len(),
            3,
            "start, the first change, the change during it: {renders:?}"
        );
        assert!(
            renders[2] >= renders[1] + ms(150),
            "after the render in progress: {renders:?}"
        );
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
        run(&rx, SHORT, LONG, &mut session);
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
        run(&rx, SHORT, LONG, &mut session);
        assert_eq!(session.calls, ["render", "render"]);
    }

    #[test]
    fn stop_wins_over_queued_changes() {
        let (tx, rx) = channel();
        tx.send(Signal::Changed).unwrap();
        tx.send(Signal::Stop).unwrap();
        tx.send(Signal::Changed).unwrap();
        let mut session = log();
        run(&rx, SHORT, LONG, &mut session);
        assert_eq!(session.calls, ["render"]);
    }

    #[test]
    fn removal_is_reported_once_then_renders_when_back() {
        let (tx, rx) = channel();
        let session = log();
        let exists = Arc::clone(&session.exists);
        let worker = std::thread::spawn(move || {
            let mut session = session;
            run(&rx, SHORT, LONG, &mut session);
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
