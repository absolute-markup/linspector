use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};
fn main() {
    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("com.absolutemarkup.linspector")
            .qml_files([
                "qml/Main.qml",
                "qml/SystemPage.qml",
                "qml/Icon.qml",
                "qml/Surface.qml",
                "qml/ActionButton.qml",
                "qml/NavItem.qml",
            ])
            .qml_file(QmlFile::from("qml/Fonts.qml").singleton(true))
            .qml_file(QmlFile::from("qml/Icons.qml").singleton(true))
            .qml_file(QmlFile::from("qml/Theme.qml").singleton(true)),
    )
    .qrc("assets.qrc")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .files(["src/backend.rs"]);
    // The callback only adds this project's header path; no compiler assumptions.
    unsafe {
        builder.cc_builder(|cc| {
            cc.include("include");
        })
    }
    .build();
}
