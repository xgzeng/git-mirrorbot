use std::cell::RefCell;
use std::rc::Rc;

pub trait FetchProgressHandler {
    fn on_transfer(&mut self, p: git2::Progress);
    fn on_update_tips(&mut self, name: &str, oid_from: git2::Oid, oid_to: git2::Oid);
    fn on_sideband(&mut self, msg: &[u8]);

    fn as_remote_callbacks<'a>(&'a mut self) -> git2::RemoteCallbacks<'a> {
        let mut callbacks = git2::RemoteCallbacks::new();

        let rc_handler = Rc::new(RefCell::new(self));

        let h1 = rc_handler.clone();
        callbacks.sideband_progress(move |msg: &[u8]| {
            h1.borrow_mut().on_sideband(msg);
            true
        });

        let h2 = rc_handler.clone();
        callbacks.transfer_progress(move |p| {
            h2.borrow_mut().on_transfer(p);
            true
        });

        let h3 = rc_handler.clone();
        callbacks.update_tips(move |name, oid_from, oid_to| {
            h3.borrow_mut().on_update_tips(name, oid_from, oid_to);
            true
        });

        // callbacks.pack_progress(|stage, m, n| {
        //     log::info!("pack_progress: {:?} {} {}", stage, m, n);
        // });
        callbacks
    }
}

pub struct LogFetchProgress {}

impl FetchProgressHandler for LogFetchProgress {
    fn on_transfer(&mut self, p: git2::Progress) {
        log::info!(
            "objects: total {}, received {},",
            p.total_objects(),
            p.received_objects()
        );
    }

    fn on_update_tips(&mut self, name: &str, oid_from: git2::Oid, oid_to: git2::Oid) {
        log::info!("update_tips: {} {} {}", name, oid_from, oid_to);
    }

    fn on_sideband(&mut self, msg: &[u8]) {
        log::info!("sideband_progress: {}", String::from_utf8_lossy(msg));
    }
}

pub struct ProgressIndicator {
    indicator: indicatif::ProgressBar,
}
// let indicator = ;
// let pcb = move |total: usize, received: usize| {
// };

impl FetchProgressHandler for ProgressIndicator {
    fn on_transfer(&mut self, p: git2::Progress) {
        // log::info!(
        //     "objects: total {}, received {},",
        //     p.total_objects(),
        //     p.received_objects()
        // );

        let total_u64 = p.total_objects().try_into().unwrap();
        if total_u64 != self.indicator.length() {
            self.indicator.set_length(total_u64);
        }

        let received_u64 = p.received_objects().try_into().unwrap();
        self.indicator.set_position(received_u64);
    }

    fn on_update_tips(&mut self, name: &str, oid_from: git2::Oid, oid_to: git2::Oid) {
        log::info!("update_tips: {} {} {}", name, oid_from, oid_to);
    }

    fn on_sideband(&mut self, msg: &[u8]) {
        log::info!("sideband_progress: {}", String::from_utf8_lossy(msg));
    }
}

impl ProgressIndicator {
    pub fn new() -> Self {
        ProgressIndicator {
            indicator: indicatif::ProgressBar::new(100),
        }
    }
}
