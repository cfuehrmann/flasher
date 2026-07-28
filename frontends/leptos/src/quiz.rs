//! Quiz tab: review due cards one at a time.
//!
//! State machine per card (mirroring the old React `QuizView`):
//! `Loading` → `Prompt` → `Solution` → rate ok/failed → `Loading` … until
//! no card is due (`Done`). Fetch and rating failures land in `Error`
//! with a retry button.
//!
//! The reveal state is mirrored into the URL (Phase 6.6): revealing the
//! solution swaps `/quiz` for `/quiz/solution` via `replaceState` (no
//! history entry — Back after revealing leaves the quiz instead of
//! un-revealing), rating swaps it back to `/quiz`. A fresh load of
//! `/quiz/solution` fetches the next due card and shows it ALREADY
//! revealed; a fresh load of `/quiz` starts at the prompt as before.
//!
//! Keyboard shortcuts: Space/Enter shows the solution, `1` rates failed,
//! `2` rates ok. The handler ignores keys while an input or textarea has
//! focus so typing in the Add card form is never hijacked.

use flasher_types::CardResponse;
use leptos::prelude::*;

use crate::api;
use crate::markdown::MarkdownView;
use crate::route;

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
    // Fresh load of `/quiz/solution` (Phase 6.6): the FIRST card fetched
    // shows already revealed. Consumed by the first fetch, so rating and
    // retrying start at the prompt again. Read once at mount — the quiz
    // remounts on every navigation to the tab, so this is always the
    // URL the user actually asked for.
    let pending_reveal = RwSignal::new(route::starts_revealed());

    // Fetches the next due card and transitions into Prompt/Done/Error
    // (into Solution directly when restoring a `/quiz/solution` load).
    // `Callback` is `Copy`, so the same action can be shared between the
    // mount effect, the retry button and the rating handler.
    let fetch_next = Callback::new(move |(): ()| {
        state.set(QuizState::Loading);
        let reveal = pending_reveal.get_untracked();
        pending_reveal.set(false);
        leptos::task::spawn_local(async move {
            state.set(match api::next_card().await {
                Ok(Some(card)) if reveal => QuizState::Solution(card),
                Ok(Some(card)) => QuizState::Prompt(card),
                Ok(None) => {
                    // A restored `/quiz/solution` load with no due card
                    // must not leave the URL lying (F4): replace it back
                    // to /quiz over the Done view.
                    if reveal {
                        route::replace_tab(route::Tab::Quiz);
                    }
                    QuizState::Done
                }
                Err(err) => {
                    // Same drift on a failed first fetch (F4): Retry
                    // must not show a prompt under a solution URL.
                    if reveal {
                        route::replace_tab(route::Tab::Quiz);
                    }
                    QuizState::Error(err)
                }
            });
        });
    });

    // Rates the current card, then loads the next one. Rating leaves the
    // revealed state, so the URL goes back to `/quiz` (replace: the
    // `/quiz/solution` entry must not linger on the stack).
    let rate = Callback::new(move |(id, ok): (String, bool)| {
        route::replace_tab(route::Tab::Quiz);
        leptos::task::spawn_local(async move {
            let result = if ok {
                api::set_ok(&id).await
            } else {
                api::set_failed(&id).await
            };
            match result {
                Ok(()) => fetch_next.run(()),
                Err(err) => state.set(QuizState::Error(err)),
            }
        });
    });

    // Reveals the solution of the card currently in Prompt state, and
    // mirrors that into the URL: `/quiz` becomes `/quiz/solution` via
    // replaceState — no history entry, so Back leaves the quiz rather
    // than just un-revealing (Phase 6.6).
    let show_solution = Callback::new(move |(): ()| {
        state.update(|current| {
            if let QuizState::Prompt(card) = current {
                *current = QuizState::Solution(card.clone());
                route::replace_quiz_solution();
            }
        });
    });

    // Fetch the first card on mount (client-side only).
    #[cfg(feature = "csr")]
    Effect::new(move |_| fetch_next.run(()));

    // Global keyboard shortcuts (client-side only; `window()` does not
    // exist under ssr).
    #[cfg(feature = "csr")]
    {
        let handle = window_event_listener(leptos::ev::keydown, move |ev| {
            if typing_in_form_field() {
                return;
            }
            match (state.get_untracked(), ev.key().as_str()) {
                (QuizState::Prompt(_), " " | "Enter") => {
                    ev.prevent_default();
                    show_solution.run(());
                }
                (QuizState::Solution(card), "1") => rate.run((card.id, false)),
                (QuizState::Solution(card), "2") => rate.run((card.id, true)),
                _ => {}
            }
        });
        on_cleanup(move || handle.remove());
    }

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
                        <p class="quiz-keys">
                            <kbd>"Space"</kbd> " show solution"
                        </p>
                    </div>
                }
                    .into_any(),
                QuizState::Solution(card) => view! {
                    <div class="quiz-card">
                        <MarkdownView id="quiz-prompt" markdown=card.prompt/>
                        <MarkdownView id="quiz-solution" class="solution" markdown=card.solution/>
                        <div class="quiz-buttons">
                            <button
                                type="button"
                                id="rate-failed"
                                class="failed"
                                on:click={
                                    let id = card.id.clone();
                                    move |_| rate.run((id.clone(), false))
                                }
                            >
                                "Failed"
                            </button>
                            <button
                                type="button"
                                id="rate-ok"
                                class="ok"
                                on:click=move |_| rate.run((card.id.clone(), true))
                            >
                                "OK"
                            </button>
                        </div>
                        <p class="quiz-keys">
                            <kbd>"1"</kbd> " failed · " <kbd>"2"</kbd> " ok"
                        </p>
                    </div>
                }
                    .into_any(),
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

/// Whether the keyboard focus is currently inside a form field — in that
/// case global quiz shortcuts must not fire.
#[cfg(feature = "csr")]
fn typing_in_form_field() -> bool {
    document().active_element().is_some_and(|el| {
        let tag = el.tag_name();
        tag == "INPUT" || tag == "TEXTAREA" || tag == "SELECT"
    })
}
