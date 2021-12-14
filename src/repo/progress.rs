use indicatif::{ProgressBar, ProgressStyle};
use std::cell::RefCell;
use std::rc::Rc;

pub trait FetchProgressHandler {
    fn on_transfer(&mut self, p: git2::Progress);

    fn on_update_tips(&mut self, name: &str, oid_from: git2::Oid, oid_to: git2::Oid);
    fn on_sideband(&mut self, msg: &[u8]);

    fn as_remote_callbacks(&mut self) -> git2::RemoteCallbacks<'_> {
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

// pub struct LogFetchProgress {}

// impl FetchProgressHandler for LogFetchProgress {
//     fn on_transfer(&mut self, p: git2::Progress) {
//         log::info!(
//             "objects: total {}, received {},",
//             p.total_objects(),
//             p.received_objects()
//         );
//     }

//     fn on_update_tips(&mut self, name: &str, oid_from: git2::Oid, oid_to: git2::Oid) {
//         log::info!("update_tips: {} {} {}", name, oid_from, oid_to);
//     }

//     fn on_sideband(&mut self, msg: &[u8]) {
//         log::info!("sideband_progress: {}", String::from_utf8_lossy(msg));
//     }
// }

enum ProgressStage {
    NotStarted,
    Download,
    Sideband,
}

pub struct ProgressIndicator {
    indicator: ProgressBar,

    stage: ProgressStage,
}

impl FetchProgressHandler for ProgressIndicator {
    fn on_transfer(&mut self, p: git2::Progress) {
        match self.stage {
            ProgressStage::Download => (),
            _ => {
                self.stage = ProgressStage::Download;
                self.indicator
                    .set_style(ProgressStyle::default_bar().template(
                        "[{elapsed_precise}] {bar:40.cyan/blue} Downloading {pos:>7}/{len:7}",
                    ));
            }
        }

        let total = p.total_objects() as u64;
        let received = p.received_objects() as u64;

        if total != self.indicator.length() {
            self.indicator.set_length(total);
        }

        self.indicator.set_position(received);
    }

    fn on_update_tips(&mut self, name: &str, oid_from: git2::Oid, oid_to: git2::Oid) {
        log::info!("update_tips: {} {} {}", name, oid_from, oid_to);
    }

    fn on_sideband(&mut self, bytes: &[u8]) {
        match self.stage {
            ProgressStage::Sideband => (),
            _ => {
                self.stage = ProgressStage::Sideband;
                self.indicator
                    .set_style(ProgressStyle::default_bar().template("[{elapsed_precise}] {msg}"));
            }
        }

        let msg = String::from_utf8_lossy(bytes);
        // log::info!("remote: {}", msg);
        self.indicator.set_message(msg.into_owned());
        self.indicator.tick();
    }
}

impl ProgressIndicator {
    pub fn new() -> Self {
        let ind = ProgressBar::new(100);
        ind.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}"),
        );
        ProgressIndicator {
            indicator: ind,
            stage: ProgressStage::NotStarted,
        }
    }

    pub fn hide(&mut self) {
        self.indicator.finish_and_clear();
    }
}
