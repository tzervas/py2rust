//! py2rust CLI — version / analyze / transpile with never-silent `.gap.json` sidecars.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use py2rust_core::{
    analyze_source, render_priority_report, render_ranked_report, transpile_batch_with,
    transpile_source, BatchOptions, GapReport,
};
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
    /// Transpile every `.py` under a tree; write a ranked corpus report.
    ///
    /// `transpile_batch` already existed in the library but was unreachable
    /// from the CLI, so running the analysis over a real codebase meant writing
    /// a driver by hand. This exposes it, and adds the ranking that makes the
    /// output actionable rather than merely aggregated.
    Batch {
        /// Directory to walk. `__pycache__`, `.venv`, `.git` and `target` are skipped.
        root: PathBuf,
        /// Where the per-file `.rs` and `.gap.json` artifacts go.
        #[arg(long, default_value = "target/py2rust-batch")]
        out: PathBuf,
        /// Ranked Markdown report (default: `<out>/corpus-report.md`).
        #[arg(long)]
        report: Option<PathBuf>,
        /// Short high-priority worklist (default: `<out>/priority-report.md`).
        #[arg(long)]
        priority_report: Option<PathBuf>,
        /// Cap on priority items. A long priority report is just the full one.
        #[arg(long, default_value_t = 15)]
        priority_budget: usize,
        /// Skip the L3 gate (do not hand the emitted Rust to `rustc`).
        ///
        /// The gate is on by default: emitted Rust that `rustc` rejects still
        /// counts as emitted in every other figure, so a run without L3 reports
        /// coverage it cannot back. It costs one `rustc` invocation per file —
        /// skip it on very large corpora where only the gap census is wanted.
        #[arg(long)]
        no_l3: bool,
        /// Also print the report to stdout.
        #[arg(long)]
        stdout: bool,
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
            write_gap(&report, gap_out.as_deref().unwrap_or(python_file.as_path()))?;
            if json {
                println!("{}", report.to_json_pretty()?);
            } else {
                print_human_summary(&report);
            }
            Ok(())
        }
        Commands::Batch {
            root,
            out,
            report,
            priority_report,
            priority_budget,
            no_l3,
            stdout,
        } => {
            if !root.is_dir() {
                anyhow::bail!("{} is not a directory", root.display());
            }
            let opts = BatchOptions { check_l3: !no_l3 };
            let (summary, union) = transpile_batch_with(&root, &out, &opts)
                .with_context(|| format!("batch {} -> {}", root.display(), out.display()))?;
            let rendered = render_ranked_report(&root, &summary, &union);
            let report_path = report.unwrap_or_else(|| out.join("corpus-report.md"));
            if let Some(parent) = report_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("mkdir {}", parent.display()))?;
                }
            }
            std::fs::write(&report_path, &rendered)
                .with_context(|| format!("write {}", report_path.display()))?;

            // Two reports on purpose: the full one is the record to drill into,
            // the priority one answers "what do I do next" and is useless the
            // moment it stops being short.
            let prio = render_priority_report(&root, &summary, &union, priority_budget);
            let prio_path = priority_report.unwrap_or_else(|| out.join("priority-report.md"));
            std::fs::write(&prio_path, &prio)
                .with_context(|| format!("write {}", prio_path.display()))?;
            if stdout {
                print!("{rendered}");
            }
            let l3_line = if summary.l3_checked == 0 {
                format!(
                    "L3 not measured ({})",
                    summary
                        .l3_not_run_reason
                        .as_deref()
                        .unwrap_or("no reason recorded")
                )
            } else {
                format!(
                    "L3 {}/{} compile, {} of those with no todo!() body",
                    summary.l3_passed, summary.l3_checked, summary.l3_passed_without_stubs
                )
            };
            eprintln!(
                "batch: {} files, {} parsed, {} gaps, {}\n  full     -> {}\n  priority -> {}",
                summary.total_files,
                summary.ok_files,
                summary.total_gaps,
                l3_line,
                report_path.display(),
                prio_path.display()
            );
            // A corpus where nothing parsed still writes a report, and that
            // report would read as "no gaps found". Say so on stderr and exit
            // non-zero rather than letting an empty run look like a clean one.
            if summary.total_files > 0 && summary.ok_files == 0 {
                anyhow::bail!(
                    "no file parsed — the report is empty, not clean ({} discovered)",
                    summary.total_files
                );
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
            let gap_target = gap_out.as_deref().unwrap_or(out.as_path());
            write_gap(&report, gap_target)?;
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

fn write_gap(report: &GapReport, path_for_stem: &Path) -> Result<PathBuf> {
    let written = report
        .write_sidecar(path_for_stem)
        .with_context(|| format!("write gap sidecar for {}", path_for_stem.display()))?;
    eprintln!("wrote {}", written.display());
    Ok(written)
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
}
