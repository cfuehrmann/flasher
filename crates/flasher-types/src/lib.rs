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
    /// Monotonic card revision used to reject stale edits.
    pub revision: i64,
    /// The card's labels (opaque names; the app attaches no semantics
    /// to any of them).
    pub labels: Vec<String>,
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

/// Body of `POST /api/cards`. `labels` is the card's initial label set:
/// the user picks it at creation time (owner decision 2026-08-01), so it
/// is required and must not be empty (422). Names are opaque — unknown
/// ones are created on demand.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreateCardRequest {
    pub prompt: String,
    pub solution: String,
    pub labels: Vec<String>,
}

/// Body of `PATCH /api/cards/{id}`: an all-optional partial update of the
/// content fields, like the old `CardUpdate` — `null`/absent leaves the
/// field unchanged. `labels` REPLACES the card's whole label set; only
/// existing label names are accepted and the set must not be empty (the
/// server rejects both with 422). A body with all three fields absent is
/// rejected with 422 by the server.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CardUpdateRequest {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub solution: Option<String>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
}

/// One label as returned by `GET /api/labels`. The numeric ID is the stable
/// identity; the name is a user-visible value that may be renamed. Card
/// filter requests still carry names at the current internal wire boundary.
/// Label names carry NO semantics anywhere in the app (owner decision 2026-08-01): they
/// are opaque strings the user invents and assigns at card-creation
/// time; a fresh database has no labels at all.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LabelResponse {
    /// Database id of the label row.
    pub id: i64,
    pub name: String,
    /// Number of cards owned by the current user that carry this label.
    pub card_count: i64,
}

/// Body of `POST /api/labels`: creates one label for the current user.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreateLabelRequest {
    pub name: String,
}

/// Body of `PATCH /api/labels/{id}`: renames one label owned by the
/// current user. The new name is trimmed and must be 1–64 characters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RenameLabelRequest {
    pub name: String,
}

/// Body of `DELETE /api/labels/{id}`. The first request uses `false` to
/// ask the server to check usage; a request with `true` explicitly permits
/// deleting a label from all cards that carry it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DeleteLabelRequest {
    pub confirm: bool,
}

/// Conflict response for deleting a label that is still attached to cards.
/// The frontend uses the exact count in the confirmation warning.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LabelDeleteConflict {
    pub affected_cards: i64,
}

/// Upper bound for the required per-request `take` of `GET /api/cards`
/// (the groom tab requests exactly as many rows as fit its viewport):
/// the server clamps to this, and the client mirrors the cap so normal
/// browser requests are already within the bound.
pub const MAX_TAKE: u32 = 100;

/// Response of `GET /api/cards`, matching the shape of the old
/// `FindResponse`: one page of cards plus the total number of cards
/// matching the search (before paging), so the UI can render pagination.
/// `page_size` echoes the effective page size — the request's `take` after
/// the server applies the [`MAX_TAKE`] clamp.
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

/// The user's pending Add card draft.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NewCardDraftResponse {
    pub prompt: String,
    pub solution: String,
    /// Unix epoch millis.
    pub updated_at: i64,
}

/// Body of `PUT /api/new-card-draft`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PutNewCardDraftRequest {
    pub prompt: String,
    pub solution: String,
}

/// A pending edit for one existing card.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CardEditDraftResponse {
    pub card_id: String,
    pub prompt: String,
    pub solution: String,
    pub labels: Vec<String>,
    pub base_revision: i64,
    /// Unix epoch millis.
    pub updated_at: i64,
}

/// Body of `PUT /api/cards/{id}/draft`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PutCardEditDraftRequest {
    pub base_revision: i64,
    pub prompt: String,
    pub solution: String,
    pub labels: Vec<String>,
}

/// A small summary used by Groom to mark cards with pending edits.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CardDraftSummary {
    pub card_id: String,
    /// Unix epoch millis.
    pub updated_at: i64,
}

/// Body of `PUT /api/cards/{id}`: commit a complete edit session.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SaveCardEditRequest {
    pub expected_revision: i64,
    pub prompt: String,
    pub solution: String,
    pub labels: Vec<String>,
}

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
    fn label_response_uses_snake_case_fields() -> TestResult {
        let label = LabelResponse {
            id: 7,
            name: "Enabled".to_owned(),
            card_count: 3,
        };
        let json = serde_json::to_string(&label)?;
        assert_eq!(json, r#"{"id":7,"name":"Enabled","card_count":3}"#);
        let parsed: LabelResponse = serde_json::from_str(&json)?;
        assert_eq!(parsed, label);
        Ok(())
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
            revision: 3,
            labels: vec!["Enabled".to_owned()],
        };
        let json = serde_json::to_string(&card)?;
        assert_eq!(
            json,
            r#"{"id":"c1","prompt":"p","solution":"s","state":"ok","change_time":1,"next_time":2,"revision":3,"labels":["Enabled"]}"#
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
            labels: vec!["Disabled".to_owned()],
        };
        let json = serde_json::to_string(&request)?;
        assert_eq!(
            json,
            r#"{"prompt":"Q?","solution":"A.","labels":["Disabled"]}"#
        );
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
                labels: None,
            }
        );
        let partial: CardUpdateRequest = serde_json::from_str(r#"{"labels":["Disabled"]}"#)?;
        assert_eq!(partial.labels, Some(vec!["Disabled".to_owned()]));
        assert_eq!(partial.prompt, None);
        let json = serde_json::to_string(&partial)?;
        assert_eq!(
            json,
            r#"{"prompt":null,"solution":null,"labels":["Disabled"]}"#
        );
        Ok(())
    }

    #[test]
    fn new_card_draft_uses_snake_case_fields() -> TestResult {
        let response = NewCardDraftResponse {
            prompt: "p".to_owned(),
            solution: "s".to_owned(),
            updated_at: 42,
        };
        let json = serde_json::to_string(&response)?;
        assert_eq!(json, r#"{"prompt":"p","solution":"s","updated_at":42}"#);
        let parsed: NewCardDraftResponse = serde_json::from_str(&json)?;
        assert_eq!(parsed, response);
        Ok(())
    }

    #[test]
    fn edit_draft_contract_is_target_scoped() -> TestResult {
        let request: PutCardEditDraftRequest = serde_json::from_str(
            r#"{"base_revision":7,"prompt":"p","solution":"s","labels":["A"]}"#,
        )?;
        assert_eq!(
            request,
            PutCardEditDraftRequest {
                base_revision: 7,
                prompt: "p".to_owned(),
                solution: "s".to_owned(),
                labels: vec!["A".to_owned()],
            }
        );
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
                revision: 0,
                labels: vec!["Disabled".to_owned()],
            }],
            count: 7,
            page_size: 10,
        };
        let json = serde_json::to_string(&response)?;
        assert_eq!(
            json,
            r#"{"cards":[{"id":"c1","prompt":"p","solution":"s","state":"new","change_time":1,"next_time":2,"revision":0,"labels":["Disabled"]}],"count":7,"page_size":10}"#
        );
        let parsed: FindCardsResponse = serde_json::from_str(&json)?;
        assert_eq!(parsed, response);
        Ok(())
    }
}
