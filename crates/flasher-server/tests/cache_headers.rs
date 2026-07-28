//! Integration test: `Cache-Control` per response class — content-hashed
//! Trunk assets immutable for a year, HTML (incl. the SPA fallback) and
//! the API always revalidated, other static files a short cache.

use flasher_auth::Auth;
use flasher_server::{AppState, serve};
use flasher_store::Store;
use tokio::net::TcpListener;

type TestResult = Result<(), Box<dyn std::error::Error>>;
type ServerHandle = tokio::task::JoinHandle<std::io::Result<()>>;

async fn start() -> Result<(String, ServerHandle), Box<dyn std::error::Error>> {
    let dist = std::env::temp_dir().join(format!("flasher-cache-dist-{}", std::process::id()));
    std::fs::create_dir_all(&dist)?;
    std::fs::write(dist.join("index.html"), "<h1>flasher</h1>")?;
    std::fs::write(dist.join("robots.txt"), "User-agent: *\n")?;
    std::fs::write(
        dist.join("flasher-leptos-c37081118a308d1d.js"),
        "// hashed bundle\n",
    )?;

    let store = Store::connect_in_memory().await?;
    let user = store.upsert_user("test").await?;
    let auth = Auth::new("localhost", "http://localhost:3000", "flasher")?;
    let state = AppState::dev_bypass(store, auth, user.id);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let server = tokio::spawn(serve(listener, dist, state));
    Ok((format!("http://{addr}"), server))
}

fn cache_control(resp: &reqwest::Response) -> Result<String, Box<dyn std::error::Error>> {
    Ok(resp
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .ok_or("response has no Cache-Control")?
        .to_str()?
        .to_owned())
}

#[tokio::test]
async fn cache_control_matches_the_response_class() -> TestResult {
    let (base, server) = start().await?;

    // Content-hashed Trunk asset: immutable for a year.
    let resp = reqwest::get(format!("{base}/flasher-leptos-c37081118a308d1d.js")).await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(cache_control(&resp)?, "public, max-age=31536000, immutable");

    // A MISSING hashed-looking asset falls through to the SPA fallback:
    // 200 text/html — and must get no-cache like any HTML, never the
    // immutable year-long cache (which would pin the error page).
    let resp = reqwest::get(format!("{base}/flasher-leptos-0000000000000000.js")).await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .ok_or("no Content-Type")?
            .to_str()?,
        "text/html"
    );
    assert_eq!(cache_control(&resp)?, "no-cache");

    // HTML (the SPA fallback serves index.html): always revalidated.
    let resp = reqwest::get(format!("{base}/some/client/route")).await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(cache_control(&resp)?, "no-cache");

    // API JSON: no-cache even though the body is not HTML.
    let resp = reqwest::get(format!("{base}/api/health")).await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(cache_control(&resp)?, "no-cache");

    // Unhashed static file: short cache only.
    let resp = reqwest::get(format!("{base}/robots.txt")).await?;
    assert_eq!(resp.status(), 200);
    assert_eq!(cache_control(&resp)?, "public, max-age=3600");

    server.abort();
    Ok(())
}
