//! `WebAuthn` browser glue for the passkey ceremonies (Phase 5B).
//!
//! The server speaks the standard `WebAuthn` JSON shapes
//! (`@simplewebauthn/browser` conventions: camelCase, binary fields as
//! base64url **without padding**). The browser's `navigator.credentials`
//! API speaks `ArrayBuffer`s. This module converts between the two:
//!
//! - [`create_credential`]: `PublicKeyCredentialCreationOptions` JSON →
//!   base64url-decode `challenge`, `user.id`, `excludeCredentials[].id` →
//!   `navigator.credentials.create()` → credential JSON.
//! - [`get_credential`]: `PublicKeyCredentialRequestOptions` JSON →
//!   base64url-decode `challenge` (and `allowCredentials[].id`, which the
//!   username-less login leaves empty) → `navigator.credentials.get()` →
//!   assertion JSON.
//!
//! The conversion walks the parsed JSON with `js_sys::Reflect` and only
//! rewrites the known binary fields, so every other field (rp, timeouts,
//! extensions, ...) passes through untouched. The result JSON is built
//! field by field and stringified, matching what
//! `startRegistration()`/`startAuthentication()` of `@simplewebauthn/browser`
//! would POST. The unstable `PublicKeyCredential.toJSON()` /
//! `parseCreationOptionsFromJSON()` browser APIs (which would need
//! `--cfg=web_sys_unstable_apis`) are deliberately not used.
//!
//! The base64url codec itself is pure Rust and compiled in every build so
//! the host-target tests can exercise it; only the browser-interacting
//! functions are `csr`-only.

/// base64url (no padding) alphabet position of `byte`.
fn b64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

/// Decodes base64url (padding optional) into bytes. Trailing bits that do
/// not form a full byte are dropped (they are zero in every encoder that
/// follows RFC 4648).
#[cfg_attr(not(test), allow(dead_code))]
fn b64url_decode(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut acc = 0_u32;
    let mut bits = 0_u32;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = b64_value(byte)
            .ok_or_else(|| format!("invalid base64url character '{}'", char::from(byte)))?;
        acc = (acc << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            let byte = u8::try_from((acc >> bits) & 0xFF).map_err(|_| "base64url overflow")?;
            out.push(byte);
        }
    }
    Ok(out)
}

/// Encodes bytes as base64url without padding.
#[cfg_attr(not(test), allow(dead_code))]
fn b64url_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        // Indices are 6-bit slices of a 24-bit value, always < 64; the
        // `unwrap_or` keeps indexing panic-free by construction.
        let at = |shift: u32| usize::try_from((n >> shift) & 0x3F).unwrap_or(0);
        out.push(char::from(*ALPHABET.get(at(18)).unwrap_or(&b'A')));
        out.push(char::from(*ALPHABET.get(at(12)).unwrap_or(&b'A')));
        if chunk.len() > 1 {
            out.push(char::from(*ALPHABET.get(at(6)).unwrap_or(&b'A')));
        }
        if chunk.len() > 2 {
            out.push(char::from(*ALPHABET.get(at(0)).unwrap_or(&b'A')));
        }
    }
    out
}

#[cfg(feature = "csr")]
mod browser {
    use js_sys::{Array, ArrayBuffer, Object, Reflect, Uint8Array};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    use super::{b64url_decode, b64url_encode};

    /// Runs the registration ceremony against the browser: converts the
    /// server's `PublicKeyCredentialCreationOptions` JSON, calls
    /// `navigator.credentials.create()` and returns the credential JSON to
    /// POST back.
    pub async fn create_credential(options_json: &str) -> Result<String, String> {
        let parsed = parse_json(options_json)?;
        let public_key = get_field(&parsed, "publicKey")?;
        decode_binary_field(&public_key, "challenge")?;
        decode_binary_field(&get_field(&public_key, "user")?, "id")?;
        // Present when the user already has passkeys (add-another flow).
        if let Some(exclude) = get_optional(&public_key, "excludeCredentials")? {
            for descriptor in Array::from(&exclude).iter() {
                decode_binary_field(&descriptor, "id")?;
            }
        }
        let credential = call_credentials("create", &public_key).await?;
        let credential: web_sys::PublicKeyCredential = credential.unchecked_into();
        let response: web_sys::AuthenticatorAttestationResponse =
            get_field(&credential, "response")?.unchecked_into();

        let response_json = Object::new();
        set_buffer_field(
            &response_json,
            "attestationObject",
            &response.attestation_object(),
        )?;
        set_buffer_field(
            &response_json,
            "clientDataJSON",
            &response.client_data_json(),
        )?;
        let transports = response.get_transports();
        set_field(
            &response_json,
            "transports",
            if transports.length() == 0 {
                JsValue::NULL
            } else {
                transports.into()
            },
        )?;

        let result = Object::new();
        set_field(&result, "id", JsValue::from(credential.id()))?;
        set_buffer_field(&result, "rawId", &credential.raw_id())?;
        set_field(&result, "response", response_json)?;
        set_field(&result, "type", JsValue::from(credential.type_()))?;
        set_field(
            &result,
            "clientExtensionResults",
            credential.get_client_extension_results(),
        )?;
        stringify(&result)
    }

    /// Runs the authentication (login) ceremony against the browser:
    /// converts the server's `PublicKeyCredentialRequestOptions` JSON,
    /// calls `navigator.credentials.get()` and returns the assertion JSON
    /// to POST back.
    pub async fn get_credential(options_json: &str) -> Result<String, String> {
        let parsed = parse_json(options_json)?;
        let public_key = get_field(&parsed, "publicKey")?;
        decode_binary_field(&public_key, "challenge")?;
        // Empty for the username-less (discoverable) login, but convert
        // it anyway so a populated list would also work.
        if let Some(allow) = get_optional(&public_key, "allowCredentials")? {
            for descriptor in Array::from(&allow).iter() {
                decode_binary_field(&descriptor, "id")?;
            }
        }
        let credential = call_credentials("get", &public_key).await?;
        let credential: web_sys::PublicKeyCredential = credential.unchecked_into();
        let response: web_sys::AuthenticatorAssertionResponse =
            get_field(&credential, "response")?.unchecked_into();

        let response_json = Object::new();
        set_buffer_field(
            &response_json,
            "authenticatorData",
            &response.authenticator_data(),
        )?;
        set_buffer_field(
            &response_json,
            "clientDataJSON",
            &response.client_data_json(),
        )?;
        set_buffer_field(&response_json, "signature", &response.signature())?;
        set_field(
            &response_json,
            "userHandle",
            response.user_handle().map_or(JsValue::NULL, |handle| {
                JsValue::from(b64url_encode(&buffer_bytes(&handle)))
            }),
        )?;

        let result = Object::new();
        set_field(&result, "id", JsValue::from(credential.id()))?;
        set_buffer_field(&result, "rawId", &credential.raw_id())?;
        set_field(&result, "response", response_json)?;
        set_field(&result, "type", JsValue::from(credential.type_()))?;
        set_field(
            &result,
            "clientExtensionResults",
            credential.get_client_extension_results(),
        )?;
        stringify(&result)
    }

    /// Calls `navigator.credentials.create/get({publicKey})` and awaits the
    /// promise, mapping browser errors to friendly messages.
    async fn call_credentials(method: &str, public_key: &JsValue) -> Result<JsValue, String> {
        let window = web_sys::window().ok_or("no browser window")?;
        let credentials = window.navigator().credentials();
        let options = Object::new();
        set_field(&options, "publicKey", public_key.clone())?;
        let promise = if method == "create" {
            credentials.create_with_options(options.unchecked_ref())
        } else {
            credentials.get_with_options(options.unchecked_ref())
        }
        .map_err(|err| friendly_ceremony_error(&err))?;
        let credential = JsFuture::from(promise)
            .await
            .map_err(|err| friendly_ceremony_error(&err))?;
        if credential.is_null() || credential.is_undefined() {
            return Err("the browser returned no credential".to_owned());
        }
        Ok(credential)
    }

    /// Maps a `DOMException` from the ceremony to a user-facing message.
    fn friendly_ceremony_error(err: &JsValue) -> String {
        let name = Reflect::get(err, &JsValue::from("name"))
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        match name.as_str() {
            "NotAllowedError" => "the passkey ceremony was cancelled or timed out".to_owned(),
            "InvalidStateError" => "this passkey is already registered".to_owned(),
            "NotSupportedError" => "this browser does not support passkeys".to_owned(),
            "SecurityError" => "the passkey ceremony is not allowed on this origin".to_owned(),
            _ => {
                let message = Reflect::get(err, &JsValue::from("message"))
                    .ok()
                    .and_then(|value| value.as_string())
                    .unwrap_or_else(|| "unknown error".to_owned());
                format!("passkey ceremony failed: {message}")
            }
        }
    }

    /// `JSON.parse` with a string error.
    fn parse_json(text: &str) -> Result<JsValue, String> {
        js_sys::JSON::parse(text).map_err(|_| "the server sent invalid JSON".to_owned())
    }

    /// `JSON.stringify` with a string error.
    fn stringify(value: &JsValue) -> Result<String, String> {
        js_sys::JSON::stringify(value)
            .map_err(|_| "could not serialize the credential".to_owned())
            .and_then(|s| {
                s.as_string()
                    .ok_or_else(|| "could not serialize the credential".to_owned())
            })
    }

    /// Reads a required object field.
    fn get_field(object: &JsValue, field: &str) -> Result<JsValue, String> {
        let value = Reflect::get(object, &JsValue::from(field))
            .map_err(|_| format!("the server sent no readable `{field}` field"))?;
        if value.is_null() || value.is_undefined() {
            return Err(format!("the server sent no `{field}` field"));
        }
        Ok(value)
    }

    /// Reads a field that may be absent; `None` when missing or null.
    fn get_optional(object: &JsValue, field: &str) -> Result<Option<JsValue>, String> {
        let value = Reflect::get(object, &JsValue::from(field))
            .map_err(|_| format!("the server sent no readable `{field}` field"))?;
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }
        Ok(Some(value))
    }

    /// Writes a field, with a string error.
    fn set_field(object: &JsValue, field: &str, value: impl Into<JsValue>) -> Result<(), String> {
        Reflect::set(object, &JsValue::from(field), &value.into())
            .map(|_| ())
            .map_err(|_| format!("could not build the `{field}` field"))
    }

    /// Replaces a base64url string field with its decoded `Uint8Array`.
    fn decode_binary_field(object: &JsValue, field: &str) -> Result<(), String> {
        let encoded = get_field(object, field)?
            .as_string()
            .ok_or_else(|| format!("the server sent a non-string `{field}` field"))?;
        let bytes = b64url_decode(&encoded)?;
        let array = Uint8Array::new_with_length(u32::try_from(bytes.len()).unwrap_or(u32::MAX));
        array.copy_from(&bytes);
        set_field(object, field, array)
    }

    /// `ArrayBuffer` → `Vec<u8>`.
    fn buffer_bytes(buffer: &ArrayBuffer) -> Vec<u8> {
        let array = Uint8Array::new(buffer);
        let mut bytes = vec![0_u8; usize::try_from(array.length()).unwrap_or(0)];
        array.copy_to(&mut bytes);
        bytes
    }

    /// Writes an `ArrayBuffer` field as a base64url string.
    fn set_buffer_field(object: &JsValue, field: &str, buffer: &ArrayBuffer) -> Result<(), String> {
        set_field(
            object,
            field,
            JsValue::from(b64url_encode(&buffer_bytes(buffer))),
        )
    }
}

#[cfg(feature = "csr")]
pub use browser::{create_credential, get_credential};

#[cfg(test)]
mod tests {
    use super::{b64url_decode, b64url_encode};

    #[test]
    fn base64url_round_trip_without_padding() {
        for bytes in [
            b"f".as_slice(),
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            &[0, 159, 146, 150],
        ] {
            let encoded = b64url_encode(bytes);
            assert!(!encoded.contains('='), "padding leaked: {encoded}");
            assert_eq!(b64url_decode(&encoded).as_deref(), Ok(bytes));
        }
    }

    #[test]
    fn base64url_uses_url_safe_alphabet() {
        // 0xFB 0xFF 0xFE hits '+'/'/' in standard base64, '-'/'_' in b64url.
        assert_eq!(b64url_encode(&[0xFB, 0xFF, 0xFE]), "-__-");
    }

    #[test]
    fn base64url_decode_accepts_padding() {
        assert_eq!(b64url_decode("Zm9vYg==").as_deref(), Ok(b"foob".as_slice()));
        assert_eq!(b64url_decode("Zm8=").as_deref(), Ok(b"fo".as_slice()));
    }

    #[test]
    fn base64url_decode_rejects_bad_characters() {
        assert!(b64url_decode("not*base64").is_err());
    }
}
