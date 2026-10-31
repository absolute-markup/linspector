import QtQuick
import QtQuick.Controls
Button {
    id: control
    property string glyph: ""
    property bool dark: false
    property bool danger: false
    property bool compact: false
    implicitHeight: compact ? 36 : 44
    leftPadding: 16; rightPadding: 16
    font.family: Fonts.sans; font.pixelSize: 13; font.weight: Font.Medium
    background: Rectangle {
        radius: control.compact ? 10 : 14
        color: !control.enabled ? "#eceee7" : control.danger ? (control.down ? "#e3c0b7" : "#fae9e3") : control.dark ? (control.down ? "#3d4640" : Theme.ink) : (control.hovered ? "#e4e7de" : "#eef0e9")
        border.width: control.activeFocus ? 2 : 0; border.color: "#779781"
    }
    contentItem: Row {
        spacing: 8
        Icon { visible: control.glyph.length > 0; glyph: control.glyph; color: control.dark && control.enabled ? "white" : control.danger ? "#983d28" : Theme.ink; anchors.verticalCenter: parent.verticalCenter; font.pixelSize: 17 }
        Text { text: control.text; font: control.font; color: !control.enabled ? "#9b9d94" : control.dark ? "white" : control.danger ? "#983d28" : Theme.ink; anchors.verticalCenter: parent.verticalCenter }
    }
    Accessible.name: text
}
