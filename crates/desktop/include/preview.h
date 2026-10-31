#pragma once
#include <QGuiApplication>
#include <QQuickWindow>
#include <QImage>
#include <QString>
#include <cstdio>
inline void linspector_diagnostics() {
    qInstallMessageHandler([](QtMsgType, const QMessageLogContext&, const QString& message) {
        std::fprintf(stderr, "%s\n", qPrintable(message));
    });
}
inline bool linspector_capture(const QString& path) {
    for (QWindow* window : QGuiApplication::topLevelWindows()) {
        if (auto* quick = qobject_cast<QQuickWindow*>(window)) {
            const QImage image = quick->grabWindow();
            return !image.isNull() && image.save(path);
        }
    }
    return false;
}
