//! Running the game as a child process and relaying its console.
//!
//! SMAPI is started directly (`./StardewModdingAPI`), not through the `StardewValley`
//! script: on macOS that script reopens itself in Terminal.app when stdout is not a tty,
//! which would detach the process and lose the log.
//!
//! With SMAPI the console comes from its log file rather than stdout: SMAPI prints the
//! `[HH:MM:SS LEVEL source]` stamp only to the file and writes a bare `[source]` to the
//! console, so stdout alone cannot tell an error from chatter.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::watch;

use crate::error::{AppError, AppResult};
use crate::game;
use crate::rosetta;

/// Lines of console output kept for a UI that opens after the game started.
const LOG_CAPACITY: usize = 2000;

/// How often the SMAPI log file is polled for new lines.
const TAIL_INTERVAL: Duration = Duration::from_millis(250);

/// How long output readers may keep going after the game exits.
const READER_GRACE: Duration = Duration::from_secs(3);

/// Severity of a console line, as SMAPI names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Alert,
}

impl Level {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "TRACE" => Self::Trace,
            "DEBUG" => Self::Debug,
            "INFO" => Self::Info,
            "WARN" => Self::Warn,
            "ERROR" => Self::Error,
            "ALERT" => Self::Alert,
            _ => return None,
        })
    }
}

/// One console line, emitted as the `game:log` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub seq: u64,
    pub level: Level,
    pub text: String,
}

/// What the game process is doing, emitted as the `game:state` event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum RunState {
    Idle,
    Running { modded: bool, pid: Option<u32> },
    Exited { code: Option<i32> },
}

/// How to run SMAPI: where its log is and which folder to load mods from.
pub struct SmapiRun {
    pub log: PathBuf,
    /// Passed as `--mods-path`; `None` keeps SMAPI's default `Mods` in the game folder.
    pub mods_path: Option<PathBuf>,
    /// Extra environment for the game process.
    pub env: Vec<(String, String)>,
}

#[derive(Default)]
struct Inner {
    stdin: Option<ChildStdin>,
    kill: Option<tokio::sync::oneshot::Sender<()>>,
    log: VecDeque<LogLine>,
    seq: u64,
    // stack traces and wrapped messages continue the stamped line above them
    last_level: Option<Level>,
    state: Option<RunState>,
}

impl Inner {
    fn push(&mut self, level: Level, text: String) -> LogLine {
        self.seq += 1;
        let line = LogLine {
            seq: self.seq,
            level,
            text,
        };
        if self.log.len() == LOG_CAPACITY {
            self.log.pop_front();
        }
        self.log.push_back(line.clone());
        line
    }
}

/// Owns at most one running game process.
#[derive(Clone, Default)]
pub struct Launcher {
    inner: Arc<Mutex<Inner>>,
}

/// Callbacks the launcher reports through, so it stays free of Tauri types.
pub trait Sink: Send + Sync + 'static {
    fn log(&self, line: &LogLine);
    fn state(&self, state: &RunState);
}

#[derive(Clone, Copy)]
enum Source {
    Stdout,
    Stderr,
    // stdout of a SMAPI run: its own lines are already in the log file
    SmapiStdout,
    SmapiFile,
}

impl Launcher {
    /// Returns the current process state.
    pub fn state(&self) -> RunState {
        self.inner
            .lock()
            .unwrap()
            .state
            .clone()
            .unwrap_or(RunState::Idle)
    }

    /// Returns the buffered console lines, oldest first.
    pub fn log(&self) -> Vec<LogLine> {
        self.inner.lock().unwrap().log.iter().cloned().collect()
    }

    /// Starts the game from `game_dir`: through SMAPI when `smapi` is given, reading
    /// the console from its log file, or vanilla otherwise.
    pub fn launch(
        &self,
        game_dir: &Path,
        smapi: Option<SmapiRun>,
        sink: impl Sink,
    ) -> AppResult<RunState> {
        let mut inner = self.inner.lock().unwrap();
        if matches!(inner.state, Some(RunState::Running { .. })) {
            return Err(AppError::AlreadyRunning);
        }
        let modded = smapi.is_some();
        let exe = if modded {
            if !game::has_smapi(game_dir) {
                return Err(AppError::SmapiMissing);
            }
            game::SMAPI_EXE
        } else {
            game::VANILLA_EXE
        };
        let started = SystemTime::now();
        let mut cmd = Command::new(game_dir.join(exe));
        if let Some(s) = &smapi {
            if let Some(mods) = &s.mods_path {
                cmd.arg("--mods-path").arg(mods);
            }
            cmd.envs(s.env.iter().map(|(k, v)| (k, v)));
        }
        let mut child = cmd
            .current_dir(game_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(false)
            .spawn()
            .map_err(rosetta::classify)?;

        let state = RunState::Running {
            modded,
            pid: child.id(),
        };
        let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();
        inner.stdin = child.stdin.take();
        inner.kill = Some(kill_tx);
        inner.log.clear();
        inner.last_level = None;
        inner.state = Some(state.clone());
        drop(inner);

        let sink = Arc::new(sink);
        sink.state(&state);
        let (exited_tx, exited_rx) = watch::channel(false);
        let stdout = if modded {
            Source::SmapiStdout
        } else {
            Source::Stdout
        };
        let mut readers: Vec<JoinHandle<()>> = [
            self.pump(child.stdout.take(), stdout, sink.clone()),
            self.pump(child.stderr.take(), Source::Stderr, sink.clone()),
        ]
        .into_iter()
        .flatten()
        .collect();
        if let Some(run) = smapi {
            readers.push(self.tail(run.log, started, exited_rx, sink.clone()));
        }
        self.wait(child, kill_rx, exited_tx, readers, sink);
        Ok(state)
    }

    /// Asks the running game to stop. The exit is reported through the sink.
    pub fn stop(&self) -> AppResult<()> {
        let tx = self
            .inner
            .lock()
            .unwrap()
            .kill
            .take()
            .ok_or(AppError::NotRunning)?;
        let _ = tx.send(());
        Ok(())
    }

    /// Sends a line to SMAPI's console, as if typed in its terminal.
    pub async fn send(&self, line: &str) -> AppResult<()> {
        let mut stdin = self
            .inner
            .lock()
            .unwrap()
            .stdin
            .take()
            .ok_or(AppError::NotRunning)?;
        let res = async {
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await
        }
        .await;
        self.inner.lock().unwrap().stdin = Some(stdin);
        res.map_err(Into::into)
    }

    fn emit(inner: &Mutex<Inner>, sink: &impl Sink, source: Source, raw: &str) {
        let text = strip_ansi(raw);
        let line = {
            let mut g = inner.lock().unwrap();
            let level = match source {
                Source::Stdout => Level::Info,
                Source::Stderr => Level::Error,
                Source::SmapiStdout => {
                    if text.trim().is_empty() || is_smapi_console_line(&text) {
                        return;
                    }
                    Level::Info
                }
                Source::SmapiFile => match stamped_level(&text) {
                    Some(l) => {
                        g.last_level = Some(l);
                        l
                    }
                    None => g.last_level.unwrap_or(Level::Info),
                },
            };
            g.push(level, text)
        };
        sink.log(&line);
    }

    fn pump<R>(
        &self,
        reader: Option<R>,
        source: Source,
        sink: Arc<impl Sink>,
    ) -> Option<JoinHandle<()>>
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let reader = reader?;
        let inner = self.inner.clone();
        Some(tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(text)) = lines.next_line().await {
                Self::emit(&inner, sink.as_ref(), source, &text);
            }
        }))
    }

    // SMAPI rewrites SMAPI-latest.txt on start; a file older than our spawn is the last run's
    fn tail(
        &self,
        path: PathBuf,
        since: SystemTime,
        mut exited: watch::Receiver<bool>,
        sink: Arc<impl Sink>,
    ) -> JoinHandle<()> {
        let inner = self.inner.clone();
        tauri::async_runtime::spawn(async move {
            let mut file: Option<tokio::fs::File> = None;
            let mut pos = 0u64;
            // bytes, not text: a 250 ms read can end inside a multi-byte character, and
            // decoding each read on its own would turn that character into U+FFFD for good
            let mut pending: Vec<u8> = Vec::new();
            loop {
                let last_round = *exited.borrow();
                if file.is_none() {
                    let fresh = tokio::fs::metadata(&path)
                        .await
                        .and_then(|m| m.modified())
                        .is_ok_and(|t| t >= since);
                    if fresh {
                        file = tokio::fs::File::open(&path).await.ok();
                    }
                }
                if let Some(f) = file.as_mut() {
                    let len = f.metadata().await.map(|m| m.len()).unwrap_or(pos);
                    if len < pos {
                        pos = 0;
                        pending.clear();
                    }
                    if len > pos && f.seek(std::io::SeekFrom::Start(pos)).await.is_ok() {
                        let mut buf = Vec::with_capacity((len - pos) as usize);
                        if let Ok(n) = f.read_to_end(&mut buf).await {
                            pos += n as u64;
                            pending.extend_from_slice(&buf);
                            while let Some(nl) = pending.iter().position(|b| *b == b'\n') {
                                let raw: Vec<u8> = pending.drain(..=nl).collect();
                                let line = String::from_utf8_lossy(&raw);
                                let line = line.trim_end_matches(['\n', '\r']);
                                Self::emit(&inner, sink.as_ref(), Source::SmapiFile, line);
                            }
                        }
                    }
                }
                if last_round {
                    if !pending.is_empty() {
                        let rest = String::from_utf8_lossy(&pending);
                        Self::emit(&inner, sink.as_ref(), Source::SmapiFile, &rest);
                    }
                    break;
                }
                tokio::select! {
                    _ = tokio::time::sleep(TAIL_INTERVAL) => {}
                    _ = exited.changed() => {}
                }
            }
        })
    }

    fn wait(
        &self,
        mut child: Child,
        kill_rx: tokio::sync::oneshot::Receiver<()>,
        exited: watch::Sender<bool>,
        readers: Vec<JoinHandle<()>>,
        sink: Arc<impl Sink>,
    ) {
        let inner = self.inner.clone();
        tauri::async_runtime::spawn(async move {
            let status = tokio::select! {
                s = child.wait() => s,
                _ = kill_rx => {
                    let _ = child.start_kill();
                    child.wait().await
                }
            };
            let _ = exited.send(true);
            // the last lines may still be in flight; "exited" must come after them. Bounded:
            // a process the game spawned can keep the pipes open long after it quit
            for mut r in readers {
                if tokio::time::timeout(READER_GRACE, &mut r).await.is_err() {
                    r.abort();
                }
            }
            let state = RunState::Exited {
                code: status.ok().and_then(|s| s.code()),
            };
            {
                let mut g = inner.lock().unwrap();
                g.stdin = None;
                g.kill = None;
                g.state = Some(state.clone());
            }
            sink.state(&state);
        });
    }
}

/// Reads the level from SMAPI's file stamp, `[HH:MM:SS LEVEL source] message`.
fn stamped_level(line: &str) -> Option<Level> {
    let header = line.strip_prefix('[')?.split_once(']')?.0;
    let mut parts = header.split_whitespace();
    let time = parts.next()?;
    if time.len() != 8 || time.as_bytes()[2] != b':' || time.as_bytes()[5] != b':' {
        return None;
    }
    Level::parse(parts.next()?)
}

// SMAPI's own console lines look like "[SMAPI] text" or "[Mod Name] text"
fn is_smapi_console_line(line: &str) -> bool {
    line.strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
        .is_some_and(|(source, _)| !source.is_empty() && !source.contains('['))
}

// SMAPI colors its console when it believes a terminal is attached
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn strips_color_codes() {
        assert_eq!(strip_ansi("\u{1b}[31m[SMAPI] hi\u{1b}[0m"), "[SMAPI] hi");
        assert_eq!(strip_ansi("plain"), "plain");
    }

    #[test]
    fn reads_stamped_level() {
        assert_eq!(
            stamped_level("[00:50:24 INFO  SMAPI] hi"),
            Some(Level::Info)
        );
        assert_eq!(
            stamped_level("[00:50:24 ERROR Content Patcher] x"),
            Some(Level::Error)
        );
        assert_eq!(stamped_level("[SMAPI] hi"), None);
        assert_eq!(stamped_level("   at Foo.Bar()"), None);
    }

    #[test]
    fn recognizes_smapi_console_lines() {
        assert!(is_smapi_console_line("[SMAPI] Loading mods..."));
        assert!(is_smapi_console_line("[Content Patcher] done"));
        assert!(!is_smapi_console_line("AL lib: (WW) whatever"));
        assert!(!is_smapi_console_line("[x"));
    }

    struct Collect;
    impl Sink for Collect {
        fn log(&self, _: &LogLine) {}
        fn state(&self, _: &RunState) {}
    }

    fn script(dir: &Path, name: &str, body: &str) {
        let exe = dir.join(name);
        std::fs::write(&exe, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    async fn wait_exit(launcher: &Launcher) {
        for _ in 0..200 {
            if matches!(launcher.state(), RunState::Exited { .. }) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("game did not exit");
    }

    #[test]
    fn runs_fake_game_and_relays_output() {
        let dir = tempfile::tempdir().unwrap();
        script(
            dir.path(),
            game::VANILLA_EXE,
            "echo hello\nread line\necho \"got $line\"\necho oops >&2\n",
        );

        tauri::async_runtime::block_on(async {
            let launcher = Launcher::default();
            launcher.launch(dir.path(), None, Collect).unwrap();
            assert!(matches!(
                launcher.launch(dir.path(), None, Collect),
                Err(AppError::AlreadyRunning)
            ));
            launcher.send("ping").await.unwrap();
            wait_exit(&launcher).await;
            assert!(matches!(
                launcher.state(),
                RunState::Exited { code: Some(0) }
            ));
            let mut lines: Vec<(Level, String)> = launcher
                .log()
                .into_iter()
                .map(|l| (l.level, l.text))
                .collect();
            lines.sort_by(|a, b| a.1.cmp(&b.1));
            assert_eq!(
                lines,
                [
                    (Level::Info, "got ping".to_owned()),
                    (Level::Info, "hello".to_owned()),
                    (Level::Error, "oops".to_owned()),
                ]
            );
        });
    }

    #[test]
    fn modded_run_reads_levels_from_smapi_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("SMAPI-latest.txt");
        std::fs::write(
            &log,
            "[00:00:00 ERROR SMAPI] stale line from the last run\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("StardewModdingAPI.dll"), b"").unwrap();
        let body = format!(
            "echo \"args: $*\"\necho '[SMAPI] short form, skipped'\necho 'AL lib: kept'\nsleep 0.3\n\
             printf '[00:00:01 WARN  SMAPI] careful\\n   at continuation\\n[00:00:02 INFO  SMAPI] fine' > '{}'\nsleep 0.4\n",
            log.display()
        );
        script(dir.path(), game::SMAPI_EXE, &body);

        tauri::async_runtime::block_on(async {
            // the stale file must be older than the spawn
            tokio::time::sleep(Duration::from_millis(20)).await;
            let launcher = Launcher::default();
            launcher
                .launch(
                    dir.path(),
                    Some(SmapiRun {
                        env: Vec::new(),
                        log: log.clone(),
                        mods_path: Some("/profiles/p1/Mods".into()),
                    }),
                    Collect,
                )
                .unwrap();
            wait_exit(&launcher).await;
            let lines: Vec<(Level, String)> = launcher
                .log()
                .into_iter()
                .map(|l| (l.level, l.text))
                .collect();
            assert_eq!(
                lines,
                [
                    (
                        Level::Info,
                        "args: --mods-path /profiles/p1/Mods".to_owned()
                    ),
                    (Level::Info, "AL lib: kept".to_owned()),
                    (Level::Warn, "[00:00:01 WARN  SMAPI] careful".to_owned()),
                    (Level::Warn, "   at continuation".to_owned()),
                    (Level::Info, "[00:00:02 INFO  SMAPI] fine".to_owned()),
                ]
            );
        });
    }

    #[test]
    fn refuses_modded_launch_without_smapi() {
        let dir = tempfile::tempdir().unwrap();
        let res = Launcher::default().launch(
            dir.path(),
            Some(SmapiRun {
                env: Vec::new(),
                log: dir.path().join("log"),
                mods_path: None,
            }),
            Collect,
        );
        assert!(matches!(res, Err(AppError::SmapiMissing)));
    }

    // starts the real game for a few seconds, so it only runs on request:
    // PELICAN_TEST_GAME_DIR=<Contents/MacOS with SMAPI> cargo test -- --ignored
    #[test]
    #[ignore]
    fn launches_real_game_through_smapi() {
        let game_dir = PathBuf::from(std::env::var("PELICAN_TEST_GAME_DIR").unwrap());
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let path = home.join(".config/StardewValley/ErrorLogs/SMAPI-latest.txt");
        tauri::async_runtime::block_on(async {
            let launcher = Launcher::default();
            launcher
                .launch(
                    &game_dir,
                    Some(SmapiRun {
                        env: Vec::new(),
                        log: path,
                        mods_path: None,
                    }),
                    Collect,
                )
                .unwrap();
            let ready = |l: &LogLine| l.text.contains("Mods loaded and ready");
            for _ in 0..150 {
                if launcher.log().iter().any(ready) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            launcher.stop().unwrap();
            wait_exit(&launcher).await;
            let log = launcher.log();
            assert!(log.iter().any(ready), "SMAPI never reported ready");
            assert!(log.iter().any(|l| l.level == Level::Trace));
            assert!(log.iter().any(|l| l.level == Level::Debug));
        });
    }

    // full path through the store with a real game: zip install, profile, symlinks,
    // SMAPI --mods-path. Needs SMAPI in the game folder; run with
    // PELICAN_TEST_GAME_DIR=<Contents/MacOS> cargo test -- --ignored
    #[test]
    #[ignore]
    fn real_game_loads_mod_from_profile() {
        use crate::profiles::{self, Profile};
        use std::io::Write;

        let game_dir = PathBuf::from(std::env::var("PELICAN_TEST_GAME_DIR").unwrap());
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let tmp = tempfile::tempdir().unwrap();

        // SaveBackup with a new UniqueID stands in for a downloaded mod
        let src = game_dir.join("Mods/SaveBackup");
        let zip_path = tmp.path().join("Backup Copy.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        for e in std::fs::read_dir(&src).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let mut body = std::fs::read(e.path()).unwrap();
            if name == "manifest.json" {
                body = String::from_utf8(body)
                    .unwrap()
                    .replace("\"SMAPI.SaveBackup\"", "\"Test.BackupCopy\"")
                    .replace("\"Save Backup\"", "\"Backup Copy\"")
                    .into_bytes();
            }
            w.start_file(
                format!("Backup Copy/{name}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            w.write_all(&body).unwrap();
        }
        w.finish().unwrap();

        let store = tmp.path().join("store");
        let installed =
            crate::library::install_archive(&store, &tmp.path().join("backups"), &zip_path)
                .unwrap();
        let profile = Profile {
            id: "test".into(),
            name: None,
            enabled: vec![installed[0].id.clone()],
        };
        let bundled = [(
            "ConsoleCommands".to_owned(),
            game_dir.join("Mods/ConsoleCommands"),
        )];
        let mods =
            profiles::deploy(&profile, &store, &bundled, &tmp.path().join("deploy")).unwrap();

        tauri::async_runtime::block_on(async {
            let launcher = Launcher::default();
            launcher
                .launch(
                    &game_dir,
                    Some(SmapiRun {
                        env: Vec::new(),
                        log: home.join(".config/StardewValley/ErrorLogs/SMAPI-latest.txt"),
                        mods_path: Some(mods),
                    }),
                    Collect,
                )
                .unwrap();
            let ready = |l: &LogLine| l.text.contains("Mods loaded and ready");
            for _ in 0..150 {
                if launcher.log().iter().any(ready) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            launcher.stop().unwrap();
            wait_exit(&launcher).await;
            let log = launcher.log();
            assert!(
                log.iter().any(|l| l.text.contains("Loaded 2 mods")),
                "{log:#?}"
            );
            assert!(log.iter().any(|l| l.text.contains("Backup Copy")));
        });
    }
}
