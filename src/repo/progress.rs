use indicatif::{ProgressBar, ProgressStyle};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

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
    Download(u64, Instant), // received bytes, receive_time
    Sideband,
}

pub struct ProgressIndicator {
    indicator: ProgressBar,
    stage: ProgressStage,
}

impl FetchProgressHandler for ProgressIndicator {
    fn on_transfer(&mut self, p: git2::Progress) {
        match self.stage {
            ProgressStage::Download(prev_received_bytes, prev_recv_time) => {
                let received = p.received_objects() as u64;
                let new_recv_time = Instant::now();
                let duration = new_recv_time.duration_since(prev_recv_time).as_secs();
                if duration != 0 {
                    let datarate = received - prev_received_bytes;
                    self.stage = ProgressStage::Download(received, new_recv_time);
                    self.indicator.set_message(format!("{}B/s", datarate));
                }
                self.indicator.set_position(received);
            }
            _ => {
                // enter download state
                self.reset(); // recreate indicator
                              // self.indicator.reset();
                              // self.indicator.finish_and_clear(); // clear indicator
                log::info!(
                    "download objects: total {}, total_deltas {}, local {}",
                    p.total_objects(),
                    p.total_deltas(),
                    p.local_objects()
                );
                self.stage = ProgressStage::Download(0, Instant::now());
                self.indicator.set_length(p.total_objects() as u64);
                self.indicator
                    .set_style(ProgressStyle::default_bar().template(
                        "[{elapsed_precise}] {bar:40.cyan/blue} Objects {pos:>7}/{len:7}, {msg}",
                    ));
            }
        }
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

    fn reset(&mut self) {
        self.indicator = ProgressBar::new(100);
    }

    // pub fn hide(&mut self) {
    //     self.indicator.finish_and_clear();
    // }
}
