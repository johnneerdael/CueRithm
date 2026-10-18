#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if std::env::args().any(|a| a == "--diagnostics") {
        if let Err(e) = cuerithm_desktop_lib::diagnostics() {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    cuerithm_desktop_lib::run();
}
