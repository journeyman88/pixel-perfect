// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use clap::{Args, Parser, Subcommand, ValueEnum};
use notify::{RecursiveMode, Watcher};
use pixel_perfect::export::{self, OutputFormat, RenderError, RenderOptions, RenderReport};
use pixel_perfect::import::{self, ImportError, ImportOptions};
use pixel_perfect::{LoadError, ResolvedWorksheet, watch};

/// Renders lossless pixel art from declarative worksheets.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate worksheets (.json, .yaml, .yml, .toml) without rendering.
    Validate {
        /// Worksheet files to validate.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Render the exported resources of a worksheet to image files.
    Render {
        #[command(flatten)]
        args: RenderArgs,
    },
    /// Render a worksheet, then render it again every time it is saved.
    ///
    /// Takes the same options as `render`. An invalid worksheet prints its
    /// diagnostics and keeps the last good output; Ctrl+C stops watching.
    Watch {
        #[command(flatten)]
        args: RenderArgs,
    },
    /// Add a PNG to a worksheet as a new palette and sprite, losslessly.
    ///
    /// Only the new entries are inserted: comments and formatting are kept.
    /// The result is validated and rendered before the file is replaced.
    Import {
        /// Worksheet to add to (.json, .yaml, .yml, .toml); edited in place.
        worksheet: PathBuf,
        /// PNG image to import.
        png: PathBuf,
        /// Id of the new sprite (and of the new palette, unless --palette-id).
        #[arg(long)]
        id: String,
        /// Reuse this palette of the worksheet instead of creating one.
        #[arg(long, value_name = "ID", conflicts_with = "palette_id")]
        palette: Option<String>,
        /// Id of the new palette (default: the sprite id).
        #[arg(long, value_name = "ID")]
        palette_id: Option<String>,
        /// The PNG is an N x enlargement: each N x N block becomes one pixel.
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=64))]
        scale: u32,
        /// Description of the new sprite.
        #[arg(long)]
        description: Option<String>,
        /// Also add the sprite to the worksheet's `exports` list, if it has one.
        #[arg(long)]
        export: bool,
        /// Most distinct colors to accept; the importer never quantizes.
        #[arg(long, value_name = "N", default_value_t = import::DEFAULT_MAX_COLORS)]
        max_colors: usize,
        /// Print the resulting worksheet instead of writing it.
        #[arg(long)]
        dry_run: bool,
    },
}

/// The worksheet and render options, shared by `render` and `watch` so that
/// the two commands always accept the same options.
#[derive(Args)]
struct RenderArgs {
    /// Worksheet file (.json, .yaml, .yml, .toml).
    worksheet: PathBuf,
    /// Root output directory; files go to <collection>/<name>.<format>.
    #[arg(short = 'o', long, default_value = ".")]
    output_path: PathBuf,
    /// Output image format.
    #[arg(long, value_enum, default_value_t = Format::Png)]
    format: Format,
    /// Integer upscale factor (each pixel becomes an N x N block).
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=64))]
    scale: u32,
    /// Name files by the BLAKE2b-64 hash of their id (any id works).
    #[arg(long)]
    safe_build: bool,
    /// Write animations as one still image per frame, under <collection>/<name>/.
    #[arg(long)]
    single_frames: bool,
    /// Largest image the engine may build, as RGBA8 bytes: a number with an
    /// optional KiB/MiB/GiB/TiB suffix, or "none". A guard against
    /// mistakes, not a memory budget.
    #[arg(long, value_name = "SIZE", default_value = "1GiB", value_parser = parse_max_image_size)]
    max_image_size: MaxImageSize,
    /// Worker threads for rendering and encoding (default: the number of
    /// physical CPU cores, or RAYON_NUM_THREADS if set). Output does not
    /// depend on it.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u16).range(1..))]
    threads: Option<u16>,
}

impl RenderArgs {
    fn options(&self) -> RenderOptions {
        RenderOptions {
            output_path: self.output_path.clone(),
            format: match self.format {
                Format::Png => OutputFormat::Png,
                Format::Webp => OutputFormat::Webp,
            },
            scale: self.scale,
            safe_build: self.safe_build,
            single_frames: self.single_frames,
            max_image_size: self.max_image_size.0,
        }
    }
}

/// Parsed `--max-image-size`: `None` means no limit. A newtype, because clap
/// would treat a bare `Option` as an optional argument.
#[derive(Clone, Copy)]
struct MaxImageSize(Option<u64>);

fn parse_max_image_size(text: &str) -> Result<MaxImageSize, String> {
    export::parse_size(text).map(MaxImageSize)
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Png,
    Webp,
}

/// Success.
const OK: u8 = 0;
/// At least one worksheet or the export request is invalid.
const INVALID: u8 = 1;
/// Usage error, unreadable or unsupported file, or write failure.
const USAGE: u8 = 2;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(if e.use_stderr() { USAGE } else { OK });
        }
    };
    let threads = match &cli.command {
        Command::Render { args } | Command::Watch { args } => args.threads.map(usize::from),
        _ => None,
    };
    if let Err(e) = configure_threads(threads) {
        eprintln!("cannot start the worker threads: {e}");
        return ExitCode::from(USAGE);
    }
    let code = match cli.command {
        Command::Validate { files } => validate(&files),
        Command::Render { args } => render(&args.worksheet, &args.options()),
        Command::Watch { args } => watch(&args.worksheet, &args.options()),
        Command::Import {
            worksheet,
            png,
            id,
            palette,
            palette_id,
            scale,
            description,
            export,
            max_colors,
            dry_run,
        } => import(&ImportOptions {
            worksheet,
            png,
            id,
            palette,
            palette_id,
            scale,
            description,
            export,
            max_colors,
            dry_run,
        }),
    };
    ExitCode::from(code)
}

/// Loads a worksheet, printing its problems. Returns the exit code on failure.
/// Worker threads to use when neither `--threads` nor `RAYON_NUM_THREADS`
/// says: the physical CPU cores, at most the CPUs this process may use.
/// Measured on an 8-core, 16-thread machine, using every logical thread gave
/// the same wall time for PNG output as one thread per core, with 20-35% more
/// CPU time and more memory: two threads sharing a core mostly compete.
fn default_threads() -> usize {
    let usable = std::thread::available_parallelism().map_or(1, |n| n.get());
    num_cpus::get_physical().clamp(1, usable)
}

/// Sizes the global rayon pool: `requested` (`--threads`) first, then
/// `RAYON_NUM_THREADS`, then [`default_threads`].
fn configure_threads(requested: Option<usize>) -> Result<(), rayon::ThreadPoolBuildError> {
    let from_env = std::env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0);
    let threads = requested.or(from_env).unwrap_or_else(default_threads);
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
}

fn load(path: &Path) -> Result<ResolvedWorksheet, u8> {
    pixel_perfect::load_file(path).map_err(|e| match e {
        LoadError::Invalid { diagnostics } => {
            for d in &diagnostics {
                eprintln!("{}: {d}", path.display());
            }
            INVALID
        }
        e => {
            eprintln!("{e}");
            USAGE
        }
    })
}

fn validate(files: &[PathBuf]) -> u8 {
    files
        .iter()
        .map(|path| load(path).err().unwrap_or(OK))
        .max()
        .unwrap_or(OK)
}

/// The outcome of one render, once its messages have been printed.
enum Rendered {
    Done(RenderReport),
    /// `code` is the exit code `render` would return; `problems` counts the
    /// diagnostics printed (1 for an error that is not a diagnostic).
    Failed {
        code: u8,
        problems: usize,
    },
}

/// Loads and renders `path`, printing notices and diagnostics as `render`
/// does. Shared by `render` and `watch`.
fn render_once(path: &Path, options: &RenderOptions) -> Rendered {
    let ws = match pixel_perfect::load_file(path) {
        Ok(ws) => ws,
        Err(LoadError::Invalid { diagnostics }) => {
            for d in &diagnostics {
                eprintln!("{}: {d}", path.display());
            }
            return Rendered::Failed {
                code: INVALID,
                problems: diagnostics.len(),
            };
        }
        Err(e) => {
            eprintln!("{e}");
            return Rendered::Failed {
                code: USAGE,
                problems: 1,
            };
        }
    };
    match export::render_report(&ws, options) {
        Ok(report) => {
            for notice in &report.plan.notices {
                eprintln!("{}: note: {notice}", path.display());
            }
            Rendered::Done(report)
        }
        Err(RenderError::Plan(diagnostics)) => {
            for d in &diagnostics {
                eprintln!("{}: {d}", path.display());
            }
            Rendered::Failed {
                code: INVALID,
                problems: diagnostics.len(),
            }
        }
        Err(e) => {
            eprintln!("{e}");
            Rendered::Failed {
                code: USAGE,
                problems: 1,
            }
        }
    }
}

fn render(path: &Path, options: &RenderOptions) -> u8 {
    match render_once(path, options) {
        Rendered::Done(_) => OK,
        Rendered::Failed { code, .. } => code,
    }
}

/// How long `watch` waits after a change for the next one, before rendering:
/// a burst of changes gives a single render, after its last change.
const QUIET: Duration = Duration::from_millis(150);

/// The longest `watch` waits, from the first change of a burst, when the
/// changes never pause: a file written without pause still renders this often.
const MAX_WAIT: Duration = Duration::from_secs(2);

/// The `watch` session: renders `path` and prints a summary line per render.
struct WatchSession<'a> {
    path: &'a Path,
    options: &'a RenderOptions,
    renders: usize,
}

impl watch::Session for WatchSession<'_> {
    fn exists(&self) -> bool {
        self.path.is_file()
    }

    fn render(&mut self) {
        self.renders += 1;
        let n = self.renders;
        let start = Instant::now();
        match render_once(self.path, self.options) {
            Rendered::Done(report) => eprintln!(
                "[{n}] rendered: {} written, {} unchanged, {} ms",
                report.written,
                report.unchanged,
                start.elapsed().as_millis()
            ),
            Rendered::Failed { problems, .. } => eprintln!(
                "[{n}] failed: {problems} {}",
                if problems == 1 {
                    "diagnostic"
                } else {
                    "diagnostics"
                }
            ),
        }
    }

    fn missing(&mut self) {
        eprintln!(
            "{}: missing, waiting for it to come back",
            self.path.display()
        );
    }
}

/// Renders `path`, then again after every change, until Ctrl+C. Returns 2 if
/// the worksheet cannot be used or watched at start, 0 when interrupted.
fn watch(path: &Path, options: &RenderOptions) -> u8 {
    // A missing file or an unsupported extension ends the command at start;
    // an invalid worksheet does not: `watch` waits for a fix.
    if let Err(e @ (LoadError::Io { .. } | LoadError::UnsupportedExtension { .. })) =
        pixel_perfect::load_file(path)
    {
        eprintln!("{e}");
        return USAGE;
    }
    let Some(name) = path.file_name().map(|n| n.to_owned()) else {
        eprintln!("{}: not a file", path.display());
        return USAGE;
    };
    // Watch the directory, not the file: editors that save by renaming a
    // temporary file over the worksheet would otherwise detach the watch.
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_owned(),
        _ => PathBuf::from("."),
    };
    let (tx, rx) = mpsc::channel();
    let events = tx.clone();
    // Raw events: the watch loop does the debouncing (see `watch::run`).
    let watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        match result {
            // Reading the worksheet, as every render does, gives access
            // events: ignoring them keeps a render from triggering the next.
            Ok(event) if !event.kind.is_access() && watch::concerns(&name, &event.paths) => {
                let _ = events.send(watch::Signal::Changed);
            }
            Ok(_) => {}
            Err(e) => eprintln!("watch: {e}"),
        }
    });
    let mut watcher = match watcher {
        Ok(w) => w,
        Err(e) => {
            eprintln!("watch: cannot start the file watcher: {e}");
            return USAGE;
        }
    };
    if let Err(e) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
        eprintln!("{}: cannot watch: {e}", dir.display());
        return USAGE;
    }
    // First Ctrl+C: stop after the render in progress. Second: exit now.
    let interrupted = AtomicBool::new(false);
    if let Err(e) = ctrlc::set_handler(move || {
        if interrupted.swap(true, Ordering::SeqCst) {
            std::process::exit(130);
        }
        let _ = tx.send(watch::Signal::Stop);
    }) {
        eprintln!("watch: cannot handle Ctrl+C: {e}");
        return USAGE;
    }
    watch::run(
        &rx,
        QUIET,
        MAX_WAIT,
        &mut WatchSession {
            path,
            options,
            renders: 0,
        },
    );
    OK
}

fn import(options: &ImportOptions) -> u8 {
    match import::run(options) {
        Ok(imported) => {
            if options.dry_run {
                print!("{}", imported.text);
                eprintln!("{}", imported.summary);
            } else {
                println!("{}", imported.summary);
            }
            OK
        }
        Err(ImportError::InvalidWorksheet { diagnostics }) => {
            for d in &diagnostics {
                eprintln!("{}: {d}", options.worksheet.display());
            }
            INVALID
        }
        Err(ImportError::Rejected(message)) => {
            eprintln!("{message}");
            INVALID
        }
        Err(ImportError::Usage(message)) => {
            eprintln!("{message}");
            USAGE
        }
    }
}
