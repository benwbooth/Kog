mod album_groups;
mod app_controller;
mod desktop_art;
mod desktop_integration;
mod file_tree_model;
mod icon_theme;
mod mml_view;
mod playlist_folders;
mod rom_import;
mod skin_library;
mod tag_editor;
mod metadata_cache;
mod workspace_tracks;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};
use std::io::IsTerminal;

/// Qt Quick Controls' Basic style, coloured by Kog's own palette (light or
/// dark as the desktop is, see kog_desktop_integration.cpp), the same on every
/// desktop. The platform theme stays whatever the session chose, so file
/// dialogs, fonts and icons still come from the desktop. KOG_STYLE picks
/// another. (Not the session's QT_QUICK_CONTROLS_STYLE: hearth sets that for
/// other apps, org.hearth.desktop, 2026-11-25.)
fn configure_controls_style() {
    // SAFETY: this runs before Qt, Rodio, or any other worker threads
    // are created, so no concurrent environment access is possible.
    set_qt_env("QT_QUICK_CONTROLS_STYLE", std::env::var("KOG_STYLE").unwrap_or_else(|_| "Basic".into()));
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
        // Kog has no +platform or +locale QML variants, but the engine's
        // file selector looks for them (five lookups) on every image a
        // playlist row loads as it scrolls into view.
        ("QT_NO_BUILTIN_SELECTORS", "1"),
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

/// What the command line asked for: a mode and, for the desktop app, the
/// audio files to open (absolute paths, in the order given).
#[derive(Debug, PartialEq, Eq)]
struct Launch {
    mode: Mode,
    files: Vec<std::path::PathBuf>,
}

/// A file argument as a local path: `file://` URIs (what file managers pass
/// for `%U`) decoded, relative paths made absolute against `cwd` so a running
/// Kog in another directory finds them. Other URI schemes are refused.
fn file_argument(arg: &str, cwd: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let path = if arg.starts_with("file:") {
        url::Url::parse(arg)
            .ok()
            .and_then(|url| url.to_file_path().ok())
            .ok_or_else(|| format!("{arg} is not a local file"))?
    } else if arg.contains("://") {
        return Err(format!("{arg} is not a local file (only local files can be opened)"));
    } else {
        std::path::PathBuf::from(arg)
    };
    Ok(if path.is_absolute() { path } else { cwd.join(path) })
}

fn select_mode(
    args: impl IntoIterator<Item = String>,
    terminal: bool,
    tui_supported: bool,
    cwd: &std::path::Path,
) -> Result<Option<Launch>, String> {
    let mut chosen = None;
    let mut files = Vec::new();
    let mut options_done = false;
    for arg in args {
        if options_done || !arg.starts_with('-') || arg == "-" {
            files.push(file_argument(&arg, cwd)?);
            continue;
        }
        let mode = match arg.as_str() {
            "--" => {
                options_done = true;
                continue;
            }
            "--gui" => Mode::Gui,
            "--tui" => Mode::Tui,
            "--server" | "--headless" => Mode::Server,
            "--help" | "-h" => {
                println!(
                    "Kog {}\n\nUsage: kog [--gui | --tui | --server] [FILE...]\n\n  --gui       Open the Qt desktop app\n  --tui       Open the terminal player\n  --server    Serve the web app without a UI\n  --help      Show this help\n  --version   Show the version\n\n  FILE...     Audio files (paths or file:// URIs) to play in the desktop\n              app: the first starts now, the rest are queued. A running\n              Kog plays them instead of a new one starting.",
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
    if !files.is_empty() && matches!(chosen, Some(Mode::Tui | Mode::Server)) {
        return Err("files can only be opened in the desktop app (--gui)".to_owned());
    }
    // Files always mean the desktop app, even typed in a terminal.
    let mode = chosen.unwrap_or(if terminal && tui_supported && files.is_empty() {
        Mode::Tui
    } else {
        Mode::Gui
    });
    Ok(Some(Launch { mode, files }))
}

fn main() {
    // The library reader's helper process (see kog_server::discover::helper).
    kog_server::discover::helper::run_if_requested();
    let terminal = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let tui_supported = cfg!(unix);
    let cwd = std::env::current_dir().unwrap_or_default();
    let Launch { mode, files } = match select_mode(std::env::args().skip(1), terminal, tui_supported, &cwd) {
        Ok(Some(launch)) => launch,
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
    configure_controls_style();
    configure_text_rendering();
    let mut application = desktop_integration::DesktopApplication::new();

    QGuiApplication::set_desktop_file_name(&QString::from("org.kog.player"));
    application.set_application_name(&QString::from("Kog"));
    // Said here as well as in the app's status line: a launcher's log shows
    // why nothing played.
    for file in &files {
        if !file.exists() {
            eprintln!("kog: can't find {}", file.display());
        }
    }
    let files_json = if files.is_empty() {
        String::new()
    } else {
        serde_json::to_string(
            &files
                .iter()
                .map(|file| file.to_string_lossy())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default()
    };
    match desktop_integration::claim_gui_instance(&files_json) {
        Ok(desktop_integration::GuiInstance::Primary) => {}
        Ok(desktop_integration::GuiInstance::Raised) => return,
        Err(error) => {
            eprintln!("kog: {error}");
            std::process::exit(1);
        }
    }

    // `import GlobalMenu` (src/cpp/kog_global_menu.cpp): the window's menus
    // in the desktop's global menu bar.
    unsafe extern "C" {
        fn kog_register_global_menu();
    }
    // SAFETY: a plain qmlRegisterType call, on the GUI thread before any QML
    // is loaded.
    unsafe { kog_register_global_menu() };
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
    restore_launch_env();
    // Files from the command line play once the window (and its player)
    // exists; a running Kog gets them over the activation socket instead.
    if !files_json.is_empty() {
        desktop_integration::open_files(&files_json);
    }

    application.exec();
    // Hand the background jobs (library index, sound model) to the server
    // or another Kog at once.
    kog_server::discover::shared().release();
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

    fn mode(args: &[&str], terminal: bool, tui: bool) -> Result<Option<Mode>, String> {
        select_mode(args.iter().map(|a| a.to_string()), terminal, tui, std::path::Path::new("/w"))
            .map(|launch| launch.map(|launch| launch.mode))
    }

    #[test]
    fn selects_terminal_only_for_interactive_launches() {
        assert_eq!(mode(&[], true, true).unwrap(), Some(Mode::Tui));
        assert_eq!(mode(&[], false, true).unwrap(), Some(Mode::Gui));
        assert_eq!(mode(&[], true, false).unwrap(), Some(Mode::Gui));
        assert_eq!(mode(&["--gui"], true, false).unwrap(), Some(Mode::Gui));
        assert_eq!(mode(&["--server"], false, false).unwrap(), Some(Mode::Server));
        assert!(mode(&["--tui"], true, false).is_err());
        assert!(mode(&["--tui", "--gui"], true, true).is_err());
        assert!(mode(&["--bogus"], false, true).is_err());
    }

    #[test]
    fn accepts_files_and_file_uris_for_the_desktop_app() {
        let launch = select_mode(
            [
                "song.wav",
                "file:///music/My%20Song.flac",
                "/abs/b.mod",
                "--",
                "--odd-name.mp3",
            ]
            .map(String::from),
            true,
            true,
            std::path::Path::new("/w"),
        )
        .unwrap()
        .unwrap();
        // Files mean the desktop app even from a terminal.
        assert_eq!(launch.mode, Mode::Gui);
        assert_eq!(
            launch.files,
            ["/w/song.wav", "/music/My Song.flac", "/abs/b.mod", "/w/--odd-name.mp3"]
                .map(std::path::PathBuf::from)
        );
        assert!(mode(&["--server", "a.wav"], false, true).is_err());
        assert!(mode(&["--tui", "a.wav"], true, true).is_err());
        assert!(mode(&["https://example.com/a.mp3"], false, true).is_err());
    }
}

/// Sets an environment variable Qt reads while starting, noting what it was
/// (as `__APP_ORIG_<name>`: empty = unset, else "=value") so
/// `restore_launch_env` puts it back once the window is loaded: programs this
/// app starts get the desktop's own. (QT_QUICK_CONTROLS_STYLE=Basic leaking
/// from our apps made KDE apps opened from them draw plain light controls,
/// 2026-11-25.)
fn set_qt_env(name: &str, value: impl AsRef<std::ffi::OsStr>) {
    let note = format!("__APP_ORIG_{name}");
    if std::env::var_os(&note).is_none() {
        let was = std::env::var_os(name)
            .map(|v| {
                let mut s = std::ffi::OsString::from("=");
                s.push(v);
                s
            })
            .unwrap_or_default();
        #[allow(unused_unsafe)]
        unsafe { std::env::set_var(&note, was) };
    }
    #[allow(unused_unsafe)]
    unsafe { std::env::set_var(name, value) };
}

/// Puts back what `set_qt_env` (or the C++ start-up) changed, once the main
/// QML has loaded: Qt has read them all by then.
#[allow(dead_code)]
fn restore_launch_env() {
    for (k, v) in std::env::vars_os() {
        let Some(name) = k.to_str().and_then(|k| k.strip_prefix("__APP_ORIG_")).map(str::to_owned) else { continue };
        let v = v.to_string_lossy().into_owned();
        #[allow(unused_unsafe)]
        unsafe {
            match v.strip_prefix('=') {
                Some(old) => std::env::set_var(&name, old),
                None => std::env::remove_var(&name),
            }
            std::env::remove_var(&k);
        }
    }
}
