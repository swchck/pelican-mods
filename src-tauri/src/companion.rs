//! The launcher end of the companion link: a WebSocket server the SMAPI companion mod
//! dials into.
//!
//! The server listens on a random loopback port with a per-run token in the URL; the
//! launcher hands both to the game in `PELICAN_COMPANION_URL`. One game connects at a
//! time, and a new connection replaces the old one. Message shapes mirror
//! companion/PelicanCompanion/Protocol.cs.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

use crate::error::{AppError, AppResult};
use crate::perfection::Catalog;

/// Bumped when a message shape changes incompatibly.
pub const PROTOCOL: u32 = 1;

/// Environment variable the companion reads its URL from.
pub const URL_VAR: &str = "PELICAN_COMPANION_URL";

/// The companion's folder name inside Mods.
pub const FOLDER: &str = "PelicanCompanion";

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_MESSAGE: usize = 8 << 20;

// a forecast runs the geode code 60 times and lists every chest; still well under this
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// What the player is doing right now.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameSnapshot {
    pub farmer: String,
    pub farm: String,
    pub season: String,
    pub day: u32,
    pub year: u32,
    /// Game clock as HHMM, 600–2600.
    pub time: u32,
    pub money: i64,
    pub location: Option<String>,
    /// "sun", "rain", "storm" or "snow"; empty from a companion older than this field.
    #[serde(default)]
    pub weather: String,
}

/// The game the companion reported on connecting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub protocol: u32,
    pub smapi: String,
    pub game: String,
    /// UniqueIDs of every loaded mod.
    pub mods: Vec<String>,
}

/// A mod's Generic Mod Config Menu, as the companion read it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GmcmMod {
    pub id: String,
    pub title_screen_only: bool,
    pub options: Vec<GmcmOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GmcmOption {
    /// config.json key path, when the option's getter revealed it.
    #[serde(default)]
    pub path: Option<Vec<String>>,
    pub name: String,
    #[serde(default)]
    pub tooltip: Option<String>,
    /// "bool", "int", "float", "string", "keybind" or "choice".
    pub kind: String,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub interval: Option<f64>,
    #[serde(default)]
    pub choices: Option<Vec<GmcmChoice>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GmcmChoice {
    pub value: String,
    pub label: String,
}

/// How applying settings in the running game went.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub ok: bool,
    /// "gmcm" when the mod's own save flow ran, "reload" when only its config object changed.
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// The link as the UI shows it.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub connected: bool,
    pub hello: Option<Hello>,
    pub state: Option<GameSnapshot>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Incoming {
    Hello(Hello),
    State {
        state: Option<GameSnapshot>,
    },
    Gmcm {
        mods: Vec<GmcmMod>,
    },
    Catalog(Box<Catalog>),
    ConfigDefaults {
        mods: HashMap<String, Value>,
    },
    /// Answers to requests: `result` for applyConfig, `reply` for data requests.
    #[serde(alias = "reply")]
    Result {
        id: u64,
        #[serde(flatten)]
        body: Value,
    },
}

/// Where the companion reports to; implemented over Tauri events in the app.
pub trait Events: Send + Sync + 'static {
    fn status(&self, status: &Status);
    fn gmcm(&self, ids: &[String]);
    fn catalog(&self);
}

#[derive(Default)]
struct Link {
    /// Bumped per connection so a closing old socket can't clear a newer one.
    generation: u64,
    out: Option<mpsc::UnboundedSender<String>>,
    status: Status,
    pending: HashMap<u64, oneshot::Sender<Value>>,
}

/// The server and what the connected game last reported.
pub struct Companion {
    url: OnceLock<String>,
    link: Mutex<Link>,
    next_id: AtomicU64,
    schemas: Mutex<HashMap<String, GmcmMod>>,
    schemas_file: PathBuf,
    catalog: Mutex<Option<Catalog>>,
    catalog_file: PathBuf,
    /// Each C# mod's config as a fresh instance writes it, by lowercased UniqueID.
    defaults: Mutex<HashMap<String, Value>>,
    defaults_file: PathBuf,
}

impl Companion {
    /// Creates the link; GMCM menus and the perfection catalog seen in earlier
    /// sessions load from `cache_dir`.
    pub fn new(cache_dir: &std::path::Path) -> Self {
        let schemas_file = cache_dir.join("gmcm.json");
        let catalog_file = cache_dir.join("catalog.json");
        let defaults_file = cache_dir.join("config-defaults.json");
        let defaults = std::fs::read(&defaults_file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let catalog = std::fs::read(&catalog_file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let schemas = std::fs::read(&schemas_file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<GmcmMod>>(&b).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|m| (m.id.to_lowercase(), m))
            .collect();
        Self {
            url: OnceLock::new(),
            link: Mutex::new(Link::default()),
            next_id: AtomicU64::new(1),
            schemas: Mutex::new(schemas),
            schemas_file,
            catalog: Mutex::new(catalog),
            catalog_file,
            defaults: Mutex::new(defaults),
            defaults_file,
        }
    }

    /// The config a fresh copy of C# mod `unique_id` writes, from the last game that ran it.
    pub fn config_defaults(&self, unique_id: &str) -> Option<Value> {
        self.defaults
            .lock()
            .unwrap()
            .get(&unique_id.to_lowercase())
            .cloned()
    }

    /// The URL to hand the game, once the server is listening.
    pub fn url(&self) -> Option<&str> {
        self.url.get().map(String::as_str)
    }

    pub fn status(&self) -> Status {
        self.link.lock().unwrap().status.clone()
    }

    /// Returns the GMCM menu of mod `unique_id`, from this or an earlier session.
    pub fn schema(&self, unique_id: &str) -> Option<GmcmMod> {
        self.schemas
            .lock()
            .unwrap()
            .get(&unique_id.to_lowercase())
            .cloned()
    }

    /// The perfection catalog from the last game that connected, if any ever did.
    pub fn catalog(&self) -> Option<Catalog> {
        self.catalog.lock().unwrap().clone()
    }

    /// Reports whether the connected game has mod `unique_id` loaded.
    pub fn has_mod(&self, unique_id: &str) -> bool {
        let link = self.link.lock().unwrap();
        link.status.connected
            && link
                .status
                .hello
                .as_ref()
                .is_some_and(|h| h.mods.iter().any(|m| m.eq_ignore_ascii_case(unique_id)))
    }

    /// Asks the game to load `config` into mod `unique_id` without a restart.
    pub async fn apply(&self, unique_id: &str, config: &Value) -> AppResult<Applied> {
        let body = self
            .request(serde_json::json!({
                "type": "applyConfig", "modId": unique_id, "config": config,
            }))
            .await?;
        serde_json::from_value(body).map_err(|_| AppError::CompanionOffline)
    }

    /// Asks the game for data: `kind` is "forecast" or "items". The game answers only
    /// with a save loaded; otherwise this fails with [`AppError::NoWorld`].
    pub async fn query(&self, kind: &str) -> AppResult<Value> {
        let mut body = self.request(serde_json::json!({ "type": kind })).await?;
        match body.get("error").and_then(Value::as_str) {
            Some("no_world") => Err(AppError::NoWorld),
            Some(e) => Err(AppError::CompanionFailed(e.to_owned())),
            None => Ok(body.get_mut("data").map(Value::take).unwrap_or_default()),
        }
    }

    async fn request(&self, mut msg: Value) -> AppResult<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        msg["id"] = id.into();
        let (tx, rx) = oneshot::channel();
        {
            let mut link = self.link.lock().unwrap();
            let out = link.out.as_ref().ok_or(AppError::CompanionOffline)?;
            out.send(msg.to_string())
                .map_err(|_| AppError::CompanionOffline)?;
            link.pending.insert(id, tx);
        }
        let result = tokio::time::timeout(REQUEST_TIMEOUT, rx).await;
        self.link.lock().unwrap().pending.remove(&id);
        match result {
            Ok(Ok(body)) => Ok(body),
            _ => Err(AppError::CompanionOffline),
        }
    }

    #[cfg(test)]
    fn send_raw(&self, msg: Value) {
        if let Some(out) = &self.link.lock().unwrap().out {
            let _ = out.send(msg.to_string());
        }
    }

    /// Shows `text` in the game's HUD; silently dropped while no game is connected.
    pub fn notify(&self, text: &str) {
        let link = self.link.lock().unwrap();
        if let Some(out) = &link.out {
            let _ = out.send(serde_json::json!({ "type": "notify", "text": text }).to_string());
        }
    }

    /// Binds the server and serves connections until the app exits.
    pub async fn serve(self: Arc<Self>, events: impl Events) -> AppResult<()> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let token = random_token()?;
        let _ = self
            .url
            .set(format!("ws://127.0.0.1:{port}/companion?token={token}"));
        let events = Arc::new(events);
        loop {
            let (stream, _) = listener.accept().await?;
            let token = token.clone();
            let events = events.clone();
            let this = self.clone();
            tauri::async_runtime::spawn(async move {
                this.connection(stream, &token, events.as_ref()).await;
            });
        }
    }

    async fn connection(&self, stream: TcpStream, token: &str, events: &impl Events) {
        let expected = format!("token={token}");
        // tungstenite's handshake Callback trait fixes this signature, large Err and all
        #[allow(clippy::result_large_err)]
        let check = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
            // the game's ClientWebSocket sends no Origin; a browser always does, and a page
            // has no business here even if it guessed the token
            let from_browser = req.headers().contains_key("origin");
            let token_ok = req
                .uri()
                .query()
                .is_some_and(|q| constant_time_eq(q.as_bytes(), expected.as_bytes()));
            if token_ok && !from_browser {
                Ok(resp)
            } else {
                let mut denied = ErrorResponse::new(None);
                *denied.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::FORBIDDEN;
                Err(denied)
            }
        };
        let mut config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
        // the biggest real message, the catalog, is a few hundred KB
        config.max_message_size = Some(MAX_MESSAGE);
        config.max_frame_size = Some(MAX_MESSAGE);
        let handshake =
            tokio_tungstenite::accept_hdr_async_with_config(stream, check, Some(config));
        let Ok(Ok(ws)) = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await else {
            return;
        };
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let generation = {
            let mut link = self.link.lock().unwrap();
            link.generation += 1;
            link.out = Some(tx);
            link.status = Status {
                connected: true,
                ..Status::default()
            };
            link.generation
        };
        let writer = tauri::async_runtime::spawn(async move {
            while let Some(text) = rx.recv().await {
                if sink.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
        });
        while let Some(Ok(msg)) = stream.next().await {
            // a newer game took over: this one's reports would overwrite the live session's
            if self.link.lock().unwrap().generation != generation {
                break;
            }
            let Message::Text(text) = msg else { continue };
            match serde_json::from_str::<Incoming>(&text) {
                Ok(incoming) => self.handle(incoming, events),
                Err(e) => eprintln!("companion: ignored message: {e}"),
            }
        }
        writer.abort();
        let status = {
            let mut link = self.link.lock().unwrap();
            if link.generation != generation {
                return;
            }
            link.out = None;
            link.status = Status::default();
            link.pending.clear();
            link.status.clone()
        };
        events.status(&status);
    }

    fn handle(&self, msg: Incoming, events: &impl Events) {
        match msg {
            Incoming::Hello(hello) => {
                if hello.protocol != PROTOCOL {
                    eprintln!(
                        "companion: speaks protocol {}, expected {PROTOCOL}",
                        hello.protocol
                    );
                }
                let status = {
                    let mut link = self.link.lock().unwrap();
                    link.status.hello = Some(hello);
                    link.status.clone()
                };
                events.status(&status);
            }
            Incoming::State { state } => {
                let status = {
                    let mut link = self.link.lock().unwrap();
                    link.status.state = state;
                    link.status.clone()
                };
                events.status(&status);
            }
            Incoming::Gmcm { mods } => {
                let ids: Vec<String> = mods.iter().map(|m| m.id.clone()).collect();
                let snapshot: Vec<GmcmMod> = {
                    let mut schemas = self.schemas.lock().unwrap();
                    for m in mods {
                        schemas.insert(m.id.to_lowercase(), m);
                    }
                    schemas.values().cloned().collect()
                };
                if let Err(e) = save_json(&self.schemas_file, &snapshot) {
                    eprintln!("companion: couldn't save GMCM menus: {e}");
                }
                events.gmcm(&ids);
            }
            Incoming::ConfigDefaults { mods } => {
                let snapshot = {
                    let mut defaults = self.defaults.lock().unwrap();
                    for (id, v) in mods {
                        defaults.insert(id.to_lowercase(), v);
                    }
                    defaults.clone()
                };
                if let Err(e) = save_json(&self.defaults_file, &snapshot) {
                    eprintln!("companion: couldn't save config defaults: {e}");
                }
            }
            Incoming::Catalog(catalog) => {
                if let Err(e) = save_json(&self.catalog_file, &*catalog) {
                    eprintln!("companion: couldn't save the catalog: {e}");
                }
                *self.catalog.lock().unwrap() = Some(*catalog);
                events.catalog();
            }
            Incoming::Result { id, body } => {
                if let Some(tx) = self.link.lock().unwrap().pending.remove(&id) {
                    let _ = tx.send(body);
                }
            }
        }
    }
}

fn save_json(file: &PathBuf, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, serde_json::to_vec(value).expect("cache serialize"))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::connect_async;

    #[derive(Default)]
    struct Recorder {
        statuses: Mutex<Vec<Status>>,
        gmcm: Mutex<Vec<String>>,
    }

    impl Events for Arc<Recorder> {
        fn status(&self, status: &Status) {
            self.statuses.lock().unwrap().push(status.clone());
        }
        fn gmcm(&self, ids: &[String]) {
            self.gmcm.lock().unwrap().extend(ids.iter().cloned());
        }
        fn catalog(&self) {}
    }

    #[test]
    fn talks_to_a_companion() {
        let dir = tempfile::tempdir().unwrap();
        let companion = Arc::new(Companion::new(dir.path()));
        let recorder = Arc::new(Recorder::default());
        tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn(companion.clone().serve(recorder.clone()));
            while companion.url().is_none() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            let url = companion.url().unwrap().to_owned();

            let wrong = url.replace("token=", "token=x");
            assert!(connect_async(wrong).await.is_err());

            let (mut game, _) = connect_async(url).await.unwrap();
            let send = |v: Value| Message::text(v.to_string());
            game.send(send(serde_json::json!({
                "type": "hello", "protocol": 1, "smapi": "4.5.2", "game": "1.6.15",
                "mods": ["Pathoschild.LookupAnything"],
            })))
            .await
            .unwrap();
            game.send(send(serde_json::json!({"type": "gmcm", "mods": [{
                "id": "Pathoschild.LookupAnything", "titleScreenOnly": false,
                "options": [{"path": ["ShowDataMiningFields"], "name": "Data mining", "kind": "bool"}],
            }]})))
            .await
            .unwrap();
            while !companion.has_mod("pathoschild.lookupanything")
                || companion.schema("Pathoschild.LookupAnything").is_none()
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }

            let config = serde_json::json!({"a": 1});
            let apply = companion.apply("Pathoschild.LookupAnything", &config);
            let answer = async {
                let Some(Ok(Message::Text(text))) = game.next().await else {
                    panic!("no request")
                };
                let req: Value = serde_json::from_str(&text).unwrap();
                assert_eq!(req["type"], "applyConfig");
                assert_eq!(req["config"]["a"], 1);
                game.send(send(serde_json::json!({
                    "type": "result", "id": req["id"], "ok": true, "method": "gmcm",
                })))
                .await
                .unwrap();
                game
            };
            let (applied, mut game) = tokio::join!(apply, answer);
            let applied = applied.unwrap();
            assert!(applied.ok);
            assert_eq!(applied.method.as_deref(), Some("gmcm"));

            game.close(None).await.unwrap();
            while companion.status().connected {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            assert!(companion.apply("x", &Value::Null).await.is_err());
        });
        assert_eq!(
            *recorder.gmcm.lock().unwrap(),
            ["Pathoschild.LookupAnything"]
        );
        let reloaded = Companion::new(dir.path());
        assert_eq!(
            reloaded
                .schema("pathoschild.lookupanything")
                .unwrap()
                .options[0]
                .name,
            "Data mining"
        );
    }

    fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    // starts the real game with GMCM, Lookup Anything and a freshly built companion:
    // scripts/build-companion.sh, then
    // PELICAN_TEST_GAME_DIR=<Contents/MacOS> PELICAN_TEST_STORE=<app data>/mods \
    //   cargo test -- --ignored companion_in_real_game --nocapture
    #[test]
    #[ignore]
    fn companion_in_real_game() {
        use crate::launcher::{Launcher, LogLine, RunState, Sink, SmapiRun};
        struct Quiet;
        impl Sink for Quiet {
            fn log(&self, _: &LogLine) {}
            fn state(&self, _: &RunState) {}
        }
        let game = PathBuf::from(std::env::var("PELICAN_TEST_GAME_DIR").unwrap());
        let store = PathBuf::from(std::env::var("PELICAN_TEST_STORE").unwrap());
        let built =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../companion/dist/PelicanCompanion");
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("Mods");
        std::fs::create_dir_all(&mods).unwrap();
        for (name, target) in [
            ("ConsoleCommands", game.join("Mods/ConsoleCommands")),
            ("GenericModConfigMenu", store.join("GenericModConfigMenu")),
            ("PelicanCompanion", built),
        ] {
            std::os::unix::fs::symlink(target, mods.join(name)).unwrap();
        }
        // a copy: applying settings makes the mod rewrite its config.json
        let lookup = mods.join("LookupAnything");
        copy_dir(&store.join("LookupAnything"), &lookup);

        let companion = Arc::new(Companion::new(dir.path()));
        let recorder = Arc::new(Recorder::default());
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn(companion.clone().serve(recorder.clone()));
            while companion.url().is_none() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            let launcher = Launcher::default();
            launcher
                .launch(
                    &game,
                    Some(SmapiRun {
                        log: home.join(".config/StardewValley/ErrorLogs/SMAPI-latest.txt"),
                        mods_path: Some(mods.clone()),
                        env: vec![
                            (URL_VAR.into(), companion.url().unwrap().into()),
                            ("PELICAN_COMPANION_TEST".into(), "1".into()),
                        ],
                    }),
                    Quiet,
                )
                .unwrap();
            let mut schema = None;
            for _ in 0..300 {
                schema = companion.schema("Pathoschild.LookupAnything");
                if schema.is_some() && companion.has_mod("Pathoschild.LookupAnything") {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            let result = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(async {
                let schema = schema.expect("Lookup Anything's GMCM menu arrives");
                let status = companion.status();
                eprintln!(
                    "hello: {:?}",
                    status.hello.as_ref().map(|h| (&h.smapi, &h.game))
                );
                let mapped = schema.options.iter().filter(|o| o.path.is_some()).count();
                eprintln!(
                    "{} options, {mapped} mapped to config keys",
                    schema.options.len()
                );
                for o in schema.options.iter().take(6) {
                    eprintln!("  {:?} {} [{}] {:?}", o.path, o.name, o.kind, o.section);
                }
                assert!(
                    mapped * 2 > schema.options.len(),
                    "most options map to keys"
                );

                for _ in 0..50 {
                    if companion.catalog().is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                let c = companion.catalog().expect("a perfection catalog arrives");
                eprintln!(
                    "catalog {} {}: {} shipped, {} fish, {} cooking, {} crafting, {} villagers, {} goals",
                    c.game, c.language, c.shipping.len(), c.fish.len(), c.cooking.len(),
                    c.crafting.len(), c.villagers.len(), c.monsters.len()
                );
                eprintln!("  e.g. {:?} / {:?} / {:?}", c.fish.first(), c.villagers.first(), c.monsters.first());
                assert!(c.shipping.len() > 100 && c.fish.len() > 50 && c.cooking.len() > 70);
                assert!(c.crafting.len() > 100 && c.villagers.len() > 25 && c.monsters.len() > 8);
                eprintln!(
                    "  museum {}, achievements {}, orders {}, crops {}, items {}",
                    c.museum.len(), c.achievements.len(), c.special_orders.len(), c.crops.len(), c.items.len()
                );
                // worked out after the catalog goes out, so they come a little later
                for _ in 0..100 {
                    if companion.config_defaults("Pathoschild.LookupAnything").is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                let defaults = companion
                    .config_defaults("Pathoschild.LookupAnything")
                    .expect("Lookup Anything's config defaults arrive");
                eprintln!("  defaults: {}", defaults.to_string().chars().take(300).collect::<String>());
                assert!(defaults.get("ShowPuzzleSolutions").is_some());
                if let Ok(out) = std::env::var("PELICAN_TEST_CATALOG_OUT") {
                    std::fs::write(out, serde_json::to_vec(&c).unwrap()).unwrap();
                }

                let forecast = companion.query("forecast").await.unwrap();
                let geodes = forecast["geodes"].as_array().unwrap();
                eprintln!(
                    "forecast: luck {}, tomorrow {}, {} geode types, first {}",
                    forecast["luck"], forecast["weatherTomorrow"], geodes.len(), geodes[0]["next"][0]
                );
                assert_eq!(geodes.len(), 6);
                assert_eq!(geodes[0]["next"].as_array().unwrap().len(), 10);
                let items = companion.query("items").await.unwrap();
                eprintln!("items on the title screen: {}", items.as_array().unwrap().len());

                let file = lookup.join("config.json");
                let mut config: Value =
                    serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
                let flipped = !config["ShowPuzzleSolutions"].as_bool().unwrap();
                config["ShowPuzzleSolutions"] = Value::Bool(flipped);
                std::fs::write(&file, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
                let applied = companion
                    .apply("Pathoschild.LookupAnything", &config)
                    .await
                    .unwrap();
                eprintln!("applied: {applied:?}");
                assert!(applied.ok, "{applied:?}");
                let written: Value =
                    serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
                assert_eq!(written["ShowPuzzleSolutions"], Value::Bool(flipped));
            }))
            .await;
            // the game must close even when an assert above failed
            launcher.stop().unwrap();
            for _ in 0..100 {
                if matches!(launcher.state(), RunState::Exited { .. }) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if let Err(panic) = result {
                std::panic::resume_unwind(panic);
            }
        });
    }

    // records the base game's lists in every language the game ships, for the launcher's
    // built-in fallback; only the companion is loaded, so no mod content gets in:
    // scripts/build-companion.sh, then
    // PELICAN_TEST_GAME_DIR=<Contents/MacOS> PELICAN_TEST_CATALOG_DIR=src-tauri/resources/catalogs \
    //   cargo test -- --ignored base_catalogs --nocapture
    #[test]
    #[ignore]
    fn base_catalogs() {
        use crate::launcher::{Launcher, LogLine, RunState, Sink, SmapiRun};
        struct Quiet;
        impl Sink for Quiet {
            fn log(&self, _: &LogLine) {}
            fn state(&self, _: &RunState) {}
        }
        let game = PathBuf::from(std::env::var("PELICAN_TEST_GAME_DIR").unwrap());
        let out = PathBuf::from(std::env::var("PELICAN_TEST_CATALOG_DIR").unwrap());
        std::fs::create_dir_all(&out).unwrap();
        let built =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../companion/dist/PelicanCompanion");
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("Mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::os::unix::fs::symlink(built, mods.join("PelicanCompanion")).unwrap();
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        // switching languages makes the game save the last one as the player's own
        let prefs_file = home.join(".config/StardewValley/startup_preferences");
        let prefs = std::fs::read(&prefs_file).unwrap();
        let own = crate::perfection::game_language(&home.join(".config/StardewValley"))
            .unwrap_or_else(|| "en".to_owned());
        let companion = Arc::new(Companion::new(dir.path()));
        tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn(companion.clone().serve(Arc::new(Recorder::default())));
            while companion.url().is_none() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            let launcher = Launcher::default();
            launcher
                .launch(
                    &game,
                    Some(SmapiRun {
                        log: home.join(".config/StardewValley/ErrorLogs/SMAPI-latest.txt"),
                        mods_path: Some(mods.clone()),
                        env: vec![
                            (URL_VAR.into(), companion.url().unwrap().into()),
                            ("PELICAN_COMPANION_TEST".into(), "1".into()),
                        ],
                    }),
                    Quiet,
                )
                .unwrap();
            let result =
                futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(async {
                    for _ in 0..300 {
                        if companion.catalog().is_some() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    assert!(companion.catalog().is_some(), "a catalog arrives");
                    // th is left out: the game has no Thai item names and shows English ones
                    let mut codes: Vec<String> = [
                        "en", "ja", "ru", "zh", "pt", "es", "de", "fr", "ko", "it", "tr", "hu",
                    ]
                    .map(String::from)
                    .to_vec();
                    codes.retain(|c| *c != own);
                    codes.push(own.clone());
                    for code in &codes {
                        companion
                            .send_raw(serde_json::json!({ "type": "testLocale", "text": code }));
                        let mut got = None;
                        for _ in 0..150 {
                            got = companion.catalog().filter(|c| c.language == *code);
                            if got.is_some() {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                        let c = got.unwrap_or_else(|| panic!("a {code} catalog arrives"));
                        eprintln!(
                            "{code}: {} items, {} museum, {} achievements, e.g. {:?}",
                            c.items.len(),
                            c.museum.len(),
                            c.achievements.len(),
                            c.items.iter().find(|i| i.id == "24").map(|i| &i.name)
                        );
                        assert!(c.museum.len() == 95 && c.items.len() > 800);
                        let abigail = c.villagers.iter().find(|v| v.key == "Abigail").unwrap();
                        eprintln!(
                            "    {} festivals, {} fish, Abigail {:?} {} loves {:?}, first fish {:?}",
                            c.festivals.len(),
                            c.fish_info.len(),
                            abigail.birth_season,
                            abigail.birth_day,
                            abigail.loves,
                            c.fish_info.first()
                        );
                        assert!(c.festivals.len() > 8 && c.fish_info.len() > 60);
                        std::fs::write(
                            out.join(format!("{code}.json")),
                            serde_json::to_vec(&c).unwrap(),
                        )
                        .unwrap();
                    }
                }))
                .await;
            // the game must close even when an assert above failed
            launcher.stop().unwrap();
            for _ in 0..100 {
                if matches!(launcher.state(), RunState::Exited { .. }) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            std::fs::write(&prefs_file, &prefs).unwrap();
            if let Err(panic) = result {
                std::panic::resume_unwind(panic);
            }
        });
    }

    // opens an existing save (never saving it) and asks for the forecast and the items:
    // scripts/build-companion.sh, then
    // PELICAN_TEST_GAME_DIR=<Contents/MacOS> PELICAN_TEST_SAVE_NAME=<save folder name> \
    //   cargo test -- --ignored forecast_on_real_save --nocapture
    #[test]
    #[ignore]
    fn forecast_on_real_save() {
        use crate::launcher::{Launcher, LogLine, RunState, Sink, SmapiRun};
        struct Quiet;
        impl Sink for Quiet {
            fn log(&self, _: &LogLine) {}
            fn state(&self, _: &RunState) {}
        }
        let game = PathBuf::from(std::env::var("PELICAN_TEST_GAME_DIR").unwrap());
        let save = std::env::var("PELICAN_TEST_SAVE_NAME").unwrap();
        let built =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../companion/dist/PelicanCompanion");
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("Mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::os::unix::fs::symlink(built, mods.join("PelicanCompanion")).unwrap();
        let home = PathBuf::from(std::env::var("HOME").unwrap());
        let companion = Arc::new(Companion::new(dir.path()));
        tauri::async_runtime::block_on(async {
            tauri::async_runtime::spawn(companion.clone().serve(Arc::new(Recorder::default())));
            while companion.url().is_none() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            let launcher = Launcher::default();
            launcher
                .launch(
                    &game,
                    Some(SmapiRun {
                        log: home.join(".config/StardewValley/ErrorLogs/SMAPI-latest.txt"),
                        mods_path: Some(mods.clone()),
                        env: vec![
                            (URL_VAR.into(), companion.url().unwrap().into()),
                            ("PELICAN_COMPANION_TEST".into(), "1".into()),
                        ],
                    }),
                    Quiet,
                )
                .unwrap();
            let result =
                futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(async {
                    for _ in 0..300 {
                        if companion.status().hello.is_some() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    assert!(companion.status().hello.is_some(), "companion connects");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    companion.send_raw(serde_json::json!({ "type": "testLoad", "text": save }));
                    for _ in 0..600 {
                        if companion.status().state.is_some() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    eprintln!("state: {:?}", companion.status().state);
                    assert!(companion.status().state.is_some(), "the save loads");
                    let f = companion.query("forecast").await.unwrap();
                    eprintln!(
                        "luck {} tomorrow {} bookseller {}",
                        f["luck"], f["weatherTomorrow"], f["bookseller"]
                    );
                    eprintln!("geode #1 {}", f["geodes"][0]["next"][0]);
                    eprintln!(
                        "mystery boxes opened {}: {}",
                        f["mysteryBoxesOpened"], f["mysteryBoxes"][0]["next"][0]
                    );
                    eprintln!("nights {}", f["nights"]);
                    eprintln!("cart {}", f["cart"]);
                    let items = companion.query("items").await.unwrap();
                    let list = items.as_array().unwrap();
                    eprintln!("items {}: first {:?}", list.len(), list.first());
                    assert!(!list.is_empty());
                }))
                .await;
            // the game must close even when an assert above failed
            launcher.stop().unwrap();
            for _ in 0..100 {
                if matches!(launcher.state(), RunState::Exited { .. }) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if let Err(panic) = result {
                std::panic::resume_unwind(panic);
            }
        });
    }
}
