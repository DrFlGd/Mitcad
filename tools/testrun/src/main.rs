// SPDX-License-Identifier: MIT
//! `mitcad-testrun`: the test runner over ctest (README.md).

use std::process::ExitCode;

use mitcad_testrun::concurrency;
use mitcad_testrun::options::{self, USAGE};
use mitcad_testrun::runner::Runner;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let options = match options::parse(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("mitcad-testrun: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    // The learnt UI entries at once, or check-all's UI runs to learn from.
    if options.ui_cap || options.record_ui.is_some() {
        return match concurrency::command(&options) {
            Ok(text) => {
                println!("{text}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("mitcad-testrun: {message}");
                ExitCode::from(2)
            }
        };
    }
    let mut runner = match Runner::prepare(&options) {
        Ok(runner) => runner,
        Err(message) => {
            eprintln!("mitcad-testrun: {message}");
            return ExitCode::from(2);
        }
    };
    if options.dry_run {
        print!("{}", runner.dry_run());
        return ExitCode::SUCCESS;
    }
    let status = runner.run();
    ExitCode::from(u8::try_from(status).unwrap_or(1))
}
