use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BLACKLIST_DURATION: u64 = 24 * 60 * 60; // 24 hours in seconds

pub struct BlacklistManager {
    file_path: PathBuf,
    blacklisted: HashMap<String, u64>, // Sender -> Expiration time (UNIX timestamp)
    invalid_counts: HashMap<String, (u32, u64)>, // Sender -> (Count, Window Start UNIX timestamp)
}

impl BlacklistManager {
    pub fn new(file_path: PathBuf) -> Self {
        let mut manager = Self {
            file_path,
            blacklisted: HashMap::new(),
            invalid_counts: HashMap::new(),
        };
        manager.load();
        manager
    }

    fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    }

    fn load(&mut self) {
        if let Ok(mut file) = File::open(&self.file_path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                for line in contents.lines() {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() == 2 {
                        if let Ok(expiry) = parts[1].parse::<u64>() {
                            if expiry > Self::now() {
                                self.blacklisted.insert(parts[0].to_string(), expiry);
                            }
                        }
                    }
                }
            }
        }
    }

    fn save(&self) {
        if let Ok(mut file) = File::create(&self.file_path) {
            for (sender, expiry) in &self.blacklisted {
                if *expiry > Self::now() {
                    let _ = writeln!(file, "{},{}", sender, expiry);
                }
            }
        }
    }

    pub fn is_blacklisted(&mut self, sender: &str) -> bool {
        let now = Self::now();
        if let Some(&expiry) = self.blacklisted.get(sender) {
            if expiry > now {
                return true;
            } else {
                self.blacklisted.remove(sender);
                self.save();
            }
        }
        false
    }

    pub fn record_invalid(&mut self, sender: &str) -> bool {
        let now = Self::now();
        let entry = self.invalid_counts.entry(sender.to_string()).or_insert((0, now));

        if now - entry.1 > 60 {
            entry.0 = 1;
            entry.1 = now;
        } else {
            entry.0 += 1;
        }

        if entry.0 > 500 {
            self.blacklisted.insert(sender.to_string(), now + BLACKLIST_DURATION);
            self.save();
            self.invalid_counts.remove(sender);
            return true; // Newly blacklisted
        }
        false
    }
}
