#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod headless;
fn main() {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.get(1).is_some_and(|a| a == "--headless") {
        if args.len() != 3 {
            eprintln!("usage: cuerithm-desktop --headless LIBRARY_DB < command.json");
            std::process::exit(1);
        }
        std::process::exit(headless::run(std::path::Path::new(&args[2])));
    }
    if std::env::args().any(|a| a == "--diagnostics") {
        if let Err(e) = cuerithm_desktop_lib::diagnostics() {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    cuerithm_desktop_lib::run();
}
