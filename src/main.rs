mod app_controller;
mod desktop_integration;
mod file_tree_model;
mod rom_import;
mod skin_library;
mod tag_editor;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

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

fn main() {
    configure_platform_theme();
    let mut application = desktop_integration::DesktopApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    QGuiApplication::set_desktop_file_name(&QString::from("org.kog.player"));
    application.set_application_name(&QString::from("Kog"));

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/org/kog/player/qml/Main.qml"));
    }
    desktop_integration::apply_application_icon();
    desktop_integration::restore_main_window();

    application.exec();
}
