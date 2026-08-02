//! Hand-rolled URL routing for the page nav (Phase 6.5), extended in
//! Phase 6.6 to carry the groom editor (`/groom/edit/{card_id}`) as a
//! real, reload-surviving route. The quiz's solution-revealed state is
//! deliberately NOT routed: it is transient in-memory state and a
//! browser refresh always starts collapsed.
//!
//! Deliberately not `leptos_router`: the app has a handful of flat
//! routes, so the whole router is two pure mapping layers
//! ([`path_to_route`]/[`path_to_tab`] and [`tab_to_path`], host-testable
//! under ssr) plus a thin csr-only layer over the History API
//! `pushState` on tab switch and editor open, `replaceState` for
//! canonicalization, a `popstate` listener for browser
//! back/forward). The popstate dispatch is on the full [`Route`], so
//! Back/Forward ONTO a `/groom/edit/{id}` entry re-opens the editor
//! (re-fetching the card, 404 → Groom tab with the URL rewritten)
//! instead of flattening to the tab underneath. The axum server already answers every non-`/api` path
//! with `index.html`, so any route loads the SPA and the client picks
//! the view.
//!
//! The editor overlay (Groom Edit, draft recovery) pushes one history
//! entry of its own — `/groom/edit/{card_id}` for a card edit, bare
//! `<tab>/edit` for a recovered new-card draft — so browser Back while
//! editing closes just the overlay (popstate back to the tab path)
//! instead of also leaving the tab. Saving or cancelling rewrites that
//! entry back to the tab path (`replaceState`), so no stale edit entries
//! pile up. A fresh load of `/groom/edit/{id}` fetches the card and
//! re-opens the editor on it (404 falls back to the Groom tab); a bare
//! `/groom/edit` still maps to Groom with no editor (there is no card id
//! to restore).
//!
//! The draft-recovery banner stays client-only state. Deep links
//! requested while logged out survive the auth flow: [`initial_route`]
//! is read once at startup into the tab signal, the auth screen ignores
//! `popstate` (a Back/Forward there must not clobber the stash), and a
//! successful login re-reads the location to restore the tab and the
//! editor.
//!
//! Every `location`/`history` access lives behind `#[cfg(feature =
//! "csr")]` with an ssr no-op twin, so the ssr build (and its host-target
//! tests) never touches a browser API; the ssr render defaults to Quiz.

// Browser API glue (window, location, history, the popstate listener) —
// csr only; the pure mapping above and the ssr build need none of it.
#[cfg(feature = "csr")]
use leptos::prelude::*;

/// The top-level pages of the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Review due cards (default).
    Quiz,
    /// Create a new card (the editor in new-card mode).
    AddCard,
    /// Search, page and maintain the whole collection.
    Groom,
    /// Identity, logout and passkey management.
    Account,
    /// Create, rename and delete the user's labels.
    Labels,
}

impl Tab {
    /// The visible heading for the top-level page represented by this tab.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Quiz => "Quiz",
            Self::AddCard => "Add card",
            Self::Groom => "Groom",
            Self::Account => "Account",
            Self::Labels => "Labels",
        }
    }
}

/// A URL the app can restore its full UI state from (Phase 6.6): beyond
/// the bare tabs, the groom editor is a real route that survives a
/// browser refresh. The quiz's reveal state is deliberately not part of
/// this: it is transient and always starts collapsed.
// The non-Tab variants are constructed only by `path_to_route` (csr and
// test builds); a pure ssr lib build only ever matches on them.
#[cfg_attr(not(any(feature = "csr", test)), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// One of the app's top-level pages.
    Tab(Tab),
    /// The groom editor open on a specific card (`/groom/edit/{id}`).
    GroomEdit(String),
}

impl Route {
    /// The tab the route renders on top of.
    pub fn tab(&self) -> Tab {
        match self {
            Self::Tab(tab) => *tab,
            Self::GroomEdit(_) => Tab::Groom,
        }
    }
}

/// Maps a URL path to a full route. Static segments match
/// case-insensitively and ignore trailing slashes; the card id of a
/// groom edit path keeps its original case. Anything unknown — including
/// typos the server answers with the SPA fallback, `/` itself, a bare
/// `/groom/edit` (no card id to restore) and malformed edit paths —
/// falls back to the tab mapping of [`path_to_tab`].
// Same gating as [`path_to_tab`]: the csr location glue and the tests.
#[cfg(any(feature = "csr", test))]
pub fn path_to_route(path: &str) -> Route {
    let trimmed = path.trim_end_matches('/');
    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("/groom/edit/") {
        // The id is sliced out of the ORIGINAL path (ids are
        // case-sensitive; the static prefix is not). More than one
        // segment is not an edit route and falls back like any typo.
        let id = &trimmed[trimmed.len() - rest.len()..];
        if !id.contains('/') {
            return Route::GroomEdit(id.to_owned());
        }
    }
    Route::Tab(path_to_tab(path))
}

/// Maps a URL path to a tab. Anything unknown — including typos the
/// server answers with the SPA fallback, and `/` itself — falls back to
/// Quiz. Trailing slashes and case differences are normalized away. A
/// trailing `/edit` segment (the editor overlay's history entry, see
/// [`push_edit`]) is stripped first, so edit paths fall back to the tab
/// underneath them.
// Compiled for csr (where the location glue uses it) and for tests (the
// host-target ssr test build exercises the pure mapping); an ssr render
// defaults to Quiz without it.
#[cfg(any(feature = "csr", test))]
pub fn path_to_tab(path: &str) -> Tab {
    let normalized = path.trim_end_matches('/').to_ascii_lowercase();
    let base = normalized
        .strip_suffix("/edit")
        .map_or(normalized.as_str(), |stem| stem.trim_end_matches('/'));
    match base {
        "/add" => Tab::AddCard,
        "/groom" => Tab::Groom,
        "/account" => Tab::Account,
        "/labels" => Tab::Labels,
        _ => Tab::Quiz,
    }
}

/// The canonical path of a tab (inverse of [`path_to_tab`]).
// Same gating as [`path_to_tab`]: only the csr history glue and the
// tests need it.
#[cfg(any(feature = "csr", test))]
pub fn tab_to_path(tab: Tab) -> &'static str {
    match tab {
        Tab::Quiz => "/quiz",
        Tab::AddCard => "/add",
        Tab::Groom => "/groom",
        Tab::Account => "/account",
        Tab::Labels => "/labels",
    }
}

/// The route the current URL asks for at startup: the location's path
/// under csr, the quiz tab under ssr (no location exists server-side).
pub fn initial_route() -> Route {
    #[cfg(feature = "csr")]
    {
        path_to_route(&current_path())
    }
    #[cfg(not(feature = "csr"))]
    {
        Route::Tab(Tab::Quiz)
    }
}

/// The tab the current URL asks for at startup (the tab part of
/// [`initial_route`]; the richer route state is restored separately).
pub fn initial_tab() -> Tab {
    initial_route().tab()
}

/// The browser's current `location.pathname`.
#[cfg(feature = "csr")]
pub fn current_path() -> String {
    window()
        .location()
        .pathname()
        .unwrap_or_else(|_| "/".to_owned())
}

/// Pushes the tab's path onto the history stack on a tab switch. A no-op
/// when the URL already matches (re-clicking the active tab must not pile
/// up duplicate history entries, and the entry created by the initial
/// load is already right).
#[cfg(feature = "csr")]
pub fn push_tab(tab: Tab) {
    let path = tab_to_path(tab);
    if current_path() != path {
        with_history(|history| {
            _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path));
        });
    }
}

/// Rewrites the current history entry to the tab's path without adding an
/// entry — used once at startup to canonicalize `/` to `/quiz`, and when
/// the editor overlay closes via Save/Cancel to turn its `/edit` entry
/// back into the tab underneath (no stale `/edit` entries on the stack).
#[cfg(feature = "csr")]
pub fn replace_tab(tab: Tab) {
    let path = tab_to_path(tab);
    if current_path() != path {
        with_history(|history| {
            _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path));
        });
    }
}

/// Pushes the editor overlay's entry (`<tab>/edit`) when an editing
/// session opens over a tab. One entry per session: browser Back while
/// editing then pops just the overlay (popstate back to the tab path)
/// instead of also navigating away from the tab.
#[cfg(feature = "csr")]
pub fn push_edit(tab: Tab) {
    let path = format!("{}/edit", tab_to_path(tab));
    if current_path() != path {
        with_history(|history| {
            _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path));
        });
    }
}

/// Pushes the groom editor's entry (`/groom/edit/{id}`) when an edit
/// session for a known card opens. Unlike the bare [`push_edit`] path
/// this one is a real route: a fresh load re-opens the editor on the
/// card. One entry per session, so browser Back closes just the overlay.
#[cfg(feature = "csr")]
pub fn push_groom_edit(id: &str) {
    let path = format!("{}/edit/{id}", tab_to_path(Tab::Groom));
    if current_path() != path {
        with_history(|history| {
            _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&path));
        });
    }
}

/// Registers the `popstate` handler (browser back/forward): the URL has
/// already changed, so the UI just follows — no `pushState` here or the
/// stack would grow on every back. The dispatch is on the full [`Route`]
/// (not just the tab): Back/Forward ONTO a `/groom/edit/{id}` entry must
/// re-open the editor, not flatten it to the tab underneath while the
/// URL still names the card. The returned handle is kept alive by the
/// caller for the app's lifetime.
#[cfg(feature = "csr")]
pub fn on_popstate(on_navigate: impl Fn(Route) + 'static) -> WindowListenerHandle {
    window_event_listener_untyped("popstate", move |_| {
        on_navigate(path_to_route(&current_path()));
    })
}

/// ssr twin of [`push_tab`]: no history exists server-side; tab clicks
/// never fire there either.
#[cfg(not(feature = "csr"))]
pub fn push_tab(_tab: Tab) {}

/// ssr twin of [`replace_tab`]: no history exists server-side.
#[cfg(not(feature = "csr"))]
pub fn replace_tab(_tab: Tab) {}

/// ssr twin of [`push_edit`]: no history exists server-side.
#[cfg(not(feature = "csr"))]
pub fn push_edit(_tab: Tab) {}

/// ssr twin of [`push_groom_edit`]: no history exists server-side.
#[cfg(not(feature = "csr"))]
pub fn push_groom_edit(_id: &str) {}

/// Runs `f` with the browser's `history` object when available (it always
/// is in a real browser; the graceful no-op keeps the wasm module from
/// panicking in exotic embeddings).
#[cfg(feature = "csr")]
fn with_history(f: impl FnOnce(&web_sys::History)) {
    if let Ok(history) = window().history() {
        f(&history);
    }
}

#[cfg(test)]
mod tests {
    use super::{Route, Tab, path_to_route, path_to_tab, tab_to_path};

    #[test]
    fn known_paths_map_to_their_tabs() {
        assert_eq!(path_to_tab("/quiz"), Tab::Quiz);
        assert_eq!(path_to_tab("/add"), Tab::AddCard);
        assert_eq!(path_to_tab("/groom"), Tab::Groom);
        assert_eq!(path_to_tab("/account"), Tab::Account);
        assert_eq!(path_to_tab("/labels"), Tab::Labels);
    }

    #[test]
    fn root_and_unknown_paths_fall_back_to_quiz() {
        assert_eq!(path_to_tab("/"), Tab::Quiz);
        assert_eq!(path_to_tab(""), Tab::Quiz);
        assert_eq!(path_to_tab("/nope"), Tab::Quiz);
        assert_eq!(path_to_tab("/groom/xyz"), Tab::Quiz);
        assert_eq!(path_to_tab("/api/cards"), Tab::Quiz);
    }

    #[test]
    fn trailing_slash_is_normalized() {
        assert_eq!(path_to_tab("/groom/"), Tab::Groom);
        assert_eq!(path_to_tab("/account/"), Tab::Account);
    }

    #[test]
    fn edit_paths_fall_back_to_the_tab_underneath() {
        assert_eq!(path_to_tab("/groom/edit"), Tab::Groom);
        assert_eq!(path_to_tab("/add/edit"), Tab::AddCard);
        assert_eq!(path_to_tab("/account/edit"), Tab::Account);
        assert_eq!(path_to_tab("/quiz/edit"), Tab::Quiz);
        assert_eq!(path_to_tab("/groom/edit/"), Tab::Groom);
        // A bare or unknown edit path is the default tab.
        assert_eq!(path_to_tab("/edit"), Tab::Quiz);
        assert_eq!(path_to_tab("/nope/edit"), Tab::Quiz);
    }

    #[test]
    fn case_is_normalized() {
        assert_eq!(path_to_tab("/GROOM"), Tab::Groom);
        assert_eq!(path_to_tab("/Add"), Tab::AddCard);
        assert_eq!(path_to_tab("/Account"), Tab::Account);
        assert_eq!(path_to_tab("/LABELS"), Tab::Labels);
    }

    #[test]
    fn tab_paths_roundtrip() {
        for tab in [
            Tab::Quiz,
            Tab::AddCard,
            Tab::Groom,
            Tab::Labels,
            Tab::Account,
        ] {
            assert_eq!(path_to_tab(tab_to_path(tab)), tab);
        }
    }

    #[test]
    fn tabs_have_visible_page_titles() {
        assert_eq!(Tab::Quiz.title(), "Quiz");
        assert_eq!(Tab::AddCard.title(), "Add card");
        assert_eq!(Tab::Groom.title(), "Groom");
        assert_eq!(Tab::Labels.title(), "Labels");
        assert_eq!(Tab::Account.title(), "Account");
    }

    #[test]
    fn legacy_quiz_solution_falls_back_to_the_quiz_tab() {
        // The retired `/quiz/solution` route (reveal state used to be
        // mirrored into the URL) is no longer special: a stale bookmark
        // or refresh on it just loads the quiz, collapsed.
        assert_eq!(path_to_route("/quiz/solution"), Route::Tab(Tab::Quiz));
        assert_eq!(path_to_route("/quiz/solution/"), Route::Tab(Tab::Quiz));
        assert_eq!(path_to_route("/QUIZ/SOLUTION"), Route::Tab(Tab::Quiz));
    }

    #[test]
    fn groom_edit_with_id_is_a_real_route() {
        assert_eq!(
            path_to_route("/groom/edit/card-1"),
            Route::GroomEdit("card-1".to_owned())
        );
        // The id keeps its original case; the static prefix does not.
        assert_eq!(
            path_to_route("/GROOM/Edit/AbC-123"),
            Route::GroomEdit("AbC-123".to_owned())
        );
        // Trailing slashes are normalized away.
        assert_eq!(
            path_to_route("/groom/edit/card-1/"),
            Route::GroomEdit("card-1".to_owned())
        );
        // It renders on the groom tab.
        assert_eq!(path_to_route("/groom/edit/card-1").tab(), Tab::Groom);
    }

    #[test]
    fn popstate_dispatches_on_the_full_route() {
        // The popstate listener maps the new location with
        // `path_to_route` (not `path_to_tab`): an editor entry stays an
        // editor route so Back/Forward onto it can re-open the editor
        // instead of flattening to the tab underneath.
        assert_eq!(
            path_to_route("/groom/edit/card-9"),
            Route::GroomEdit("card-9".to_owned())
        );
        assert_eq!(path_to_route("/add"), Route::Tab(Tab::AddCard));
    }

    #[test]
    fn malformed_edit_paths_fall_back() {
        // A bare /groom/edit has no card id to restore: the groom tab.
        assert_eq!(path_to_route("/groom/edit"), Route::Tab(Tab::Groom));
        assert_eq!(path_to_route("/groom/edit/"), Route::Tab(Tab::Groom));
        // More than one segment below /edit is not an edit route.
        assert_eq!(path_to_route("/groom/edit/a/b"), Route::Tab(Tab::Quiz));
        // Edit ids only exist for the groom tab.
        assert_eq!(path_to_route("/add/edit/card-1"), Route::Tab(Tab::Quiz));
        assert_eq!(path_to_route("/quiz/edit/card-1"), Route::Tab(Tab::Quiz));
    }
}
