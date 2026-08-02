//! Flasher Leptos frontend (CSR).
//!
//! Phase 5B added passkey authentication: on startup the app asks
//! `GET /api/auth/session` who it is talking to. A 200 `null` (auth mode,
//! no session) swaps the whole app for the centered auth screen
//! ([`AuthScreen`]) — first-run register or one-button login, decided by
//! `GET /api/auth/bootstrap`. A 200 with the user means logged in; a
//! second probe with cookies suppressed still answers with the user only
//! in dev-bypass mode, which is how the UI tells the two modes apart. Any
//! mid-session 401 from a data API call bounces back to the auth screen
//! via [`api::on_unauthorized`] (auth ceremony 401s are surfaced locally
//! instead — a failed ceremony says nothing about the session).
//! Logged-in users get dedicated Account and Labels pages with passkey and
//! label management respectively.
//!
//! Phase 6.5 added hand-rolled URL routing (see [`route`]): each page owns
//! a path (`/quiz`, `/add`, `/groom`, `/labels`, `/account`), page switches push it
//! via the History API, browser back/forward follows via `popstate`, and
//! a deep link requested while logged out survives the auth flow (the
//! auth screen ignores `popstate`; a successful login re-reads the
//! location). The editor overlay pushes one `/groom/edit/{id}` entry
//! (bare `<tab>/edit` for a recovered new-card draft) when it opens, so
//! Back while editing closes just the overlay — and Back/Forward ONTO
//! such an entry re-opens the editor on the card (the popstate dispatch
//! is on the full route, not just the tab). Any overlay close that
//! was neither Save nor Cancel (tab switch, Back) leaves the server-side
//! autosave draft behind, so it re-fetches `GET /api/autosave` and
//! re-arms the recovery banner immediately — the orphaned draft never
//! stays invisible until the next app start.
//!
//! Phase 6.6 made the UI session state survive a browser refresh:
//! a fresh load of `/groom/edit/{id}` fetches the card and re-opens the
//! editor on it (404 falls back to the Groom tab with the URL
//! rewritten). When the
//! restored editor and the server-side draft match (same card for the
//! groom editor, a new-card draft for the Add card tab), the editor is
//! prefilled with the draft content — F5 becomes a mini crash recovery
//! — and the banner is suppressed for that draft; a non-matching draft
//! still prompts the banner. Only fresh-load restores prefill: opening
//! the editor by clicking Groom's Edit keeps the banner as the recovery
//! surface, so an in-progress session is never silently overwritten.
//!
//! Phase 4C: top-level pages switched client-side via the responsive nav —
//! Quiz (review due cards, the default), Add card (the card editor in
//! new-card mode), Groom (search, page and maintain the whole collection),
//! Labels (label CRUD), and Account. The same editor opens over the tabs when a Groom row's
//! Edit button is clicked or an autosave draft is recovered. On mount
//! the app checks `GET /api/autosave` once; a leftover draft shows a
//! dismissible recovery banner (Recover opens the editor with the draft,
//! Discard deletes it). A small health line keeps proving the
//! same-origin `/api/health` round-trip through the shared
//! `flasher-types` contract crate.

// Leptos components are consumed by the `view!` macro, never called as
// plain functions, so the pedantic must-use lint is noise here.
#![allow(clippy::must_use_candidate)]

mod api;
mod auth;
mod editor;
mod groom;
mod labels;
mod markdown;
mod quiz;
mod route;
mod webauthn;

use auth::{Account, AuthScreen};
use editor::{CloseOutcome, Editor};
use flasher_types::{AutoSaveResponse, HealthResponse};
use groom::Groom;
use labels::LabelManager;
use leptos::prelude::*;
use quiz::Quiz;
use route::Tab;

// Re-exported so host-target ssr tests can render the views directly.
pub use auth::{Account as AccountTab, AuthScreen as AuthScreenTab};
pub use editor::{CloseOutcome as EditorCloseOutcome, EditTarget, Editor as EditorTab};
pub use groom::Groom as GroomTab;
pub use labels::LabelManager as LabelManagerTab;
pub use quiz::Quiz as QuizTab;

/// Where the startup session check stands. The app starts in `Checking`
/// (a neutral splash); the session probe then decides between the auth
/// screen and the app. Any mid-session API 401 flips an `Authed` state
/// back to `Unauthenticated`.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// The session probe has not answered yet.
    Checking,
    /// Auth mode without a session: show the auth screen.
    Unauthenticated,
    /// Logged in (real session or dev bypass).
    Authed {
        /// The username behind the session.
        username: String,
        /// Dev-bypass mode (`FLASHER_USER`): hide the logout button.
        dev_mode: bool,
    },
}

/// Root application component.
// Nav, recovery banner, the auth gate and the tab/editor switch make
// this long; splitting them further would only add indirection.
#[cfg(feature = "csr")]
fn focus_nav_toggle() {
    use wasm_bindgen::JsCast;

    let Some(element) = leptos::prelude::document().get_element_by_id("nav-toggle") else {
        return;
    };
    if let Ok(button) = element.dyn_into::<web_sys::HtmlElement>() {
        let _ = button.focus();
    }
}

fn close_navigation(nav_open: RwSignal<bool>, nav_narrow: RwSignal<bool>) {
    nav_open.set(false);
    #[cfg(not(feature = "csr"))]
    let _ = nav_narrow;
    #[cfg(feature = "csr")]
    if nav_narrow.get_untracked() {
        focus_nav_toggle();
    }
}

#[allow(clippy::too_many_lines)]
#[component]
pub fn App() -> impl IntoView {
    let auth_state = RwSignal::new(AuthState::Checking);
    // The tab comes from the URL at startup (Phase 6.5), so a deep link
    // requested while logged out survives the auth flow: the auth screen
    // never touches this signal (popstate is ignored while
    // unauthenticated) and a successful login re-reads the location to
    // select the tab. Under ssr there is no location; Quiz then.
    let tab = RwSignal::new(route::initial_tab());
    // An open editing session overlays the tabs (Groom Edit, draft
    // recovery, a `/groom/edit/{id}` deep link); `None` shows the
    // selected tab. Opening the overlay pushes a `/groom/edit/{id}`
    // history entry (see `route::push_groom_edit`), so browser Back
    // closes just the overlay; a fresh load of that URL re-opens the
    // editor on the card (Phase 6.6, see the restore effect below).
    let editing = RwSignal::new(None::<EditTarget>);
    // The startup draft check (re-armed whenever an editing session is
    // abandoned): `None` = not answered yet, `Some(None)` = no draft (or
    // the check failed — no banner then), `Some(Some(draft))` = show the
    // recovery banner.
    let draft = RwSignal::new(None::<Option<AutoSaveResponse>>);
    let health = RwSignal::new(None::<Result<HealthResponse, String>>);
    // The responsive navigation is a persistent labeled sidebar on wide
    // screens and the same closed-by-default labeled drawer below that
    // breakpoint. The signal only controls the drawer; desktop presentation
    // is CSS-driven so resizing does not mutate application state.
    let nav_open = RwSignal::new(false);
    // Mirrors the drawer breakpoint so the closed drawer can be removed
    // from the accessibility tree without hiding the wide sidebar.
    let nav_narrow = RwSignal::new(false);
    // Phase 6.6: `false` until the fresh-load session restore (draft
    // check + `/groom/edit/{id}` editor re-open) has settled, so the
    // Add card tab's editor never mounts blank only to be prefilled a
    // moment later. `add_prefill` carries the matching new-card draft
    // into the Add tab's editor; non-reactive and consumed on first
    // render, so later switches to the tab start blank.
    let restored = RwSignal::new(false);
    let add_prefill = StoredValue::new(None::<AutoSaveResponse>);

    // Effects only run client-side (csr); under ssr this is compiled out.
    #[cfg(feature = "csr")]
    {
        // URL routing (Phase 6.5): canonicalize `/` to `/quiz` (no new
        // history entry), and follow browser back/forward — the URL has
        // already changed on popstate, so the tab just follows (no push,
        // or the stack would grow on every back). StoredValue keeps the
        // listener handle alive for the app's lifetime.
        if route::current_path() == "/" {
            route::replace_tab(Tab::Quiz);
        }
        let popstate = route::on_popstate(move |next| {
            // While unauthenticated the tab signal holds the stashed
            // deep link: a Back/Forward on the auth screen must not
            // clobber it — the tab is re-read from the location on
            // login instead.
            if !matches!(auth_state.get_untracked(), AuthState::Authed { .. }) {
                return;
            }
            match next {
                // Back/Forward ONTO an editor URL (F1): re-fetch the
                // card and re-open the editor, mirroring the fresh-load
                // restore (404 → Groom tab, URL rewritten; a matching
                // draft prefills inline, anything else banners).
                route::Route::GroomEdit(id) => {
                    leptos::task::spawn_local(async move {
                        let found = api::get_autosave().await.unwrap_or_default();
                        if let Some(target) = groom_edit_restore(&id, found.as_ref(), draft).await {
                            tab.set(Tab::Groom);
                            editing.set(Some(target));
                        } else {
                            editing.set(None);
                            tab.set(Tab::Groom);
                            route::replace_tab(Tab::Groom);
                        }
                    });
                }
                // A plain tab route just selects the tab.
                route::Route::Tab(_) => {
                    // Back/Forward with the editor overlay open pops its
                    // /edit entry and closes just the overlay; leaving
                    // the Add card tab abandons its editor the same way.
                    // Either close was neither Save nor Cancel, so a
                    // server-side draft may have been orphaned: re-arm
                    // the recovery banner right away.
                    let abandoned =
                        editing.get_untracked().is_some() || tab.get_untracked() == Tab::AddCard;
                    editing.set(None);
                    tab.set(next.tab());
                    if abandoned {
                        leptos::task::spawn_local(async move {
                            draft.set(Some(api::get_autosave().await.unwrap_or_default()));
                        });
                    }
                }
            }
        });
        let _keep = StoredValue::new(popstate);
        let update_nav_narrow = move || {
            let narrow = leptos::prelude::window()
                .inner_width()
                .ok()
                .and_then(|width| width.as_f64())
                .is_some_and(|width| width <= 1_024.0);
            nav_narrow.set(narrow);
        };
        update_nav_narrow();
        // Sticky chrome (owner wish 2026-07-31): export the header's
        // measured height as --top-h so the groom tab's sticky
        // controls/paging bar parks directly below it. Re-measured once
        // the authed shell first renders (the header does not exist
        // before) and on every resize (the header flex-wraps to two rows
        // on narrow screens).
        Effect::new(move |_| {
            if matches!(auth_state.get(), AuthState::Authed { .. }) {
                export_top_height();
            }
        });
        let resize = window_event_listener_untyped("resize", move |_| {
            export_top_height();
            update_nav_narrow();
        });
        let _keep_resize = StoredValue::new(resize);
        // Any mid-session 401 (expired session) bounces back to the auth
        // screen. In dev-bypass mode 401s never occur. The first-load
        // restore state is reset too (F2): without it a stale
        // `restored == true` would let the Add tab mount blank after
        // re-login while the orphaned draft stayed invisible (banner
        // suppressed, prefill already consumed) until the next autosave
        // tick overwrote it. Explicit logout does NOT go through here —
        // it navigates to /quiz first, so there is no draft to lose.
        api::on_unauthorized(move || {
            restored.set(false);
            add_prefill.set_value(None);
            auth_state.set(AuthState::Unauthenticated);
        });
        // Startup session probe: 200 with the user → app; 200 `null` (or
        // an error) → auth screen. A second probe with cookies suppressed
        // tells dev bypass (still answers with the user) from a real
        // session (200 `null` without the cookie).
        Effect::new(move |_| {
            leptos::task::spawn_local(async move {
                let next = match api::session(true).await {
                    Ok(Some(username)) => {
                        let dev_mode = matches!(api::session(false).await, Ok(Some(_)));
                        AuthState::Authed { username, dev_mode }
                    }
                    // A network/server error also lands on the auth
                    // screen, whose bootstrap fetch then shows the error.
                    Ok(None) | Err(_) => AuthState::Unauthenticated,
                };
                auth_state.set(next);
            });
        });
        // The health line and the fresh-load session restore (Phase
        // 6.6) only make sense once logged in; re-runs on each login.
        // The restore reads the URL: a `/groom/edit/{id}` deep link
        // re-opens the editor on that card (404 → groom tab, URL
        // rewritten), and a server-side draft MATCHING the restored
        // editor (same card id; a new-card draft for the Add card tab)
        // prefills it inline — F5 as a mini crash recovery — instead of
        // prompting the recovery banner. A draft for anything else
        // still prompts the banner. `restored` holds back the tab view
        // until this settles (one GET round-trip), so the Add tab's
        // editor never mounts blank and then gets recreated prefilled.
        Effect::new(move |_| {
            if matches!(auth_state.get(), AuthState::Authed { .. }) {
                // Capture the route SYNCHRONOUSLY, before any await
                // (F5): reading the location only after the fetches
                // would restore whatever URL a fast tab click left
                // behind, yanking the user back when the restore lands.
                let start_route = route::initial_route();
                leptos::task::spawn_local(async move {
                    health.set(Some(api::health().await));
                    let found = api::get_autosave().await.unwrap_or_default();
                    // Re-check after the awaits (F5): if the user
                    // navigated to another tab during the fetch window,
                    // drop the restore instead of yanking them back —
                    // but always release the `restored` gate below.
                    if tab.get_untracked() == start_route.tab() {
                        // The wildcard is every Tab route except AddCard —
                        // spelling the three variants out would only add
                        // noise (and a newer clippy flags the wildcard).
                        #[allow(clippy::match_wildcard_for_single_variants)]
                        match start_route {
                            route::Route::GroomEdit(id) => {
                                if let Some(target) =
                                    groom_edit_restore(&id, found.as_ref(), draft).await
                                {
                                    editing.set(Some(target));
                                } else {
                                    // The card is gone (or the fetch
                                    // failed): the groom tab, URL
                                    // rewritten to match; a leftover
                                    // draft still prompts the banner.
                                    route::replace_tab(Tab::Groom);
                                }
                            }
                            route::Route::Tab(Tab::AddCard) => match &found {
                                // A new-card draft matches the Add tab's
                                // editor: prefill it, no banner.
                                Some(d) if d.card_id.is_none() => {
                                    add_prefill.set_value(Some(d.clone()));
                                }
                                _ => draft.set(Some(found)),
                            },
                            _ => draft.set(Some(found)),
                        }
                    }
                    restored.set(true);
                });
            }
        });
    }

    // The one place every user-driven tab switch goes through: close an
    // open editor overlay, select the tab and push its path onto the
    // history stack (Phase 6.5). `popstate` is the only other source of
    // tab changes and deliberately does not push. A tab switch that
    // abandons an open overlay was neither Save nor Cancel, so the
    // autosave draft it leaves behind re-arms the recovery banner (same
    // as the popstate path above).
    let navigate = Callback::new(move |next: Tab| {
        // Clicking the already-active tab is a no-op while no editor
        // overlay is open (F3): RwSignal::set notifies unconditionally,
        // so without this guard the click would remount the tab
        // component and wipe unsaved state (the Add editor's text, the
        // groom search/page, the quiz's reveal — plus a junk history
        // entry). With an overlay open the click IS meaningful: it
        // closes the overlay back to the tab underneath.
        if next == tab.get_untracked() && editing.get_untracked().is_none() {
            close_navigation(nav_open, nav_narrow);
            return;
        }
        close_navigation(nav_open, nav_narrow);
        let abandoned = editing.get_untracked().is_some() || tab.get_untracked() == Tab::AddCard;
        editing.set(None);
        tab.set(next);
        route::push_tab(next);
        if abandoned {
            leptos::task::spawn_local(async move {
                draft.set(Some(api::get_autosave().await.unwrap_or_default()));
            });
        }
    });

    // Successful login on the auth screen: straight into the app. The
    // tab is (re-)read from the location — the auth screen ignored
    // popstate, so the URL is the one source of truth for where the
    // user wanted to go.
    let on_login = Callback::new(move |username: String| {
        tab.set(route::initial_tab());
        auth_state.set(AuthState::Authed {
            username,
            dev_mode: false,
        });
    });

    // Logout from the Account tab: back to the auth screen.
    let on_logout = Callback::new(move |(): ()| {
        navigate.run(Tab::Quiz);
        auth_state.set(AuthState::Unauthenticated);
    });

    // Save of an existing card returns to Groom (fresh list); Cancel
    // returns where the session came from (Groom for an edit, Quiz for
    // the Add card tab). The editor unmounts either way, which stops
    // its autosave interval; Cancel has already deleted the draft
    // server-side and Save (edit mode) let the server's PATCH clear it,
    // so neither re-arms the recovery banner. An overlay close rewrites
    // its `<tab>/edit` history entry back to the tab path (no stale
    // entry for Back to trip over); closing the Add card tab's editor
    // pushes like any other tab switch.
    let on_editor_close = Callback::new(move |outcome: CloseOutcome| {
        let was_overlay = editing.get_untracked().is_some();
        editing.set(None);
        let target = match outcome {
            CloseOutcome::Saved => Tab::Groom,
            CloseOutcome::Cancelled if was_overlay => Tab::Groom,
            CloseOutcome::Cancelled => Tab::Quiz,
        };
        tab.set(target);
        if was_overlay {
            route::replace_tab(target);
        } else {
            route::push_tab(target);
        }
    });

    let open_edit = Callback::new(move |card: flasher_types::CardResponse| {
        route::push_groom_edit(&card.id);
        editing.set(Some(EditTarget::edit(&card)));
    });

    // Recover: open the editor with the draft — edit mode when the
    // draft's card still exists, new-card mode otherwise. The banner is
    // hidden immediately and not shown again while the editor is open
    // (abandoning that session re-arms it via the navigate/popstate
    // paths above). The overlay's history entry is the real
    // `/groom/edit/{id}` route for an existing card (reload-safe), the
    // bare `<tab>/edit` entry for a new-card draft.
    let recover = move |_| {
        if let Some(Some(found)) = draft.get_untracked() {
            draft.set(Some(None));
            leptos::task::spawn_local(async move {
                let still_exists = match &found.card_id {
                    Some(id) => api::get_card(id).await.ok().flatten().is_some(),
                    None => false,
                };
                match &found.card_id {
                    Some(id) if still_exists => route::push_groom_edit(id),
                    _ => route::push_edit(tab.get_untracked()),
                }
                editing.set(Some(EditTarget::from_draft(&found, still_exists)));
            });
        }
    };

    // Discard: drop the draft server-side and hide the banner. A failed
    // delete leaves the draft for the next app start to re-prompt.
    let discard = move |_| {
        if let Some(Some(_)) = draft.get_untracked() {
            draft.set(Some(None));
            leptos::task::spawn_local(async move {
                _ = api::delete_autosave().await;
            });
        }
    };

    view! {
        {move || match auth_state.get() {
            AuthState::Checking => {
                view! {
                    <main class="auth-screen" id="auth-checking">
                        <section class="auth-card">
                            <h1>"Flasher"</h1>
                            <p class="auth-hint">"Loading…"</p>
                        </section>
                    </main>
                }
                    .into_any()
            }
            AuthState::Unauthenticated => {
                view! { <AuthScreen on_login=on_login/> }.into_any()
            }
            AuthState::Authed { username, dev_mode } => {
                view! {
        <main
            class="app-shell"
            on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                if ev.key() == "Escape" && nav_open.get_untracked() {
                    ev.prevent_default();
                    close_navigation(nav_open, nav_narrow);
                }
            }
        >
            <aside
                class:open=move || nav_open.get()
                class="side-nav"
                id="primary-nav"
                aria-hidden=move || (nav_narrow.get() && !nav_open.get()).to_string()
                inert=move || nav_narrow.get() && !nav_open.get()
            >
                <div class="side-nav-brand">
                    <img src="/favicon.svg" alt="" class="side-nav-logo"/>
                    <span class="side-nav-name">"Flasher"</span>
                    <button
                        type="button"
                        class="nav-close"
                        id="nav-close"
                        aria-label="Close navigation"
                        on:click=move |_| close_navigation(nav_open, nav_narrow)
                    >
                        <span aria-hidden="true">"×"</span>
                    </button>
                </div>
                <nav class="side-nav-links" aria-label="Primary navigation">
                    <button
                        type="button"
                        id="tab-quiz"
                        class="nav-item"
                        aria-label="Quiz"
                        aria-current=move || if tab.get() == Tab::Quiz { "page" } else { "false" }
                        class:active=move || tab.get() == Tab::Quiz
                        on:click=move |_| navigate.run(Tab::Quiz)
                    >
                        <span class="nav-label">"Quiz"</span>
                    </button>
                    <button
                        type="button"
                        id="tab-add-card"
                        class="nav-item"
                        aria-label="Add card"
                        aria-current=move || if tab.get() == Tab::AddCard { "page" } else { "false" }
                        class:active=move || tab.get() == Tab::AddCard
                        on:click=move |_| navigate.run(Tab::AddCard)
                    >
                        <span class="nav-label">"Add card"</span>
                    </button>
                    <button
                        type="button"
                        id="tab-groom"
                        class="nav-item"
                        aria-label="Groom"
                        aria-current=move || if tab.get() == Tab::Groom { "page" } else { "false" }
                        class:active=move || tab.get() == Tab::Groom
                        on:click=move |_| navigate.run(Tab::Groom)
                    >
                        <span class="nav-label">"Groom"</span>
                    </button>
                    <button
                        type="button"
                        id="tab-labels"
                        class="nav-item"
                        aria-label="Labels"
                        aria-current=move || if tab.get() == Tab::Labels { "page" } else { "false" }
                        class:active=move || tab.get() == Tab::Labels
                        on:click=move |_| navigate.run(Tab::Labels)
                    >
                        <span class="nav-label">"Labels"</span>
                    </button>
                    <button
                        type="button"
                        id="tab-account"
                        class="nav-item"
                        aria-label="Account"
                        aria-current=move || if tab.get() == Tab::Account { "page" } else { "false" }
                        class:active=move || tab.get() == Tab::Account
                        on:click=move |_| navigate.run(Tab::Account)
                    >
                        <span class="nav-label">"Account"</span>
                    </button>
                </nav>
                <div class="side-nav-footer">
                    <span class="side-nav-user">{username.clone()}</span>
                </div>
            </aside>
            {move || nav_open.get().then(|| view! {
                <button
                    type="button"
                    class="nav-backdrop"
                    id="nav-backdrop"
                    aria-label="Close navigation"
                    on:click=move |_| close_navigation(nav_open, nav_narrow)
                ></button>
            })}
            <div class="app">
                <header class="top">
                    <button
                        type="button"
                        class="nav-toggle"
                        id="nav-toggle"
                        aria-controls="primary-nav"
                        aria-expanded=move || nav_open.get().to_string()
                        aria-label=move || if nav_open.get() {
                            "Close navigation"
                        } else {
                            "Open navigation"
                        }
                        on:click=move |_| {
                            if nav_open.get_untracked() {
                                close_navigation(nav_open, nav_narrow);
                            } else {
                                nav_open.set(true);
                            }
                        }
                    >
                        <span aria-hidden="true">{move || if nav_open.get() { "×" } else { "☰" }}</span>
                    </button>
                    <div class="mobile-brand">
                        <img src="/favicon.svg" alt="Flasher" class="brand-logo"/>
                        <span>"Flasher"</span>
                    </div>
                </header>
            {move || draft.get().flatten().map(|found| {
                let age = relative_age(now_ms(), found.updated_at);
                view! {
                    <div class="draft-banner" id="recovery-banner" role="alert">
                        <p class="draft-banner-text" id="recovery-text">
                            "An unsaved draft from " {age} " exists."
                        </p>
                        <div class="draft-banner-buttons">
                            <button type="button" id="recover-draft" class="primary" on:click=recover>
                                "Recover"
                            </button>
                            <button type="button" id="discard-draft" on:click=discard>
                                "Discard"
                            </button>
                        </div>
                    </div>
                }
            })}
            {move || {
                // Held back until the fresh-load session restore has
                // settled (see the effect above): the restored editor
                // or tab then mounts exactly once, already prefilled.
                if !restored.get() {
                    view! {
                        <p class="quiz-status" id="app-restoring">"Loading…"</p>
                    }
                        .into_any()
                } else if let Some(target) = editing.get() {
                    view! { <Editor target=target on_close=on_editor_close/> }.into_any()
                } else {
                    match tab.get() {
                        Tab::Quiz => view! { <Quiz/> }.into_any(),
                        Tab::AddCard => {
                            // A matching new-card draft prefills the
                            // editor on a fresh /add load; consumed
                            // here (StoredValue is non-reactive), so
                            // later tab switches start blank.
                            let prefilled = add_prefill.get_value();
                            if prefilled.is_some() {
                                add_prefill.set_value(None);
                            }
                            let target = prefilled.map_or_else(EditTarget::new_card, |d| {
                                EditTarget::from_draft(&d, false)
                            });
                            view! {
                                <Editor
                                    target=target
                                    on_close=on_editor_close
                                />
                            }
                                .into_any()
                        }
                        Tab::Groom => view! { <Groom on_edit=open_edit/> }.into_any(),
                        Tab::Labels => view! { <LabelManager/> }.into_any(),
                        Tab::Account => {
                            view! {
                                <Account
                                    username=username.clone()
                                    dev_mode=dev_mode
                                    on_logout=on_logout
                                />
                            }
                                .into_any()
                        }
                    }
                }
            }}
            <footer class="bottom">
                <p class="health">
                    {move || match health.get() {
                        None => "connecting…".to_owned(),
                        Some(Ok(resp)) => {
                            format!("status: {} — version: {}", resp.status, resp.version)
                        }
                        Some(Err(err)) => format!("error: {err}"),
                    }}
                </p>
            </footer>
            </div>
        </main>
                }
                    .into_any()
            }
        }}
    }
}

/// Resolves the editor target for a `/groom/edit/{id}` route — shared by
/// the fresh-load restore and the popstate (Back/Forward) dispatch, so
/// both apply the same rules: a draft belonging to this very card
/// recovers inline (prefill, banner suppressed AND cleared), a draft for
/// anything else prompts the banner while the editor opens on the card's
/// own content. Returns `None` when the card is gone or the fetch
/// failed; the caller then falls back to the Groom tab and rewrites the
/// URL (a leftover draft still banners).
// The nested Option is the app's deliberate draft state (None = the
// startup check has not answered yet, Some(None) = no draft, Some(Some)
// = banner); flattening it here would lose the unanswered case.
#[allow(clippy::option_option)]
#[cfg(feature = "csr")]
async fn groom_edit_restore(
    id: &str,
    found: Option<&AutoSaveResponse>,
    draft: RwSignal<Option<Option<AutoSaveResponse>>>,
) -> Option<EditTarget> {
    let Ok(Some(card)) = api::get_card(id).await else {
        draft.set(Some(found.cloned()));
        return None;
    };
    let target = match found {
        // The draft belongs to this very card: recover inline, no banner.
        Some(d) if d.card_id.as_deref() == Some(id) => {
            draft.set(Some(None));
            EditTarget::from_draft(d, true)
        }
        // No draft, or one for something else: card content into the
        // editor, draft into the banner.
        _ => {
            draft.set(Some(found.cloned()));
            EditTarget::edit(&card)
        }
    };
    Some(target)
}

/// Exports the sticky header's measured height as the `--top-h` CSS
/// variable on `<html>`: the groom tab's sticky controls/paging bar
/// (`.groom-head`) parks directly below the header (owner wish
/// 2026-07-31). Measured, not a constant — `.top` flex-wraps to two rows
/// on narrow screens. A missing header (auth screens) just skips the
/// update; the CSS fallback covers the first frames.
#[cfg(feature = "csr")]
fn export_top_height() {
    use wasm_bindgen::JsCast;
    let Some(document) = leptos::prelude::window().document() else {
        return;
    };
    let Ok(Some(header)) = document.query_selector(".top") else {
        return;
    };
    let height = header.get_bounding_client_rect().height();
    if let Some(root) = document.document_element() {
        let _ = root
            .unchecked_into::<web_sys::HtmlElement>()
            .style()
            .set_property("--top-h", &format!("{height}px"));
    }
}

/// Human-friendly draft age: "just now" under a minute, then minutes,
/// hours or days.
fn relative_age(now_ms: i64, then_ms: i64) -> String {
    let secs = now_ms.saturating_sub(then_ms) / 1_000;
    if secs < 60 {
        "just now".to_owned()
    } else if secs < 3_600 {
        let n = secs / 60;
        format!("{n} minute{} ago", if n == 1 { "" } else { "s" })
    } else if secs < 86_400 {
        let n = secs / 3_600;
        format!("{n} hour{} ago", if n == 1 { "" } else { "s" })
    } else {
        let n = secs / 86_400;
        format!("{n} day{} ago", if n == 1 { "" } else { "s" })
    }
}

/// Current unix epoch millis via the browser clock (wasm-only).
#[cfg(feature = "csr")]
fn now_ms() -> i64 {
    // Date::now() is milliseconds since the epoch as f64; the value is
    // far from any truncation boundary that matters here.
    #[allow(clippy::cast_possible_truncation)]
    {
        js_sys::Date::now() as i64
    }
}

/// SSR stand-in: the banner never renders server-side (the draft check
/// is csr-only), so this only keeps the view linkable.
#[cfg(not(feature = "csr"))]
fn now_ms() -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::relative_age;

    #[test]
    fn relative_age_buckets() {
        assert_eq!(relative_age(10_000, 9_000), "just now");
        assert_eq!(relative_age(60_000, 0), "1 minute ago");
        assert_eq!(relative_age(70_000, 10_000), "1 minute ago");
        assert_eq!(relative_age(600_000, 0), "10 minutes ago");
        assert_eq!(relative_age(3_600_000, 0), "1 hour ago");
        assert_eq!(relative_age(3_700_000, 0), "1 hour ago");
        assert_eq!(relative_age(7_300_000, 0), "2 hours ago");
        assert_eq!(relative_age(86_400_000, 0), "1 day ago");
        assert_eq!(relative_age(90_000_000, 0), "1 day ago");
        assert_eq!(relative_age(200_000_000, 0), "2 days ago");
    }

    #[test]
    fn relative_age_clamps_clock_skew() {
        // A draft timestamped in the future reads as "just now", never
        // as a negative duration.
        assert_eq!(relative_age(1_000, 5_000), "just now");
    }
}
