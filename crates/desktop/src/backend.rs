use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use linspector_inventory::{
    model::DependencyReport,
    providers::{self, PreparedRemoval},
    review::PendingRemoval,
    services,
    system::{self, Entry, Snapshot},
};
use std::{
    pin::Pin,
    sync::mpsc::{self, Receiver, TryRecvError},
};
#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("preview.h");
        fn linspector_diagnostics();
        fn linspector_capture(path: &QString) -> bool;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, provider_label)]
        #[qproperty(QString, inventory)]
        #[qproperty(QString, source_statuses)]
        #[qproperty(QString, platform_details)]
        #[qproperty(bool, package_ready)]
        #[qproperty(bool, can_remove)]
        #[qproperty(bool, can_execute_removal)]
        #[qproperty(bool, can_control_services)]
        #[qproperty(QString, system_details)]
        #[qproperty(QString, disks)]
        #[qproperty(QString, status)]
        #[qproperty(QString, source)]
        #[qproperty(QString, warnings)]
        #[qproperty(QString, dependency_notes)]
        #[qproperty(QString, reverse)]
        #[qproperty(QString, service_states)]
        #[qproperty(QString, selection)]
        #[qproperty(QString, action_message)]
        #[qproperty(QString, plan_label)]
        #[qproperty(QString, plan_json)]
        #[qproperty(QString, smoke_view)]
        #[qproperty(QString, capture_path)]
        #[qproperty(bool, plan_ready)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, action_running)]
        #[qproperty(bool, smoke)]
        #[qproperty(bool, demo)]
        type InventoryBackend = super::Backend;
        #[qinvokable]
        fn smoke_check(&self);
        #[qinvokable]
        fn capture_native(&self) -> bool;
        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);
        #[qinvokable]
        fn poll(self: Pin<&mut Self>);
        #[qinvokable]
        fn find_reverse(self: Pin<&mut Self>, identity: &QString);
        #[qinvokable]
        fn inspect_services(self: Pin<&mut Self>, identity: &QString);
        #[qinvokable]
        fn preview_remove(self: Pin<&mut Self>, identity: &QString);
        #[qinvokable]
        fn discard_plan(self: Pin<&mut Self>);
        #[qinvokable]
        fn confirm_remove(self: Pin<&mut Self>);
        #[qinvokable]
        fn change_startup(
            self: Pin<&mut Self>,
            identity: &QString,
            unit: &QString,
            scope: &QString,
            enable: bool,
        );
    }
}
enum Event {
    Scan(Result<Snapshot, String>),
    Reverse(String, Result<DependencyReport, String>),
    Services(String, Vec<serde_json::Value>),
    Plan(u64, Result<Box<dyn PreparedRemoval>, String>),
    Action(Result<String, String>),
}
pub struct Backend {
    inventory: QString,
    source_statuses: QString,
    provider_label: QString,
    disks: QString,
    system_details: QString,
    platform_details: QString,
    package_ready: bool,
    can_remove: bool,
    can_execute_removal: bool,
    can_control_services: bool,
    status: QString,
    source: QString,
    warnings: QString,
    reverse: QString,
    dependency_notes: QString,
    service_states: QString,
    selection: QString,
    action_message: QString,
    plan_json: QString,
    plan_label: QString,
    capture_path: QString,
    smoke_view: QString,
    plan_ready: bool,
    busy: bool,
    action_running: bool,
    smoke: bool,
    demo: bool,
    entries: Vec<Entry>,
    receiver: Option<Receiver<Event>>,
    review: PendingRemoval,
}
fn qs(s: impl AsRef<str>) -> QString {
    QString::from(s.as_ref())
}
impl Default for Backend {
    fn default() -> Self {
        let args: Vec<_> = std::env::args().collect();
        let capture = args
            .iter()
            .position(|s| s == "--capture")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_default();
        Self {
            inventory: qs("[]"),
            source_statuses: qs("[]"),
            provider_label: qs("Software"),
            disks: qs("[]"),
            system_details: qs("{}"),
            platform_details: qs(
                serde_json::to_string(&linspector_inventory::platform::detect()).unwrap(),
            ),
            package_ready: false,
            can_remove: false,
            can_execute_removal: false,
            can_control_services: false,
            status: qs("Ready to scan"),
            source: qs("Live system"),
            warnings: qs(""),
            reverse: qs(""),
            dependency_notes: qs(""),
            service_states: qs("[]"),
            selection: qs(""),
            action_message: qs(""),
            plan_json: qs("[]"),
            plan_label: qs("Package manager"),
            capture_path: qs(capture),
            smoke_view: qs(args
                .iter()
                .position(|s| s == "--smoke-view")
                .and_then(|i| args.get(i + 1))
                .map(String::as_str)
                .unwrap_or("Overview")),
            plan_ready: false,
            busy: false,
            action_running: false,
            smoke: args.iter().any(|s| s == "--smoke"),
            demo: args.iter().any(|s| s == "--demo"),
            entries: vec![],
            receiver: None,
            review: PendingRemoval::default(),
        }
    }
}
pub fn diagnostics() {
    ffi::linspector_diagnostics();
}
impl ffi::InventoryBackend {
    pub fn capture_native(&self) -> bool {
        if !*self.smoke() || self.capture_path().to_string().is_empty() {
            return false;
        }
        let saved = ffi::linspector_capture(self.capture_path());
        println!("Native screenshot saved: {saved}");
        saved
    }
    pub fn smoke_check(&self) {
        if !*self.smoke() {
            return;
        }
        let p = &self.rust().entries;
        println!(
            "Linspector native smoke: {} entries; apps {}; libraries {}; source {}",
            p.len(),
            p.iter().filter(|e| e.category == "Applications").count(),
            p.iter().filter(|e| e.category == "Libraries").count(),
            self.source()
        );
        println!("Linspector sources: {}", self.source_statuses());
        if *self.busy() || self.system_details().to_string() == "{}" {
            std::process::exit(2);
        }
    }
    fn selected_entry(&self, id: &QString) -> Option<Entry> {
        self.rust()
            .entries
            .iter()
            .find(|e| e.package.identity() == id.to_string())
            .cloned()
    }
    fn channel(mut self: Pin<&mut Self>) -> mpsc::Sender<Event> {
        let (tx, rx) = mpsc::channel();
        self.as_mut().rust_mut().receiver = Some(rx);
        self.as_mut().set_busy(true);
        tx
    }
    pub fn refresh(mut self: Pin<&mut Self>) {
        if *self.busy() {
            return;
        }
        self.as_mut().discard_plan();
        self.as_mut().set_can_remove(false);
        self.as_mut().set_can_execute_removal(false);
        self.as_mut().set_can_control_services(false);
        let demo = *self.demo();
        self.as_mut()
            .set_status(qs("Scanning installed software and storage…"));
        let tx = self.as_mut().channel();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Scan(if demo {
                system::demo()
            } else {
                system::scan()
            }));
        });
    }
    pub fn poll(mut self: Pin<&mut Self>) {
        let result = match self.rust().receiver.as_ref() {
            Some(rx) => rx.try_recv(),
            None => return,
        };
        let event = match result {
            Ok(e) => e,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                self.as_mut().rust_mut().receiver = None;
                self.as_mut().set_busy(false);
                let changing = *self.action_running();
                self.as_mut().set_action_running(false);
                self.as_mut().set_can_remove(false);
                self.as_mut().set_can_execute_removal(false);
                self.as_mut().set_can_control_services(false);
                self.as_mut().set_package_ready(false);
                self.as_mut().set_status(qs(
                    "Background task stopped unexpectedly; scan again before changing software.",
                ));
                if changing {
                    self.as_mut().set_action_message(qs("The operation stopped unexpectedly. State may have changed; refreshing before another review."));
                    self.as_mut().refresh();
                }
                return;
            }
        };
        self.as_mut().rust_mut().receiver = None;
        self.as_mut().set_busy(false);
        match event {
            Event::Scan(Ok(s)) => {
                self.as_mut().set_provider_label(qs(&s.provider_label));
                self.as_mut()
                    .set_platform_details(qs(serde_json::to_string(&s.platform).unwrap()));
                self.as_mut()
                    .set_package_ready(s.sources.iter().any(|source| source.complete));
                self.as_mut()
                    .set_source_statuses(qs(serde_json::to_string(&s.sources).unwrap()));
                self.as_mut().set_can_remove(
                    s.platform.capabilities.package_inventory
                        && s.platform.capabilities.removal_preview,
                );
                self.as_mut().set_can_execute_removal(
                    s.platform.capabilities.package_inventory
                        && s.platform.capabilities.package_removal,
                );
                self.as_mut().set_can_control_services(
                    s.platform.capabilities.package_inventory
                        && s.platform.capabilities.service_startup,
                );
                self.as_mut()
                    .set_system_details(qs(serde_json::to_string(&s.host).unwrap()));
                self.as_mut()
                    .set_disks(qs(serde_json::to_string(&s.disks).unwrap()));
                self.as_mut().set_source(qs(&s.source));
                self.as_mut().set_warnings(qs(s.warnings.join("\n")));
                self.as_mut().set_status(qs(if s.inventory_complete {
                    format!(
                        "{} {} entries · scan complete",
                        s.entries.len(),
                        s.provider_label
                    )
                } else {
                    format!(
                        "{} entries · partial scan; one or more software sources are unavailable",
                        s.entries.len()
                    )
                }));
                self.as_mut()
                    .set_inventory(qs(serde_json::to_string(&s.entries).unwrap()));
                self.as_mut().rust_mut().entries = s.entries;
            }
            Event::Scan(Err(e)) => {
                self.as_mut().set_package_ready(false);
                self.as_mut().set_inventory(qs("[]"));
                self.as_mut().rust_mut().entries.clear();
                self.as_mut().set_status(qs(format!("Scan failed: {e}")));
            }
            Event::Reverse(id, result) => {
                if self.selection().to_string() == id {
                    match result {
                        Ok(report) => {
                            self.as_mut()
                                .set_dependency_notes(qs(report.limitations.join("\n")));
                            let rows: Vec<_> = report
                                .reverse_candidates
                                .into_iter()
                                .map(|id| id.native)
                                .collect();
                            let text = if rows.is_empty() {
                                "No direct candidates reported. This does not establish removal safety.".into()
                            } else {
                                rows.join("\n")
                            };
                            self.as_mut().set_reverse(qs(text));
                        }
                        Err(e) => self.as_mut().set_reverse(qs(format!("Query failed: {e}"))),
                    }
                }
            }
            Event::Services(id, states) => {
                if self.selection().to_string() == id {
                    self.as_mut()
                        .set_service_states(qs(serde_json::to_string(&states).unwrap()));
                }
            }
            Event::Plan(generation, Ok(p)) => {
                let selection = self.selection().to_string();
                if !self
                    .as_mut()
                    .rust_mut()
                    .review
                    .accept(generation, &selection, p)
                {
                    self.as_mut().set_action_message(qs(
                        "Preview was cancelled or selection changed. Review a new plan.",
                    ));
                    return;
                }
                let review = self.rust().review.review().unwrap();
                let label = review.executor_label.clone();
                let json = serde_json::to_string(&review.changes).unwrap();
                self.as_mut().set_plan_label(qs(label));
                self.as_mut().set_plan_json(qs(json));
                self.as_mut().set_plan_ready(true);
                self.as_mut().set_action_message(qs(
                    "Review every package in the removal plan before continuing.",
                ));
            }
            Event::Plan(_, Err(e)) => self.as_mut().set_action_message(qs(e)),
            Event::Action(Ok(msg)) => {
                self.as_mut().set_action_running(false);
                self.as_mut().set_action_message(qs(msg));
                self.as_mut().refresh();
            }
            Event::Action(Err(e)) => {
                self.as_mut().set_action_running(false);
                self.as_mut().set_action_message(qs(e));
                // Failure/cancellation may follow partial native changes: always observe afresh.
                self.as_mut().refresh();
            }
        }
    }
    pub fn find_reverse(mut self: Pin<&mut Self>, id: &QString) {
        if *self.busy() || *self.demo() {
            return;
        }
        let Some(e) = self.selected_entry(id) else {
            return;
        };
        let key = id.to_string();
        self.as_mut()
            .set_reverse(qs("Checking package dependency capabilities…"));
        let tx = self.as_mut().channel();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Reverse(
                key,
                providers::for_package(&linspector_inventory::platform::detect(), &e.package.id)
                    .and_then(|provider| provider.dependencies(&e.package.id, true))
                    .map_err(|e| e.to_string()),
            ));
        });
    }
    pub fn inspect_services(mut self: Pin<&mut Self>, id: &QString) {
        if *self.busy() || *self.demo() {
            return;
        }
        let Some(e) = self.selected_entry(id) else {
            return;
        };
        let key = id.to_string();
        self.as_mut().set_service_states(qs("[]"));
        let tx = self.as_mut().channel();
        std::thread::spawn(move || {
            let states=e.services.iter().map(|s|serde_json::json!({"unit":s.unit,"scope":s.scope,"state":services::service_state(s).unwrap_or_else(|e|format!("Unavailable: {e}"))})).collect();
            let _ = tx.send(Event::Services(key, states));
        });
    }
    pub fn preview_remove(mut self: Pin<&mut Self>, id: &QString) {
        if *self.busy() || *self.demo() || !*self.can_remove() {
            return;
        }
        let Some(e) = self.selected_entry(id) else {
            return;
        };
        if !e.capabilities.removal_preview {
            self.as_mut().set_action_message(qs(
                "This software source does not support removal previews.",
            ));
            return;
        }
        self.as_mut().discard_plan();
        self.as_mut().set_action_message(qs(
            "Preparing a dependency-aware removal plan. No packages are being changed.",
        ));
        let generation = self.rust().review.generation();
        let tx = self.as_mut().channel();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Plan(
                generation,
                providers::for_package(&linspector_inventory::platform::detect(), &e.package.id)
                    .and_then(|provider| provider.prepare_removal(&e.package.id))
                    .map_err(|e| e.to_string()),
            ));
        });
    }
    pub fn discard_plan(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().review.invalidate();
        self.as_mut().set_plan_ready(false);
        self.as_mut().set_plan_json(qs("[]"));
    }
    pub fn confirm_remove(mut self: Pin<&mut Self>) {
        if *self.busy() || *self.demo() || !*self.can_execute_removal() {
            return;
        }
        self.as_mut().set_plan_ready(false);
        let selection = self.selection().to_string();
        let p = match self.as_mut().rust_mut().review.take(&selection) {
            Ok(plan) => plan,
            Err(error) => {
                self.as_mut().set_action_message(qs(error));
                return;
            }
        };
        self.as_mut().set_action_message(qs(
            "Waiting for authentication or the package manager to finish…",
        ));
        self.as_mut().set_action_running(true);
        let tx = self.as_mut().channel();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Action(p.execute().map_err(|e| e.to_string())));
        });
    }
    pub fn change_startup(
        mut self: Pin<&mut Self>,
        id: &QString,
        unit: &QString,
        scope: &QString,
        enable: bool,
    ) {
        if *self.busy() || *self.demo() || !*self.can_control_services() {
            return;
        }
        let Some(e) = self.selected_entry(id) else {
            return;
        };
        let Some(s) = e
            .services
            .iter()
            .find(|s| s.unit == unit.to_string() && s.scope == scope.to_string())
            .cloned()
        else {
            return;
        };
        self.as_mut()
            .set_action_message(qs("Applying service startup change…"));
        self.as_mut().set_action_running(true);
        let tx = self.as_mut().channel();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Action(services::change_startup(
                &e.package.id,
                &s,
                enable,
            )));
        });
    }
}
