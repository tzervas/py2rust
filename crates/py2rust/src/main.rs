//! py2rust CLI — version / analyze / transpile with never-silent `.gap.json` sidecars.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use py2rust_core::{analyze_source, transpile_source, GapReport};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "py2rust",
    version,
    about = "Python → Rust transpiler (honest gap reporting)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Print version (also available as global --version).
    Version,
    /// Analyze Python for conversion gaps; write `<stem>.gap.json`.
    Analyze {
        /// Python source file.
        python_file: PathBuf,
        /// Write gap JSON here (default: alongside source as `<stem>.gap.json`).
        #[arg(long)]
        gap_out: Option<PathBuf>,
        /// Print gap JSON to stdout.
        #[arg(long)]
        json: bool,
    },
    /// Transpile Python → Rust + `.gap.json` sidecar.
    Transpile {
        /// Python source file.
        python_file: PathBuf,
        /// Output Rust file (default: `<stem>.rs` next to source).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Rust module name for header comment.
        #[arg(short, long)]
        module: Option<String>,
        /// Write gap JSON here (default: next to Rust output as `<stem>.gap.json`).
        #[arg(long)]
        gap_out: Option<PathBuf>,
        /// Print gap JSON to stdout after transpile.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Version => {
            println!("py2rust {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Commands::Analyze {
            python_file,
            gap_out,
            json,
        } => {
            let source = std::fs::read_to_string(&python_file)
                .with_context(|| format!("read {}", python_file.display()))?;
            let label = python_file.display().to_string();
            let report = analyze_source(&source, &label)?;
            write_gap_or_sidecar(&report, gap_out.as_deref(), python_file.as_path())?;
            if json {
                println!("{}", report.to_json_pretty()?);
            } else {
                print_human_summary(&report);
            }
            Ok(())
        }
        Commands::Transpile {
            python_file,
            output,
            module,
            gap_out,
            json,
        } => {
            let source = std::fs::read_to_string(&python_file)
                .with_context(|| format!("read {}", python_file.display()))?;
            let label = python_file.display().to_string();
            let mod_name = module
                .as_deref()
                .or_else(|| python_file.file_stem().and_then(|s| s.to_str()));
            let (report, rust) = transpile_source(&source, &label, mod_name)?;
            let out = output.unwrap_or_else(|| python_file.with_extension("rs"));
            if let Some(parent) = out.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("mkdir {}", parent.display()))?;
                }
            }
            std::fs::write(&out, &rust).with_context(|| format!("write {}", out.display()))?;
            write_gap_or_sidecar(&report, gap_out.as_deref(), out.as_path())?;
            eprintln!(
                "wrote {} (emitted={}, gaps={}, expressible={:.0}%)",
                out.display(),
                report.emitted_items.len(),
                report.real_gap_count(),
                report.expressible_fraction() * 100.0
            );
            if json {
                println!("{}", report.to_json_pretty()?);
            } else {
                print_human_summary(&report);
            }
            Ok(())
        }
    }
}

fn write_gap_or_sidecar(
    report: &GapReport,
    gap_out: Option<&Path>,
    default_path_for_sidecar: &Path,
) -> Result<PathBuf> {
    if let Some(custom) = gap_out {
        if let Some(parent) = custom.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("mkdir {}", parent.display()))?;
            }
        }
        report
            .write_json_file(custom)
            .with_context(|| format!("write gap report to {}", custom.display()))?;
        eprintln!("wrote {}", custom.display());
        Ok(custom.to_path_buf())
    } else {
        let written = report
            .write_sidecar(default_path_for_sidecar)
            .with_context(|| {
                format!(
                    "write gap sidecar for {}",
                    default_path_for_sidecar.display()
                )
            })?;
        eprintln!("wrote {}", written.display());
        Ok(written)
    }
}

fn print_human_summary(report: &GapReport) {
    println!(
        "source: {} | top_level={} | emitted={} | gaps={} | expressible={:.1}% | never_silent={}",
        report.source,
        report.total_top_level_items,
        report.emitted_items.len(),
        report.real_gap_count(),
        report.expressible_fraction() * 100.0,
        report.never_silent_holds()
    );
    if !report.emitted_items.is_empty() {
        println!("emitted: {}", report.emitted_items.join(", "));
    }
    for g in &report.gaps {
        println!(
            "  L{}:{} [{}] {} — {}",
            g.line,
            g.col,
            g.category,
            g.item_name.as_deref().unwrap_or("-"),
            g.reason
        );
    }
}

#[cfg(test)]
mod tests {
    use py2rust_core::transpile_source;

    #[test]
    fn lib_roundtrip_smoke() {
        let (r, rust) =
            transpile_source("def f(x: int) -> int:\n    return 1\n", "t.py", None).unwrap();
        assert_eq!(r.emitted_items.len(), 1);
        assert!(rust.contains("fn f"));
    }

    #[test]
    fn test_write_gap_or_sidecar() {
        use super::write_gap_or_sidecar;
        use py2rust_core::GapReport;
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let report = GapReport::new("test.py", 0);

        // Test with custom gap_out path
        let custom_out = temp_dir.join("py2rust_test_custom_gap_out_1.json");
        if custom_out.exists() {
            let _ = fs::remove_file(&custom_out);
        }
        let result_path =
            write_gap_or_sidecar(&report, Some(&custom_out), &temp_dir.join("fallback.py"))
                .unwrap();
        assert_eq!(result_path, custom_out);
        assert!(custom_out.exists());
        let content = fs::read_to_string(&custom_out).unwrap();
        assert!(content.contains("\"schema_version\""));
        let _ = fs::remove_file(&custom_out);

        // Test with fallback / sidecar path
        let fallback_py = temp_dir.join("py2rust_test_fallback.py");
        let expected_sidecar = temp_dir.join("py2rust_test_fallback.gap.json");
        if expected_sidecar.exists() {
            let _ = fs::remove_file(&expected_sidecar);
        }
        let result_path2 = write_gap_or_sidecar(&report, None, &fallback_py).unwrap();
        assert_eq!(result_path2, expected_sidecar);
        assert!(expected_sidecar.exists());
        let _ = fs::remove_file(&expected_sidecar);
    }
}
