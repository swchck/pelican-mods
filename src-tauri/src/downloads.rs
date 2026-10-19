//! Downloads queue: fetches mod archives and hands them to the library.
//!
//! The list is small and lives only in memory; the UI gets the whole list on every
//! change as the `downloads:changed` event.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

use crate::error::AppResult;

/// Progress events are throttled to this interval per download.
const REPORT_EVERY: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum State {
    Resolving,
    Downloading {
        received: u64,
        total: u64,
    },
    Installing,
    Done,
    /// `error` is the serialized `AppError`, so the UI translates it like any other.
    Failed {
        error: serde_json::Value,
    },
}

/// One entry in the queue.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub id: u64,
    pub name: String,
    pub nexus_mod_id: Option<u64>,
    #[serde(flatten)]
    pub state: State,
}

/// All downloads of this session.
#[derive(Default)]
pub struct Downloads {
    items: Mutex<Vec<Download>>,
    next: AtomicU64,
}

impl Downloads {
    /// Adds an entry and returns its id.
    pub fn add(&self, name: String, nexus_mod_id: Option<u64>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        self.items.lock().unwrap().push(Download {
            id,
            name,
            nexus_mod_id,
            state: State::Resolving,
        });
        id
    }

    pub fn set(&self, id: u64, state: State) {
        if let Some(d) = self.items.lock().unwrap().iter_mut().find(|d| d.id == id) {
            d.state = state;
        }
    }

    pub fn rename(&self, id: u64, name: String) {
        if let Some(d) = self.items.lock().unwrap().iter_mut().find(|d| d.id == id) {
            d.name = name;
        }
    }

    pub fn list(&self) -> Vec<Download> {
        self.items.lock().unwrap().clone()
    }

    /// Drops finished and failed entries.
    pub fn clear_finished(&self) {
        self.items
            .lock()
            .unwrap()
            .retain(|d| !matches!(d.state, State::Done | State::Failed { .. }));
    }
}

/// Streams `url` into `dir`, reporting `(received, total)` at most every [`REPORT_EVERY`].
///
/// The file name comes from the URL's last path segment, which Nexus' CDN sets to the
/// uploaded archive name.
pub async fn fetch(
    client: &reqwest::Client,
    url: &str,
    dir: &Path,
    mut progress: impl FnMut(u64, u64),
) -> AppResult<PathBuf> {
    let resp = client.get(url).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(0);
    let name = file_name(resp.url());
    tokio::fs::create_dir_all(dir).await?;
    let (path, mut file) = create_unique(dir, &name).await?;
    let written: AppResult<u64> = async {
        let mut stream = resp.bytes_stream();
        let mut received = 0u64;
        let mut last = Instant::now();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            received += chunk.len() as u64;
            if last.elapsed() >= REPORT_EVERY {
                last = Instant::now();
                progress(received, total);
            }
        }
        file.flush().await?;
        Ok(received)
    }
    .await;
    match written {
        Ok(received) => {
            progress(received, total.max(received));
            Ok(path)
        }
        Err(e) => {
            // a partial archive left behind would look installable next time
            drop(file);
            let _ = tokio::fs::remove_file(&path).await;
            Err(e)
        }
    }
}

fn file_name(url: &reqwest::Url) -> String {
    let raw = url
        .path_segments()
        .and_then(|mut s| s.next_back())
        .filter(|s| !s.is_empty())
        .unwrap_or("download");
    let decoded = url::form_urlencoded::parse(raw.as_bytes())
        .map(|(k, v)| {
            if v.is_empty() {
                k.into_owned()
            } else {
                format!("{k}={v}")
            }
        })
        .collect::<String>();
    safe_file_name(&decoded)
}

// create_new, not exists-then-create: two downloads of the same file can pick a name in
// the same instant, and the second would truncate the first
async fn create_unique(dir: &Path, name: &str) -> AppResult<(PathBuf, tokio::fs::File)> {
    for i in 1u32.. {
        let path = if i == 1 {
            dir.join(name)
        } else {
            dir.join(format!("{i}-{name}"))
        };
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!("u32 names run out first")
}

/// Cleans a server-supplied file name so it can't leave the download folder.
pub fn safe_file_name(raw: &str) -> String {
    let clean: String = raw
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim_start_matches('.').trim();
    if clean.is_empty() {
        "download".into()
    } else {
        clean.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_from_cdn_urls() {
        let u = reqwest::Url::parse(
            "https://supporter-files.nexus-cdn.com/1303/541/Lookup%20Anything%201.55.0-541-1-55-0-1792.zip?md5=x&expires=1",
        )
        .unwrap();
        assert_eq!(file_name(&u), "Lookup Anything 1.55.0-541-1-55-0-1792.zip");
        let u = reqwest::Url::parse("https://x/..%2F..%2Fevil").unwrap();
        assert_eq!(file_name(&u), "_.._evil");
        let u = reqwest::Url::parse("https://x/").unwrap();
        assert_eq!(file_name(&u), "download");
    }

    #[test]
    fn tracks_queue() {
        let d = Downloads::default();
        let a = d.add("a.zip".into(), Some(1));
        let b = d.add("b.zip".into(), None);
        d.set(a, State::Done);
        d.set(
            b,
            State::Downloading {
                received: 1,
                total: 2,
            },
        );
        d.clear_finished();
        let list = d.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, b);
    }
}
