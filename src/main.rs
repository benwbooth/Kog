mod app_controller;
mod desktop_integration;
mod file_tree_model;
mod rom_import;
mod skin_library;
mod tag_editor;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};
use std::io::IsTerminal;

fn configure_platform_theme() {
    #[cfg(target_os = "linux")]
    {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let is_plasma = desktop
            .split(':')
            .any(|name| name.eq_ignore_ascii_case("KDE") || name.eq_ignore_ascii_case("Plasma"));
        if !is_plasma {
            return;
        }

        // Qt reads both values while constructing QGuiApplication. This is the
        // same integration a packaged Plasma application receives from its
        // launcher, while preserving an explicit user override.
        if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
            // SAFETY: this runs before Qt, Rodio, or any other worker threads
            // are created, so no concurrent environment access is possible.
            unsafe { std::env::set_var("QT_QPA_PLATFORMTHEME", "kde") };
        }
        if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
            // SAFETY: see the pre-thread initialization guarantee above.
            unsafe { std::env::set_var("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop") };
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Gui,
    Tui,
    Server,
}

fn select_mode(
    args: impl IntoIterator<Item = String>,
    terminal: bool,
) -> Result<Option<Mode>, String> {
    let mut chosen = None;
    for arg in args {
        let mode = match arg.as_str() {
            "--gui" => Mode::Gui,
            "--tui" => Mode::Tui,
            "--server" | "--headless" => Mode::Server,
            "--help" | "-h" => {
                println!(
                    "Kog {}\n\nUsage: kog [--gui | --tui | --server]\n\n  --gui       Open the Qt desktop app\n  --tui       Open the terminal player\n  --server    Serve the web app without a UI\n  --help      Show this help\n  --version   Show the version",
                    env!("CARGO_PKG_VERSION")
                );
                return Ok(None);
            }
            "--version" | "-V" => {
                println!("kog {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            _ => return Err(format!("unknown option: {arg} (try --help)")),
        };
        if chosen.replace(mode).is_some() {
            return Err("choose only one of --gui, --tui, or --server".to_owned());
        }
    }
    Ok(Some(chosen.unwrap_or(if terminal {
        Mode::Tui
    } else {
        Mode::Gui
    })))
}

fn main() {
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let mode = match select_mode(std::env::args().skip(1), terminal) {
        Ok(Some(mode)) => mode,
        Ok(None) => return,
        Err(error) => {
            eprintln!("kog: {error}");
            std::process::exit(2);
        }
    };
    match mode {
        Mode::Tui => {
            if !terminal {
                eprintln!("kog: --tui requires a terminal on stdin and stdout");
                std::process::exit(2);
            }
            if let Err(error) = kog_terminal::run_tui() {
                eprintln!("kog: {error}");
                std::process::exit(1);
            }
            return;
        }
        Mode::Server => {
            if let Err(error) = kog_terminal::run_server() {
                eprintln!("kog-server: {error}");
                std::process::exit(1);
            }
            return;
        }
        Mode::Gui => {}
    }
    configure_platform_theme();
    let mut application = desktop_integration::DesktopApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    QGuiApplication::set_desktop_file_name(&QString::from("org.kog.player"));
    application.set_application_name(&QString::from("Kog"));

    if let Some(engine) = engine.as_mut() {
        // Development shortcut: with KOG_QML_DIR pointing at the source tree,
        // load the QML from disk so editing it needs no rebuild at all. Debug
        // builds only, so a packaged binary cannot be redirected. Components
        // in the same directory and the org.kog.player types (registered by the
        // generated module, which is still embedded) both resolve as usual.
        let from_disk = std::env::var_os("KOG_QML_DIR")
            .filter(|_| cfg!(debug_assertions))
            .map(std::path::PathBuf::from);
        match from_disk {
            Some(directory) => {
                let main = directory.join("Main.qml");
                engine.load(&QUrl::from(&format!("file://{}", main.display())));
            }
            None => engine.load(&QUrl::from("qrc:/qt/qml/org/kog/player/qml/Main.qml")),
        }
    }
    desktop_integration::apply_application_icon();
    desktop_integration::restore_main_window();

    application.exec();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_terminal_only_for_interactive_launches() {
        assert_eq!(select_mode(Vec::new(), true).unwrap(), Some(Mode::Tui));
        assert_eq!(select_mode(Vec::new(), false).unwrap(), Some(Mode::Gui));
        assert_eq!(
            select_mode(["--gui".to_owned()], true).unwrap(),
            Some(Mode::Gui)
        );
        assert_eq!(
            select_mode(["--server".to_owned()], false).unwrap(),
            Some(Mode::Server)
        );
        assert!(select_mode(["--tui".to_owned(), "--gui".to_owned()], true).is_err());
    }
}
