import QtQuick
import QtQuick.Controls
Button {
    id: control
    property string glyph: ""
    property string count: ""
    property bool selected: false
    implicitHeight: 46
    leftPadding: 14; rightPadding: 14
    background: Rectangle { radius: 13; color: control.selected ? Theme.ink : control.hovered ? "#e9ebe3" : "transparent"; border.width: control.activeFocus ? 2 : 0; border.color: "#9ebba5" }
    contentItem: Item {
        Icon { id: icon; anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; glyph: control.glyph; font.pixelSize: 20; color: control.selected ? "#fff" : "#75796f" }
        Text { anchors.left: icon.right; anchors.leftMargin: 12; anchors.verticalCenter: parent.verticalCenter; text: control.text; font.family: Fonts.sans; font.pixelSize: 14; color: control.selected ? "#fff" : Theme.ink }
        Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: control.count; font.family: Fonts.sans; font.pixelSize: 12; color: control.selected ? "#bbc6bb" : "#8b8e85" }
    }
}
