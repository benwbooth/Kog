fn main() {
    if let Err(error) = kog_terminal::run_tui() {
        eprintln!("kog-tui: {error}");
        std::process::exit(1);
    }
}
