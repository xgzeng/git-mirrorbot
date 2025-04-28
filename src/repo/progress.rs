use anyhow::{anyhow, Result};
use indicatif::HumanBytes;
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub trait FetchProgressHandler {
    fn on_transfer(&mut self, p: git2::Progress);

    fn on_update_tips(&mut self, name: &str, oid_from: git2::Oid, oid_to: git2::Oid);
    fn on_sideband(&mut self, msg: &[u8]);

    fn on_pack(&mut self, stage: git2::PackBuilderStage, m: usize, n: usize);

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

        let h4 = rc_handler.clone();
        callbacks.pack_progress(move |stage, m, n| {
            h4.borrow_mut().on_pack(stage, m, n);
        });
        callbacks
    }
}

pub struct ProgressIndicator {
    indicator: ProgressBar,
    sideband_buffer: Vec<u8>,                       // sideband byte buffer
    last_transfer_update: Option<(Instant, usize)>, // for data rate calculation
}

struct SidebandProgress {
    prefix: String,
    percent: usize,
    m: usize,
    n: usize,
    done: bool,
}

lazy_static! {
    // Regex to capture:
    // 1: Prefix (any characters before the colon)
    // 2: Percentage
    // 3: Current count (m)
    // 4: Total count (n)
    // 5: Optional ", done." part
    static ref SIDEBAND_RE: regex::Regex = regex::Regex::new(
        // Match any non-colon characters for the prefix
        r"^([^:]+):\s+(\d+)%\s+\((\d+)/(\d+)\)(, done\.)?$"
    ).unwrap();
}

/// parse sideband message like:
///   "Compressing objects: 100% (129/146), done."
///   "Counting objects:  54% (108/200)"
///   "Receiving objects:  50% (10/20)"
fn parse_sideband_msg(msg: &str) -> Result<SidebandProgress> {
    if let Some(caps) = SIDEBAND_RE.captures(msg.trim()) {
        let prefix = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let percent = caps.get(2).map_or("0", |m| m.as_str()).parse::<usize>()?;
        let m = caps.get(3).map_or("0", |m| m.as_str()).parse::<usize>()?;
        let n = caps.get(4).map_or("0", |m| m.as_str()).parse::<usize>()?;
        let done = caps.get(5).is_some();

        Ok(SidebandProgress {
            prefix,
            percent,
            m,
            n,
            done,
        })
    } else {
        Err(anyhow!("unknown or malformed sideband message: {}", msg))
    }
}

impl FetchProgressHandler for ProgressIndicator {
    fn on_transfer(&mut self, p: git2::Progress) {
        self.indicator.set_length(p.total_objects() as u64);
        self.indicator.set_position(p.received_objects() as u64);

        // calculate transfer rate
        let current_bytes = p.received_bytes();
        let now = Instant::now();
        if let Some((last_time, last_bytes)) = self.last_transfer_update {
            let duration = now.duration_since(last_time);
            // make datarate calculation only if we have a reasonable time difference
            if duration < Duration::from_millis(500) {
                return;
            }
            let byte_diff = current_bytes.saturating_sub(last_bytes);
            let datarate = (byte_diff as f64 / duration.as_secs_f64()) / 1024.0; // Rate in KiB/s
            self.indicator.set_message(format!(
                "({:.1} KiB/s, {})",
                datarate,
                HumanBytes(current_bytes as u64)
            ));
            self.last_transfer_update = Some((now, current_bytes));
        } else {
            // first time we get transfer data
            self.last_transfer_update = Some((now, current_bytes));
        }
    }

    fn on_pack(&mut self, stage: git2::PackBuilderStage, m: usize, n: usize) {
        self.indicator
            .println(format!("pack: {:?} {} {}", stage, m, n));
    }

    fn on_update_tips(&mut self, name: &str, oid_from: git2::Oid, oid_to: git2::Oid) {
        if oid_from.is_zero() {
            self.indicator
                .println(format!("update refs: {} -> {}", name, oid_to));
        } else {
            self.indicator
                .println(format!("update refs: {} {} -> {}", name, oid_from, oid_to));
        }
    }

    // sideband data is continue streaming
    fn on_sideband(&mut self, bytes: &[u8]) {
        // Append new data bytes directly to the buffer
        self.sideband_buffer.extend_from_slice(bytes);

        // Process complete lines from the byte buffer
        loop {
            // Find the position of the first newline character
            let newline_pos = match self.sideband_buffer.iter().position(|&b| b == b'\n') {
                Some(pos) => pos,
                None => break, // No complete line found, exit loop
            };

            // Drain the line (including the newline) from the buffer
            // split_off leaves the remaining part in self.sideband_buffer
            let remaining = self.sideband_buffer.split_off(newline_pos + 1);
            let line_bytes = std::mem::replace(&mut self.sideband_buffer, remaining);

            // Convert the line bytes to a string (lossily)
            let line = String::from_utf8_lossy(&line_bytes);
            // Trim whitespace (including the newline itself and potential carriage returns)
            let trimmed_line = line.trim();

            if trimmed_line.is_empty() {
                continue; // Skip empty lines
            }

            // Attempt to parse the line as a progress message
            if let Ok(progress) = parse_sideband_msg(trimmed_line) {
                self.indicator.set_length(100); // Sideband progress is usually percentage based
                self.indicator.set_position(progress.percent as u64);
                // Clear transfer rate when sideband starts reporting progress
                self.last_transfer_update = None;
                if progress.done {
                    self.indicator.println(format!("{} done", progress.prefix));
                    // Reset message or set to a default after "done"
                    self.indicator.set_length(0);
                    self.indicator.set_position(0);
                    self.indicator.set_message("");
                } else {
                    self.indicator
                        .set_message(format!("{} {}/{}", progress.prefix, progress.m, progress.n));
                }
            } else {
                // If parsing fails, print the line as a regular sideband message
                self.indicator.println(format!("{}", trimmed_line));
            }
        }
        // Any remaining data in self.sideband_buffer is an incomplete line
    }
}

impl ProgressIndicator {
    pub fn new() -> Self {
        let indicator = ProgressBar::new(0); // Start with 0 length until known
        indicator.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .unwrap(),
        );
        Self {
            indicator,
            sideband_buffer: Vec::new(),
            last_transfer_update: None, // Initialize last update state
        }
    }
}

impl Drop for ProgressIndicator {
    fn drop(&mut self) {
        self.indicator.finish_and_clear();
    }
}
