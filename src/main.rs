// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use pixel_perfect::export::{self, OutputFormat, RenderError, RenderOptions};
use pixel_perfect::import::{self, ImportError, ImportOptions};
use pixel_perfect::{LoadError, ResolvedWorksheet};

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
        /// Largest image the engine may build, as RGBA8 bytes: a number with an
        /// optional KiB/MiB/GiB/TiB suffix, or "none". A guard against
        /// mistakes, not a memory budget.
        #[arg(long, value_name = "SIZE", default_value = "1GiB", value_parser = parse_max_image_size)]
        max_image_size: MaxImageSize,
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
    let code = match cli.command {
        Command::Validate { files } => validate(&files),
        Command::Render {
            worksheet,
            output_path,
            format,
            scale,
            safe_build,
            max_image_size,
        } => {
            let options = RenderOptions {
                output_path,
                format: match format {
                    Format::Png => OutputFormat::Png,
                    Format::Webp => OutputFormat::Webp,
                },
                scale,
                safe_build,
                max_image_size: max_image_size.0,
            };
            render(&worksheet, &options)
        }
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

fn render(path: &Path, options: &RenderOptions) -> u8 {
    let ws = match load(path) {
        Ok(ws) => ws,
        Err(code) => return code,
    };
    match export::render(&ws, options) {
        Ok((plan, _)) => {
            for notice in &plan.notices {
                eprintln!("{}: note: {notice}", path.display());
            }
            OK
        }
        Err(RenderError::Plan(diagnostics)) => {
            for d in &diagnostics {
                eprintln!("{}: {d}", path.display());
            }
            INVALID
        }
        Err(e) => {
            eprintln!("{e}");
            USAGE
        }
    }
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
