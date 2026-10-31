pragma Singleton
import QtQuick
QtObject {
    readonly property FontLoader loader: FontLoader { source: "qrc:/assets/fonts/remixicon.ttf" }
    readonly property string family: loader.name
    readonly property string dashboardLine: "\uec14"
    readonly property string apps2Line: "\uea42"
    readonly property string archiveStackLine: "\uf428"
    readonly property string stackLine: "\uf181"
    readonly property string hardDrive3Line: "\uf395"
    readonly property string searchLine: "\uf0d1"
    readonly property string refreshLine: "\uf064"
    readonly property string arrowRightUpLine: "\uea70"
    readonly property string arrowRightLine: "\uea6c"
    readonly property string closeLine: "\ueb99"
    readonly property string deleteBin6Line: "\uec26"
    readonly property string shutDownLine: "\uf126"
    readonly property string checkboxCircleLine: "\ueb81"
    readonly property string terminalBoxLine: "\uf1f6"
    readonly property string puzzleLine: "\uf452"
    readonly property string settings3Line: "\uf0e6"
    readonly property string arrowDownSLine: "\uea4e"
    readonly property string shieldCheckLine: "\uf100"
    readonly property string downloadLine: "\uec5a"
    readonly property string box3Line: "\uf2f5"
    readonly property string codeSSlashLine: "\uebad"
    readonly property string informationLine: "\uee59"
}
