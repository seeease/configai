pub mod api;
pub mod core;
pub mod error;
pub mod models;
pub mod storage;
pub mod tui;

fn main() {
    tracing_subscriber::fmt::init();

    let data_path = std::path::PathBuf::from("data.json");
    let mut app = match tui::App::new(&data_path) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Failed to initialize: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = app.run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
