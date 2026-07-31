//! Groom tab: search, page and maintain the whole card collection.
//!
//! Search-as-you-type with a ~300 ms debounce (a generation counter makes
//! stale timers no-ops, so no cancel handle is needed); changing the query
//! resets to page 0 and refetches. The page size is not hard-coded: each
//! find response carries the server's configured page size, which drives
//! the skip arithmetic, the paging buttons and the "showing X–Y of Z"
//! line. A second generation counter guards the fetch itself, so a stale
//! in-flight response (rapid paging, search-while-paging) can never
//! overwrite newer rows. Row layout is two lines: the clamped prompt,
//! then a meta line with badges + due date on the left and the actions
//! right-aligned on the same line. Row actions: edit (opens the card
//! editor via the `on_edit` callback) and the enable/disable toggle
//! (immediate) stay inline; the rare destructive ones (delete,
//! reset-progress) sit in a per-row "⋯" menu behind a confirm modal.
//! Toggle and reset
//! refetch the current page because the mutations can change the
//! server-side ordering (enabled first, `next_time` asc, disabled last);
//! deleting the last row of a page beyond the first steps back one page,
//! any other delete refetches in place so the count stays exact.
//!
//! Loading, error (with retry) and empty states mirror the quiz tab.

use std::time::Duration;

use flasher_types::{CardResponse, CardState};
use leptos::prelude::*;

use crate::api;

/// Page size assumed until the first find response reports the server's
/// configured one (matches the server default, so usually exact).
const FALLBACK_PAGE_SIZE: usize = 10;

/// Search-as-you-type debounce delay.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// What the list area currently shows.
#[derive(Clone)]
enum LoadState {
    /// A fetch is in flight (also the initial state).
    Loading,
    /// One page of cards, the total match count before paging and the
    /// server's page size (drives skip arithmetic and the paging line).
    Loaded {
        cards: Vec<CardResponse>,
        count: i64,
        page_size: usize,
    },
    /// A fetch or row action failed; retry refetches the current page.
    Error(String),
}

/// The destructive action an open confirm modal is armed with.
#[derive(Clone)]
enum ConfirmAction {
    /// Delete the card entirely.
    Delete(CardResponse),
    /// Reset the card's learning progress to state `new`.
    ResetProgress(CardResponse),
}

impl ConfirmAction {
    /// The card the action targets.
    fn card(&self) -> &CardResponse {
        match self {
            Self::Delete(card) | Self::ResetProgress(card) => card,
        }
    }

    /// The question shown above the prompt in the modal.
    fn question(&self) -> &'static str {
        match self {
            Self::Delete(_) => "Really delete this card?",
            Self::ResetProgress(_) => "Reset learning progress for this card?",
        }
    }

    /// The label of the confirm button.
    fn confirm_label(&self) -> &'static str {
        match self {
            Self::Delete(_) => "Delete",
            Self::ResetProgress(_) => "Reset",
        }
    }

    /// Shown in the modal when deleting a card that has learning progress
    /// (owner decision 2026-07-27: full delete stays available for learned
    /// cards, but the confirmation must surface the existing progress).
    fn progress_warning(&self) -> Option<String> {
        match self {
            Self::Delete(card) if card.state != CardState::New => {
                let state = match card.state {
                    CardState::Ok => "ok",
                    CardState::Failed => "failed",
                    CardState::New => unreachable!("guarded above"),
                };
                Some(format!(
                    "This card has learning progress (state: {state}). Deleting removes it permanently."
                ))
            }
            Self::Delete(_) | Self::ResetProgress(_) => None,
        }
    }
}

/// Converts a small UI count into the API's `i64` without a cast lint;
/// page arithmetic never leaves the low range.
fn as_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// The Groom tab.
// The list, paging and modal arms of the view make this long; splitting
// them further would only add indirection (same reasoning as the quiz).
#[allow(clippy::too_many_lines)]
#[component]
pub fn Groom(
    /// Opens the card editor for a row (owned by the App so the editor
    /// can overlay the tabs).
    on_edit: Callback<CardResponse>,
) -> impl IntoView {
    // The raw input value and the debounced query actually searched for.
    let input = RwSignal::new(String::new());
    let query = RwSignal::new(String::new());
    let page = RwSignal::new(0_usize);
    let state = RwSignal::new(LoadState::Loading);
    let confirm = RwSignal::new(None::<ConfirmAction>);
    // Server page size, learned from every find response (fallback until
    // the first one lands); drives the skip arithmetic of the next fetch.
    let page_size = RwSignal::new(FALLBACK_PAGE_SIZE);
    // Bumped by every fetch; a response that lands after a newer fetch was
    // armed is stale and must not touch the state (rapid paging).
    let fetch_generation = RwSignal::new(0_u64);

    // Fetches one page for a query; the single entry point for loading,
    // paging, retry and post-mutation refresh.
    let fetch = Callback::new(move |(q, p): (String, usize)| {
        state.set(LoadState::Loading);
        fetch_generation.update(|count| *count += 1);
        let armed = fetch_generation.get_untracked();
        leptos::task::spawn_local(async move {
            let skip = u32::try_from(p * page_size.get_untracked()).unwrap_or(u32::MAX);
            let result = api::find_cards(&q, skip).await;
            if fetch_generation.get_untracked() != armed {
                // A newer fetch is already in flight; ignore this one.
                return;
            }
            state.set(match result {
                Ok(found) => {
                    let size = usize::try_from(found.page_size).unwrap_or(FALLBACK_PAGE_SIZE);
                    page_size.set(size);
                    LoadState::Loaded {
                        cards: found.cards,
                        count: found.count,
                        page_size: size,
                    }
                }
                Err(err) => LoadState::Error(err),
            });
        });
    });

    // (Re)fetch whenever the debounced query or the page changes.
    #[cfg(feature = "csr")]
    Effect::new(move |_| fetch.run((query.get(), page.get())));

    // Search-as-you-type: every keystroke bumps the generation and arms a
    // timer; when it fires and is still the latest, the query goes live.
    // `batch` makes the page reset and the query change a single trigger,
    // so the effect fires one fetch, not two.
    let generation = RwSignal::new(0_u64);
    let on_search_input = move |ev: leptos::ev::Event| {
        let value = event_target_value(&ev);
        input.set(value.clone());
        generation.update(|count| *count += 1);
        let armed = generation.get_untracked();
        set_timeout(
            move || {
                if generation.get_untracked() == armed {
                    batch(|| {
                        page.set(0);
                        query.set(value);
                    });
                }
            },
            DEBOUNCE,
        );
    };

    // Enable/disable toggle — immediate, no confirmation. The mutation can
    // change the server-side ordering (disabled cards sort last), so the
    // current page is refetched instead of patching the row in place.
    let toggle_disabled = Callback::new(move |card: CardResponse| {
        leptos::task::spawn_local(async move {
            match api::set_disabled(&card.id, !card.disabled).await {
                Ok(_updated) => fetch.run((query.get_untracked(), page.get_untracked())),
                Err(err) => state.set(LoadState::Error(err)),
            }
        });
    });

    // Confirmed delete: step back one page when the last row of a later
    // page vanished (the effect refetches), otherwise refetch in place so
    // the next card slides in and the count stays exact.
    let do_delete = Callback::new(move |card: CardResponse| {
        leptos::task::spawn_local(async move {
            match api::delete_card(&card.id).await {
                Ok(()) => {
                    let was_single = matches!(
                        state.get_untracked(),
                        LoadState::Loaded { ref cards, .. } if cards.len() == 1
                    );
                    if was_single && page.get_untracked() > 0 {
                        page.update(|p| *p -= 1);
                    } else {
                        fetch.run((query.get_untracked(), page.get_untracked()));
                    }
                }
                Err(err) => state.set(LoadState::Error(err)),
            }
        });
    });

    // Confirmed progress reset: like the toggle, the reset can change the
    // ordering (next_time moves), so the current page is refetched.
    let do_reset = Callback::new(move |card: CardResponse| {
        leptos::task::spawn_local(async move {
            match api::delete_history(&card.id).await {
                Ok(_updated) => fetch.run((query.get_untracked(), page.get_untracked())),
                Err(err) => state.set(LoadState::Error(err)),
            }
        });
    });

    view! {
        <section class="groom">
            <label for="groom-search">"Search"</label>
            <input
                id="groom-search"
                type="text"
                placeholder="Search cards…"
                prop:value=input
                on:input=on_search_input
            />
            {move || match state.get() {
                LoadState::Loading => view! {
                    <p class="groom-status" id="groom-loading">"Loading cards…"</p>
                }
                    .into_any(),
                LoadState::Error(err) => view! {
                    <div class="groom-status" id="groom-error">
                        <p class="form-error">"Something went wrong: " {err}</p>
                        <button
                            type="button"
                            id="groom-retry"
                            on:click=move |_| {
                                fetch.run((query.get_untracked(), page.get_untracked()));
                            }
                        >
                            "Retry"
                        </button>
                    </div>
                }
                    .into_any(),
                LoadState::Loaded {
                    cards,
                    count,
                    page_size,
                } => {
                    if cards.is_empty() {
                        view! {
                            <p class="groom-status" id="groom-empty">"No cards match."</p>
                        }
                            .into_any()
                    } else {
                        let skip = page.get() * page_size;
                        let first = as_i64(skip) + 1;
                        let last = as_i64(skip + cards.len());
                        let has_prev = page.get() > 0;
                        let has_next = last < count;
                        view! {
                            // Paging bar ABOVE the list (Phase 6.5, owner
                            // complaint): on a full page the user no longer
                            // has to scroll to reach Previous/Next. Ids and
                            // disabled-when-single-page behavior unchanged.
                            <div class="groom-paging">
                                <button
                                    type="button"
                                    id="groom-prev"
                                    disabled=!has_prev
                                    on:click=move |_| page.update(|p| *p -= 1)
                                >
                                    "Previous"
                                </button>
                                <span id="groom-page-info">
                                    {format!("showing {first}–{last} of {count}")}
                                </span>
                                <button
                                    type="button"
                                    id="groom-next"
                                    disabled=!has_next
                                    on:click=move |_| page.update(|p| *p += 1)
                                >
                                    "Next"
                                </button>
                            </div>
                            <div class="groom-results" id="groom-results">
                                {cards
                                    .into_iter()
                                    .map(|card| {
                                        view! {
                                            <GroomRow
                                                card=card
                                                on_edit=on_edit
                                                toggle_disabled=toggle_disabled
                                                ask_delete=Callback::new(move |card| {
                                                    confirm.set(Some(ConfirmAction::Delete(card)));
                                                })
                                                ask_reset=Callback::new(move |card| {
                                                    confirm.set(Some(
                                                        ConfirmAction::ResetProgress(card),
                                                    ));
                                                })
                                            />
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }
                }
            }}
            {move || confirm.get().map(|action| {
                let card = action.card().clone();
                let question = action.question();
                let confirm_label = action.confirm_label();
                let progress_warning = action.progress_warning();
                // Delete is destructive (red); reset progress is not.
                let destructive = matches!(action, ConfirmAction::Delete(_));
                view! {
                    <div class="modal-backdrop" id="groom-modal">
                        <div class="modal" role="dialog" aria-modal="true">
                            <p class="modal-text" id="groom-modal-text">
                                {question}
                                <br />
                                <span class="modal-prompt">{card.prompt}</span>
                            </p>
                            {progress_warning.map(|warning| view! {
                                <p class="modal-progress-warning" id="modal-progress-warning">
                                    {warning}
                                </p>
                            })}
                            <div class="modal-buttons">
                                <button
                                    type="button"
                                    id="modal-confirm"
                                    class:failed=destructive
                                    on:click=move |_| {
                                        confirm.set(None);
                                        match &action {
                                            ConfirmAction::Delete(card) => {
                                                do_delete.run(card.clone());
                                            }
                                            ConfirmAction::ResetProgress(card) => {
                                                do_reset.run(card.clone());
                                            }
                                        }
                                    }
                                >
                                    {confirm_label}
                                </button>
                                <button
                                    type="button"
                                    id="modal-cancel"
                                    on:click=move |_| confirm.set(None)
                                >
                                    "Cancel"
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })}
        </section>
    }
}

/// One card row of the groom list: truncated prompt on the first line,
/// and a single meta line below it — state badge (plus a `disabled`
/// badge) and due date on the left, the row actions right-aligned on
/// the SAME line (owner decision: one visual row less per card than the
/// old three-line layout). The everyday, reversible actions (edit,
/// enable/disable) stay one click away; the rare destructive ones
/// (reset progress, delete) live in a "⋯" overflow menu and still arm
/// the same confirm modal. The menu closes via a transparent
/// full-viewport backdrop — the same pattern the modal uses, so no
/// window listeners are needed.
#[component]
fn GroomRow(
    card: CardResponse,
    on_edit: Callback<CardResponse>,
    toggle_disabled: Callback<CardResponse>,
    ask_delete: Callback<CardResponse>,
    ask_reset: Callback<CardResponse>,
) -> impl IntoView {
    let id = card.id.clone();
    let row_id = format!("groom-row-{id}");
    let state_badge_id = format!("state-{id}");
    let disabled_badge_id = format!("disabled-{id}");
    let due_id = format!("due-{id}");
    let edit_id = format!("edit-{id}");
    let toggle_id = format!("toggle-disabled-{id}");
    let menu_id = format!("menu-{id}");
    let reset_id = format!("reset-{id}");
    let delete_id = format!("delete-{id}");
    let state = card.state.as_str();
    let disabled = card.disabled;
    let due = due_label(card.next_time);
    let toggle_label = if disabled { "Enable" } else { "Disable" };
    let menu_open = RwSignal::new(false);
    // One owned clone per click handler (each handler moves its capture).
    let card_edit = card.clone();
    let card_toggle = card.clone();
    let card_reset = card.clone();
    let card_delete = card.clone();

    view! {
        <div class="groom-row" id=row_id>
            <p class="groom-prompt">{card.prompt.clone()}</p>
            <div class="groom-meta">
                <span class=format!("badge state-{state}") id=state_badge_id>
                    {state}
                </span>
                {disabled.then(|| {
                    view! {
                        <span class="badge disabled" id=disabled_badge_id>
                            "disabled"
                        </span>
                    }
                })}
                <span class="groom-due" id=due_id>{due}</span>
                <div class="groom-actions">
                    <button
                        type="button"
                        id=edit_id
                        on:click=move |_| on_edit.run(card_edit.clone())
                    >
                        "Edit"
                    </button>
                    <button
                        type="button"
                        id=toggle_id
                        on:click=move |_| toggle_disabled.run(card_toggle.clone())
                    >
                        {toggle_label}
                    </button>
                    <button
                        type="button"
                        class="groom-menu-button"
                        id=menu_id
                        aria-label="More actions"
                        aria-expanded=move || menu_open.get().to_string()
                        on:click=move |_| menu_open.update(|open| *open = !*open)
                    >
                        "⋯"
                    </button>
                    {move || {
                        menu_open.get().then(|| {
                            let reset_id = reset_id.clone();
                            let delete_id = delete_id.clone();
                            let card_reset = card_reset.clone();
                            let card_delete = card_delete.clone();
                            view! {
                                <div
                                    class="groom-menu-backdrop"
                                    on:click=move |_| menu_open.set(false)
                                ></div>
                                // Deliberately a plain group of buttons,
                                // not role="menu": the app has no
                                // keyboard menu semantics anywhere (no
                                // Escape/arrow handling — the modal is
                                // the same), and the role would promise
                                // them (adversarial review 2026-07-31).
                                <div class="groom-menu">
                                    <button
                                        type="button"
                                        id=reset_id
                                        on:click=move |_| {
                                            menu_open.set(false);
                                            ask_reset.run(card_reset.clone());
                                        }
                                    >
                                        "Reset progress"
                                    </button>
                                    <button
                                        type="button"
                                        class="failed"
                                        id=delete_id
                                        on:click=move |_| {
                                            menu_open.set(false);
                                            ask_delete.run(card_delete.clone());
                                        }
                                    >
                                        "Delete"
                                    </button>
                                </div>
                            }
                        })
                    }}
                </div>
            </div>
        </div>
    }
}

/// Human-friendly due label: `due now` when the time has passed,
/// otherwise `due YYYY-MM-DD` (UTC, formatted by `flasher-core`).
fn due_label(next_time: i64) -> String {
    if next_time <= now_ms() {
        "due now".to_owned()
    } else {
        format!("due {}", flasher_core::format_utc_date(next_time))
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

/// SSR stand-in: the initial groom render is the Loading state, so no due
/// label is ever computed server-side; this only keeps the view linkable.
#[cfg(not(feature = "csr"))]
fn now_ms() -> i64 {
    0
}
