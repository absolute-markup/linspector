mod backend;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};
fn main() {
    if std::env::args().any(|s| s == "--smoke") {
        backend::diagnostics();
    }
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    engine
        .as_mut()
        .expect("QML engine")
        .on_object_creation_failed(|_, url| {
            eprintln!("Unable to load native QML interface: {url}");
            std::process::exit(1);
        })
        .release();
    engine.as_mut().expect("QML engine").load(&QUrl::from(
        "qrc:/qt/qml/com/absolutemarkup/linspector/qml/Main.qml",
    ));
    app.as_mut().expect("Qt application").exec();
}
