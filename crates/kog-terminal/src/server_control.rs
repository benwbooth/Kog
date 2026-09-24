//! Embedded API server lifetime for the terminal frontend.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use kog_audio::settings::AppSettings;
use kog_server::TlsMode;
use kog_server::api::Library;
use kog_server::config::ServerConfig;
use kog_server::radio::Radio;
use kog_server::routes::AppState;

pub struct RunningServer {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    finished: Receiver<Result<(), String>>,
    pub url: String,
    pub library: Arc<Library>,
}

impl RunningServer {
    pub fn start(config: ServerConfig) -> Result<Self, String> {
        config.validate()?;
        if !config.enabled {
            return Err("Enable the API server first".to_owned());
        }
        let address = config.socket_address();
        if config.tls.mode != TlsMode::Off {
            kog_server::tls::server_config(&config.tls, address)?;
        }
        // Match the desktop's early bind check so a busy port is reported in
        // the menu instead of claiming a server that failed on its worker.
        std::net::TcpListener::bind(address)
            .map_err(|error| format!("could not bind {address}: {error}"))?;
        let settings = AppSettings::load();
        let state = AppState::with_radio(
            config.clone(),
            env!("CARGO_PKG_VERSION"),
            AppState::stream_service(&config, settings.decoder_settings()),
            Library::open(),
            Radio::from_settings(),
        );
        let library = state.library.clone();
        let scheme = if config.tls.mode == TlsMode::Off {
            "http"
        } else {
            "https"
        };
        let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
        let (finished_tx, finished) = mpsc::channel();
        std::thread::Builder::new()
            .name("kog-tui-api-server".to_owned())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| format!("starting server runtime: {error}"))
                    .and_then(|runtime| {
                        runtime.block_on(kog_server::routes::serve_with_shutdown(
                            state,
                            async move {
                                let _ = shutdown_rx.await;
                            },
                        ))
                    });
                let _ = finished_tx.send(result);
            })
            .map_err(|error| format!("starting server thread: {error}"))?;
        Ok(Self {
            shutdown: Some(shutdown),
            finished,
            url: format!("{scheme}://{address}"),
            library,
        })
    }

    pub fn finished(&self) -> Option<Result<(), String>> {
        match self.finished.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Disconnected) => {
                Some(Err("the server thread exited unexpectedly".to_owned()))
            }
            Err(TryRecvError::Empty) => None,
        }
    }

    pub fn stop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.stop();
    }
}
