//! Browser end-to-end test harness for Flasher.
//!
//! Each test owns a [`TestHarness`]:
//! - the real `flasher` server binary (`target/debug/flasher`, built on
//!   demand) listening on an ephemeral port, serving the
//!   production `frontends/leptos/dist` bundle, with its **own fresh
//!   `SQLite` database** in a per-test tempdir (`FLASHER_DB`,
//!   `FLASHER_USER` = [`E2E_USER`]),
//! - a headless Chromium instance driven over CDP via
//!   [`chromiumoxide::Browser`], with a per-test `--user-data-dir`
//!   (a unique [`TempDir`]) so concurrent tests never fight over
//!   Chromium's `SingletonLock`,
//! - a [`Page`] already navigated to `/`.
//!
//! [`TestHarness::start`] runs the server in dev-bypass mode
//! (`FLASHER_USER` set, no auth UI, origin `http://127.0.0.1:<port>`).
//! [`TestHarness::start_with_auth`] runs it in passkey auth mode
//! (`FLASHER_USER` unset) on `http://localhost:<port>` — `WebAuthn`
//! requires a registrable-domain origin matching the relying-party
//! config. Pair it with [`TestHarness::add_virtual_authenticator`],
//! which drives the raw `WebAuthn.*` CDP commands (ctap2 internal
//! authenticator, user verification always succeeding, automatic
//! presence simulation) so ceremonies complete without hardware.
//!
//! `Drop` kills the server subprocess; the browser child is killed via
//! chromiumoxide's `kill_on_drop`, and the tempdirs clean up after it.
//!
//! # Seeding and white-box assertions
//!
//! Driving the app happens exclusively through the browser, but tests may
//! prepare and verify database state directly:
//!
//! - [`TestHarness::seed_store`] opens a second [`flasher_store::Store`]
//!   connection to the test's database (`SQLite` WAL mode allows this
//!   while the server holds its own connection). Use it to insert users
//!   and cards with exact `next_time` values. The server reads through
//!   SQL per request and caches nothing, so seeding is safe at any point,
//!   though seeding *before* the first request is recommended.
//! - [`TestHarness::db_path`] exposes the database file for raw SQL
//!   assertions after browser-driven actions.
//!
//! The user [`E2E_USER`] already exists (the server upserts it at
//! startup); look up its id via `Store::get_user_by_name`.
//!
//! # Running
//!
//! Tests in `tests/*.rs` are individually marked `#[ignore = "browser"]`
//! so the workspace `cargo test` / `cargo nextest run` (run by
//! `just rust-gate`) compiles them but never launches a browser. The full
//! `just gate` runs them via `just e2e` at the end. Run the suite
//! manually with:
//!
//! ```sh
//! just e2e          # all browser tests via nextest --run-ignored ignored-only
//! just screenshots  # only the 02_screenshots capture test
//! ```
//!
//! # Parallel safety
//!
//! Every test binds its own ephemeral port and its own Chromium profile
//! dir, and nextest runs each test in its own process, so the suite is
//! safe under any `--test-threads` count.
//!
//! # Doctrine
//!
//! The web app — real browser, real clicks — is the only valid public
//! test surface. E2e tests must exercise the app the way a user does
//! ([`TestHarness::click`], [`TestHarness::type_into`], navigation, DOM
//! assertions). Do not call the HTTP API to drive or assert behavior;
//! the API is internal.

// Internal test harness: the pedantic doc and cast lints add no value
// here (same reasoning as the proven harness this is modeled on).
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::page::{Page, ScreenshotParams};
use futures::StreamExt;
use tempfile::TempDir;
use thiserror::Error;
use tokio::task::JoinHandle;

/// Default timeout for the `wait_*` helpers. Generous on purpose: the
/// wasm bundle has to download and boot before anything renders.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// The username the harness passes to the server via `FLASHER_USER`.
/// The server upserts this user at startup; look up its id via
/// `Store::get_user_by_name(E2E_USER)`.
pub const E2E_USER: &str = "e2e";

/// Errors surfaced by the harness. Every public method returns
/// [`Result`]; tests use `?` and never unwrap.
#[derive(Debug, Error)]
pub enum Error {
    /// The Leptos bundle has not been built. Tests refuse to invoke
    /// trunk themselves; the message points at `just build`.
    #[error(
        "frontend bundle not found at {0}; build it first with `just build` \
         (tests never run trunk themselves)"
    )]
    DistMissing(PathBuf),
    /// Sources under `frontends/leptos` are newer than the bundle, so
    /// the tests would exercise a stale frontend. Tests refuse to invoke
    /// trunk themselves; the message points at `just build`.
    #[error(
        "frontend bundle at {0} is stale (sources are newer); rebuild it with `just build` \
         (tests never run trunk themselves)"
    )]
    DistStale(PathBuf),
    /// `cargo build -p flasher-server` failed while ensuring the binary.
    #[error("`cargo build -p flasher-server` failed with {0}")]
    ServerBuildFailed(std::process::ExitStatus),
    /// The build reported success but the binary is still missing.
    #[error("flasher server binary not found at {0} even after a successful build")]
    ServerBinaryMissing(PathBuf),
    /// The server never answered `GET /api/health` with 200 in time.
    #[error("flasher server at {url} did not become healthy within {timeout:?}")]
    HealthTimeout {
        /// Base URL that was polled.
        url: String,
        /// Deadline that elapsed.
        timeout: Duration,
    },
    /// No directory containing both `justfile` and `Cargo.toml` was
    /// found walking up from `CARGO_MANIFEST_DIR`.
    #[error("no repo root (justfile + Cargo.toml) found above {0}")]
    RepoRootNotFound(PathBuf),
    /// A `wait_for_text` deadline elapsed.
    #[error(
        "timed out waiting for text {needle:?} in selector {selector:?}; \
         last content: {last:?}"
    )]
    WaitForText {
        /// CSS selector that was polled.
        selector: String,
        /// Substring that never appeared.
        needle: String,
        /// Last observed text content, if the element existed.
        last: Option<String>,
    },
    /// `querySelector` found no element.
    #[error("element not found: {0}")]
    ElementNotFound(String),
    /// Free-form failure (config build errors, test assertions).
    #[error("{0}")]
    Message(String),
    /// I/O failure (spawn, screenshot write, port probe).
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Health-probe HTTP failure.
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    /// CDP / chromiumoxide failure.
    #[error(transparent)]
    Cdp(#[from] chromiumoxide::error::CdpError),
}

/// Harness result type.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Build a free-form error (used by tests for plain assertions).
    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

/// One-test fixture: real `flasher` server subprocess + headless
/// Chromium page. Construct with [`TestHarness::start`]; drop tears
/// everything down.
pub struct TestHarness {
    /// Page already at `/`.
    pub page: Page,
    /// `http://127.0.0.1:<port>` of the spawned server.
    pub base_url: String,
    /// Repo root the harness resolved (parent of `test-output/`).
    repo_root: PathBuf,
    /// Flasher server child. Killed in `Drop`.
    server_child: Option<Child>,
    /// chromiumoxide handler driver. Aborted in `Drop`.
    handler_task: Option<JoinHandle<()>>,
    /// Browser handle; chromiumoxide sets `kill_on_drop` on the
    /// Chromium child, so dropping this field kills the browser.
    _browser: Browser,
    /// Per-test Chromium `--user-data-dir`. Declared after `_browser`
    /// so it drops last (Chromium first, then its profile dir). The
    /// unique profile per test is what makes the suite parallel-safe:
    /// without it every instance shares `$TMPDIR/chromiumoxide-runner`
    /// and a second concurrent launch aborts on `SingletonLock`.
    _profile_dir: TempDir,
    /// Per-test database tempdir holding `flasher.db`. Declared last so
    /// it drops after the server child (killed in `Drop`) released the
    /// database files.
    db_dir: TempDir,
}

impl TestHarness {
    /// Spawn the server on an ephemeral port with a fresh per-test
    /// database, wait for health, launch headless Chromium with a fresh
    /// profile, and open `/`.
    pub async fn start() -> Result<Self> {
        Self::start_inner(Mode::DevBypass).await
    }

    /// Like [`Self::start`] but starts the server in passkey auth mode
    /// (`FLASHER_USER` unset): every `/api/*` route except `health` and
    /// `auth/*` requires a session. The browser talks to
    /// `http://localhost:<port>` (not 127.0.0.1) because `WebAuthn`
    /// requires the origin to match the relying-party configuration
    /// (`FLASHER_ORIGIN`/`FLASHER_RP_ID`, both `localhost`).
    pub async fn start_with_auth() -> Result<Self> {
        Self::start_inner(Mode::PasskeyAuth).await
    }

    /// Shared body of [`Self::start`] and [`Self::start_with_auth`].
    async fn start_inner(mode: Mode) -> Result<Self> {
        let repo_root = repo_root()?;
        let dist = dist_dir(&repo_root)?;
        let bin = ensure_server_binary(&repo_root)?;
        let port = pick_free_port()?;
        let host = match mode {
            Mode::DevBypass => "127.0.0.1",
            // WebAuthn: origin and rp id must be a registrable domain —
            // `localhost`, not an IP literal (Chromium rejects IP rpIds).
            Mode::PasskeyAuth => "localhost",
        };
        let base_url = format!("http://{host}:{port}");
        let db_dir = TempDir::new()?;
        let db_path = db_dir.path().join("flasher.db");

        let server_child =
            spawn_server(&bin, &repo_root, &dist, port, &base_url, &db_path, mode).await?;
        let (browser, handler_task, profile_dir) = launch_browser().await?;
        let page = browser
            .new_page(format!("{base_url}/"))
            .await
            .map_err(Error::Cdp)?;
        // Chromium's headless default CSS viewport is narrower than the
        // window size above. Keep the harness's baseline on the desktop
        // layout; tests that cover responsive layouts set their viewport
        // explicitly.
        page.execute(SetDeviceMetricsOverrideParams::new(1280, 800, 1.0, false))
            .await
            .map_err(Error::Cdp)?;
        page.wait_for_navigation().await.map_err(Error::Cdp)?;

        Ok(Self {
            page,
            base_url,
            repo_root,
            server_child: Some(server_child),
            handler_task: Some(handler_task),
            _browser: browser,
            _profile_dir: profile_dir,
            db_dir,
        })
    }

    /// Enables the `WebAuthn` CDP domain and adds a virtual authenticator
    /// (ctap2, internal transport, resident keys, user verification
    /// always succeeding) so passkey ceremonies complete without
    /// hardware. Automatic presence simulation is switched on: without
    /// it the ceremony stalls waiting for a human "touch". Returns the
    /// `authenticatorId`.
    ///
    /// chromiumoxide does not wrap the `WebAuthn` domain, so the
    /// commands go out as raw CDP ([`RawCdpCommand`]).
    pub async fn add_virtual_authenticator(&self) -> Result<String> {
        self.raw_cdp("WebAuthn.enable", serde_json::json!({}))
            .await?;
        let result = self
            .raw_cdp(
                "WebAuthn.addVirtualAuthenticator",
                serde_json::json!({
                    "options": {
                        "protocol": "ctap2",
                        "transport": "internal",
                        "hasResidentKey": true,
                        "hasUserVerification": true,
                        "isUserVerified": true,
                    }
                }),
            )
            .await?;
        let id = result
            .get("authenticatorId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                Error::message(format!(
                    "WebAuthn.addVirtualAuthenticator returned no authenticatorId: {result}"
                ))
            })?
            .to_owned();
        self.raw_cdp(
            "WebAuthn.setAutomaticPresenceSimulation",
            serde_json::json!({ "authenticatorId": id, "enabled": true }),
        )
        .await?;
        Ok(id)
    }

    /// Value of the `__Host-session` cookie, read via the CDP
    /// `Storage.getCookies` domain (the cookie is `HttpOnly`, so
    /// `document.cookie` cannot see it). `None` when no session cookie
    /// is set. Tests use it to identify the live session server-side —
    /// e.g. to delete it via `Store::delete_session` and simulate a
    /// mid-session expiry.
    pub async fn session_token(&self) -> Result<Option<String>> {
        let result = self
            .raw_cdp("Storage.getCookies", serde_json::json!({}))
            .await?;
        let token = result
            .get("cookies")
            .and_then(serde_json::Value::as_array)
            .and_then(|cookies| {
                cookies.iter().find_map(|cookie| {
                    if cookie.get("name").and_then(serde_json::Value::as_str)
                        == Some("__Host-session")
                    {
                        cookie
                            .get("value")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned)
                    } else {
                        None
                    }
                })
            });
        Ok(token)
    }

    /// Removes a virtual authenticator added by
    /// [`Self::add_virtual_authenticator`]. Needed for the
    /// add-a-second-passkey flow: an authenticator that already holds a
    /// credential listed in `excludeCredentials` refuses `create()`, so
    /// the second passkey must be made on a fresh authenticator.
    pub async fn remove_virtual_authenticator(&self, authenticator_id: &str) -> Result<()> {
        self.raw_cdp(
            "WebAuthn.removeVirtualAuthenticator",
            serde_json::json!({ "authenticatorId": authenticator_id }),
        )
        .await?;
        Ok(())
    }

    /// Sends a raw CDP command on the page's session and returns the
    /// result JSON. For domains chromiumoxide does not model.
    async fn raw_cdp(
        &self,
        method: &'static str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let response = self
            .page
            .execute(RawCdpCommand { method, params })
            .await
            .map_err(Error::Cdp)?;
        Ok(response.result)
    }

    /// Path of this test's `SQLite` database file. Use for white-box
    /// assertions of the final state after browser-driven actions (or
    /// via [`Self::seed_store`]).
    pub fn db_path(&self) -> PathBuf {
        self.db_dir.path().join("flasher.db")
    }

    /// Opens a second [`flasher_store::Store`] connection to this test's
    /// database for seeding (exact `next_time` values, extra cards, ...)
    /// and white-box verification. `SQLite` WAL mode allows the second
    /// connection while the server holds its own; the server reads
    /// through SQL per request and caches nothing, so seeded rows are
    /// visible immediately.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened.
    pub async fn seed_store(
        &self,
    ) -> std::result::Result<flasher_store::Store, flasher_store::Error> {
        flasher_store::Store::connect(self.db_path()).await
    }

    /// Navigate the page to `path` (joined onto [`Self::base_url`]).
    pub async fn goto(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        self.page.goto(url).await.map_err(Error::Cdp)?;
        Ok(())
    }

    /// `document.title` of the current page.
    pub async fn title(&self) -> Result<Option<String>> {
        self.page.get_title().await.map_err(Error::Cdp)
    }

    /// `document.body.innerText` — the rendered text a user sees.
    pub async fn page_text(&self) -> Result<String> {
        self.text_content("body").await
    }

    /// `textContent` of the first element matching `sel`.
    pub async fn text_content(&self, sel: &str) -> Result<String> {
        let v: Option<String> = self
            .page
            .evaluate(format!(
                "(() => {{ const el = document.querySelector({}); \
                  return el ? (el.textContent || '') : null; }})()",
                json_str(sel)
            ))
            .await
            .map_err(Error::Cdp)?
            .into_value()
            .map_err(|e| Error::message(format!("text_content into_value: {e}")))?;
        v.ok_or_else(|| Error::ElementNotFound(sel.to_owned()))
    }

    /// Poll `sel`'s `textContent` until it contains `needle` or the
    /// timeout elapses. Use `"body"` to search the whole rendered page.
    pub async fn wait_for_text(&self, sel: &str, needle: &str, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        let mut last = None;
        loop {
            if let Ok(text) = self.text_content(sel).await {
                if text.contains(needle) {
                    return Ok(());
                }
                last = Some(text);
            }
            if Instant::now() >= deadline {
                return Err(Error::WaitForText {
                    selector: sel.to_owned(),
                    needle: needle.to_owned(),
                    last,
                });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Wait for at least one element matching `sel` to be in the DOM.
    pub async fn wait_for_selector(&self, sel: &str, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            let exists: bool = self
                .page
                .evaluate(format!("!!document.querySelector({})", json_str(sel)))
                .await
                .ok()
                .and_then(|r| r.into_value().ok())
                .unwrap_or(false);
            if exists {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::ElementNotFound(sel.to_owned()));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Click the first element matching `sel`, like a user would.
    pub async fn click(&self, sel: &str) -> Result<()> {
        self.wait_for_selector(sel, DEFAULT_TIMEOUT).await?;
        let el = self.page.find_element(sel).await.map_err(Error::Cdp)?;
        el.click().await.map_err(Error::Cdp)?;
        Ok(())
    }

    /// Focus the first match, clear it, and type `text` character by
    /// character so per-key event handlers fire — like a user typing.
    pub async fn type_into(&self, sel: &str, text: &str) -> Result<()> {
        self.wait_for_selector(sel, DEFAULT_TIMEOUT).await?;
        let el = self.page.find_element(sel).await.map_err(Error::Cdp)?;
        el.click().await.map_err(Error::Cdp)?;
        if !text.is_empty() {
            el.type_str(text).await.map_err(Error::Cdp)?;
        }
        Ok(())
    }

    /// Override the rendering viewport via
    /// `Emulation.setDeviceMetricsOverride` (mobile flag off).
    pub async fn set_viewport(&self, width: u32, height: u32) -> Result<()> {
        self.page
            .execute(SetDeviceMetricsOverrideParams::new(
                i64::from(width),
                i64::from(height),
                1.0,
                false,
            ))
            .await
            .map_err(Error::Cdp)?;
        Ok(())
    }

    /// Capture the current viewport as PNG into
    /// `test-output/screenshots/<name>.png` at the repo root. `name`
    /// may contain `/` to group captures per test
    /// (e.g. `screenshot("01_smoke/boot")`). Parent dirs are created.
    /// Returns the path written.
    pub async fn screenshot(&self, name: &str) -> Result<PathBuf> {
        let png = self
            .page
            .screenshot(ScreenshotParams::builder().build())
            .await
            .map_err(Error::Cdp)?;
        let path = self
            .repo_root
            .join("test-output/screenshots")
            .join(format!("{name}.png"));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &png)?;
        Ok(path)
    }

    /// Run a JS expression and deserialize the JSON result.
    pub async fn eval<T: serde::de::DeserializeOwned>(&self, expr: &str) -> Result<T> {
        let v = self
            .page
            .evaluate(expr.to_owned())
            .await
            .map_err(Error::Cdp)?
            .into_value()
            .map_err(|e| Error::message(format!("eval into_value: {e}")))?;
        Ok(v)
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        if let Some(task) = self.handler_task.take() {
            task.abort();
        }
        if let Some(mut child) = self.server_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Server auth mode for [`TestHarness::start_inner`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `FLASHER_USER` set: every request acts as the e2e user, no
    /// sessions (how the whole pre-auth suite runs).
    DevBypass,
    /// `FLASHER_USER` unset: passkey auth, session cookie required.
    PasskeyAuth,
}

/// A raw CDP command for domains chromiumoxide does not model
/// (`WebAuthn.*`). Serializes to the params object; the response is
/// handed back as untyped JSON.
#[derive(Debug, serde::Serialize)]
struct RawCdpCommand {
    /// CDP method, e.g. `WebAuthn.addVirtualAuthenticator`.
    #[serde(skip)]
    method: &'static str,
    /// Command params, inlined as the request payload.
    #[serde(flatten)]
    params: serde_json::Value,
}

impl chromiumoxide::Method for RawCdpCommand {
    fn identifier(&self) -> chromiumoxide::types::MethodId {
        self.method.into()
    }
}

impl chromiumoxide::Command for RawCdpCommand {
    type Response = serde_json::Value;
}

/// Walk up from `CARGO_MANIFEST_DIR` to the directory that holds both
/// `justfile` and `Cargo.toml` — the repo root.
fn repo_root() -> Result<PathBuf> {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    for dir in start.ancestors() {
        if dir.join("justfile").is_file() && dir.join("Cargo.toml").is_file() {
            return Ok(dir.to_path_buf());
        }
    }
    Err(Error::RepoRootNotFound(start.to_path_buf()))
}

/// The Leptos bundle the server hosts. Tests never build it themselves
/// — a missing or stale bundle is a setup error pointing at
/// `just build`.
fn dist_dir(root: &Path) -> Result<PathBuf> {
    let dist = root.join("frontends/leptos/dist");
    let index = dist.join("index.html");
    if !index.is_file() {
        return Err(Error::DistMissing(dist));
    }
    let built = index.metadata()?.modified()?;
    if newest_source_mtime(root)?.is_some_and(|mtime| mtime > built) {
        return Err(Error::DistStale(dist));
    }
    Ok(dist)
}

/// Newest modification time of any frontend source file
/// (`frontends/leptos/src/**`, `index.html`, `Trunk.toml`,
/// `public/**`, `vendor/**`), or `None` if there are none.
fn newest_source_mtime(root: &Path) -> Result<Option<std::time::SystemTime>> {
    let base = root.join("frontends/leptos");
    let mut newest = None;
    let mut consider = |mtime: std::time::SystemTime| {
        if newest.is_none_or(|n| mtime > n) {
            newest = Some(mtime);
        }
    };
    for file in ["index.html", "Trunk.toml"] {
        let path = base.join(file);
        if path.is_file() {
            consider(path.metadata()?.modified()?);
        }
    }
    for dir in ["src", "public", "vendor"] {
        visit_files(&base.join(dir), &mut consider)?;
    }
    Ok(newest)
}

/// Calls `visit` with the modification time of every file below `dir`
/// (recursively). A missing directory is fine.
fn visit_files(dir: &Path, visit: &mut impl FnMut(std::time::SystemTime)) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            visit_files(&path, visit)?;
        } else {
            visit(path.metadata()?.modified()?);
        }
    }
    Ok(())
}

/// Absolute path of the server binary, (re)building it first. The build
/// is a no-op when fresh; running it unconditionally closes the
/// stale-binary hole where tests exercised an old server.
fn ensure_server_binary(root: &Path) -> Result<PathBuf> {
    let bin = root.join("target/debug/flasher");
    let status = Command::new("cargo")
        .args(["build", "-p", "flasher-server"])
        .current_dir(root)
        .status()?;
    if !status.success() {
        return Err(Error::ServerBuildFailed(status));
    }
    if bin.is_file() {
        Ok(bin)
    } else {
        Err(Error::ServerBinaryMissing(bin))
    }
}

fn pick_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// Spawn `flasher` with `FLASHER_PORT`/`FLASHER_DIST`/`FLASHER_DB`
/// overrides and poll `GET /api/health` until it answers 200. Kills the
/// child if the server never becomes healthy.
///
/// [`Mode::DevBypass`] also sets `FLASHER_USER` = [`E2E_USER`];
/// [`Mode::PasskeyAuth`] leaves it unset and points the `WebAuthn`
/// relying-party config (`FLASHER_RP_ID`/`FLASHER_ORIGIN`) at the
/// localhost origin the browser actually visits.
async fn spawn_server(
    bin: &Path,
    root: &Path,
    dist: &Path,
    port: u16,
    base_url: &str,
    db_path: &Path,
    mode: Mode,
) -> Result<Child> {
    let mut command = Command::new(bin);
    command
        .current_dir(root)
        .env("FLASHER_PORT", port.to_string())
        .env("FLASHER_DIST", dist)
        .env("FLASHER_DB", db_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match mode {
        Mode::DevBypass => {
            command.env("FLASHER_USER", E2E_USER);
        }
        Mode::PasskeyAuth => {
            command
                .env("FLASHER_RP_ID", "localhost")
                .env("FLASHER_ORIGIN", base_url);
        }
    }
    let mut child = command.spawn()?;

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let deadline = Instant::now() + DEFAULT_TIMEOUT;
    loop {
        if let Ok(resp) = http.get(format!("{base_url}/api/health")).send().await
            && resp.status().is_success()
        {
            return Ok(child);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::HealthTimeout {
                url: base_url.to_owned(),
                timeout: DEFAULT_TIMEOUT,
            });
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Launch headless Chromium with a unique per-test profile dir (see
/// [`TestHarness::profile_dir`] for why this matters).
async fn launch_browser() -> Result<(Browser, JoinHandle<()>, TempDir)> {
    let profile_dir = TempDir::new()?;
    let config = BrowserConfig::builder()
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .window_size(1280, 800)
        .user_data_dir(profile_dir.path())
        .build()
        .map_err(Error::Message)?;
    let (browser, mut handler) = Browser::launch(config).await.map_err(Error::Cdp)?;
    let task = tokio::spawn(async move {
        while let Some(ev) = handler.next().await {
            if ev.is_err() {
                break;
            }
        }
    });
    Ok((browser, task, profile_dir))
}

/// Quote a Rust `&str` as a JS-safe string literal.
fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}
