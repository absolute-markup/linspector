use cxx_qt_build::{CxxQtBuilder, QmlModule};
fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.absolutemarkup.linspector").qml_file("qml/Main.qml"),
    )
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .files(["src/backend.rs"])
    .build();
}
