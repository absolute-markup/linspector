import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.absolutemarkup.linspector

ApplicationWindow {
    id: window
    visible: true
    width: 1480; height: 960
    minimumWidth: 1060; minimumHeight: 760
    title: "Linspector"
    onClosing: function(event) {
        if (backend.action_running) {
            event.accepted = false
            backend.action_message = "Finish or dismiss authentication, then wait for the active operation before closing Linspector."
        }
    }
    color: Theme.paper
    font.family: Fonts.sans
    font.pixelSize: Fonts.body
    palette.window: Theme.paper
    palette.windowText: Theme.ink
    palette.base: "#ffffff"
    palette.text: Theme.ink
    palette.buttonText: Theme.ink
    palette.highlight: "#a9c3a1"
    palette.highlightedText: Theme.ink
    property var packages: []
    readonly property var sourceStates: JSON.parse(backend.source_statuses)
    readonly property var flatpakState: sourceStates.find(s => s.provider === "flatpak") || null
    readonly property var snapState: sourceStates.find(s => s.provider === "snap") || null
    readonly property bool exclusiveSourceUnavailable: page === "Snap" ? !snapState || !snapState.complete : page === "Flatpak" ? !flatpakState || !flatpakState.complete : false
    readonly property int snapCount: packages.filter(p => p.id.provider === "snap").length
    readonly property int flatpakCount: packages.filter(p => p.id.provider === "flatpak").length
    readonly property bool selectedCanRemove: !!selected && !!selected.capabilities && selected.capabilities.removal_preview && backend.can_remove
    readonly property string removalReason: selected && selected.capabilities && selected.capabilities.reasons.removal ? selected.capabilities.reasons.removal : platformInfo.capabilities.removal_reason || "Removal is unavailable for this source."
    property var volumes: []
    property var systemReport: ({})
    readonly property var platformInfo: JSON.parse(backend.platform_details)
    property var filtered: []
    property var selected: null
    property string page: "Overview"
    property string category: "All packages"
    property var serviceRows: []
    property var removalRows: []
    property var startupTarget: null
    property bool startupEnable: false
    readonly property var rootDisk: volumes.find(d => d.mount === "/") || null
    readonly property double packageBytes: packages.reduce((sum,p) => sum + Number(p.installed_bytes || 0), 0)
    readonly property int appCount: packages.filter(p => p.category === "Applications").length
    readonly property int libCount: packages.filter(p => p.category === "Libraries").length
    readonly property int devCount: packages.filter(p => p.category === "Development").length
    function bytes(n) {
        if (n === null || n === undefined) return "Unknown"
        n = Number(n || 0)
        if (n < 1024) return n.toFixed(0) + " B"
        const units = ["KiB", "MiB", "GiB", "TiB"]
        let index = -1
        do { n /= 1024; index++ } while (n >= 1024 && index < units.length-1)
        return n.toFixed(n >= 10 ? 1 : 2) + " " + units[index]
    }
    function inspectorRows(p) {
        if (!p) return []
        const m = p.native_metadata
        const installed = p.id.provider === "snap" && m["snap.install-date"] ? m["snap.install-date"] : Number(p.install_time) ? new Date(Number(p.install_time)*1000).toLocaleDateString() : "Not recorded"
        let rows = [["Source",p.id.provider + " · " + p.id.scope], ["Installed version",p.version_display], ["Architecture",p.arch], [p.id.provider === "snap" ? "Snap reported size" : "Installed size",bytes(p.installed_bytes)], ["License",p.license || "Not recorded"], [p.id.provider === "snap" ? "Publisher" : "Vendor",p.vendor || "Not recorded"], ["Source package",p.source_package || "Not recorded"], [p.id.provider === "flatpak" ? "Remote" : "Repository",p.repository || "Unknown"], ["Installed",installed]]
        if (p.id.provider === "dpkg") rows.push(["Maintainer",m["dpkg.maintainer"] || "Not recorded"],["Package state",m["dpkg.status"] || "Not recorded"])
        if (p.id.provider === "flatpak") rows.push(["Branch",m["flatpak.branch"]],["Ref",p.id.native],["Commit",m["flatpak.commit"] || "Unavailable"])
        if (p.id.provider === "pacman") rows.push(["Packager",m["pacman.packager"] || "Not recorded"],["Install reason",m["pacman.reason"] === "1" ? "Dependency" : m["pacman.reason"] === undefined || m["pacman.reason"] === "0" ? "Explicit" : m["pacman.reason"]])
        if (p.id.provider === "snap") rows.push(["Revision",m["snap.revision"] || "Not recorded"],["State",m["snap.status"] === "installed" ? "Inactive installed revision" : m["snap.status"] || "Not recorded"],["Kind",m["snap.type"] || "Not recorded"],["Confinement",m["snap.confinement"] || "Not recorded"],["Tracking channel",m["snap.tracking-channel"] || m["snap.channel"] || "Not recorded"])
        if (m["snap.component-revision"]) rows.push(["Component revision",m["snap.component-revision"]])
        return rows
    }
    function identity(p) { return p.key }
    function catIcon(cat) { return cat === "Applications" ? Icons.apps2Line : cat === "Libraries" ? Icons.stackLine : cat === "Development" ? Icons.codeSSlashLine : Icons.box3Line }
    function catColor(cat) { return cat === "Applications" ? "#d9e7d4" : cat === "Libraries" ? "#e4def3" : cat === "Development" ? "#fae1bf" : "#e8eae4" }
    function count(cat) { if (cat === "Flatpak") return flatpakCount; if (cat === "Snap") return snapCount; return cat === "Applications" ? appCount : cat === "Libraries" ? libCount : cat === "Development" ? devCount : packages.length }
    function navigate(next) {
        if (["Flatpak","Snap"].includes(next) && selected && selected.id.provider !== next.toLowerCase()) selectPackage(null)
        page = next; category = ["Applications","Libraries","Development"].includes(next) ? next : "All packages"; filter()
    }
    function filter() {
        const terms = search.text.toLowerCase().trim().split(/\s+/).filter(t => t.length)
        let items = packages.filter(p => (page !== "Flatpak" || p.id.provider === "flatpak") && (page !== "Snap" || p.id.provider === "snap") && (category === "All packages" || p.category === category) && terms.every(t => [p.name,p.title,p.description,p.arch,p.source_package,p.id.provider,p.id.scope,p.repository,p.native_metadata["flatpak.branch"]].concat(p.applications.map(a => a.name)).join(" ").toLowerCase().includes(t)))
        items.sort(sort.currentIndex === 1 ? (a,b) => Number(b.installed_bytes)-Number(a.installed_bytes) : (a,b) => a.name.localeCompare(b.name))
        filtered = items
    }
    function exerciseViews() {
        navigate("Libraries")
        if (filtered.some(p => p.category !== "Libraries")) Qt.exit(6)
        navigate("Packages")
        search.text = "gcc-c++"
        if (packages.some(p => p.id.provider === "rpm" && p.name === "gcc-c++") && !filtered.some(p => p.name === "gcc-c++")) Qt.exit(7)
        const nativeSample = packages.find(p => p.id.provider === "rpm")
        if (nativeSample) selectPackage(nativeSample)
        navigate("Flatpak")
        if (filtered.some(p => p.id.provider !== "flatpak") || selected && selected.id.provider !== "flatpak") Qt.exit(9)
        if (nativeSample) selectPackage(nativeSample)
        navigate("Snap")
        if (selected && selected.id.provider !== "snap") Qt.exit(10)
        search.text = ""
        navigate("Overview")
    }
    function selectPackage(p) {
        selected = p; backend.selection = p ? identity(p) : ""; backend.reverse = ""; backend.dependency_notes = ""; backend.service_states = "[]"
        if (!p) backend.discard_plan()
    }
    InventoryBackend {
        id: backend
        onInventoryChanged: {
            const old = window.selected ? window.identity(window.selected) : ""
            window.packages = JSON.parse(inventory)
            window.selected = window.packages.find(p => window.identity(p) === old) || null
            if (!window.selected) backend.selection = ""
            window.filter()
        }
        onSystem_detailsChanged: window.systemReport = JSON.parse(system_details)
        onDisksChanged: window.volumes = JSON.parse(disks)
        onService_statesChanged: window.serviceRows = JSON.parse(service_states)
        onPlan_jsonChanged: window.removalRows = JSON.parse(plan_json)
        onPlan_readyChanged: { if (plan_ready) removalDialog.open() }
    }
    Component.onCompleted: backend.refresh()
    Timer { interval: 100; running: true; repeat: true; onTriggered: backend.poll() }
    Timer {
        interval: 250; running: backend.smoke; repeat: true
        onTriggered: {
            if (backend.busy) return
            stop()
            window.exerciseViews()
            if (backend.capture_path.length) {
                if (["Flatpak","Snap"].includes(backend.smoke_view)) window.navigate(backend.smoke_view)
                const target = ["Flatpak","Snap"].includes(backend.smoke_view) ? window.filtered.find(p => p.id.native.startsWith("app/") || p.category === "Applications") || window.filtered[0] : window.packages.find(p => p.name === "gcc-c++") || window.packages[0]
                if (target) window.selectPackage(target)
                if (["Storage","System"].includes(backend.smoke_view)) window.navigate(backend.smoke_view)
                if (backend.smoke_view === "Removal" && target && !backend.demo) backend.preview_remove(window.identity(target))
                if (backend.smoke_view === "Dependencies") dependencyDialog.open()
                captureTimer.start()
            } else { backend.smoke_check(); Qt.quit() }
        }
    }
    Timer {
        id: captureTimer; interval: 300; repeat: true
        onTriggered: {
            if (backend.busy) return
            stop()
            if (backend.smoke_view === "Removal" && !backend.plan_ready) Qt.exit(8)
            if (!backend.capture_native()) Qt.exit(3)
            backend.smoke_check(); Qt.quit()
        }
    }
    RowLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.preferredWidth: 216; Layout.fillHeight: true; color: "#eceee6"
            ColumnLayout {
                anchors.fill: parent; anchors.margins: 18; spacing: 6
                RowLayout {
                    Layout.topMargin: 14; Layout.bottomMargin: 36; spacing: 10
                    Rectangle { width: 34; height: 34; radius: 11; color: Theme.ink; Icon { anchors.centerIn: parent; glyph: Icons.archiveStackLine; color: "#dce8d5"; font.pixelSize: 21 } }
                    Text { text: "linspector"; font.family: Fonts.sans; font.pixelSize: 23; font.weight: Font.DemiBold; font.letterSpacing: -1; color: Theme.ink }
                }
                Text { text: "WORKSPACE"; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1.5; color: Theme.muted; Layout.leftMargin: 14; Layout.bottomMargin: 9 }
                Repeater {
                    model: [ {label:"Overview",icon:Icons.dashboardLine}, {label:"Applications",icon:Icons.apps2Line}, {label:"Packages",icon:Icons.box3Line}, {label:"Libraries",icon:Icons.stackLine}, {label:"Development",icon:Icons.codeSSlashLine}, {label:"Flatpak",icon:Icons.apps2Line}, {label:"Snap",icon:Icons.box3Line}, {label:"Storage",icon:Icons.hardDrive3Line}, {label:"System",icon:Icons.settings3Line} ]
                    NavItem {
                        required property var modelData
                        Layout.fillWidth: true
                        text: modelData.label; glyph: modelData.icon; selected: window.page === modelData.label
                        count: ["Overview","Storage","System"].includes(modelData.label) ? "" : modelData.label === "Flatpak" ? (window.flatpakState && window.flatpakState.complete ? window.flatpakCount.toLocaleString() : "—") : modelData.label === "Snap" ? (window.snapState && window.snapState.complete ? window.snapCount.toLocaleString() : "—") : (backend.package_ready ? window.count(modelData.label).toLocaleString() : "—")
                        onClicked: window.navigate(modelData.label)
                    }
                }
                Item { Layout.fillHeight: true }
                Rectangle {
                    Layout.fillWidth: true; implicitHeight: 106; radius: 17; color: "#e2e6da"
                    Column {
                        anchors.fill: parent; anchors.margins: 15; spacing: 8
                        Row { spacing: 7; Rectangle { width: 7; height: 7; radius: 4; color: backend.demo ? "#d7954b" : "#568563"; anchors.verticalCenter: parent.verticalCenter } Text { text: backend.demo ? "Demo inventory" : (window.platformInfo.id || "Linux") + " " + (window.platformInfo.version || ""); font.family: Fonts.sans; font.pixelSize: 12; font.bold: true; color: Theme.ink } }
                        Text { width: parent.width; text: backend.demo ? "Sample data · actions disabled" : (window.platformInfo.package_system || "Detecting platform…") + " · " + (window.platformInfo.management_model || ""); font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; wrapMode: Text.Wrap }
                        Text { text: backend.busy ? "Working…" : backend.can_execute_removal ? "Fedora management available" : "Inspection mode"; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted }
                    }
                }
                Text { Layout.topMargin: 12; Layout.leftMargin: 5; text: "ABSOLUTE MARKUP"; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1; color: "#72776b" }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; Layout.margins: 28; spacing: 18
            RowLayout {
                Layout.fillWidth: true
                ColumnLayout {
                    spacing: 3
                    Text { text: "YOUR LINUX, AT A GLANCE"; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1.8; color: Theme.muted }
                    Text { text: window.page === "Overview" ? "Your system, unpacked." : window.page; font.family: Fonts.sans; font.pixelSize: 34; font.weight: Font.Medium; font.letterSpacing: -1.3; color: Theme.ink }
                }
                Item { Layout.fillWidth: true }
                BusyIndicator { visible: backend.busy; running: backend.busy; Layout.preferredWidth: 28; Layout.preferredHeight: 28 }
                ActionButton { text: "Scan system"; glyph: Icons.refreshLine; dark: true; enabled: !backend.busy; onClicked: backend.refresh() }
            }
            RowLayout {
                Layout.fillWidth: true; Layout.preferredHeight: 214; Layout.minimumHeight: 214; Layout.maximumHeight: 214; Layout.fillHeight: false; spacing: 14; visible: window.page === "Overview"
                Surface {
                    Layout.fillWidth: true; Layout.preferredWidth: 420; Layout.fillHeight: true; color: Theme.dark
                    ColumnLayout {
                        anchors.fill: parent; anchors.margins: 24; spacing: 5
                        RowLayout { Layout.fillWidth: true; Text { text: "Installed software"; font.family: Fonts.sans; font.pixelSize: 14; color: "#c4ccc2" } Item { Layout.fillWidth: true } Icon { glyph: Icons.arrowRightUpLine; color: "#d4dfce" } }
                        RowLayout { spacing: 9; Text { text: backend.package_ready ? window.packages.length.toLocaleString() : backend.busy ? "…" : "Unavailable"; font.family: Fonts.sans; font.pixelSize: backend.package_ready ? 58 : 32; font.weight: Font.Normal; font.letterSpacing: -2.5; color: "#f8fbf5" } Text { text: backend.package_ready ? backend.provider_label + " entries" : ""; font.family: Fonts.sans; font.pixelSize: 13; color: "#a3b09f"; Layout.alignment: Qt.AlignBottom; Layout.bottomMargin: 13 } }
                        Item { Layout.fillHeight: true }
                        Row { Layout.fillWidth: true; spacing: 3
                            Repeater { model: 50; Rectangle { required property int index; width: Math.max(2,(parent.width-147)/50); height: 15; radius: 3; color: index < Math.round(50*window.appCount/Math.max(1,window.packages.length)) ? "#d5e4b8" : index < Math.round(50*(window.appCount+window.libCount)/Math.max(1,window.packages.length)) ? "#b5a5da" : index < Math.round(50*(window.appCount+window.libCount+window.devCount)/Math.max(1,window.packages.length)) ? "#eca967" : "#6d8975" } }
                        }
                        Text { text: backend.package_ready ? window.appCount + " app entries   ·   " + window.libCount + " libraries   ·   " + window.devCount + " development" : "System and Storage can be used independently"; font.family: Fonts.sans; font.pixelSize: 11; color: "#aab6a6"; Layout.topMargin: 8 }
                    }
                }
                Surface {
                    Layout.fillWidth: true; Layout.preferredWidth: 280; Layout.fillHeight: true; color: "#dce7d2"
                    ColumnLayout {
                        anchors.fill: parent; anchors.margins: 23; spacing: 6
                        RowLayout { Layout.fillWidth: true; Text { text: "Root filesystem"; font.family: Fonts.sans; font.pixelSize: 14; color: "#42523a" } Item { Layout.fillWidth: true } Icon { glyph: Icons.hardDrive3Line; color: "#576d4a" } }
                        Item { Layout.fillHeight: true }
                        Text { text: window.rootDisk ? window.bytes(window.rootDisk.available) : "Unavailable"; font.family: Fonts.sans; font.pixelSize: window.rootDisk ? 37 : 24; font.letterSpacing: -1.3; color: "#2b3b25" }
                        Text { text: "available to you"; font.family: Fonts.sans; font.pixelSize: 12; color: "#5b7050" }
                        Rectangle { Layout.fillWidth: true; height: 7; radius: 4; color: "#c0cfb5"; Layout.topMargin: 8; Rectangle { width: parent.width * (window.rootDisk ? window.rootDisk.used/Math.max(1,window.rootDisk.total) : 0); height: parent.height; radius: 4; color: "#59764b" } }
                        Text { text: window.rootDisk ? window.bytes(window.rootDisk.used) + " used of " + window.bytes(window.rootDisk.total) : "No mounted disk data in demo"; font.family: Fonts.sans; font.pixelSize: 11; color: "#607252"; Layout.topMargin: 3 }
                    }
                }
                Surface {
                    Layout.fillWidth: true; Layout.preferredWidth: 260; Layout.fillHeight: true
                    gradient: Gradient { GradientStop { position: 0; color: "#ebac68" } GradientStop { position: 1; color: "#d77d42" } }
                    Icon { anchors.right: parent.right; anchors.bottom: parent.bottom; anchors.margins: 16; glyph: Icons.stackLine; font.pixelSize: 125; color: "#4b2c19"; opacity: 0.12; rotation: -12 }
                    ColumnLayout {
                        anchors.fill: parent; anchors.margins: 23; spacing: 6
                        Text { text: "Known package footprint"; font.family: Fonts.sans; font.pixelSize: 14; color: "#57371c" }
                        Item { Layout.fillHeight: true }
                        Text { text: backend.package_ready ? window.bytes(window.packageBytes) : "Unavailable"; font.family: Fonts.sans; font.pixelSize: 40; font.letterSpacing: -1.5; color: "#382616" }
                        Text { Layout.fillWidth: true; text: "Reported footprints exclude some app data and may count shared objects more than once."; font.family: Fonts.sans; font.pixelSize: 11; color: "#674323"; wrapMode: Text.Wrap }
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true; visible: !["Storage","System"].includes(window.page)
                Text { text: "Installed software"; font.family: Fonts.sans; font.pixelSize: 21; font.weight: Font.Medium; font.letterSpacing: -0.4; color: Theme.ink }
                Text { text: window.exclusiveSourceUnavailable ? "—" : window.filtered.length.toLocaleString(); font.family: Fonts.sans; font.pixelSize: 12; color: Theme.muted }
                Item { Layout.fillWidth: true }
                ComboBox {
                    id: sort; model: ["Name A–Z","Largest first"]; font.family: Fonts.sans; font.pixelSize: 12
                    implicitWidth: 142; implicitHeight: 36; leftPadding: 12; rightPadding: 32
                    background: Rectangle { radius: 10; color: "#e9ece3"; border.width: sort.activeFocus ? 2 : 0; border.color: "#879f7f" }
                    indicator: Icon { x: sort.width-26; y: (sort.height-height)/2; glyph: Icons.arrowDownSLine; font.pixelSize: 17; color: Theme.ink }
                    contentItem: Text { text: sort.displayText; font: sort.font; color: Theme.ink; verticalAlignment: Text.AlignVCenter }
                    onCurrentIndexChanged: window.filter()
                }
            }
            Text {
                Layout.fillWidth: true; visible: !["Storage","System"].includes(window.page)
                text: window.sourceStates.map(s => s.label + ": " + (s.complete ? s.entry_count + " entries" : "unavailable")).join("   ·   ")
                font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; textFormat: Text.PlainText
            }
            Text {
                Layout.fillWidth: true; visible: window.page === "Flatpak"
                text: window.flatpakState ? (window.flatpakState.complete ? "Applications, runtimes and extensions across user and system installations. Shared objects make per-ref sizes different from unique disk use." : window.flatpakState.message) : "Flatpak discovery is unavailable here. The tool may be missing or host access restricted."
                wrapMode: Text.Wrap; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.muted; textFormat: Text.PlainText
            }
            Text {
                Layout.fillWidth: true; visible: window.page === "Snap"
                text: window.snapState ? (window.snapState.complete ? "Installed Snap revisions, bases and components. Inactive revisions remain visible; package changes are unavailable in this version." : window.snapState.message) : "Snap discovery is unavailable here. The tool or daemon may be missing, or access restricted."
                wrapMode: Text.Wrap; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.muted; textFormat: Text.PlainText
            }
            RowLayout {
                Layout.fillWidth: true; spacing: 8; visible: !["Storage","System"].includes(window.page)
                Repeater {
                    model: ["All packages","Applications","Libraries","Development"]
                    ActionButton { required property string modelData; text: modelData; compact: true; dark: window.category === modelData; onClicked: { window.category = modelData; window.filter() } }
                }
                Item { Layout.fillWidth: true }
                Rectangle {
                    Layout.preferredWidth: Math.min(280,window.width/5); height: 38; color: "white"; radius: 12
                    Icon { id: searchIcon; anchors.left: parent.left; anchors.leftMargin: 12; anchors.verticalCenter: parent.verticalCenter; glyph: Icons.searchLine; font.pixelSize: 17; color: "#8d9487" }
                    TextField { id: search; anchors.left: searchIcon.right; anchors.right: parent.right; anchors.top: parent.top; anchors.bottom: parent.bottom; placeholderText: "Find a package…"; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.ink; background: Item {} onTextChanged: window.filter(); Accessible.name: "Search installed software" }
                }
            }
            RowLayout {
                Layout.fillWidth: true; Layout.fillHeight: true; Layout.minimumHeight: 260; spacing: 14; visible: !["Storage","System"].includes(window.page)
                Surface {
                    Layout.fillWidth: true; Layout.fillHeight: true
                    ColumnLayout {
                        anchors.fill: parent; spacing: 0
                        RowLayout { Layout.fillWidth: true; Layout.margins: 17; Text { text: "PACKAGE / DESCRIPTION"; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1; color: Theme.muted } Item { Layout.fillWidth: true } Text { text: "INSTALLED SIZE"; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1; color: Theme.muted } }
                        Rectangle { height: 1; Layout.fillWidth: true; color: Theme.line }
                        ListView {
                            id: packageList; Layout.fillWidth: true; Layout.fillHeight: true; clip: true; model: window.filtered; reuseItems: true
                            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                            delegate: ItemDelegate {
                                id: packageRow; required property var modelData
                                width: ListView.view.width; height: 78
                                highlighted: window.selected && window.identity(window.selected) === window.identity(modelData)
                                background: Rectangle { color: packageRow.highlighted ? "#eef3e8" : packageRow.hovered ? "#f7f8f3" : "transparent"; Rectangle { anchors.bottom: parent.bottom; anchors.left: parent.left; anchors.right: parent.right; anchors.leftMargin: 18; height: 1; color: "#f0f1eb" } }
                                contentItem: RowLayout {
                                    spacing: 12
                                    Rectangle { Layout.preferredWidth: 40; Layout.preferredHeight: 40; radius: 13; color: window.catColor(packageRow.modelData.category); Icon { anchors.centerIn: parent; glyph: window.catIcon(packageRow.modelData.category); font.pixelSize: 21; color: "#4c594a" } }
                                    ColumnLayout {
                                        Layout.fillWidth: true; spacing: 3
                                        Text { Layout.fillWidth: true; text: packageRow.modelData.id.provider === "flatpak" ? packageRow.modelData.title : packageRow.modelData.applications.length ? packageRow.modelData.applications[0].name : packageRow.modelData.name; font.family: Fonts.sans; font.pixelSize: 14; font.weight: Font.Medium; color: Theme.ink; elide: Text.ElideRight; textFormat: Text.PlainText }
                                        Text { Layout.fillWidth: true; text: packageRow.modelData.id.provider === "flatpak" ? packageRow.modelData.name + " · " + packageRow.modelData.id.scope + " · " + packageRow.modelData.native_metadata["flatpak.branch"] : packageRow.modelData.title; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; elide: Text.ElideRight; textFormat: Text.PlainText }
                                    }
                                    ColumnLayout { spacing: 5; Text { Layout.alignment: Qt.AlignRight; text: window.bytes(packageRow.modelData.installed_bytes); font.family: Fonts.sans; font.pixelSize: 12; color: Theme.ink } Text { Layout.alignment: Qt.AlignRight; text: packageRow.modelData.id.provider.toUpperCase() + " · " + packageRow.modelData.arch; font.family: Fonts.sans; font.pixelSize: 10; color: Theme.muted } }
                                }
                                onClicked: window.selectPackage(modelData)
                                Accessible.name: modelData.title + ", " + modelData.id.provider + ", " + modelData.id.scope + ", " + modelData.name + ", " + window.bytes(modelData.installed_bytes)
                            }
                            Text { anchors.centerIn: parent; visible: packageList.count === 0; width: parent.width-50; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.Wrap; text: backend.busy ? "Scanning your system…" : window.page === "Snap" && (!window.snapState || !window.snapState.complete) ? "Snap inventory unavailable" : window.page === "Flatpak" && (!window.flatpakState || !window.flatpakState.complete) ? "Flatpak inventory unavailable" : backend.package_ready ? "No matching entries" : window.platformInfo.capabilities.inventory_reason || "Package inventory unavailable"; font.family: Fonts.sans; color: Theme.muted }
                        }
                    }
                }
                Surface {
                    Layout.preferredWidth: Math.max(310,Math.min(370,window.width*0.26)); Layout.fillHeight: true
                    ColumnLayout {
                        anchors.fill: parent; anchors.margins: 22; spacing: 13
                        RowLayout {
                            Layout.fillWidth: true
                            Rectangle { width: 42; height: 42; radius: 14; color: window.selected ? window.catColor(window.selected.category) : "#e8ebe2"; Icon { anchors.centerIn: parent; glyph: window.selected ? window.catIcon(window.selected.category) : Icons.informationLine; color: "#59664e"; font.pixelSize: 23 } }
                            Item { Layout.fillWidth: true }
                            Text { text: window.selected ? window.selected.category.toUpperCase() : "PACKAGE INSPECTOR"; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1; color: Theme.muted }
                        }
                        Text { Layout.fillWidth: true; text: window.selected ? ["flatpak","snap"].includes(window.selected.id.provider) ? window.selected.title : window.selected.name : "A closer look."; font.family: Fonts.sans; font.pixelSize: 24; font.weight: Font.Medium; font.letterSpacing: -0.8; color: Theme.ink; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                        ScrollView {
                            id: details; Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                            ColumnLayout {
                                width: details.availableWidth; spacing: 14
                                Text { Layout.fillWidth: true; text: window.selected ? window.selected.description : "Choose any package to explore its purpose, footprint and connections."; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.muted; wrapMode: Text.Wrap; textFormat: Text.PlainText }
                                Rectangle { Layout.fillWidth: true; height: 1; color: Theme.line }
                                Repeater {
                                    model: window.inspectorRows(window.selected)
                                    ColumnLayout { required property var modelData; Layout.fillWidth: true; spacing: 3; Text { text: modelData[0]; font.family: Fonts.sans; font.pixelSize: 10; color: Theme.muted } Text { Layout.fillWidth: true; text: modelData[1]; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.ink; wrapMode: Text.Wrap; textFormat: Text.PlainText } }
                                }
                                Text { visible: !!window.selected; text: "CONNECTIONS"; font.family: Fonts.sans; font.pixelSize: 10; font.letterSpacing: 1; color: Theme.muted }
                                Text { Layout.fillWidth: true; visible: !!window.selected; text: window.selected ? window.selected.requires.length + " required capabilities  ·  " + window.selected.provides.length + " provided" : ""; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.ink; wrapMode: Text.Wrap }
                                ActionButton { visible: !!window.selected; text: "Explore dependencies"; glyph: Icons.stackLine; compact: true; onClicked: dependencyDialog.open() }
                                ActionButton { visible: !!window.selected && window.selected.services.length > 0; text: "Manage service startup"; glyph: Icons.shutDownLine; compact: true; enabled: !backend.busy && !backend.demo && backend.can_control_services; onClicked: { backend.inspect_services(window.identity(window.selected)); serviceDialog.open() } }
                                Text { Layout.fillWidth: true; visible: !!window.selected && window.selected.services.length === 0; text: window.selected && window.selected.id.provider === "flatpak" ? "Flatpak applications and runtimes are inspected read-only in this version." : "No eligible systemd service was found in this package. Libraries and ordinary packages cannot be disabled."; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; wrapMode: Text.Wrap }
                            }
                        }
                        ActionButton { Layout.fillWidth: true; visible: !!window.selected; text: "Review removal"; glyph: Icons.deleteBin6Line; danger: true; enabled: !backend.busy && !backend.demo && window.selectedCanRemove; onClicked: backend.preview_remove(window.identity(window.selected)) }
                        Text { Layout.fillWidth: true; visible: !!window.selected; text: window.selectedCanRemove ? "You’ll review all affected packages before any change." : window.removalReason; font.family: Fonts.sans; font.pixelSize: 10; color: Theme.muted; wrapMode: Text.Wrap }
                    }
                }
            }
            SystemPage { Layout.fillWidth: true; Layout.fillHeight: true; visible: window.page === "System"; report: window.systemReport }
            ScrollView {
                id: storageView; visible: window.page === "Storage"; Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                ColumnLayout {
                    width: storageView.availableWidth; spacing: 16
                    Text { Layout.fillWidth: true; text: "A little breathing room."; font.family: Fonts.sans; font.pixelSize: 28; font.weight: Font.Medium; color: Theme.ink }
                    Text { Layout.fillWidth: true; text: "Live filesystem usage. Shared Btrfs volumes can report the same capacity; these figures are not added together."; font.family: Fonts.sans; font.pixelSize: 13; color: Theme.muted; wrapMode: Text.Wrap }
                    Repeater {
                        model: window.volumes
                        Surface {
                            required property var modelData; Layout.fillWidth: true; implicitHeight: 164
                            ColumnLayout {
                                anchors.fill: parent; anchors.margins: 24; spacing: 10
                                RowLayout { Layout.fillWidth: true; Icon { glyph: Icons.hardDrive3Line; font.pixelSize: 25 } Text { text: modelData.mount; font.family: Fonts.sans; font.pixelSize: 22; color: Theme.ink; textFormat: Text.PlainText } Item { Layout.fillWidth: true } Text { text: window.bytes(modelData.available) + " available"; font.family: Fonts.sans; font.pixelSize: 20; color: "#5d7950" } }
                                Text { text: modelData.source + " · " + modelData.filesystem; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.muted; textFormat: Text.PlainText }
                                Rectangle { Layout.fillWidth: true; height: 9; radius: 5; color: "#edf0e6"; Rectangle { height: parent.height; width: parent.width * modelData.used/Math.max(1,modelData.total); radius: 5; color: "#a8bd9a" } }
                                Text { text: window.bytes(modelData.used) + " used   /   " + window.bytes(modelData.total) + " total"; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.muted }
                            }
                        }
                    }
                    Surface { Layout.fillWidth: true; implicitHeight: 130; ColumnLayout { anchors.fill: parent; anchors.margins: 24; Text { text: "Where do your packages fit?"; font.family: Fonts.sans; font.pixelSize: 20; color: Theme.ink } Text { text: backend.provider_label + " reports " + window.bytes(window.packageBytes) + " across installed package entries. Shared files and filesystem compression can change actual usage."; Layout.fillWidth: true; wrapMode: Text.Wrap; font.family: Fonts.sans; font.pixelSize: 12; color: Theme.muted } ActionButton { text: "See largest packages"; glyph: Icons.arrowRightLine; compact: true; onClicked: { sort.currentIndex = 1; window.navigate("Packages") } } } }
                }
            }
            RowLayout {
                Layout.fillWidth: true; visible: backend.action_message.length > 0
                Text { Layout.fillWidth: true; text: backend.action_message; font.family: Fonts.sans; font.pixelSize: 12; color: "#8a4e2e"; wrapMode: Text.Wrap; maximumLineCount: 4; elide: Text.ElideRight; textFormat: Text.PlainText }
                ActionButton { text: "Details"; compact: true; onClicked: actionDetails.open() }
            }
            RowLayout { Layout.fillWidth: true; Rectangle { width: 6; height: 6; radius: 3; color: backend.busy ? "#d69a56" : "#7d9d71" } Text { Layout.fillWidth: true; text: backend.warnings.length ? backend.warnings : backend.status; elide: Text.ElideRight; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; textFormat: Text.PlainText } Text { text: "Designed and developed by Absolute Markup"; font.family: Fonts.sans; font.pixelSize: 10; color: "#999d93" } }
        }
    }
    Dialog {
        id: actionDetails; anchors.centerIn: parent; width: 760; height: Math.min(window.height-80,560); modal: true; title: "Operation details"; standardButtons: Dialog.Close
        ScrollView { anchors.fill: parent; clip: true; TextArea { readOnly: true; selectByMouse: true; wrapMode: Text.Wrap; textFormat: TextEdit.PlainText; text: backend.action_message; font.family: Fonts.sans; font.pixelSize: 13 } }
    }
    Dialog {
        id: removalDialog; anchors.centerIn: parent; width: 690; height: Math.min(window.height-80,650); modal: true; title: "Review removal"; closePolicy: Popup.NoAutoClose
        background: Rectangle { color: Theme.paper; radius: 22 }
        ColumnLayout {
            anchors.fill: parent; spacing: 15
            Text { Layout.fillWidth: true; text: backend.plan_label + " removal preview · " + window.removalRows.length + " package(s)"; font.family: Fonts.sans; font.pixelSize: 22; color: Theme.ink }
            Text { Layout.fillWidth: true; text: backend.can_execute_removal ? "This includes dependent packages. Unused dependencies are kept. Review the complete list; authentication follows confirmation. After authentication, an active transaction must finish before you close the app." : "This preview changes no packages. It includes dependent packages; unused dependencies are kept."; font.family: Fonts.sans; font.pixelSize: 13; color: Theme.muted; wrapMode: Text.Wrap }
            ScrollView { Layout.fillWidth: true; Layout.fillHeight: true; clip: true; ListView { model: window.removalRows; delegate: ItemDelegate { required property var modelData; width: ListView.view.width; height: 62; contentItem: Column { spacing: 4; Text { text: modelData.native_id; width: parent.width; elide: Text.ElideMiddle; font.family: Fonts.sans; font.pixelSize: 13; color: Theme.ink; textFormat: Text.PlainText } Text { text: modelData.reason === "User" ? "Selected package" : "Required by this removal · " + modelData.reason; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted } } } } }
            Text { Layout.fillWidth: true; visible: !backend.can_execute_removal; text: window.platformInfo.capabilities.removal_reason || "Management is unavailable."; wrapMode: Text.Wrap; color: Theme.muted; font.family: Fonts.sans; font.pixelSize: 12 }
            RowLayout { Layout.fillWidth: true; Item { Layout.fillWidth: true } ActionButton { text: backend.can_execute_removal ? "Cancel" : "Close preview"; onClicked: { backend.discard_plan(); removalDialog.close() } } ActionButton { text: "Remove these packages"; danger: true; enabled: backend.plan_ready && !backend.busy && backend.can_execute_removal; onClicked: { backend.confirm_remove(); removalDialog.close() } } }
        }
    }
    Dialog {
        id: dependencyDialog; anchors.centerIn: parent; width: 740; height: Math.min(window.height-70,780); modal: true; title: "Package connections"; standardButtons: Dialog.Close
        ScrollView { id: depScroll; anchors.fill: parent; clip: true; ColumnLayout { width: depScroll.availableWidth; spacing: 14
            Text { text: "Required capabilities"; font.family: Fonts.sans; font.bold: true; color: Theme.ink }
            TextArea { Layout.fillWidth: true; readOnly: true; selectByMouse: true; wrapMode: Text.Wrap; textFormat: TextEdit.PlainText; font.family: Fonts.sans; text: window.selected ? window.selected.requires.join("\n") : "" }
            Text { text: "Provided capabilities"; font.family: Fonts.sans; font.bold: true; color: Theme.ink }
            TextArea { Layout.fillWidth: true; readOnly: true; selectByMouse: true; wrapMode: Text.Wrap; textFormat: TextEdit.PlainText; font.family: Fonts.sans; text: window.selected ? window.selected.provides.join("\n") : "" }
            ActionButton { text: "Find reverse dependency candidates"; glyph: Icons.stackLine; enabled: !backend.busy && !backend.demo; onClicked: backend.find_reverse(window.identity(window.selected)) }
            TextArea { Layout.fillWidth: true; readOnly: true; selectByMouse: true; wrapMode: Text.Wrap; textFormat: TextEdit.PlainText; font.family: Fonts.sans; text: backend.reverse }
            Text { Layout.fillWidth: true; text: backend.dependency_notes || "Required and provided relationships are reported by the selected package provider. Reverse results are candidates, not proof that removal is safe."; font.family: Fonts.sans; font.pixelSize: 12; wrapMode: Text.Wrap; color: Theme.muted }
        } }
    }
    Dialog {
        id: serviceDialog; anchors.centerIn: parent; width: 670; height: 500; modal: true; title: "Service startup"; standardButtons: Dialog.Close
        ColumnLayout { anchors.fill: parent; spacing: 16
            Text { Layout.fillWidth: true; text: "Enable or disable startup for package-owned systemd services. Disabling startup leaves any currently running service running. Static, masked and unavailable services cannot be changed here."; font.family: Fonts.sans; font.pixelSize: 13; color: Theme.muted; wrapMode: Text.Wrap }
            ScrollView { Layout.fillWidth: true; Layout.fillHeight: true; clip: true; ListView { model: window.serviceRows; delegate: ItemDelegate { required property var modelData; width: ListView.view.width; height: 80; contentItem: RowLayout { ColumnLayout { Layout.fillWidth: true; Text { Layout.fillWidth: true; text: modelData.unit; elide: Text.ElideRight; font.family: Fonts.sans; color: Theme.ink; textFormat: Text.PlainText } Text { text: modelData.scope + " · " + modelData.state; font.family: Fonts.sans; font.pixelSize: 11; color: Theme.muted; textFormat: Text.PlainText } } ActionButton { text: modelData.state === "enabled" ? "Disable startup" : "Enable startup"; compact: true; enabled: !backend.busy && ["enabled","disabled"].includes(modelData.state); onClicked: { window.startupTarget = modelData; window.startupEnable = modelData.state === "disabled"; startupDialog.open() } } } } } }
            Text { visible: backend.busy; text: "Checking services…"; color: Theme.muted; font.family: Fonts.sans }
        }
    }
    Dialog {
        id: startupDialog; anchors.centerIn: parent; width: 520; modal: true; title: window.startupEnable ? "Enable startup?" : "Disable startup?"; standardButtons: Dialog.Cancel | Dialog.Ok
        Text { width: parent.width; text: window.startupTarget ? (window.startupEnable ? "Enable" : "Disable") + " startup for " + window.startupTarget.unit + " (" + window.startupTarget.scope + "). Its running state will remain unchanged. System service changes require authentication." : ""; font.family: Fonts.sans; color: Theme.ink; wrapMode: Text.Wrap; textFormat: Text.PlainText }
        onAccepted: { const s = window.startupTarget; backend.change_startup(window.identity(window.selected),s.unit,s.scope,window.startupEnable); serviceDialog.close() }
    }
}
