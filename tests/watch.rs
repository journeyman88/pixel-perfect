// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! `pixel-perfect watch` end to end: the real binary, a real file watcher and
//! a worksheet edited on disk. Every wait polls for up to 10 s, so a slow
//! machine makes these tests slower, not flaky.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

const TIMEOUT: Duration = Duration::from_secs(10);

/// A running `watch` on a copy of `tests/fixtures/valid/watch.json`, with its
/// standard error captured in a file. Killed when dropped.
struct Watch {
    child: Child,
    dir: tempfile::TempDir,
}

impl Watch {
    fn start() -> Watch {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(
            common::fixture("valid/watch.json"),
            dir.path().join("watch.json"),
        )
        .unwrap();
        let log = std::fs::File::create(dir.path().join("watch.log")).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_pixel-perfect"))
            .args(["watch", "watch.json", "-o", "out"])
            .current_dir(dir.path())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .unwrap();
        let watch = Watch { child, dir };
        watch.wait_for_renders(1);
        watch
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path("watch.log")).unwrap_or_default()
    }

    /// Render attempts reported so far, successful or not.
    fn renders(&self) -> usize {
        self.log()
            .lines()
            .filter(|l| l.contains("] rendered:") || l.contains("] failed:"))
            .count()
    }

    fn wait_for_renders(&self, n: usize) {
        wait_until(|| self.renders() >= n, || self.log());
    }

    /// Writes the worksheet in place, as most editors do.
    fn save(&self, text: &str) {
        std::fs::write(self.path("watch.json"), text).unwrap();
    }

    fn running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_until(mut done: impl FnMut() -> bool, context: impl Fn() -> String) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < TIMEOUT, "timed out; log:\n{}", context());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The fixture with sprite `hero` drawn as `lines`.
fn worksheet(hero: [&str; 2]) -> String {
    let text = std::fs::read_to_string(common::fixture("valid/watch.json")).unwrap();
    text.replace(
        r#"["rg", "gr"]"#,
        &format!(r#"["{}", "{}"]"#, hero[0], hero[1]),
    )
}

fn mtime(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

/// Sets the modification time of `path` an hour in the past, so a rewrite is
/// visible whatever the file system's time resolution.
fn age(path: &Path) -> SystemTime {
    let past = SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(past)
        .unwrap();
    mtime(path)
}

#[test]
fn edit_triggers_a_render() {
    let watch = Watch::start();
    let hero = watch.path("out/sprites/hero.png");
    let before = std::fs::read(&hero).unwrap();
    let coin_time = age(&watch.path("out/sprites/coin.png"));
    watch.save(&worksheet(["gg", "rr"]));
    wait_until(|| std::fs::read(&hero).unwrap() != before, || watch.log());
    watch.wait_for_renders(2);
    assert_eq!(mtime(&watch.path("out/sprites/coin.png")), coin_time);
    assert!(
        watch.log().contains("[2] rendered: 1 written, 2 unchanged"),
        "{}",
        watch.log()
    );
}

#[test]
fn save_by_rename_is_detected_every_time() {
    let watch = Watch::start();
    let hero = watch.path("out/sprites/hero.png");
    for (n, lines) in [(2, ["gg", "rr"]), (3, ["rr", "rr"])] {
        let before = std::fs::read(&hero).unwrap();
        let tmp = watch.path("watch.json.tmp");
        std::fs::write(&tmp, worksheet(lines)).unwrap();
        std::fs::rename(&tmp, watch.path("watch.json")).unwrap();
        wait_until(|| std::fs::read(&hero).unwrap() != before, || watch.log());
        watch.wait_for_renders(n);
    }
}

#[test]
fn error_keeps_the_last_output_until_fixed() {
    let mut watch = Watch::start();
    let hero = watch.path("out/sprites/hero.png");
    let before = std::fs::read(&hero).unwrap();
    watch.save("{\n  \"palettes\": [\n");
    watch.wait_for_renders(2);
    let log = watch.log();
    assert!(log.contains("watch.json: 3:"), "line and column: {log}");
    assert!(log.contains("[2] failed: 1 diagnostic"), "{log}");
    assert_eq!(std::fs::read(&hero).unwrap(), before);
    assert!(watch.running());
    watch.save(&worksheet(["gg", "rr"]));
    wait_until(|| std::fs::read(&hero).unwrap() != before, || watch.log());
    watch.wait_for_renders(3);
    assert!(watch.log().contains("[3] rendered:"), "{}", watch.log());
}

#[test]
fn idle_watch_does_not_render_again() {
    // Each render reads the worksheet; the access events this causes must
    // not trigger another render.
    let watch = Watch::start();
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(watch.renders(), 1, "{}", watch.log());
}

#[test]
fn missing_worksheet_at_start_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_pixel-perfect"))
        .args(["watch", "missing.json"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("missing.json: cannot read file"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn interrupt_ends_watch_with_code_0() {
    let mut watch = Watch::start();
    let status = Command::new("kill")
        .args(["-INT", &watch.child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let start = Instant::now();
    let code = loop {
        if let Some(status) = watch.child.try_wait().unwrap() {
            break status.code();
        }
        assert!(start.elapsed() < TIMEOUT, "still running");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(code, Some(0), "{}", watch.log());
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(watch.path("out/manifest.json")).unwrap())
            .unwrap();
    for entry in manifest["files"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        assert!(watch.path("out").join(path).is_file(), "{path}");
    }
}
