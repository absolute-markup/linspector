import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.absolutemarkup.linspector

ApplicationWindow {
    id: window
    visible: true
    width: 1220; height: 820
    minimumWidth: 800; minimumHeight: 600
    title: "Linspector"
    color: "#f2f1ec"
    property var packages: []
    property var filtered: []
    property var selected: null
    function identity(p) { return p.name + "-" + p.epoch + ":" + p.version + "-" + p.release + "." + p.arch }
    function filter() {
        const terms = search.text.toLowerCase().trim().split(/\s+/).filter(t => t.length)
        filtered = packages.filter(p => {
            const hay = [p.name,p.title,p.description,p.arch,p.source_rpm].join(" ").toLowerCase()
            return terms.every(t => hay.includes(t))
        })
    }
    InventoryBackend {
        id: backend
        onInventoryChanged: {
            window.packages = JSON.parse(inventory)
            window.selected = null
            window.filter()
        }
    }
    Component.onCompleted: backend.refresh()
    Timer { interval: 100; running: true; repeat: true; onTriggered: backend.poll() }
    Timer { interval: 5000; running: backend.smoke; onTriggered: { backend.smoke_check(); Qt.quit() } }
    ColumnLayout {
        anchors.fill: parent; anchors.margins: 28; spacing: 18
        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                Label { text: "Linspector"; font.pixelSize: 32; font.weight: Font.DemiBold; color: "#172d2a" }
                Label { text: "Every package. A clearer picture."; color: "#56635d" }
            }
            Item { Layout.fillWidth: true }
            BusyIndicator { running: backend.busy; visible: running; Layout.preferredWidth: 36; Layout.preferredHeight: 36 }
            Button { text: "Refresh inventory"; enabled: !backend.busy; onClicked: backend.refresh() }
        }
        TextField {
            id: search; Layout.fillWidth: true
            placeholderText: "Search every RPM package — names, titles, descriptions, architecture…"
            onTextChanged: window.filter()
        }
        Label { text: backend.status + " · " + window.filtered.length + " shown"; color: "#56635d"; textFormat: Text.PlainText }
        SplitView {
            Layout.fillWidth: true; Layout.fillHeight: true
            ScrollView {
                SplitView.preferredWidth: 480; SplitView.minimumWidth: 260; clip: true
                ListView {
                    id: list; model: window.filtered; spacing: 4
                    delegate: ItemDelegate {
                        required property var modelData
                        width: ListView.view.width
                        height: 82
                        highlighted: window.selected !== null && window.identity(window.selected) === window.identity(modelData)
                        onClicked: { window.selected = modelData; backend.reverse = "Select ‘Find reverse dependencies’ to query RPM." }
                        contentItem: Column {
                            spacing: 3
                            Label { width: parent.width; text: modelData.name; font.bold: true; elide: Text.ElideRight; textFormat: Text.PlainText }
                            Label { width: parent.width; text: modelData.title; elide: Text.ElideRight; textFormat: Text.PlainText }
                            Label { width: parent.width; text: modelData.epoch + ":" + modelData.version + "-" + modelData.release + " · " + modelData.arch; color: "#56635d"; elide: Text.ElideRight; textFormat: Text.PlainText }
                        }
                    }
                    Label { anchors.centerIn: parent; visible: list.count === 0 && !backend.busy; text: "No matching packages" }
                }
            }
            ScrollView {
                id: detail; SplitView.fillWidth: true; SplitView.minimumWidth: 300; clip: true
                ColumnLayout {
                    width: detail.availableWidth; spacing: 14
                    property var p: window.selected
                    Label { Layout.fillWidth: true; text: parent.p ? parent.p.name : "Select a package"; font.pixelSize: 24; font.bold: true; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                    Label { Layout.fillWidth: true; text: parent.p ? parent.p.title : "Libraries, tools and development packages are all included."; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                    TextArea { Layout.fillWidth: true; visible: window.selected !== null; text: window.selected ? window.selected.description : ""; readOnly: true; wrapMode: Text.Wrap; textFormat: TextEdit.PlainText; selectByMouse: true }
                    Label {
                        Layout.fillWidth: true; wrapMode: Text.Wrap; textFormat: Text.PlainText
                        text: parent.p ? "Installed: " + window.identity(parent.p) + "\nSource RPM: " + parent.p.source_rpm + "\nRepository: " + parent.p.repository + "\nVendor: " + parent.p.vendor + "\nLicense: " + parent.p.license + "\nURL: " + parent.p.url + "\nInstalled bytes: " + parent.p.installed_bytes + "\nInstalled at (Unix seconds): " + parent.p.install_time + "\nEvidence: " + parent.p.provenance : ""
                    }
                    Label { text: "Required capabilities"; font.bold: true; visible: window.selected !== null }
                    TextArea { Layout.fillWidth: true; visible: window.selected !== null; text: window.selected ? window.selected.requires.join("\n") : ""; readOnly: true; wrapMode: Text.Wrap; textFormat: TextEdit.PlainText; selectByMouse: true }
                    Label { text: "Provided capabilities"; font.bold: true; visible: window.selected !== null }
                    TextArea { Layout.fillWidth: true; visible: window.selected !== null; text: window.selected ? window.selected.provides.join("\n") : ""; readOnly: true; wrapMode: Text.Wrap; textFormat: TextEdit.PlainText; selectByMouse: true }
                    Button { visible: window.selected !== null; enabled: !backend.busy; text: "Find reverse dependencies"; onClicked: backend.find_reverse(window.identity(window.selected)) }
                    TextArea { Layout.fillWidth: true; visible: window.selected !== null; text: backend.reverse; readOnly: true; wrapMode: Text.Wrap; textFormat: TextEdit.PlainText; selectByMouse: true }
                    Label { Layout.fillWidth: true; wrapMode: Text.Wrap; text: "Inspection only. Reverse results are direct candidates, not a removal plan. File and rich dependencies may be incomplete; no package is marked safe to remove."; color: "#56635d" }
                }
            }
        }
        Label { text: "Designed and developed by Absolute Markup"; color: "#56635d"; font.pixelSize: 12 }
    }
}
