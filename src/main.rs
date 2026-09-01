mod adlmidi;
mod adlmidi_decoder;
mod adplug;
mod adplug_decoder;
mod apl;
mod apl_decoder;
mod app_controller;
mod archive;
mod cuesheet;
mod cuesheet_decoder;
mod decoder;
mod ffmpeg;
mod ffmpeg_decoder;
mod file_tree_model;
mod gme;
mod gme_decoder;
mod gsf;
mod gsf_decoder;
mod hively;
mod hively_decoder;
mod libvgm;
mod libvgm_decoder;
mod mt32;
mod ncsf;
mod ncsf_decoder;
mod openmpt;
mod openmpt_decoder;
mod opl3;
mod organya;
mod organya_decoder;
mod playback;
mod playlist;
mod psf;
mod psf_decoder;
mod qsf;
mod qsf_decoder;
mod sc55;
mod sdsf;
mod sdsf_decoder;
mod settings;
mod sfm;
mod sfm_decoder;
mod sid;
mod sid_decoder;
mod syntrax;
mod syntrax_decoder;
mod track;
mod usf;
mod usf_decoder;
mod vgmstream;
mod vgmstream_decoder;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

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
    let mut application = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/org/kog/player/qml/Main.qml"));
    }

    if let Some(application) = application.as_mut() {
        application.exec();
    }
}
