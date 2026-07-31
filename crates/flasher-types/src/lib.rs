//! Single contract authority for all Flasher API payloads.
//!
//! Both the server (`flasher-server`) and the frontend (`flasher-leptos`)
//! depend on this crate, so request/response types can never drift apart
//! between the two sides of the API.
//!
//! # JSON shapes
//!
//! All payloads use the serde defaults: struct fields serialize as
//! `snake_case` exactly as declared, and [`CardState`] serializes as a
//! lowercase string (`"new"` / `"ok"` / `"failed"`). Timestamps are unix
//! epoch millis (`i64`).

use serde::{Deserialize, Serialize};

/// Response of `GET /api/health`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HealthResponse {
    /// Liveness indicator; `"ok"` when the server is up.
    pub status: String,
    /// Version of the running `flasher-server` crate.
    pub version: String,
}

/// Spaced-repetition state of a card, serialized as a lowercase string.
///
/// Stored as TEXT in `SQLite` (`new`/`ok`/`failed`): the `sqlx::Type`
/// impl is available behind the `sqlx` cargo feature, which only
/// `flasher-store` enables (the frontend gets a sqlx-free build). The
/// old .NET file store wrote the enum as `PascalCase` strings
/// (`New`/`Ok`/`Failed`); the importer in `flasher-migrate` maps those
/// onto this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "TEXT", rename_all = "lowercase"))]
#[serde(rename_all = "lowercase")]
pub enum CardState {
    New,
    Ok,
    Failed,
}

impl CardState {
    /// The lowercase database/JSON representation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Ok => "ok",
            Self::Failed => "failed",
        }
    }

    /// Parses the lowercase database/JSON representation.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "new" => Some(Self::New),
            "ok" => Some(Self::Ok),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// A card as returned by the cards API (`GET /api/cards/next`,
/// `POST /api/cards`, `POST /api/cards/{id}/set-ok|set-failed`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CardResponse {
    pub id: String,
    pub prompt: String,
    pub solution: String,
    pub state: CardState,
    /// Unix epoch millis.
    pub change_time: i64,
    /// Unix epoch millis.
    pub next_time: i64,
    pub disabled: bool,
}

/// Body of `POST /api/cards/{id}/set-ok|set-failed`: the `change_time`
/// of the card as the client last saw it. The server applies the rating
/// only when the stored `change_time` still matches (conditional update);
/// otherwise it answers 409, so a duplicated or stale rating can never
/// silently re-schedule the card off its just-written `change_time`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SetCardStateRequest {
    /// Unix epoch millis.
    pub change_time: i64,
}

/// Body of `POST /api/cards`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreateCardRequest {
    pub prompt: String,
    pub solution: String,
}

/// Body of `PATCH /api/cards/{id}`: an all-optional partial update of the
/// content fields, like the old `CardUpdate` — `null`/absent leaves the
/// field unchanged. A body with all three fields absent is rejected with
/// 422 by the server.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CardUpdateRequest {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub solution: Option<String>,
    #[serde(default)]
    pub disabled: Option<bool>,
}

/// Groom list filter on the `disabled` flag: the `disabled_filter` query
/// parameter of `GET /api/cards` (`snake_case` values). Both sides share
/// this enum, so the wire values cannot drift. The default is `All`
/// (owner decision 2026-07-31, revised the same day): the groom list
/// shows everything on first use; the UI persists the user's choice
/// (localStorage), so the default only ever applies to a fresh browser.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisabledFilter {
    /// Only enabled cards.
    Enabled,
    /// Only disabled cards.
    Disabled,
    /// No filtering — enabled and disabled cards.
    #[default]
    All,
}

impl DisabledFilter {
    /// The `snake_case` wire representation (query parameter value).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::All => "all",
        }
    }

    /// Parses the `snake_case` wire representation.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "enabled" => Some(Self::Enabled),
            "disabled" => Some(Self::Disabled),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

/// Response of `GET /api/cards`, matching the shape of the old
/// `FindResponse`: one page of cards plus the total number of cards
/// matching the search (before paging), so the UI can render pagination.
/// `page_size` echoes the server's configured page size
/// (`FLASHER_PAGE_SIZE`), so clients never have to hard-code it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FindCardsResponse {
    pub cards: Vec<CardResponse>,
    pub count: i64,
    pub page_size: i64,
}

/// Response of `GET /api/cards/next`: the next due enabled card, or
/// `null` when there is none. Serialized as plain
/// `Option<CardResponse>`.
pub type NextCardResponse = Option<CardResponse>;

/// The autosaved draft of a card edit session, as returned by
/// `GET`/`PUT /api/autosave`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AutoSaveResponse {
    /// The card being edited (the old `AutoSave.Id`); `null` for a draft
    /// of a brand-new card.
    pub card_id: Option<String>,
    pub prompt: String,
    pub solution: String,
    /// Unix epoch millis.
    pub updated_at: i64,
}

/// Body of `PUT /api/autosave`: upserts the current user's draft.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PutAutoSaveRequest {
    #[serde(default)]
    pub card_id: Option<String>,
    pub prompt: String,
    pub solution: String,
}

/// Response of `GET /api/autosave`: the current user's draft, or `null`
/// when there is none. Serialized as plain `Option<AutoSaveResponse>`.
pub type GetAutoSaveResponse = Option<AutoSaveResponse>;

// ---------------------------------------------------------------------- auth
//
// The `WebAuthn` *ceremony* payloads (the options the server sends to
// `navigator.credentials.create/get` and the credential objects the browser
// returns) are deliberately NOT modelled as structs here: they are the
// standard WebAuthn JSON shapes (camelCase, binary fields as base64url
// strings) exactly as produced/consumed by the browser and by webauthn-rs.
// Modelling them by hand would only risk drifting away from what the
// browser emits. They are opaque pass-through `serde_json::Value`s on the
// wire; the exact shapes with full example payloads are documented in the
// `flasher-auth` crate docs (and pinned by its contract tests).

/// Body of `POST /api/auth/register/start`.
///
/// `username` is 1–64 chars after trimming. On the open bootstrap (zero
/// passkeys in the system, no session) it names the user to claim or
/// create; with a session it is ignored (the passkey is added to the
/// session's user). `token` is the bootstrap token, required only while
/// bootstrapping with `FLASHER_BOOTSTRAP_TOKEN` configured on the server.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RegisterStartRequest {
    pub username: String,
    #[serde(default)]
    pub token: Option<String>,
}

/// Response of `POST /api/auth/register/start`: the standard `WebAuthn`
/// `PublicKeyCredentialCreationOptions` JSON (wrapped as
/// `{"publicKey": {...}}`), passed to `navigator.credentials.create()`
/// after base64url-decoding the binary fields. See the `flasher-auth`
/// crate docs for a full example payload.
pub type RegisterStartResponse = serde_json::Value;

/// Body of `POST /api/auth/register/finish`: the credential JSON the
/// browser produced (`PublicKeyCredential` with an attestation response,
/// camelCase fields, binary fields base64url-encoded — the shape
/// `@simplewebauthn/browser`'s `startRegistration()` returns). Sent back
/// verbatim; the server pairs it with the in-memory ceremony state keyed
/// by the `flasher-ceremony` cookie set by register/start. See the
/// `flasher-auth` crate docs for a full example payload.
pub type RegisterFinishRequest = serde_json::Value;

/// Response of `POST /api/auth/login/start`: the standard `WebAuthn`
/// `PublicKeyCredentialRequestOptions` JSON (wrapped as
/// `{"publicKey": {...}}`, `allowCredentials: []` — username-less,
/// discoverable login). See the `flasher-auth` crate docs for a full
/// example payload.
pub type LoginStartResponse = serde_json::Value;

/// Body of `POST /api/auth/login/finish`: the assertion JSON the browser
/// produced (`PublicKeyCredential` with an assertion response, the shape
/// `@simplewebauthn/browser`'s `startAuthentication()` returns). See the
/// `flasher-auth` crate docs for a full example payload.
pub type LoginFinishRequest = serde_json::Value;

/// Response of `GET /api/auth/session` and of a successful
/// `POST /api/auth/login/finish`: the authenticated user.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SessionResponse {
    pub username: String,
}

/// Response of `GET /api/auth/bootstrap`: `registration_open` is true
/// while the system has zero passkeys, i.e. `POST /api/auth/register/start`
/// can be called without a session. `token_required` is true when the
/// server is configured with `FLASHER_BOOTSTRAP_TOKEN`: the open bootstrap
/// registration must then send it as `RegisterStartRequest.token`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BootstrapResponse {
    pub registration_open: bool,
    pub token_required: bool,
}

/// One passkey as returned by `GET /api/auth/passkeys` (list) and
/// `POST /api/auth/register/finish` (the newly created one, auto-named
/// `Passkey N`). Timestamps are unix epoch millis.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PasskeyResponse {
    /// Database id of the passkey row (used by PATCH/DELETE).
    pub id: i64,
    pub name: String,
    /// Unix epoch millis.
    pub created_at: i64,
    /// Unix epoch millis; `null` if the passkey was never used to log in.
    pub last_used_at: Option<i64>,
}

/// Body of `PATCH /api/auth/passkeys/{id}`: renames the passkey
/// (1–64 chars after trimming; own passkeys only).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RenamePasskeyRequest {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), serde_json::Error>;

    #[test]
    fn health_response_round_trips_through_json() -> TestResult {
        let original = HealthResponse {
            status: "ok".to_owned(),
            version: "0.1.0".to_owned(),
        };
        let json = serde_json::to_string(&original)?;
        let parsed: HealthResponse = serde_json::from_str(&json)?;
        assert_eq!(original, parsed);
        Ok(())
    }

    #[test]
    fn card_state_serializes_lowercase() -> TestResult {
        assert_eq!(serde_json::to_string(&CardState::New)?, "\"new\"");
        assert_eq!(serde_json::to_string(&CardState::Ok)?, "\"ok\"");
        assert_eq!(serde_json::to_string(&CardState::Failed)?, "\"failed\"");
        assert_eq!(
            serde_json::from_str::<CardState>("\"failed\"")?,
            CardState::Failed
        );
        Ok(())
    }

    #[test]
    fn card_state_str_round_trip() {
        for state in [CardState::New, CardState::Ok, CardState::Failed] {
            assert_eq!(CardState::parse(state.as_str()), Some(state));
        }
        assert_eq!(CardState::parse("unknown"), None);
        assert_eq!(CardState::parse("New"), None);
    }

    #[test]
    fn disabled_filter_serializes_snake_case() -> TestResult {
        assert_eq!(
            serde_json::to_string(&DisabledFilter::Enabled)?,
            "\"enabled\""
        );
        assert_eq!(
            serde_json::to_string(&DisabledFilter::Disabled)?,
            "\"disabled\""
        );
        assert_eq!(serde_json::to_string(&DisabledFilter::All)?, "\"all\"");
        assert_eq!(
            serde_json::from_str::<DisabledFilter>("\"disabled\"")?,
            DisabledFilter::Disabled
        );
        assert!(serde_json::from_str::<DisabledFilter>("\"bogus\"").is_err());
        Ok(())
    }

    #[test]
    fn disabled_filter_str_round_trip_and_default() {
        for filter in [
            DisabledFilter::Enabled,
            DisabledFilter::Disabled,
            DisabledFilter::All,
        ] {
            assert_eq!(DisabledFilter::parse(filter.as_str()), Some(filter));
        }
        assert_eq!(DisabledFilter::parse("bogus"), None);
        // Owner decision 2026-07-31 (revised same day): the groom list
        // defaults to all on first use; the choice persists client-side.
        assert_eq!(DisabledFilter::default(), DisabledFilter::All);
    }

    #[test]
    fn card_response_uses_snake_case_fields() -> TestResult {
        let card = CardResponse {
            id: "c1".to_owned(),
            prompt: "p".to_owned(),
            solution: "s".to_owned(),
            state: CardState::Ok,
            change_time: 1,
            next_time: 2,
            disabled: false,
        };
        let json = serde_json::to_string(&card)?;
        assert_eq!(
            json,
            r#"{"id":"c1","prompt":"p","solution":"s","state":"ok","change_time":1,"next_time":2,"disabled":false}"#
        );
        let parsed: CardResponse = serde_json::from_str(&json)?;
        assert_eq!(parsed, card);
        Ok(())
    }

    #[test]
    fn next_card_response_is_plain_option() -> TestResult {
        let none: NextCardResponse = None;
        assert_eq!(serde_json::to_string(&none)?, "null");
        let parsed: NextCardResponse = serde_json::from_str("null")?;
        assert_eq!(parsed, None);
        Ok(())
    }

    #[test]
    fn create_card_request_round_trips() -> TestResult {
        let request = CreateCardRequest {
            prompt: "Q?".to_owned(),
            solution: "A.".to_owned(),
        };
        let json = serde_json::to_string(&request)?;
        assert_eq!(json, r#"{"prompt":"Q?","solution":"A."}"#);
        let parsed: CreateCardRequest = serde_json::from_str(&json)?;
        assert_eq!(parsed, request);
        Ok(())
    }

    #[test]
    fn card_update_request_is_all_optional() -> TestResult {
        let empty: CardUpdateRequest = serde_json::from_str("{}")?;
        assert_eq!(
            empty,
            CardUpdateRequest {
                prompt: None,
                solution: None,
                disabled: None,
            }
        );
        let partial: CardUpdateRequest = serde_json::from_str(r#"{"disabled":true}"#)?;
        assert_eq!(partial.disabled, Some(true));
        assert_eq!(partial.prompt, None);
        let json = serde_json::to_string(&partial)?;
        assert_eq!(json, r#"{"prompt":null,"solution":null,"disabled":true}"#);
        Ok(())
    }

    #[test]
    fn autosave_response_uses_snake_case_fields() -> TestResult {
        let response = AutoSaveResponse {
            card_id: Some("c1".to_owned()),
            prompt: "p".to_owned(),
            solution: "s".to_owned(),
            updated_at: 42,
        };
        let json = serde_json::to_string(&response)?;
        assert_eq!(
            json,
            r#"{"card_id":"c1","prompt":"p","solution":"s","updated_at":42}"#
        );
        let parsed: AutoSaveResponse = serde_json::from_str(&json)?;
        assert_eq!(parsed, response);

        let new_card_draft = AutoSaveResponse {
            card_id: None,
            ..response
        };
        let json = serde_json::to_string(&new_card_draft)?;
        assert_eq!(
            json,
            r#"{"card_id":null,"prompt":"p","solution":"s","updated_at":42}"#
        );
        Ok(())
    }

    #[test]
    fn put_auto_save_request_card_id_defaults_to_none() -> TestResult {
        let request: PutAutoSaveRequest = serde_json::from_str(r#"{"prompt":"p","solution":"s"}"#)?;
        assert_eq!(
            request,
            PutAutoSaveRequest {
                card_id: None,
                prompt: "p".to_owned(),
                solution: "s".to_owned(),
            }
        );
        let with_card: PutAutoSaveRequest =
            serde_json::from_str(r#"{"card_id":"c1","prompt":"p","solution":"s"}"#)?;
        assert_eq!(with_card.card_id.as_deref(), Some("c1"));
        Ok(())
    }

    #[test]
    fn get_auto_save_response_is_plain_option() -> TestResult {
        let none: GetAutoSaveResponse = None;
        assert_eq!(serde_json::to_string(&none)?, "null");
        let parsed: GetAutoSaveResponse = serde_json::from_str("null")?;
        assert_eq!(parsed, None);
        Ok(())
    }

    #[test]
    fn auth_types_use_snake_case_fields() -> TestResult {
        let start = RegisterStartRequest {
            username: "kakimena".to_owned(),
            token: None,
        };
        assert_eq!(
            serde_json::to_string(&start)?,
            r#"{"username":"kakimena","token":null}"#
        );
        let session = SessionResponse {
            username: "kakimena".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&session)?,
            r#"{"username":"kakimena"}"#
        );
        let bootstrap = BootstrapResponse {
            registration_open: true,
            token_required: false,
        };
        assert_eq!(
            serde_json::to_string(&bootstrap)?,
            r#"{"registration_open":true,"token_required":false}"#
        );
        let passkey = PasskeyResponse {
            id: 7,
            name: "Passkey 1".to_owned(),
            created_at: 1,
            last_used_at: None,
        };
        assert_eq!(
            serde_json::to_string(&passkey)?,
            r#"{"id":7,"name":"Passkey 1","created_at":1,"last_used_at":null}"#
        );
        let rename = RenamePasskeyRequest {
            name: "Yubikey".to_owned(),
        };
        assert_eq!(serde_json::to_string(&rename)?, r#"{"name":"Yubikey"}"#);
        Ok(())
    }

    #[test]
    fn register_start_request_token_defaults_to_none() -> TestResult {
        let request: RegisterStartRequest = serde_json::from_str(r#"{"username":"a"}"#)?;
        assert_eq!(request.token, None);
        Ok(())
    }

    #[test]
    fn find_cards_response_matches_old_find_response_shape() -> TestResult {
        let response = FindCardsResponse {
            cards: vec![CardResponse {
                id: "c1".to_owned(),
                prompt: "p".to_owned(),
                solution: "s".to_owned(),
                state: CardState::New,
                change_time: 1,
                next_time: 2,
                disabled: true,
            }],
            count: 7,
            page_size: 10,
        };
        let json = serde_json::to_string(&response)?;
        assert_eq!(
            json,
            r#"{"cards":[{"id":"c1","prompt":"p","solution":"s","state":"new","change_time":1,"next_time":2,"disabled":true}],"count":7,"page_size":10}"#
        );
        let parsed: FindCardsResponse = serde_json::from_str(&json)?;
        assert_eq!(parsed, response);
        Ok(())
    }
}
