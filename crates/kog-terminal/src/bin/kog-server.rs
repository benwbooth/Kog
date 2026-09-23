fn main() {
    if let Err(error) = kog_terminal::run_server() {
        eprintln!("kog-server: {error}");
        std::process::exit(1);
    }
}
