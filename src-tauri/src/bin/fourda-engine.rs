// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! `fourda-engine` — headless 4DA refresh engine.
//!
//! Runs the same fetch+score pipeline as the desktop app, but with no window, so the SQLite
//! database stays fresh for the MCP server even when the GUI isn't open. This is a console
//! binary (no `windows_subsystem = "windows"`) so a Task Scheduler / cron job — or an external
//! verifier re-running it as a proof — sees its stdout and exit code.
//!
//! Usage:
//!   fourda-engine            run one refresh cycle if data is stale, then exit (default)
//!   fourda-engine --once     same as above (explicit)
//!   fourda-engine --daemon   run continuously on the monitoring interval until Ctrl-C
//!   fourda-engine --force    refresh even if data is already fresh (combine with --once/--daemon)
//!   fourda-engine --help     print usage and exit 0
//!
//! Without `--force`, a cycle is skipped when the data is already fresh (last fetch within the
//! refresh interval), so running alongside an active GUI does not double-fetch rate-limited sources.
//!
//! Exit codes (--once): 0 = success or skipped-fresh, 1 = scoring failed, 2 = app build failed,
//! 64 = bad usage.

use fourda_lib::{run_headless, HeadlessMode};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let force = args.iter().any(|a| a == "--force" || a == "-f");
    // The mode is the first argument that isn't the force flag.
    let mode_arg = args.iter().find(|a| *a != "--force" && *a != "-f");
    let mode = match mode_arg.map(String::as_str) {
        None | Some("--once") | Some("once") => HeadlessMode::Once,
        Some("--daemon") | Some("daemon") => HeadlessMode::Daemon,
        Some("--help") | Some("-h") => {
            print_usage();
            std::process::exit(0);
        }
        Some(other) => {
            eprintln!("fourda-engine: unknown argument '{other}'\n");
            print_usage();
            std::process::exit(64); // EX_USAGE
        }
    };
    run_headless(mode, force); // diverges — terminates the process with an explicit exit code
}

fn print_usage() {
    eprintln!(
        "fourda-engine — headless 4DA refresh engine\n\n\
         USAGE:\n\
         \x20 fourda-engine [--once | --daemon] [--force]\n\n\
         MODES:\n\
         \x20 --once     run one fetch+score cycle if data is stale, then exit (default)\n\
         \x20 --daemon   run continuously on the monitoring interval until Ctrl-C\n\n\
         FLAGS:\n\
         \x20 --force    refresh even if data is already fresh\n\n\
         EXIT CODES (--once):\n\
         \x20 0 success/skipped-fresh   1 scoring failed   2 app build failed   64 bad usage"
    );
}
