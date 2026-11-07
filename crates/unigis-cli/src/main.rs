//! `ug`: the UniGIS engine command line (see the library for the commands).

use std::process::ExitCode;

fn main() -> ExitCode {
    unigis_cli::main_with(std::env::args().collect())
}
