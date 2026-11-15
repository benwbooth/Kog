fn main() {
    // The library reader's helper process (see kog_server::discover::helper).
    kog_server::discover::helper::run_if_requested();
    if let Err(error) = kog_terminal::run_tui() {
        eprintln!("kog-tui: {error}");
        std::process::exit(1);
    }
}
