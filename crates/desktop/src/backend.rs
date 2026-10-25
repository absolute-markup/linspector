use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use linspector_inventory::Package;
use std::{
    pin::Pin,
    sync::mpsc::{self, Receiver, TryRecvError},
};

#[cxx_qt::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, inventory)]
        #[qproperty(QString, status)]
        #[qproperty(QString, reverse)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, smoke)]
        type InventoryBackend = super::Backend;
        #[qinvokable]
        fn smoke_check(&self);
        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);
        #[qinvokable]
        fn poll(self: Pin<&mut Self>);
        #[qinvokable]
        fn find_reverse(self: Pin<&mut Self>, identity: &QString);
    }
}
enum Event {
    Inventory(Result<Vec<Package>, String>),
    Reverse(Result<Vec<String>, String>),
}
pub struct Backend {
    inventory: QString,
    status: QString,
    reverse: QString,
    busy: bool,
    smoke: bool,
    packages: Vec<Package>,
    receiver: Option<Receiver<Event>>,
}
impl Default for Backend {
    fn default() -> Self {
        Self {
            inventory: QString::from("[]"),
            status: QString::from("Ready — read-only RPM inspection"),
            reverse: QString::from("Select ‘Find reverse dependencies’ to query RPM."),
            busy: false,
            smoke: std::env::args().any(|s| s == "--smoke"),
            packages: vec![],
            receiver: None,
        }
    }
}
impl ffi::InventoryBackend {
    pub fn smoke_check(&self) {
        if !*self.smoke() {
            return;
        }
        let count = self.rust().packages.len();
        println!(
            "Linspector native smoke: {} packages; {}",
            count,
            self.status()
        );
        if *self.busy() || count == 0 {
            std::process::exit(2);
        }
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        if *self.busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.as_mut().rust_mut().receiver = Some(rx);
        self.as_mut().set_busy(true);
        self.as_mut()
            .set_status(QString::from("Reading all installed RPM packages…"));
        std::thread::spawn(move || {
            let result = if std::env::args().any(|s| s == "--demo") {
                serde_json::from_str(include_str!("../../../fixtures/sample-inventory.json"))
                    .map_err(|e| e.to_string())
            } else {
                linspector_inventory::installed()
            };
            let _ = tx.send(Event::Inventory(result));
        });
    }
    pub fn poll(mut self: Pin<&mut Self>) {
        let result = match self.rust().receiver.as_ref() {
            Some(rx) => rx.try_recv(),
            None => return,
        };
        match result {
            Ok(event) => {
                self.as_mut().rust_mut().receiver = None;
                self.as_mut().set_busy(false);
                match event {
                    Event::Inventory(Ok(p)) => {
                        self.as_mut().set_inventory(QString::from(
                            serde_json::to_string(&p).unwrap().as_str(),
                        ));
                        self.as_mut().set_status(QString::from(
                            format!("{} packages · all architectures · read-only", p.len())
                                .as_str(),
                        ));
                        self.as_mut().rust_mut().packages = p;
                    }
                    Event::Reverse(Ok(rows)) => self.as_mut().set_reverse(QString::from(
                        if rows.is_empty() {
                            "No direct candidates reported. This does not establish removal safety."
                                .into()
                        } else {
                            rows.join("\n")
                        }
                        .as_str(),
                    )),
                    Event::Inventory(Err(e)) => self
                        .as_mut()
                        .set_status(QString::from(format!("Inventory failed: {e}").as_str())),
                    Event::Reverse(Err(e)) => self
                        .as_mut()
                        .set_reverse(QString::from(format!("Reverse query failed: {e}").as_str())),
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.as_mut().rust_mut().receiver = None;
                self.as_mut().set_busy(false);
                self.as_mut()
                    .set_status(QString::from("Inventory worker stopped unexpectedly"));
            }
        }
    }
    pub fn find_reverse(mut self: Pin<&mut Self>, identity: &QString) {
        if *self.busy() {
            return;
        }
        let Some(p) = self
            .rust()
            .packages
            .iter()
            .find(|p| p.identity() == identity.to_string())
            .cloned()
        else {
            return;
        };
        if p.provenance.starts_with("Synthetic") {
            self.as_mut()
                .set_reverse(QString::from("Unavailable for synthetic demo data."));
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.as_mut().rust_mut().receiver = Some(rx);
        self.as_mut().set_busy(true);
        self.as_mut().set_reverse(QString::from("Querying RPM…"));
        std::thread::spawn(move || {
            let _ = tx.send(Event::Reverse(linspector_inventory::reverse_candidates(&p)));
        });
    }
}
