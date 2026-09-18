use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::path::PathBuf;

fn main() {
    // QML, icons, and web runtime files are embedded into the binary:
    // without these, local builds silently keep shipping stale UI after
    // QML-only edits.
    println!("cargo:rerun-if-changed=qml");
    println!("cargo:rerun-if-changed=web");
    println!("cargo:rerun-if-changed=build.rs");
    // Switching branch or commit rewrites .git/HEAD, which is when the stamped
    // revision below can change. Absent when building from a source package.
    if std::path::Path::new(".git/HEAD").exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
    }
    emit_build_revision();
    let qt_builder = CxxQtBuilder::new_qml_module(QmlModule::new("org.kog.player").qml_files([
        "qml/CogButton.qml",
        "qml/AudioVisualization.qml",
        "qml/ClassicBitmapText.qml",
        "qml/SkinResizeGrip.qml",
        "qml/ClassicPlayer.qml",
        "qml/ClassicPlaylist.qml",
        "qml/ModernPlayer.qml",
        "qml/ModernLibraryPanel.qml",
        "qml/SkinBrowser.qml",
        "qml/SkinSprite.qml",
        "qml/Visualizer.qml",
        "qml/Equalizer.qml",
        "qml/InfoInspector.qml",
        "qml/AboutKog.qml",
        "qml/PaneSplitView.qml",
        "qml/KineticWheelHandler.qml",
        "qml/Lyrics.qml",
        "qml/Main.qml",
        "qml/MainWindowSettings.qml",
        "qml/MiniPlayer.qml",
        "qml/NowPlayingNotification.qml",
        "qml/PlaylistHeader.qml",
        "qml/PlaylistRow.qml",
        "qml/Preferences.qml",
        "qml/TagEditor.qml",
        "qml/TreeSearchLayout.qml",
        "qml/SearchHighlightLabel.qml",
        "qml/PlaybackTitle.qml",
        "qml/RemoteBrowser.qml",
    ]))
    // Keep the bridge include root limited to Kog's hand-written integration
    // header instead of recursively tracking the whole repository.
    .crate_include_root(Some("native".to_owned()))
    .qrc("web/modern/runtime.qrc")
    .qrc_resources([
        "qml/NotificationLayerShell.qml",
        "qml/icons/application-menu.svg",
        "qml/icons/application-menu-light.svg",
        "qml/icons/audio-volume-high.svg",
        "qml/icons/audio-volume-high-light.svg",
        "qml/icons/audio-x-generic.svg",
        "qml/icons/audio-x-generic-light.svg",
        "qml/icons/dialog-information.svg",
        "qml/icons/dialog-information-light.svg",
        "qml/icons/edit-find.svg",
        "qml/icons/edit-find-light.svg",
        "qml/icons/edit-clear-list.svg",
        "qml/icons/edit-clear-list-light.svg",
        "qml/icons/folder-open.svg",
        "qml/icons/folder-open-light.svg",
        "qml/icons/folder.svg",
        "qml/icons/folder-light.svg",
        "qml/icons/go-up.svg",
        "qml/icons/go-up-light.svg",
        "qml/icons/kog.svg",
        "qml/icons/kog-symbolic.svg",
        "qml/icons/kog-symbolic-play.svg",
        "qml/icons/kog-symbolic-pause.svg",
        "qml/icons/media-playback-pause.svg",
        "qml/icons/media-playback-pause-light.svg",
        "qml/icons/media-playback-start.svg",
        "qml/icons/media-playback-start-light.svg",
        "qml/icons/media-playback-stop.svg",
        "qml/icons/media-playback-stop-light.svg",
        "qml/icons/media-playlist-repeat.svg",
        "qml/icons/media-playlist-repeat-light.svg",
        "qml/icons/media-playlist-shuffle.svg",
        "qml/icons/media-playlist-shuffle-light.svg",
        "qml/icons/media-skip-backward.svg",
        "qml/icons/media-skip-backward-light.svg",
        "qml/icons/media-skip-forward.svg",
        "qml/icons/media-skip-forward-light.svg",
        "qml/icons/star-filled.svg",
        "qml/icons/star-outline.svg",
        "qml/icons/view-list-tree.svg",
        "qml/icons/view-list-tree-light.svg",
        "qml/icons/view-restore.svg",
        "qml/icons/view-restore-light.svg",
        "qml/icons/window-close.svg",
        "qml/icons/window-close-light.svg",
    ])
    .files([
        "src/app_controller.rs",
        "src/desktop_integration.rs",
        "src/file_tree_model.rs",
        "src/skin_library.rs",
    ])
    .cpp_file("native/kog_desktop_integration.cpp")
    .cpp_file("native/kog_window_state.cpp")
    .cpp_file("native/kog_skin_network.cpp")
    .cpp_file("native/kog_cover_art_network.cpp")
    .cpp_file("native/kog_modern_skin.h")
    .cpp_file("native/kog_modern_skin.cpp")
    .cpp_file("native/kog_file_tree_search.h")
    .cpp_file("native/kog_media_path.h")
    .cpp_file("native/kog_file_tree_search.cpp")
    .cpp_file("native/kog_tree_archive.cpp")
    .cpp_file("native/kog_tree_archive_bridge.cpp")
    .qt_module("Concurrent")
    .qt_module("Gui")
    .qt_module("Network")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Widgets")
    .qt_module("WebEngineQuick")
    .qt_module("WebEngineCore")
    .qt_module("WebChannel");
    let session_headers = wayland_session_headers();
    // The archive tree uses the same in-process libarchive as compress-tools.
    let archive = pkg_config::Config::new()
        .probe("libarchive")
        .expect("libarchive development headers are required for archive browsing");
    // SAFETY: only add include paths for the same Qt installation used by
    // cxx-qt, and a define for our own bridge. Do not alter Qt's ABI flags.
    let qt_builder = unsafe {
        qt_builder.cc_builder(|cc| {
            cc.includes(&archive.include_paths);
            if !session_headers.is_empty() {
                cc.includes(&session_headers);
                cc.define("KOG_WAYLAND_SESSION_RESTORE", None);
            }
        })
    };
    qt_builder.build();
}

fn wayland_session_headers() -> Vec<PathBuf> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return Vec::new();
    }
    let qmake = std::env::var("QMAKE").unwrap_or_else(|_| "qmake6".to_owned());
    let query = |key: &str| {
        std::process::Command::new(&qmake)
            .args(["-query", key])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let (Some(headers), Some(version)) = (query("QT_INSTALL_HEADERS"), query("QT_VERSION")) else {
        return Vec::new();
    };
    let parts: Vec<u32> = version.split('.').filter_map(|part| part.parse().ok()).collect();
    // QWaylandWindow exists in 6.10, but setSessionRestoreId was added in
    // 6.11. The Flatpak KDE 6.10 SDK must use geometry-only restoration.
    if parts.len() < 2 || (parts[0], parts[1]) < (6, 11) {
        return Vec::new();
    }
    let headers = PathBuf::from(headers);
    let gui = headers.join("QtGui").join(&version);
    let core = headers.join("QtCore").join(&version);
    if !gui.join("QtGui/qpa/qplatformwindow_p.h").is_file() {
        return Vec::new();
    }
    vec![gui.join("QtGui"), gui, core.join("QtCore"), core]
}

/// Stamp the revision this build came from, so the About window and the title
/// bar can show which build is running. Packaged builds have no `.git`, so
/// `KOG_BUILD_REV` (set by a release job) wins when it is present.
fn emit_build_revision() {
    let revision = std::env::var("KOG_BUILD_REV")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let head = git(&["rev-parse", "--short", "HEAD"])?;
            let dirty = git(&["status", "--porcelain"])
                .is_some_and(|status| !status.trim().is_empty());
            Some(if dirty { format!("{head}-dirty") } else { head })
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=KOG_BUILD_REV={revision}");
}

/// Run a git command, returning its trimmed stdout when it succeeds.
fn git(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
