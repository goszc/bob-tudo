#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--elevated-task") {
        let path = std::env::args().nth(2).unwrap_or_default();
        bob_tudo::run_elevated(&path);
        return;
    }
    bob_tudo::run();
}
