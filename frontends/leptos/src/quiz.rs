//! Quiz tab: review due cards one at a time.
//!
//! State machine per card (mirroring the old React `QuizView`):
//! `Loading` → `Prompt` → `Solution` → rate ok/failed → `Loading` … until
//! no card is due (`Done`). Fetch and rating failures land in `Error`
//! with a retry button.
//!
//! The reveal state is deliberately NOT mirrored into the URL or anywhere
//! else: it is transient in-memory state, so a browser refresh (or a tab
//! switch, which remounts this component) always starts collapsed at the
//! prompt.

use flasher_types::CardResponse;
use leptos::prelude::*;

use crate::api;
use crate::markdown::MarkdownView;

/// The quiz state machine.
#[derive(Clone)]
enum QuizState {
    /// Fetching the next card from the server.
    Loading,
    /// A due card is showing its prompt, solution hidden.
    Prompt(CardResponse),
    /// The solution is revealed; the card waits for a rating.
    Solution(CardResponse),
    /// No card is due.
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

    // Fetches the next due card and transitions into Prompt/Done/Error.
    // `Callback` is `Copy`, so the same action can be shared between the
    // mount effect, the retry button and the rating handler.
    let fetch_next = Callback::new(move |(): ()| {
        state.set(QuizState::Loading);
        leptos::task::spawn_local(async move {
            state.set(match api::next_card().await {
                Ok(Some(card)) => QuizState::Prompt(card),
                Ok(None) => QuizState::Done,
                Err(err) => QuizState::Error(err),
            });
        });
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

    // Fetch the first card on mount (client-side only).
    #[cfg(feature = "csr")]
    Effect::new(move |_| fetch_next.run(()));

    view! {
        <section class="quiz">
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
                        <p class="quiz-status">"All done for now — no cards are due."</p>
                        <p class="quiz-hint">
                            "Cards you add start out disabled; enable them in the Groom tab."
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
                                on:click=move |_| fetch_next.run(())
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
