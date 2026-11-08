//! Console-subsystem twin of the desktop program (particularly for Windows).
fn main() -> std::process::ExitCode {
    rustgis_cli::main_with(std::env::args().collect())
}
