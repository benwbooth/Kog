mod adplug;
mod adplug_decoder;
mod apl;
mod apl_decoder;
mod app_controller;
mod decoder;
mod ffmpeg;
mod ffmpeg_decoder;
mod gme;
mod gme_decoder;
mod hively;
mod hively_decoder;
mod libvgm;
mod libvgm_decoder;
mod openmpt;
mod openmpt_decoder;
mod opl3;
mod organya;
mod organya_decoder;
mod playback;
mod settings;
mod sid;
mod sid_decoder;
mod track;
mod vgmstream;
mod vgmstream_decoder;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    let mut application = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/org/kog/player/qml/Main.qml"));
    }

    if let Some(application) = application.as_mut() {
        application.exec();
    }
}
