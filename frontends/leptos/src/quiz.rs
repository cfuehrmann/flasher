//! Quiz tab: review due cards one at a time.
//!
//! State machine per card (mirroring the old React `QuizView`):
//! `Loading` → `Prompt` → `Solution` → rate ok/failed → `Loading` … until
//! no card is due (`Done`). Fetch and rating failures land in `Error`
//! with a retry button.
//!
//! The label filter above the card (labels dissolved the hardcoded
//! enabled-only rule, owner decision 2026-08-01) selects which labels
//! may be quizzed — union semantics, any selected label matches. The
//! first-usage default is `Enabled` (the old behavior); the selection
//! persists in `localStorage`, independent of the groom tab's own
//! persisted selection. Changing it refetches the next card.
//!
//! The reveal state is deliberately NOT mirrored into the URL or anywhere
//! else: it is transient in-memory state, so a browser refresh (or a tab
//! switch, which remounts this component) always starts collapsed at the
//! prompt.

use flasher_types::{CardResponse, ENABLED_LABEL, LabelResponse};
use leptos::prelude::*;

use crate::api;
use crate::labels::{LabelFilter, join_labels};
#[cfg(feature = "csr")]
use crate::labels::{split_labels, storage_get, storage_set};
use crate::markdown::MarkdownView;

/// `localStorage` key for the quiz's label selection — deliberately NOT
/// the groom tab's key: the two filters are independent (owner decision
/// 2026-08-01).
#[cfg(feature = "csr")]
const STORAGE_LABELS_KEY: &str = "flasher-quiz-labels";

/// The persisted quiz label selection, or the first-usage default
/// (`Enabled` — the pre-labels behavior).
fn initial_labels() -> Vec<String> {
    #[cfg(feature = "csr")]
    if let Some(raw) = storage_get(STORAGE_LABELS_KEY) {
        return split_labels(&raw);
    }
    vec![ENABLED_LABEL.to_owned()]
}

/// The quiz state machine.
#[derive(Clone)]
enum QuizState {
    /// Fetching the next card from the server.
    Loading,
    /// A due card is showing its prompt, solution hidden.
    Prompt(CardResponse),
    /// The solution is revealed; the card waits for a rating.
    Solution(CardResponse),
    /// No card is due (for the selected labels).
    Done,
    /// A fetch or rating request failed.
    Error(String),
}

/// The Quiz tab.
// The five state arms of the view make this long; splitting them into
// sub-components would only add indirection.
#[allow(clippy::too_many_lines)]
#[component]
pub fn Quiz() -> impl IntoView {
    let state = RwSignal::new(QuizState::Loading);
    // The label filter selection (persisted, independent of groom's).
    let selected = RwSignal::new(initial_labels());
    // The user's labels (for the filter's checkbox panel).
    let all_labels = RwSignal::new(Vec::<LabelResponse>::new());

    // Bumped by every next-card fetch; a response that lands after a
    // newer fetch was armed is stale and must not touch the state
    // (rapid filter toggles, or a rating completing right after a
    // toggle — same guard as the groom tab's; adversarial review
    // 2026-08-01).
    let fetch_generation = RwSignal::new(0_u64);

    // Fetches the next due card matching the selection and transitions
    // into Prompt/Done/Error. `Callback` is `Copy`, so the same action
    // can be shared between the mount effect, the retry button and the
    // rating handler.
    let fetch_next = Callback::new(move |(): ()| {
        state.set(QuizState::Loading);
        fetch_generation.update(|count| *count += 1);
        let armed = fetch_generation.get_untracked();
        let labels = join_labels(&selected.get_untracked());
        leptos::task::spawn_local(async move {
            let result = api::next_card(&labels).await;
            if fetch_generation.get_untracked() != armed {
                // A newer fetch is already in flight; ignore this one.
                return;
            }
            state.set(match result {
                Ok(Some(card)) => QuizState::Prompt(card),
                Ok(None) => QuizState::Done,
                Err(err) => QuizState::Error(err),
            });
        });
    });

    // Loads the labels list (the filter's panel). Shared by the mount
    // load and the error retry (adversarial review 2026-08-01: a retry
    // must reload the labels too, or a transient failure leaves an
    // empty panel).
    let load_labels = Callback::new(move |(): ()| {
        leptos::task::spawn_local(async move {
            match api::labels().await {
                Ok(labels) => all_labels.set(labels),
                Err(err) => state.set(QuizState::Error(err)),
            }
        });
    });
    #[cfg(feature = "csr")]
    load_labels.run(());

    // Selection change: persist (localStorage) and refetch the next
    // card — a deliberate act, so abandoning the current card is right
    // (it may no longer match).
    let on_label_change = Callback::new(move |next: Vec<String>| {
        #[cfg(feature = "csr")]
        storage_set(STORAGE_LABELS_KEY, &join_labels(&next));
        selected.set(next);
    });

    // Fetch the first card on mount, and refetch whenever the selection
    // changes (client-side only).
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        let _ = selected.get();
        fetch_next.run(());
    });

    // Set while a rating POST is in flight. Guards the rating buttons
    // against double-taps (issue #124): every extra click within the
    // response window would otherwise fire another set-ok/set-failed,
    // and the second request would re-schedule the card off the first
    // rating's just-written change_time, collapsing the SRS interval.
    let rating_in_flight = RwSignal::new(false);

    // Rates the current card, then loads the next one. The card's
    // `change_time` goes along so the server can reject the rating when
    // the card moved since it was rendered (compare-and-set, 409).
    let rate = Callback::new(move |(card, ok): (CardResponse, bool)| {
        if rating_in_flight.get_untracked() {
            return;
        }
        rating_in_flight.set(true);
        leptos::task::spawn_local(async move {
            let result = if ok {
                api::set_ok(&card.id, card.change_time).await
            } else {
                api::set_failed(&card.id, card.change_time).await
            };
            rating_in_flight.set(false);
            match result {
                Ok(()) => fetch_next.run(()),
                Err(err) => state.set(QuizState::Error(err)),
            }
        });
    });

    // Reveals the solution of the card currently in Prompt state.
    let show_solution = Callback::new(move |(): ()| {
        state.update(|current| {
            if let QuizState::Prompt(card) = current {
                *current = QuizState::Solution(card.clone());
            }
        });
    });

    view! {
        <section class="quiz">
            <div class="quiz-controls">
                <LabelFilter
                    labels=all_labels
                    selected=selected
                    on_change=on_label_change
                    id_prefix="quiz"
                />
            </div>
            {move || match state.get() {
                QuizState::Loading => view! {
                    <p class="quiz-status" id="quiz-loading">"Loading the next card…"</p>
                }
                    .into_any(),
                QuizState::Prompt(card) => view! {
                    <div class="quiz-card">
                        <MarkdownView id="quiz-prompt" markdown=card.prompt/>
                        <div class="quiz-buttons">
                            <button
                                type="button"
                                id="show-solution"
                                class="primary"
                                on:click=move |_| show_solution.run(())
                            >
                                "Show solution"
                            </button>
                        </div>
                    </div>
                }
                    .into_any(),
                QuizState::Solution(card) => {
                    // The buttons need the whole card (id + change_time)
                    // after prompt/solution moved into the views.
                    let rated = card.clone();
                    view! {
                    <div class="quiz-card">
                        <MarkdownView id="quiz-prompt" markdown=card.prompt/>
                        <MarkdownView id="quiz-solution" class="solution" markdown=card.solution/>
                        <div class="quiz-buttons">
                            <button
                                type="button"
                                id="rate-failed"
                                class="failed"
                                disabled=move || rating_in_flight.get()
                                on:click={
                                    let card = rated.clone();
                                    move |_| rate.run((card.clone(), false))
                                }
                            >
                                "Failed"
                            </button>
                            <button
                                type="button"
                                id="rate-ok"
                                class="ok"
                                disabled=move || rating_in_flight.get()
                                on:click=move |_| rate.run((rated.clone(), true))
                            >
                                "OK"
                            </button>
                        </div>
                    </div>
                }
                    .into_any()
                }
                QuizState::Done => view! {
                    <div class="quiz-card" id="quiz-done">
                        <p class="quiz-status">
                            "All done for now — no due cards match the selected labels."
                        </p>
                        <p class="quiz-hint">
                            "Cards you add start with the Disabled label; enable them in the Groom tab, or widen the label filter above."
                        </p>
                    </div>
                }
                    .into_any(),
                QuizState::Error(err) => view! {
                    <div class="quiz-card" id="quiz-error">
                        <p class="quiz-status error">"Something went wrong: " {err}</p>
                        <div class="quiz-buttons">
                            <button
                                type="button"
                                id="quiz-retry"
                                on:click=move |_| {
                                    load_labels.run(());
                                    fetch_next.run(());
                                }
                            >
                                "Retry"
                            </button>
                        </div>
                    </div>
                }
                    .into_any(),
            }}
        </section>
    }
}
