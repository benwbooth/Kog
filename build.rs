use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("org.kog.player").qml_file("qml/main.qml"))
        .files(["src/app_controller.rs"])
        .qt_module("Network")
        .build();
}
