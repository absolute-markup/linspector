import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.absolutemarkup.linspector

ScrollView {
    id: systemPage
    property var report: ({})
    clip: true
    function bytes(n) {
        if (!n) return "Unavailable"
        return (Number(n)/1073741824).toFixed(1) + " GiB"
    }
    function duration(seconds) {
        if (seconds === undefined) return "Unavailable"
        return Math.floor(seconds/86400) + "d " + Math.floor(seconds/3600)%24 + "h " + Math.floor(seconds/60)%60 + "m"
    }
    function sections() {
        const query = detailSearch.text.trim().toLowerCase()
        return (report.sections || []).map(s => ({title:s.title,subtitle:s.subtitle,fields: !query || s.title.toLowerCase().includes(query) ? s.fields : s.fields.filter(f => (f.label + " " + f.value).toLowerCase().includes(query))})).filter(s => s.fields.length)
    }
    function expandAll(open) {
        for (let i=0; i<sectionRepeater.count; i++) {
            const card = sectionRepeater.itemAt(i)
            if (card) card.open = open
        }
    }
    ColumnLayout {
        width: systemPage.availableWidth; spacing: 16
        Surface {
            Layout.fillWidth: true; implicitHeight: 158; color: Theme.dark
            RowLayout {
                anchors.fill: parent; anchors.margins: 25; spacing: 24
                ColumnLayout {
                    Layout.fillWidth: true; spacing: 8
                    Text { text: "YOUR MACHINE"; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1.8; color: "#b1bfa9" }
                    Text { Layout.fillWidth: true; text: report.demo ? "Demo mode" : report.hostname || "Scanning your system…"; font.family: Fonts.sans; font.pixelSize: 32; font.weight: Font.Medium; font.letterSpacing: -0.8; color: "#f6f8f2"; elide: Text.ElideRight; textFormat: Text.PlainText }
                    Text { Layout.fillWidth: true; text: report.demo ? "Real account and hardware details are not loaded in demo mode." : report.os || "OS information will appear after the scan."; font.family: Fonts.sans; font.pixelSize: 13; color: "#c2ccb9"; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                }
                Rectangle { Layout.preferredWidth: 72; Layout.preferredHeight: 72; radius: 24; color: "#303c32"; Icon { anchors.centerIn: parent; glyph: Icons.settings3Line; font.pixelSize: 36; color: "#c6d9b9" } }
            }
        }
        GridLayout {
            Layout.fillWidth: true; columns: systemPage.availableWidth > 850 ? 4 : 2; columnSpacing: 12; rowSpacing: 12
            Repeater {
                model: [ {label:"LOGICAL CPUS",value:report.logical_cpus ? report.logical_cpus.toString() : "Unavailable",detail:report.cpu || "Processor information unavailable",tint:"#e2e9d9"}, {label:"USABLE MEMORY",value:systemPage.bytes(report.memory_total),detail:report.memory_available ? systemPage.bytes(report.memory_available) + " available at scan" : "Memory information unavailable",tint:"#e9e3f1"}, {label:"UPTIME",value:systemPage.duration(report.uptime_seconds),detail:"Since the current system boot",tint:"#f3dfc5"}, {label:"DETAIL SECTIONS",value:(report.sections || []).length.toString(),detail:"Live details available to your account",tint:"#e8ebe3"} ]
                Surface {
                    required property var modelData
                    Layout.fillWidth: true; implicitHeight: 132; color: modelData.tint
                    ColumnLayout { anchors.fill: parent; anchors.margins: 19; spacing: 7
                        Text { text: modelData.label; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1; color: "#66705d" }
                        Text { text: modelData.value; font.family: Fonts.sans; font.pixelSize: 27; font.weight: Font.Medium; font.letterSpacing: -0.7; color: Theme.ink }
                        Text { Layout.fillWidth: true; text: modelData.detail; font.family: Fonts.sans; font.pixelSize: 11; color: "#6e7567"; maximumLineCount: 2; wrapMode: Text.Wrap; elide: Text.ElideRight; textFormat: Text.PlainText }
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true; Layout.topMargin: 5; spacing: 8
            Text { text: "The details."; font.family: Fonts.sans; font.pixelSize: 24; font.weight: Font.Medium; color: Theme.ink; font.letterSpacing: -0.6 }
            Item { Layout.fillWidth: true }
            ActionButton { text: "Expand all"; compact: true; onClicked: systemPage.expandAll(true) }
            ActionButton { text: "Collapse all"; compact: true; onClicked: systemPage.expandAll(false) }
        }
        TextField {
            id: detailSearch; Layout.fillWidth: true; implicitHeight: 42; leftPadding: 15; rightPadding: 15
            placeholderText: "Search system details — username, kernel, graphics, network…"
            font.family: Fonts.sans; font.pixelSize: 13; color: Theme.ink
            background: Rectangle { color: "#ffffff"; radius: 13; border.width: detailSearch.activeFocus ? 2 : 0; border.color: "#9db596" }
            Accessible.name: "Search system details"
        }
        GridLayout {
            Layout.fillWidth: true; columns: systemPage.availableWidth > 850 ? 2 : 1; columnSpacing: 14; rowSpacing: 14
            Repeater {
                id: sectionRepeater; model: systemPage.sections()
                Surface {
                    id: sectionCard
                    required property var modelData
                    property bool open: ["Operating system","Account & session"].includes(modelData.title)
                    readonly property bool expanded: open || detailSearch.text.trim().length > 0
                    Layout.fillWidth: true; Layout.alignment: Qt.AlignTop
                    implicitHeight: sectionContent.implicitHeight + 40
                    ColumnLayout {
                        id: sectionContent; anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 20; spacing: 15
                        Button {
                            Layout.fillWidth: true; implicitHeight: 54; padding: 0
                            background: Rectangle { color: "transparent"; radius: 8; border.width: parent.activeFocus ? 2 : 0; border.color: "#a4b99a" }
                            contentItem: RowLayout {
                                spacing: 12
                                ColumnLayout {
                                    Layout.fillWidth: true; spacing: 4
                                    Text { text: sectionCard.modelData.title; font.family: Fonts.sans; font.pixelSize: 19; font.weight: Font.Medium; color: Theme.ink }
                                    Text { Layout.fillWidth: true; text: sectionCard.modelData.subtitle; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; wrapMode: Text.Wrap }
                                }
                                Text { text: sectionCard.modelData.fields.length; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted }
                                Icon { glyph: Icons.arrowDownSLine; rotation: sectionCard.expanded ? 180 : 0; color: Theme.muted }
                            }
                            onClicked: sectionCard.open = !sectionCard.open
                            Accessible.name: sectionCard.modelData.title + (sectionCard.expanded ? ", expanded" : ", collapsed")
                        }
                        ColumnLayout {
                            Layout.fillWidth: true; visible: sectionCard.expanded; spacing: 11
                            Repeater {
                                model: sectionCard.modelData.fields
                                ColumnLayout {
                                    required property var modelData
                                    Layout.fillWidth: true; spacing: 3
                                    Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: "#eef0e8" }
                                    Text { Layout.fillWidth: true; text: modelData.label; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                                    TextEdit { Layout.fillWidth: true; text: modelData.value; readOnly: true; selectByMouse: true; wrapMode: TextEdit.Wrap; font.family: Fonts.sans; font.pixelSize: 13; color: modelData.value === "Unavailable" ? "#969b90" : Theme.ink; textFormat: TextEdit.PlainText; Accessible.name: modelData.label + ": " + modelData.value }
                                }
                            }
                        }
                    }
                }
            }
        }
        Text { Layout.fillWidth: true; visible: sectionRepeater.count === 0; text: report.demo ? "Switch to a live scan to see system details." : "No matching details."; font.family: Fonts.sans; color: Theme.muted }
        Text { Layout.fillWidth: true; text: "A snapshot of information available to your account. Some firmware and security details may be restricted. Refresh with Scan system."; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; wrapMode: Text.Wrap }
    }
}
