mod app_controller;
mod desktop_art;
mod desktop_integration;
mod file_tree_model;
mod mml_view;
mod rom_import;
mod skin_library;
mod tag_editor;
mod metadata_cache;
mod workspace_tracks;

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

/// Text drawn the way egui draws the Earshot-look demo: each glyph rasterised
/// at the screen's real pixel size with grey-scale edges. Qt Quick otherwise
/// scales distance-field glyphs with coloured sub-pixel fringes, which look
/// soft and rough on dark backgrounds at fractional display scales. Explicit
/// settings in the environment win.
fn configure_text_rendering() {
    for (name, value) in [
        ("QML_DISABLE_DISTANCEFIELD", "1"),
        ("QSG_DISTANCEFIELD_ANTIALIASING", "gray"),
    ] {
        if std::env::var_os(name).is_none() {
            // SAFETY: this runs before Qt, Rodio, or any other worker threads
            // are created, so no concurrent environment access is possible.
            unsafe { std::env::set_var(name, value) };
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
    tui_supported: bool,
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
    if chosen == Some(Mode::Tui) && !tui_supported {
        return Err("--tui is currently supported on Unix terminals only".to_owned());
    }
    Ok(Some(chosen.unwrap_or(if terminal && tui_supported {
        Mode::Tui
    } else {
        Mode::Gui
    })))
}

fn main() {
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let tui_supported = cfg!(unix);
    let mode = match select_mode(std::env::args().skip(1), terminal, tui_supported) {
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
    configure_text_rendering();
    let mut application = desktop_integration::DesktopApplication::new();

    QGuiApplication::set_desktop_file_name(&QString::from("org.kog.player"));
    application.set_application_name(&QString::from("Kog"));
    match desktop_integration::claim_gui_instance() {
        Ok(desktop_integration::GuiInstance::Primary) => {}
        Ok(desktop_integration::GuiInstance::Raised) => return,
        Err(error) => {
            eprintln!("kog: {error}");
            std::process::exit(1);
        }
    }

    let mut engine = QQmlApplicationEngine::new();

    if let Some(mut engine) = engine.as_mut() {
        // Development shortcut: with KOG_QML_DIR pointing at the source tree,
        // load the QML from disk so editing it needs no rebuild at all. Only
        // in builds made with KOG_QML_DIR set (devloop's; see build.rs), so a
        // packaged binary cannot be redirected. The dev profile has no debug
        // assertions, so they can't tell.
        let from_disk = std::env::var_os("KOG_QML_DIR")
            .filter(|_| cfg!(kog_dev_qml))
            .map(std::path::PathBuf::from);
        match from_disk {
            Some(directory) => {
                // `import org.kog.player` would otherwise give every other
                // component the embedded copy: an import path of our own
                // maps the module's files to the directory's.
                if let Some(imports) = dev_qml_imports(&directory) {
                    let imports = imports.to_string_lossy().into_owned();
                    engine.as_mut().add_import_path(&QString::from(imports.as_str()));
                }
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

/// An import path whose org.kog.player module is the embedded one with each
/// QML file taken from `directory` when it has it (test directories hold only
/// a few). The module's Rust types still come from its plugin.
#[cfg(kog_dev_qml)]
fn dev_qml_imports(directory: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::hash::{Hash, Hasher};
    let directory = directory.canonicalize().ok()?;
    let mut qmldir = String::from(
        "module org.kog.player\noptional plugin org_kog_player\nclassname org_kog_player_plugin\n",
    );
    for file in env!("KOG_QML_MODULE_FILES").split(':') {
        let file = std::path::Path::new(file);
        let (Some(name), Some(stem)) = (file.file_name(), file.file_stem()) else {
            continue;
        };
        let on_disk = directory.join(name);
        let url = if on_disk.is_file() {
            format!("file://{}", on_disk.display())
        } else {
            format!("qrc:/qt/qml/org/kog/player/{}", file.display())
        };
        qmldir.push_str(&format!("{} 1.0 {url}\n", stem.to_string_lossy()));
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    directory.hash(&mut hasher);
    let root = std::env::temp_dir().join(format!("kog-dev-qml-{:016x}", hasher.finish()));
    let module = root.join("org/kog/player");
    std::fs::create_dir_all(&module).ok()?;
    // Written whole, then renamed: another instance may be reading it.
    let partial = module.join(format!("qmldir.{}", std::process::id()));
    std::fs::write(&partial, qmldir).ok()?;
    std::fs::rename(&partial, module.join("qmldir")).ok()?;
    Some(root)
}

#[cfg(not(kog_dev_qml))]
fn dev_qml_imports(_directory: &std::path::Path) -> Option<std::path::PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_terminal_only_for_interactive_launches() {
        assert_eq!(
            select_mode(Vec::new(), true, true).unwrap(),
            Some(Mode::Tui)
        );
        assert_eq!(
            select_mode(Vec::new(), false, true).unwrap(),
            Some(Mode::Gui)
        );
        assert_eq!(
            select_mode(Vec::new(), true, false).unwrap(),
            Some(Mode::Gui)
        );
        assert_eq!(
            select_mode(["--gui".to_owned()], true, false).unwrap(),
            Some(Mode::Gui)
        );
        assert_eq!(
            select_mode(["--server".to_owned()], false, false).unwrap(),
            Some(Mode::Server)
        );
        assert!(select_mode(["--tui".to_owned()], true, false).is_err());
        assert!(select_mode(["--tui".to_owned(), "--gui".to_owned()], true, true).is_err());
    }
}
