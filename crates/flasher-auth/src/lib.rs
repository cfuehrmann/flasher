//! Passkey (`WebAuthn`) authentication for Flasher, wrapping webauthn-rs.
//!
//! This crate owns everything that touches webauthn-rs types: the
//! registration and username-less (discoverable) authentication ceremonies,
//! the in-memory challenge store, and the opaque session/ceremony token
//! generation. The HTTP contract types live in `flasher-types`; the routes
//! live in `flasher-server`.
//!
//! # HTTP / JSON contract (what the frontend implements against)
//!
//! All endpoints are under `/api/auth`. Ceremony payloads are the standard
//! `WebAuthn` JSON shapes: `camelCase` fields, binary values as base64url
//! strings **without padding** — exactly what `@simplewebauthn/browser`'s
//! `startRegistration()` / `startAuthentication()` consume and produce
//! (and what the browser's `navigator.credentials` API uses after
//! base64url-decoding the binary fields). Error bodies are plain text.
//!
//! Two cookies matter:
//!
//! - `__Host-session` — the login session (`Path=/; HttpOnly; Secure;
//!   SameSite=Strict; Max-Age=604800`). Set by login/finish, cleared by
//!   logout. Opaque 64-char lowercase-hex token (256 bits of entropy).
//! - `flasher-ceremony` — one-shot ceremony handle (`Path=/api/auth;
//!   HttpOnly; Secure; SameSite=Strict; Max-Age=300`). Set by
//!   register/start and login/start, consumed by the matching finish call
//!   (whose response clears the cookie). The browser sends it
//!   automatically; the client never reads it.
//!
//! ## `GET /api/auth/bootstrap` (public)
//!
//! Tells the UI which screen to show. Response `200`:
//!
//! ```json
//! { "registration_open": true, "token_required": false }
//! ```
//!
//! `registration_open` is `true` while the system has ZERO passkeys (any
//! user): only then can register/start be called without a session.
//! `token_required` is `true` when the server has `FLASHER_BOOTSTRAP_TOKEN`
//! configured — the UI then asks for the token on the register screen.
//!
//! ## `GET /api/auth/session` (public route, session-dependent result)
//!
//! `200 {"username": "kakimena"}` with a valid session cookie, else
//! `200 null` (a 200 keeps the routine logged-out probe out of the
//! browser console; protected endpoints still answer `401`).
//! In dev-bypass mode (`FLASHER_USER` set) always `200` with that user.
//!
//! ## `POST /api/auth/register/start`
//!
//! Two modes:
//!
//! - **Bootstrap** (zero passkeys in the system, no session): open to
//!   anyone. If the server has `FLASHER_BOOTSTRAP_TOKEN` set, the body must
//!   carry it as `token` (`403` on mismatch). The `username` (1–64 chars
//!   after trimming) is *claimed*: if a user with that name exists
//!   (case-insensitive) — e.g. a migrated user without passkeys — the new
//!   passkey is attached to that user; otherwise a new user is created.
//! - **Add another passkey** (session present): `username` is ignored, the
//!   passkey is created for the session's user. Without a session and with
//!   ≥1 passkey in the system: `401`.
//!
//! Request:
//!
//! ```json
//! { "username": "kakimena", "token": null }
//! ```
//!
//! Response `200` — `PublicKeyCredentialCreationOptions` for
//! `navigator.credentials.create()` (exact shape, values abbreviated):
//!
//! ```json
//! {
//!   "publicKey": {
//!     "rp": { "name": "flasher", "id": "localhost" },
//!     "user": { "id": "Zmxhc2hlciEAAAAAAAAAAQ", "name": "kakimena", "displayName": "kakimena" },
//!     "challenge": "OykC9KVR4jccBS476Mc784_w6Gv2DfJWjg_6BKN-H1Y",
//!     "pubKeyCredParams": [
//!       { "type": "public-key", "alg": -7 },
//!       { "type": "public-key", "alg": -257 }
//!     ],
//!     "timeout": 300000,
//!     "authenticatorSelection": {
//!       "residentKey": "required",
//!       "requireResidentKey": true,
//!       "userVerification": "required"
//!     },
//!     "attestation": "none",
//!     "extensions": {
//!       "credentialProtectionPolicy": "userVerificationRequired",
//!       "enforceCredentialProtectionPolicy": false,
//!       "uvm": true,
//!       "credProps": true
//!     }
//!   }
//! }
//! ```
//!
//! Notes: `residentKey: "required"` is deliberate — username-less login
//! needs a discoverable credential. With a session (add-another-passkey),
//! an `excludeCredentials` array of the user's existing credentials
//! (`[{ "type": "public-key", "id": "...", "transports": null }]`) is
//! present; on bootstrap it is omitted. Also sets the `flasher-ceremony`
//! cookie. Errors: `401`, `403` (bad bootstrap token), `422` (bad
//! username).
//!
//! ## `POST /api/auth/register/finish`
//!
//! Body: the credential JSON the browser produced, sent back verbatim —
//! the shape `startRegistration()` returns:
//!
//! ```json
//! {
//!   "id": "Az_hMVIXhUdumCNyxlEYpVy6HLAb7XgmGasJ3AZHcp0",
//!   "rawId": "Az_hMVIXhUdumCNyxlEYpVy6HLAb7XgmGasJ3AZHcp0",
//!   "response": {
//!     "attestationObject": "o2NmbXRmcGFja2VkZ2F0dFN0bXSi...",
//!     "clientDataJSON": "eyJ0eXBlIjoid2ViYXV0aG4uY3JlYXRlIi...",
//!     "transports": null
//!   },
//!   "type": "public-key",
//!   "clientExtensionResults": { "credProps": { "rk": true } }
//! }
//! ```
//!
//! (`clientExtensionResults` — the `SimpleWebAuthn` name — or `extensions`:
//! both are accepted.)
//! Response `201` with the created passkey (auto-named `Passkey N`, N =
//! the user's passkey count + 1):
//!
//! ```json
//! { "id": 1, "name": "Passkey 1", "created_at": 1785000000000, "last_used_at": null }
//! ```
//!
//! Does NOT log the user in (no session cookie) — the client proceeds to
//! login/start. Errors: `400` (unknown/expired ceremony or failed
//! verification), `401` (the open bootstrap window closed between start
//! and finish), `409` (credential already registered).
//!
//! ## `POST /api/auth/login/start` (public)
//!
//! Username-less: no body needed (`{}` or empty). Response `200` —
//! `PublicKeyCredentialRequestOptions` with `allowCredentials: []`
//! (discoverable login; the browser offers any passkey it holds for this
//! site; exact shape, challenge abbreviated):
//!
//! ```json
//! {
//!   "publicKey": {
//!     "challenge": "1NQrPqNhml6fKYvjeun4NqYN43T-MLHmzR5Byi4bcKY",
//!     "timeout": 300000,
//!     "rpId": "localhost",
//!     "allowCredentials": [],
//!     "userVerification": "required",
//!     "extensions": { "uvm": true }
//!   }
//! }
//! ```
//!
//! Also sets the `flasher-ceremony` cookie.
//!
//! ## `POST /api/auth/login/finish` (public)
//!
//! Body: the assertion JSON the browser produced, verbatim — the shape
//! `startAuthentication()` returns (values abbreviated; `userHandle` is
//! the base64url user handle the passkey was registered with):
//!
//! ```json
//! {
//!   "id": "L78n1ZsOwTTo6KC2MXicgbHy2Vy4saYxUsutpuNq5hc",
//!   "rawId": "L78n1ZsOwTTo6KC2MXicgbHy2Vy4saYxUsutpuNq5hc",
//!   "response": {
//!     "authenticatorData": "SZYN5YgOjGh0NBcPZHZgW4_k...",
//!     "clientDataJSON": "eyJ0eXBlIjoid2ViYXV0aG4uZ2V0Ii...",
//!     "signature": "MEQCIBjzkbMeryhppvbwfZjx...",
//!     "userHandle": "Zmxhc2hlciEAAAAAAAAAAQ"
//!   },
//!   "type": "public-key",
//!   "clientExtensionResults": {}
//! }
//! ```
//!
//! The user is identified by `userHandle` + `rawId` (credential id).
//! Response `200 {"username": "kakimena"}` and sets the `__Host-session`
//! cookie. Errors: `400` (unknown/expired ceremony), `401` (unknown
//! credential or failed verification).
//!
//! ## `POST /api/auth/logout` (session)
//!
//! Empty body. Deletes the session row and clears the cookie. `204`.
//!
//! ## Passkey management (session required)
//!
//! - `GET /api/auth/passkeys` → `200` array of
//!   `{ "id": 1, "name": "Passkey 1", "created_at": 1785000000000, "last_used_at": null }`
//!   (timestamps are unix epoch millis).
//! - `PATCH /api/auth/passkeys/{id}` `{ "name": "Yubikey" }` (1–64 chars
//!   trimmed; own passkeys only) → `200` with the updated passkey JSON,
//!   `404` for unknown/other-user ids, `422` for a bad name.
//! - `DELETE /api/auth/passkeys/{id}` → `204`; `404` unknown/other-user;
//!   `409` "cannot delete your last passkey" when it is the user's only
//!   one.
//!
//! # Ceremony state
//!
//! Challenges live only in this process's memory (`Mutex<HashMap>`),
//! expire after 5 minutes, are consumed one-shot on finish, and stale
//! entries are evicted on every insert. At [`MAX_LIVE_CHALLENGES`] live
//! (unexpired) entries, starting a new ceremony fails with
//! [`Error::TooManyChallenges`] (mapped to `503` by the server) — a bound
//! on memory growth. A server restart loses in-flight ceremonies —
//! acceptable: the client simply restarts the ceremony.
//!
//! # User handles
//!
//! `WebAuthn` user handles (`user.id` at registration, `userHandle` at
//! login) are deterministic 16-byte uuids derived from the database user
//! id (see [`Auth::user_handle_for`]). They contain no PII (per the spec
//! they must not), are stable per user, and let the username-less login
//! map a discovered credential back to its user.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use webauthn_rs::Webauthn;
use webauthn_rs::prelude::{WebauthnBuilder, WebauthnError};

// Re-exported so `flasher-server` never needs its own webauthn-rs dep.
pub use webauthn_rs::prelude::{
    AuthenticationResult, Base64UrlSafeData, CreationChallengeResponse, CredentialID,
    DiscoverableAuthentication, DiscoverableKey, Passkey, PasskeyRegistration, PublicKeyCredential,
    RegisterPublicKeyCredential, RequestChallengeResponse, Uuid,
};

/// How long a ceremony challenge stays valid.
pub const CHALLENGE_TTL: Duration = Duration::from_mins(5);

/// Maximum number of live (unexpired) challenges held at once. Starting a
/// ceremony beyond the cap fails with [`Error::TooManyChallenges`] (the
/// server maps it to `503`): without a cap, repeated start calls would
/// grow the in-memory store without bound. Expired entries are evicted on
/// every insert and never count against the cap.
pub const MAX_LIVE_CHALLENGES: usize = 100;

/// High 64 bits of every Flasher user handle: `b"flasher!"` — marks the
/// handle as ours and leaves the low 64 bits for the database user id.
const USER_HANDLE_MAGIC: u64 = u64::from_be_bytes(*b"flasher!");

/// Errors of the auth ceremonies.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The relying-party configuration was rejected (bad origin URL).
    #[error("invalid relying-party configuration: {0}")]
    Config(String),
    /// A webauthn-rs ceremony step failed (challenge mismatch, bad
    /// signature, policy violation, ...).
    #[error("webauthn ceremony failed: {0}")]
    Webauthn(#[from] WebauthnError),
    /// The ceremony cookie named no live challenge (unknown id, already
    /// consumed, or expired).
    #[error("unknown or expired ceremony; start again")]
    UnknownCeremony,
    /// The ceremony cookie was created by the *other* ceremony kind
    /// (register vs login).
    #[error("ceremony kind mismatch")]
    CeremonyKind,
    /// The challenge store is at its [`MAX_LIVE_CHALLENGES`] cap; the
    /// ceremony cannot be started right now (mapped to `503` by the
    /// server — the client may retry).
    #[error("too many in-flight ceremonies; try again later")]
    TooManyChallenges,
    /// Serializing/reshaping ceremony JSON failed (internal).
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// The stored half of one in-flight ceremony.
enum ChallengeState {
    Registration {
        state: PasskeyRegistration,
        user_handle: Uuid,
    },
    Authentication(DiscoverableAuthentication),
}

struct ChallengeEntry {
    state: ChallengeState,
    expires: Instant,
}

/// The `WebAuthn` ceremony driver: relying-party configuration plus the
/// in-memory challenge store. Cheap to construct, `Send + Sync`, share
/// one per server (put it in an `Arc` or clone the owning state).
pub struct Auth {
    webauthn: Webauthn,
    challenges: Mutex<HashMap<String, ChallengeEntry>>,
    challenge_ttl: Duration,
}

impl Auth {
    /// Builds the ceremony driver from the relying-party config:
    /// `rp_id` is the effective domain (`FLASHER_RP_ID`, default
    /// `localhost`), `rp_origin` the exact origin the browser talks to
    /// (`FLASHER_ORIGIN`, default `http://localhost:3000`).
    ///
    /// # Errors
    /// Returns [`Error::Config`] if `rp_origin` is not a valid URL or
    /// webauthn-rs rejects the configuration.
    pub fn new(rp_id: &str, rp_origin: &str, rp_name: &str) -> Result<Self, Error> {
        let origin = webauthn_rs::prelude::Url::parse(rp_origin)
            .map_err(|err| Error::Config(format!("invalid rp origin '{rp_origin}': {err}")))?;
        let webauthn = WebauthnBuilder::new(rp_id, &origin)
            .and_then(|builder| builder.rp_name(rp_name).build())
            .map_err(|err| Error::Config(format!("invalid rp configuration: {err}")))?;
        Ok(Self {
            webauthn,
            challenges: Mutex::new(HashMap::new()),
            challenge_ttl: CHALLENGE_TTL,
        })
    }

    /// Overrides the challenge TTL (tests).
    #[must_use]
    pub fn with_challenge_ttl(mut self, ttl: Duration) -> Self {
        self.challenge_ttl = ttl;
        self
    }

    /// The deterministic `WebAuthn` user handle for a database user id:
    /// 16 bytes = `b"flasher!"` magic (high 8) + user id (low 8). Stable
    /// per user, no PII. Not a secret — the spec only forbids PII in
    /// user handles.
    #[must_use]
    pub fn user_handle_for(user_id: i64) -> Uuid {
        // Database ids are non-negative, so the bit pattern round-trips.
        #[allow(clippy::cast_sign_loss)]
        Uuid::from_u64_pair(USER_HANDLE_MAGIC, user_id as u64)
    }

    /// The database user id behind a handle from
    /// [`Auth::user_handle_for`], or `None` for foreign handles.
    #[must_use]
    pub fn user_id_from_handle(handle: &Uuid) -> Option<i64> {
        let (magic, low) = handle.as_u64_pair();
        #[allow(clippy::cast_possible_wrap)]
        (magic == USER_HANDLE_MAGIC).then_some(low as i64)
    }

    /// A fresh opaque token (ceremony id or session token): 64 lowercase
    /// hex chars = 256 bits of entropy.
    #[must_use]
    pub fn generate_token() -> String {
        format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
    }

    /// Starts a passkey registration for `user_handle`/`username`,
    /// excluding the user's existing passkeys (so a device cannot be
    /// registered twice). Returns the creation options for the client
    /// and the ceremony id (for the `flasher-ceremony` cookie).
    ///
    /// The client options are forced to `residentKey: "required"` (and
    /// `requireResidentKey: true`): login here is username-less
    /// (`allowCredentials: []`), so only a discoverable (resident)
    /// credential can ever log in. webauthn-rs would otherwise emit
    /// `discouraged`, which on plain security keys produces a credential
    /// that can never authenticate — better to fail registration loudly
    /// on such a device than to strand the user.
    ///
    /// # Errors
    /// Returns [`Error::Webauthn`] if the challenge cannot be built, or
    /// [`Error::TooManyChallenges`] when the challenge store is full.
    pub fn start_registration(
        &self,
        user_handle: Uuid,
        username: &str,
        existing: &[Passkey],
    ) -> Result<(CreationChallengeResponse, String), Error> {
        let exclude: Vec<CredentialID> = existing.iter().map(|key| key.cred_id().clone()).collect();
        let exclude = (!exclude.is_empty()).then_some(exclude);
        let (ccr, state) =
            self.webauthn
                .start_passkey_registration(user_handle, username, username, exclude)?;
        let ccr = require_resident_key(&ccr)?;
        let ceremony = self.store_challenge(ChallengeState::Registration { state, user_handle })?;
        Ok((ccr, ceremony))
    }

    /// Completes a registration: consumes the ceremony (one-shot) and
    /// verifies the browser's attestation. Returns the new [`Passkey`]
    /// (serialize it with `serde_json` for the `passkeys.data` column)
    /// and the user handle the ceremony was started with (so the caller
    /// can attach the passkey to the right user without a session).
    ///
    /// # Errors
    /// [`Error::UnknownCeremony`] for an unknown/expired/consumed ceremony
    /// id, [`Error::CeremonyKind`] if the ceremony was a login one,
    /// [`Error::Webauthn`] if verification fails.
    pub fn finish_registration(
        &self,
        ceremony: &str,
        reg: &RegisterPublicKeyCredential,
    ) -> Result<(Passkey, Uuid), Error> {
        let state = self.take_challenge(ceremony)?;
        let ChallengeState::Registration { state, user_handle } = state else {
            return Err(Error::CeremonyKind);
        };
        let passkey = self.webauthn.finish_passkey_registration(reg, &state)?;
        Ok((passkey, user_handle))
    }

    /// Starts a username-less (discoverable) authentication:
    /// `allowCredentials: []`, no user hint. Returns the request options
    /// for the client and the ceremony id.
    ///
    /// Uses webauthn-rs' conditional-ui machinery but drops the forced
    /// `mediation: "conditional"` hint: login here is button-driven
    /// (`navigator.credentials.get({ publicKey })`), not autofill.
    ///
    /// # Errors
    /// Returns [`Error::Webauthn`] if the challenge cannot be built, or
    /// [`Error::TooManyChallenges`] when the challenge store is full.
    pub fn start_authentication(&self) -> Result<(RequestChallengeResponse, String), Error> {
        let (mut rcr, state) = self.webauthn.start_discoverable_authentication()?;
        rcr.mediation = None;
        let ceremony = self.store_challenge(ChallengeState::Authentication(state))?;
        Ok((rcr, ceremony))
    }

    /// Extracts `(user_handle, credential_id)` from the browser's
    /// assertion before verification. `credential_id` is base64url (no
    /// padding) — the lookup key of the `passkeys.credential_id` column.
    /// The caller then loads that passkey and its user, checks
    /// [`Auth::user_id_from_handle`] against the passkey's owner, and
    /// calls [`Auth::finish_authentication`].
    ///
    /// # Errors
    /// Returns [`Error::Webauthn`] if the assertion carries no usable
    /// user handle.
    pub fn identify_authentication(
        &self,
        reg: &PublicKeyCredential,
    ) -> Result<(Uuid, String), Error> {
        let (uuid, cred_id) = self.webauthn.identify_discoverable_authentication(reg)?;
        Ok((uuid, base64url(cred_id)))
    }

    /// Completes an authentication: consumes the ceremony (one-shot) and
    /// verifies the assertion against the passkey the caller looked up
    /// via [`Auth::identify_authentication`]. On success the caller
    /// should apply `Passkey::update_credential(&result)` and persist
    /// the blob (counter / backup flags may have changed).
    ///
    /// # Errors
    /// [`Error::UnknownCeremony`] for an unknown/expired/consumed ceremony
    /// id, [`Error::CeremonyKind`] if the ceremony was a registration one,
    /// [`Error::Webauthn`] if verification fails.
    pub fn finish_authentication(
        &self,
        ceremony: &str,
        reg: &PublicKeyCredential,
        passkey: &Passkey,
    ) -> Result<AuthenticationResult, Error> {
        let state = self.take_challenge(ceremony)?;
        let ChallengeState::Authentication(state) = state else {
            return Err(Error::CeremonyKind);
        };
        let keys = [DiscoverableKey::from(passkey)];
        Ok(self
            .webauthn
            .finish_discoverable_authentication(reg, state, &keys)?)
    }

    /// Inserts a challenge under a fresh random id, evicting expired
    /// entries first (cleanup on insert — no background task). At
    /// [`MAX_LIVE_CHALLENGES`] live entries new ceremonies are refused
    /// with [`Error::TooManyChallenges`] (unbounded growth guard).
    fn store_challenge(&self, state: ChallengeState) -> Result<String, Error> {
        self.store_challenge_at(state, Instant::now())
    }

    /// [`Auth::store_challenge`] with the current time passed in, so
    /// tests can pin the exact-expiry eviction boundary.
    fn store_challenge_at(&self, state: ChallengeState, now: Instant) -> Result<String, Error> {
        let id = Self::generate_token();
        let mut challenges = self
            .challenges
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        challenges.retain(|_, entry| entry.expires > now);
        if challenges.len() >= MAX_LIVE_CHALLENGES {
            return Err(Error::TooManyChallenges);
        }
        challenges.insert(
            id.clone(),
            ChallengeEntry {
                state,
                expires: now + self.challenge_ttl,
            },
        );
        Ok(id)
    }

    /// Removes and returns a live challenge (one-shot consumption);
    /// expired or unknown ids are [`Error::UnknownCeremony`].
    fn take_challenge(&self, ceremony: &str) -> Result<ChallengeState, Error> {
        let mut challenges = self
            .challenges
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let entry = challenges.remove(ceremony).ok_or(Error::UnknownCeremony)?;
        if entry.expires <= Instant::now() {
            return Err(Error::UnknownCeremony);
        }
        Ok(entry.state)
    }
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Auth").finish_non_exhaustive()
    }
}

/// Forces `residentKey: "required"` / `requireResidentKey: true` into the
/// client-facing creation options (see [`Auth::start_registration`]).
/// webauthn-rs keeps its server-side registration state untouched (it does
/// not verify residence unless it asked for it), so this only changes what
/// the browser is told to create. Done via JSON because webauthn-rs does
/// not export the options' member types.
fn require_resident_key(
    ccr: &CreationChallengeResponse,
) -> Result<CreationChallengeResponse, Error> {
    let mut value = serde_json::to_value(ccr)?;
    value["publicKey"]["authenticatorSelection"]["residentKey"] =
        serde_json::Value::from("required");
    value["publicKey"]["authenticatorSelection"]["requireResidentKey"] =
        serde_json::Value::from(true);
    Ok(serde_json::from_value(value)?)
}

/// Base64url (no padding) encoding of raw bytes — the format of the
/// `passkeys.credential_id` column and of every binary field in the
/// `WebAuthn` JSON contract.
#[must_use]
pub fn base64url_string(bytes: &[u8]) -> String {
    base64url(bytes)
}

/// Base64url (RFC 4648 §5) without padding — the encoding of the
/// `passkeys.credential_id` column and of every binary field in the
/// `WebAuthn` JSON contract. Identical to what `Base64UrlSafeData`
/// serializes to, spelled out here to avoid a base64 dependency.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests;
