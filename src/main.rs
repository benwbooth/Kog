mod app_controller;
mod decoder;
mod gme;
mod gme_decoder;
mod libvgm;
mod libvgm_decoder;
mod opl3;
mod playback;
mod settings;
mod track;

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
