//! Ceremony tests: a full registration + login round-trip against the
//! `softpasskey` software authenticator, plus challenge-store semantics
//! (one-shot, TTL, kind separation) and handle/token helpers.
//!
//! Limitation of the software authenticator: it can neither pick a
//! credential without an `allowCredentials` hint nor return a `userHandle`,
//! so the username-less *discovery* step ([`Auth::identify_authentication`])
//! cannot be exercised here — the browser e2e covers it. What IS exercised:
//! registration, the discoverable server state (`DiscoverableAuthentication`
//! with `allowCredentials: []`), and assertion verification with the
//! identified credential injected — i.e. every server-side step.

use std::time::{Duration, Instant};

use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::prelude::Url;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const RP_ID: &str = "localhost";
const ORIGIN: &str = "http://localhost:3000";

fn test_auth() -> Result<Auth, Error> {
    Auth::new(RP_ID, ORIGIN, "flasher")
}

/// Runs registration + login against the software authenticator and
/// returns its artifacts for the caller to assert on.
fn round_trip() -> Result<(Auth, Passkey, AuthenticationResult), Box<dyn std::error::Error>> {
    let auth = test_auth()?;
    let handle = Auth::user_handle_for(1);
    let origin = Url::parse(ORIGIN)?;
    // falsify_uv = true: the software token claims user verification was
    // performed (our ceremonies require UV).
    let mut token = WebauthnAuthenticator::new(SoftPasskey::new(true));

    // Registration ceremony. The soft token cannot create resident keys,
    // so it gets a COPY of the options with residence downgraded (a
    // browser happily creates one; the production options say "required").
    let (ccr, reg_ceremony) = auth.start_registration(handle, "alice", &[])?;
    assert_eq!(
        ccr.public_key.authenticator_selection.as_ref().map(|sel| {
            serde_json::to_value(sel).ok().and_then(|v| {
                v.get("residentKey")
                    .and_then(|r| r.as_str())
                    .map(str::to_owned)
            })
        }),
        Some(Some("required".to_owned()))
    );
    let mut downgraded = serde_json::to_value(&ccr)?;
    downgraded["publicKey"]["authenticatorSelection"]["residentKey"] =
        serde_json::Value::from("discouraged");
    downgraded["publicKey"]["authenticatorSelection"]["requireResidentKey"] =
        serde_json::Value::from(false);
    let downgraded: CreationChallengeResponse = serde_json::from_value(downgraded)?;
    let reg = token.do_registration(origin.clone(), downgraded)?;
    let (passkey, _handle) = auth.finish_registration(&reg_ceremony, &reg)?;

    // Login ceremony: discoverable — no credential hint, no mediation.
    let (rcr, auth_ceremony) = auth.start_authentication()?;
    assert!(rcr.public_key.allow_credentials.is_empty());
    assert!(rcr.mediation.is_none());

    // The soft token needs an allowCredentials hint to find its credential
    // (a browser's resident key does not). Inject it into a COPY of the
    // request options — the signature ends up over the same challenge, so
    // the server-side verification is unaffected.
    let mut hinted = serde_json::to_value(&rcr)?;
    hinted["publicKey"]["allowCredentials"] = serde_json::json!([{
        "type": "public-key",
        "id": super::base64url(passkey.cred_id()),
    }]);
    let hinted: RequestChallengeResponse = serde_json::from_value(hinted)?;
    let assertion = token.do_authentication(origin, hinted)?;

    // The soft token never returns a userHandle, so identify cannot map it
    // to a user (documented limitation; the browser e2e covers discovery).
    assert!(auth.identify_authentication(&assertion).is_err());

    // The caller looked the passkey up by credential id: verify.
    let result = auth.finish_authentication(&auth_ceremony, &assertion, &passkey)?;
    Ok((auth, passkey, result))
}

#[test]
fn passkey_registration_and_login_round_trip() -> TestResult {
    let (_auth, passkey, result) = round_trip()?;
    assert_eq!(result.cred_id(), passkey.cred_id());
    assert!(result.user_verified());
    Ok(())
}

#[test]
fn base64url_rfc4648_no_pad_vectors() {
    // RFC 4648 §5 test vectors, padding stripped (the WebAuthn contract
    // is base64url WITHOUT padding), plus a URL-safe-alphabet vector:
    // bytes that map to indices 62/63 must be '-'/'_', never '+'/'/'.
    let cases: &[(&[u8], &str)] = &[
        (b"", ""),
        (b"f", "Zg"),
        (b"fo", "Zm8"),
        (b"foo", "Zm9v"),
        (b"foob", "Zm9vYg"),
        (b"fooba", "Zm9vYmE"),
        (b"foobar", "Zm9vYmFy"),
        (&[0xfb, 0xff, 0xfe], "-__-"),
        (&[0xff], "_w"),
        (&[0x00], "AA"),
    ];
    for (bytes, expected) in cases {
        assert_eq!(base64url_string(bytes), *expected, "input: {bytes:02x?}");
        assert!(!base64url_string(bytes).contains('='), "no padding");
    }
}

#[test]
fn base64url_all_byte_values_match_the_reference() -> TestResult {
    // Every byte value (also exercises every 6-bit index of the alphabet)
    // against the reference encoding webauthn-rs serializes.
    let bytes: Vec<u8> = (0..=255).collect();
    let reference = serde_json::to_value(Base64UrlSafeData::from(bytes.clone()))?;
    assert_eq!(
        base64url_string(&bytes),
        reference.as_str().ok_or("not a string")?
    );
    Ok(())
}

#[test]
fn base64url_matches_what_webauthn_serializes() -> TestResult {
    // Pin the hand-rolled encoder against the reference encoding
    // (Base64UrlSafeData's serde output) for every chunk remainder.
    for bytes in [
        &b""[..],
        b"f",
        b"fo",
        b"foo",
        b"foob",
        b"fooba",
        b"foobar",
        &[0xff, 0xee, 0xdd],
        &[0x00],
    ] {
        let reference = serde_json::to_value(Base64UrlSafeData::from(bytes.to_vec()))?;
        assert_eq!(
            super::base64url(bytes),
            reference.as_str().ok_or("not a string")?
        );
    }
    Ok(())
}

#[test]
fn registered_passkey_serializes_through_json_for_storage() -> TestResult {
    let (_auth, passkey, _result) = round_trip()?;
    // The passkeys.data column holds this exact serialization.
    let json = serde_json::to_string(&passkey)?;
    let restored: Passkey = serde_json::from_str(&json)?;
    assert_eq!(restored, passkey);
    Ok(())
}

#[test]
fn registration_challenge_is_one_shot() -> TestResult {
    let auth = test_auth()?;
    let (_ccr, ceremony) = auth.start_registration(Auth::user_handle_for(1), "alice", &[])?;
    // A bogus finish consumes the challenge: the retry must report an
    // unknown ceremony, not a verification error.
    let bogus: RegisterPublicKeyCredential = serde_json::from_value(serde_json::json!({
        "id": "AA",
        "rawId": "AA",
        "response": {
            "attestationObject": "AA",
            "clientDataJSON": "AA"
        },
        "type": "public-key"
    }))?;
    let first = auth.finish_registration(&ceremony, &bogus);
    assert!(matches!(first, Err(Error::Webauthn(_))), "got: {first:?}");
    let second = auth.finish_registration(&ceremony, &bogus);
    assert!(
        matches!(second, Err(Error::UnknownCeremony)),
        "got: {second:?}"
    );
    Ok(())
}

#[test]
fn expired_challenge_is_unknown() -> TestResult {
    let auth = test_auth()?.with_challenge_ttl(Duration::from_millis(0));
    let (_rcr, ceremony) = auth.start_authentication()?;
    let bogus: PublicKeyCredential = serde_json::from_value(serde_json::json!({
        "id": "AA",
        "rawId": "AA",
        "response": {
            "authenticatorData": "AA",
            "clientDataJSON": "AA",
            "signature": "AA",
            "userHandle": "AA"
        },
        "type": "public-key"
    }))?;
    let (_auth2, passkey, _r) = round_trip()?;
    let result = auth.finish_authentication(&ceremony, &bogus, &passkey);
    assert!(
        matches!(result, Err(Error::UnknownCeremony)),
        "got: {result:?}"
    );
    Ok(())
}

#[test]
fn ceremony_kinds_are_not_interchangeable() -> TestResult {
    let auth = test_auth()?;
    let (_rcr, login_ceremony) = auth.start_authentication()?;
    // Using a LOGIN ceremony id to finish a REGISTRATION is a kind error
    // (and consumes the challenge).
    let bogus: RegisterPublicKeyCredential = serde_json::from_value(serde_json::json!({
        "id": "AA",
        "rawId": "AA",
        "response": {
            "attestationObject": "AA",
            "clientDataJSON": "AA"
        },
        "type": "public-key"
    }))?;
    let result = auth.finish_registration(&login_ceremony, &bogus);
    assert!(
        matches!(result, Err(Error::CeremonyKind)),
        "got: {result:?}"
    );
    Ok(())
}

#[test]
fn unknown_ceremony_is_rejected() -> TestResult {
    let auth = test_auth()?;
    let bogus: RegisterPublicKeyCredential = serde_json::from_value(serde_json::json!({
        "id": "AA",
        "rawId": "AA",
        "response": {
            "attestationObject": "AA",
            "clientDataJSON": "AA"
        },
        "type": "public-key"
    }))?;
    let result = auth.finish_registration("no-such-ceremony", &bogus);
    assert!(
        matches!(result, Err(Error::UnknownCeremony)),
        "got: {result:?}"
    );
    Ok(())
}

#[test]
fn user_handle_round_trips_and_rejects_foreign_handles() {
    for user_id in [1, 42, i64::MAX] {
        let handle = Auth::user_handle_for(user_id);
        assert_eq!(Auth::user_id_from_handle(&handle), Some(user_id));
    }
    assert_eq!(Auth::user_id_from_handle(&Uuid::new_v4()), None);
}

#[test]
fn generated_tokens_are_64_hex_chars_and_distinct() {
    let a = Auth::generate_token();
    let b = Auth::generate_token();
    assert_eq!(a.len(), 64);
    assert!(a.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_ne!(a, b);
}

#[test]
fn invalid_origin_is_a_config_error() {
    let result = Auth::new(RP_ID, "not a url", "flasher");
    assert!(matches!(result, Err(Error::Config(_))), "got: {result:?}");
}

#[test]
fn challenge_store_refuses_to_grow_past_the_cap() -> TestResult {
    let auth = test_auth()?;
    for _ in 0..MAX_LIVE_CHALLENGES {
        auth.start_authentication()?;
    }
    let result = auth.start_authentication();
    assert!(
        matches!(result, Err(Error::TooManyChallenges)),
        "got: {result:?}"
    );
    let result = auth.start_registration(Auth::user_handle_for(1), "alice", &[]);
    assert!(
        matches!(result, Err(Error::TooManyChallenges)),
        "got: {result:?}"
    );
    Ok(())
}

#[test]
fn expired_challenges_do_not_count_against_the_cap() -> TestResult {
    // TTL 0: every entry is already expired, so the insert-time eviction
    // clears the store and the cap never bites.
    let auth = test_auth()?.with_challenge_ttl(Duration::from_millis(0));
    for _ in 0..MAX_LIVE_CHALLENGES * 2 {
        auth.start_authentication()?;
    }
    Ok(())
}

#[test]
fn a_challenge_expiring_exactly_at_insert_time_is_evicted() -> TestResult {
    // Boundary of the insert-time eviction: an entry with
    // `expires == now` is EXPIRED (the retain keeps only `expires > now`)
    // and must not count against the cap. The clock is injected via
    // `store_challenge_at` so the boundary is hit exactly, not by luck.
    let auth = test_auth()?;
    let t0 = Instant::now();
    // Fill the store to the cap; every entry expires at t0 + TTL.
    for _ in 0..MAX_LIVE_CHALLENGES {
        let (_rcr, state) = auth.webauthn.start_discoverable_authentication()?;
        auth.store_challenge_at(ChallengeState::Authentication(state), t0)?;
    }
    // At exactly t0 + TTL all entries are expired: the insert succeeds.
    let (_rcr, state) = auth.webauthn.start_discoverable_authentication()?;
    auth.store_challenge_at(ChallengeState::Authentication(state), t0 + CHALLENGE_TTL)?;
    Ok(())
}

#[test]
fn exclude_credentials_is_absent_without_and_present_with_passkeys() -> TestResult {
    let auth = test_auth()?;
    // First passkey: no excludeCredentials (a present-but-empty array
    // would be wrong — webauthn-rs emits the field only when it is Some).
    let (ccr, _ceremony) = auth.start_registration(Auth::user_handle_for(1), "alice", &[])?;
    let options = serde_json::to_value(&ccr)?;
    assert!(
        !options["publicKey"]["excludeCredentials"].is_array(),
        "no passkeys: excludeCredentials must be absent, got {options}"
    );
    // With an existing passkey its credential id is excluded (so the
    // same device cannot be registered twice).
    let (_auth2, passkey, _r) = round_trip()?;
    let (ccr, _ceremony) = auth.start_registration(
        Auth::user_handle_for(1),
        "alice",
        std::slice::from_ref(&passkey),
    )?;
    let options = serde_json::to_value(&ccr)?;
    let excluded = options["publicKey"]["excludeCredentials"]
        .as_array()
        .ok_or("excludeCredentials must list the existing passkey")?;
    assert_eq!(excluded.len(), 1);
    assert_eq!(
        excluded[0]["id"],
        serde_json::Value::from(super::base64url(passkey.cred_id()))
    );
    Ok(())
}

#[test]
fn auth_debug_shows_no_ceremony_state() -> TestResult {
    // The Debug impl exists so `AppState` can derive Debug; its contract
    // is to reveal NOTHING (challenges are session-bound secrets-adjacent
    // state): a fixed non-exhaustive placeholder, even with live
    // ceremonies in the store.
    let auth = test_auth()?;
    let (_ccr, ceremony) = auth.start_registration(Auth::user_handle_for(1), "alice", &[])?;
    let debug = format!("{auth:?}");
    assert_eq!(debug, "Auth { .. }");
    assert!(!debug.contains(&ceremony));
    Ok(())
}
