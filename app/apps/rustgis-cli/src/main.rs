//! `ug`: the UniGIS command line (see the library for the commands).

use std::process::ExitCode;

fn main() -> ExitCode {
    rustgis_cli::main_with(std::env::args().collect())
}
