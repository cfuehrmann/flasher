//! Passkey authentication screens (Phase 5B).
//!
//! - [`AuthScreen`]: shown in auth mode while there is no session. The
//!   `GET /api/auth/bootstrap` answer picks the variant: the first-run
//!   register form (username, plus a bootstrap-token field when the
//!   server requires `FLASHER_BOOTSTRAP_TOKEN`, and "Create passkey") or
//!   the one-button username-less login ("Sign in with passkey"). After a
//!   successful registration the screen flips to the login variant
//!   (register/finish deliberately does not log the user in).
//! - [`Account`]: the fourth tab, reachable once logged in (both auth
//!   modes). Shows the username, the logout button (auth mode only — in
//!   dev-bypass mode there is no session to end) and the passkey
//!   management card: list, inline rename, delete behind a confirm modal
//!   (the server's "cannot delete your last passkey" 409 is surfaced
//!   there) and an "Add passkey" ceremony button.

use leptos::prelude::*;

use crate::api;

/// Full registration ceremony: fetch options, run the browser ceremony,
/// send the credential back. `username` is only used by the open
/// bootstrap; with a session the server ignores it. `token` is the
/// bootstrap token, sent only on the open first-run registration when the
/// server requires it.
#[cfg(feature = "csr")]
async fn register_ceremony(username: &str, token: Option<&str>) -> Result<(), String> {
    let options = api::register_start(username, token).await?;
    let credential = crate::webauthn::create_credential(&options).await?;
    api::register_finish(&credential).await
}

/// Full login ceremony: fetch options, run the browser ceremony, send
/// the assertion back; returns the logged-in username.
#[cfg(feature = "csr")]
async fn login_ceremony() -> Result<String, String> {
    let options = api::login_start().await?;
    let assertion = crate::webauthn::get_credential(&options).await?;
    api::login_finish(&assertion).await
}

/// `register_ceremony` (ssr stub, never called — the buttons live behind
/// event handlers that never fire server-side).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
async fn register_ceremony(_username: &str, _token: Option<&str>) -> Result<(), String> {
    Err("the passkey ceremony is only available in the browser build".to_owned())
}

/// `login_ceremony` (ssr stub, never called).
#[cfg(not(feature = "csr"))]
#[allow(clippy::unused_async)]
async fn login_ceremony() -> Result<String, String> {
    Err("the passkey ceremony is only available in the browser build".to_owned())
}

/// The centered auth card: first-run register or one-button login.
// The two variants share the card shell and the busy/error signals;
// splitting them into sub-components would only add indirection.
#[allow(clippy::too_many_lines)]
#[component]
pub fn AuthScreen(
    /// Called with the username after a successful login.
    on_login: Callback<String>,
) -> impl IntoView {
    // `None` while the bootstrap answer is pending.
    let registration_open = RwSignal::new(None::<bool>);
    // The open bootstrap is gated by FLASHER_BOOTSTRAP_TOKEN: the
    // register screen then asks for it.
    let token_required = RwSignal::new(false);
    let bootstrap_token = RwSignal::new(String::new());
    // Set after a successful registration: flips to the login variant.
    let registered = RwSignal::new(false);
    let username = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let bootstrap_error = RwSignal::new(None::<String>);

    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            match api::bootstrap().await {
                Ok(bootstrap) => {
                    token_required.set(bootstrap.token_required);
                    registration_open.set(Some(bootstrap.registration_open));
                }
                Err(err) => bootstrap_error.set(Some(err)),
            }
        });
    });

    let create_passkey = move |_| {
        let name = username.get_untracked().trim().to_owned();
        if name.is_empty() {
            error.set(Some("Enter a username first.".to_owned()));
            return;
        }
        let token = token_required
            .get_untracked()
            .then(|| bootstrap_token.get_untracked());
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let result = register_ceremony(&name, token.as_deref()).await;
            busy.set(false);
            match result {
                Ok(()) => registered.set(true),
                Err(err) => error.set(Some(err)),
            }
        });
    };

    let sign_in = move |_| {
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let result = login_ceremony().await;
            busy.set(false);
            match result {
                Ok(name) => on_login.run(name),
                Err(err) => error.set(Some(err)),
            }
        });
    };

    view! {
        <main class="auth-screen" id="auth-screen">
            <section class="auth-card">
                <h1>"Flasher"</h1>
                {move || {
                    if let Some(err) = bootstrap_error.get() {
                        view! { <p class="error" id="auth-error">{err}</p> }.into_any()
                    } else if registered.get() || registration_open.get() == Some(false) {
                        view! {
                            {move || {
                                registered.get().then(|| {
                                    view! {
                                        <p class="form-ok" id="auth-note">
                                            "Passkey created — sign in to continue."
                                        </p>
                                    }
                                })
                            }}
                            <p class="auth-hint">"Sign in with your passkey."</p>
                            <button
                                type="button"
                                id="sign-in"
                                class="primary"
                                disabled=move || busy.get()
                                on:click=sign_in
                            >
                                {move || {
                                    if busy.get() {
                                        "Waiting for passkey…"
                                    } else {
                                        "Sign in with passkey"
                                    }
                                }}
                            </button>
                            {move || {
                                error.get().map(|err| {
                                    view! { <p class="error" id="auth-error" role="alert">{err}</p> }
                                })
                            }}
                        }
                            .into_any()
                    } else if registration_open.get() == Some(true) {
                        view! {
                            <p class="auth-hint">"Create the first passkey to get started."</p>
                            <label class="auth-label" for="register-username">"Username"</label>
                            <input
                                type="text"
                                id="register-username"
                                autocomplete="username webauthn"
                                prop:value=move || username.get()
                                on:input=move |ev| {
                                    username.set(event_target_value(&ev));
                                    error.set(None);
                                }
                            />
                            {move || {
                                token_required.get().then(|| {
                                    view! {
                                        <label class="auth-label" for="bootstrap-token">
                                            "Bootstrap token"
                                        </label>
                                        <input
                                            type="password"
                                            id="bootstrap-token"
                                            autocomplete="off"
                                            prop:value=move || bootstrap_token.get()
                                            on:input=move |ev| {
                                                bootstrap_token.set(event_target_value(&ev));
                                                error.set(None);
                                            }
                                        />
                                    }
                                })
                            }}
                            <button
                                type="button"
                                id="create-passkey"
                                class="primary"
                                disabled=move || busy.get()
                                on:click=create_passkey
                            >
                                {move || {
                                    if busy.get() {
                                        "Waiting for passkey…"
                                    } else {
                                        "Create passkey"
                                    }
                                }}
                            </button>
                            {move || {
                                error.get().map(|err| {
                                    view! { <p class="error" id="auth-error" role="alert">{err}</p> }
                                })
                            }}
                        }
                            .into_any()
                    } else {
                        view! { <p class="auth-hint" id="auth-loading">"Loading…"</p> }.into_any()
                    }
                }}
            </section>
        </main>
    }
}

/// The Account tab: identity, logout (auth mode only) and passkey
/// management.
// The passkey card's row/rename/modal arms make this long; splitting
// them into sub-components would only add indirection.
#[allow(clippy::too_many_lines)]
#[component]
pub fn Account(
    /// The logged-in username.
    username: String,
    /// Dev-bypass mode: no session exists, so the logout button is
    /// hidden (the passkey endpoints work either way).
    dev_mode: bool,
    /// Called after a successful logout.
    on_logout: Callback<()>,
) -> impl IntoView {
    // `None` while the first load is pending.
    let passkeys = RwSignal::new(None::<Vec<flasher_types::PasskeyResponse>>);
    let list_error = RwSignal::new(None::<String>);
    // Row id currently in inline-rename mode, plus the edit field value.
    let renaming = RwSignal::new(None::<i64>);
    let rename_value = RwSignal::new(String::new());
    let rename_error = RwSignal::new(None::<String>);
    // Row id the delete confirm modal is open for, plus its error (the
    // last-passkey 409 lands here).
    let deleting = RwSignal::new(None::<i64>);
    let delete_error = RwSignal::new(None::<String>);
    let add_busy = RwSignal::new(false);
    let action_error = RwSignal::new(None::<String>);
    let logout_error = RwSignal::new(None::<String>);

    // (Re)loads the passkey list.
    let reload = Callback::new(move |(): ()| {
        leptos::task::spawn_local(async move {
            match api::list_passkeys().await {
                Ok(list) => {
                    list_error.set(None);
                    passkeys.set(Some(list));
                }
                Err(err) => list_error.set(Some(err)),
            }
        });
    });

    #[cfg(feature = "csr")]
    Effect::new(move |_| reload.run(()));

    let start_rename = move |(id, name): (i64, String)| {
        rename_error.set(None);
        rename_value.set(name);
        renaming.set(Some(id));
    };

    let save_rename = move |(): ()| {
        let Some(id) = renaming.get_untracked() else {
            return;
        };
        let name = rename_value.get_untracked().trim().to_owned();
        if name.is_empty() {
            rename_error.set(Some("The name must not be empty.".to_owned()));
            return;
        }
        leptos::task::spawn_local(async move {
            match api::rename_passkey(id, &name).await {
                Ok(()) => {
                    renaming.set(None);
                    reload.run(());
                }
                Err(err) => rename_error.set(Some(err)),
            }
        });
    };

    let cancel_rename = move |(): ()| renaming.set(None);

    let ask_delete = move |id: i64| {
        delete_error.set(None);
        deleting.set(Some(id));
    };

    let confirm_delete = move |_| {
        let Some(id) = deleting.get_untracked() else {
            return;
        };
        leptos::task::spawn_local(async move {
            match api::delete_passkey(id).await {
                Ok(()) => {
                    deleting.set(None);
                    reload.run(());
                }
                // Stays open: the 409 "last passkey" guard (and any other
                // error) is shown inside the modal.
                Err(err) => delete_error.set(Some(err)),
            }
        });
    };

    let cancel_delete = move |_| deleting.set(None);

    let add_passkey = move |_| {
        add_busy.set(true);
        action_error.set(None);
        leptos::task::spawn_local(async move {
            // With a session the server ignores the username and attaches
            // the passkey to the session's user; the bootstrap token only
            // applies to the open first-run registration.
            let result = register_ceremony("", None).await;
            add_busy.set(false);
            match result {
                Ok(()) => reload.run(()),
                Err(err) => action_error.set(Some(err)),
            }
        });
    };

    let do_logout = move |_| {
        logout_error.set(None);
        leptos::task::spawn_local(async move {
            match api::logout().await {
                Ok(()) => on_logout.run(()),
                Err(err) => logout_error.set(Some(err)),
            }
        });
    };

    view! {
        <section class="account" id="account">
            <div class="account-identity">
                <p class="account-user">
                    "Signed in as " <strong id="account-username">{username}</strong>
                </p>
                {(!dev_mode).then(|| {
                    view! {
                        <button type="button" id="logout" on:click=do_logout>
                            "Log out"
                        </button>
                    }
                })}
            </div>
            {move || {
                logout_error
                    .get()
                    .map(|err| view! { <p class="error" id="logout-error">{err}</p> })
            }}
            <section class="passkeys-card" id="passkeys-card">
                <h2>"Passkeys"</h2>
                {move || {
                    if let Some(err) = list_error.get() {
                        view! { <p class="error" id="passkeys-error">{err}</p> }.into_any()
                    } else {
                        match passkeys.get() {
                            None => {
                                view! { <p class="auth-hint" id="passkeys-loading">"Loading passkeys…"</p> }
                                    .into_any()
                            }
                            Some(list) => {
                                view! {
                                    <ul class="passkeys-list" id="passkeys-list">
                                        {list
                                            .into_iter()
                                            .map(|passkey| {
                                                view! {
                                                    <PasskeyRow
                                                        passkey=passkey
                                                        renaming=renaming
                                                        rename_value=rename_value
                                                        rename_error=rename_error
                                                        on_start_rename=Callback::new(start_rename)
                                                        on_save_rename=Callback::new(save_rename)
                                                        on_cancel_rename=Callback::new(cancel_rename)
                                                        on_delete=Callback::new(ask_delete)
                                                    />
                                                }
                                            })
                                            .collect_view()}
                                    </ul>
                                }
                                    .into_any()
                            }
                        }
                    }
                }}
                <div class="passkeys-footer">
                    <button
                        type="button"
                        id="add-passkey"
                        disabled=move || add_busy.get()
                        on:click=add_passkey
                    >
                        {move || {
                            if add_busy.get() {
                                "Waiting for passkey…"
                            } else {
                                "Add passkey"
                            }
                        }}
                    </button>
                    {move || {
                        action_error
                            .get()
                            .map(|err| view! { <p class="error" id="account-error">{err}</p> })
                    }}
                </div>
            </section>
            {move || {
                deleting.get().map(|id| {
                    let name = passkeys
                        .get()
                        .unwrap_or_default()
                        .into_iter()
                        .find(|passkey| passkey.id == id)
                        .map_or_else(|| "this passkey".to_owned(), |passkey| passkey.name);
                    view! {
                        <div class="modal-backdrop" id="confirm-delete-modal">
                            <div class="modal" role="dialog" aria-modal="true">
                                <p class="modal-text">
                                    "Delete passkey “" {name} "”? This cannot be undone."
                                </p>
                                {move || {
                                    delete_error
                                        .get()
                                        .map(|err| {
                                            view! { <p class="error" id="delete-error" role="alert">{err}</p> }
                                        })
                                }}
                                <div class="modal-buttons">
                                    <button type="button" id="confirm-delete" class="failed" on:click=confirm_delete>
                                        "Delete"
                                    </button>
                                    <button type="button" id="cancel-delete" on:click=cancel_delete>
                                        "Cancel"
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                })
            }}
        </section>
    }
}

/// One row of the passkey list: name + dates with rename/delete actions,
/// swapped for the inline rename editor while this row is being renamed.
#[component]
fn PasskeyRow(
    passkey: flasher_types::PasskeyResponse,
    /// Row id currently in rename mode (owned by the parent card).
    renaming: RwSignal<Option<i64>>,
    /// Edit field value while renaming.
    rename_value: RwSignal<String>,
    /// Last rename error (422 from the server, empty name).
    rename_error: RwSignal<Option<String>>,
    /// Enters rename mode for (id, current name).
    on_start_rename: Callback<(i64, String)>,
    /// Saves the rename.
    on_save_rename: Callback<()>,
    /// Aborts the rename.
    on_cancel_rename: Callback<()>,
    /// Opens the delete confirm modal for the row id.
    on_delete: Callback<i64>,
) -> impl IntoView {
    let id = passkey.id;
    let row_id = format!("passkey-row-{id}");
    let name_id = format!("passkey-name-{id}");
    let rename_id = format!("rename-passkey-{id}");
    let delete_id = format!("delete-passkey-{id}");
    let dates = format!(
        "created {} · last used {}",
        flasher_core::format_utc_date(passkey.created_at),
        passkey
            .last_used_at
            .map_or_else(|| "never".to_owned(), flasher_core::format_utc_date),
    );
    let name = passkey.name;
    view! {
        <li class="passkey-row" id=row_id>
            {move || {
                if renaming.get() == Some(id) {
                    view! {
                        <span class="passkey-rename">
                            <input
                                type="text"
                                id="rename-input"
                                prop:value=move || rename_value.get()
                                on:input=move |ev| {
                                    rename_value.set(event_target_value(&ev));
                                    rename_error.set(None);
                                }
                            />
                            <button
                                type="button"
                                id="rename-save"
                                on:click=move |_| on_save_rename.run(())
                            >
                                "Save"
                            </button>
                            <button
                                type="button"
                                id="rename-cancel"
                                on:click=move |_| on_cancel_rename.run(())
                            >
                                "Cancel"
                            </button>
                        </span>
                        {move || {
                            rename_error
                                .get()
                                .map(|err| view! { <p class="error" id="rename-error">{err}</p> })
                        }}
                    }
                        .into_any()
                } else {
                    view! {
                        <span class="passkey-info">
                            <span class="passkey-name" id=name_id.clone()>{name.clone()}</span>
                            <span class="passkey-dates">{dates.clone()}</span>
                        </span>
                        <span class="passkey-actions">
                            <button
                                type="button"
                                id=rename_id.clone()
                                on:click={
                                    let name = name.clone();
                                    move |_| on_start_rename.run((id, name.clone()))
                                }
                            >
                                "Rename"
                            </button>
                            <button
                                type="button"
                                id=delete_id.clone()
                                on:click=move |_| on_delete.run(id)
                            >
                                "Delete"
                            </button>
                        </span>
                    }
                        .into_any()
                }
            }}
        </li>
    }
}
