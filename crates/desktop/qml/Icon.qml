import QtQuick
Text {
    property string glyph: ""
    text: glyph
    font.family: Icons.family
    font.pixelSize: 20
    color: "#272823"
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
    renderType: Text.NativeRendering
    Accessible.ignored: true
}
