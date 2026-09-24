mod columns;
mod remote;
#[path = "../../../src/tag_editor.rs"]
mod tag_editor;
mod tui;

pub use tui::run as run_tui;

/// Run the same web/API server as the desktop application's Server pane,
/// honoring its persisted address, authentication, TLS and codec settings.
pub fn run_server() -> Result<(), String> {
    let mut config = kog_server::config::load_config();
    config.enabled = true;
    config.validate()?;
    let settings = kog_audio::settings::AppSettings::load();
    let state = kog_server::routes::AppState::with_radio(
        config.clone(),
        env!("CARGO_PKG_VERSION"),
        kog_server::routes::AppState::stream_service(&config, settings.decoder_settings()),
        kog_server::api::Library::open(),
        kog_server::radio::Radio::from_settings(),
    );
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("starting runtime: {error}"))?
        .block_on(kog_server::routes::serve_with_shutdown(state, async {
            let _ = tokio::signal::ctrl_c().await;
        }))
}
