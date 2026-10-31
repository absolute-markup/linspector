pragma Singleton
import QtQuick
QtObject {
    readonly property FontLoader uiLoader: FontLoader { source: "qrc:/assets/fonts/HankenGrotesk.ttf" }
    readonly property string sans: uiLoader.status === FontLoader.Ready ? uiLoader.name : Qt.application.font.family
    readonly property int small: 12
    readonly property int body: 14
    readonly property int title: 32
    readonly property int display: 58
}
