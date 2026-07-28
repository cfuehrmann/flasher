//! Prints the exact JSON shapes of the ceremony payloads (for docs).

use webauthn_authenticator_rs::WebauthnAuthenticator;
use webauthn_authenticator_rs::prelude::Url;
use webauthn_authenticator_rs::softpasskey::SoftPasskey;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auth = flasher_auth::Auth::new("localhost", "http://localhost:3000", "flasher")?;
    let handle = flasher_auth::Auth::user_handle_for(1);
    let origin = Url::parse("http://localhost:3000")?;
    let mut token = WebauthnAuthenticator::new(SoftPasskey::new(true));

    let (ccr, reg_ceremony) = auth.start_registration(handle, "kakimena", &[])?;
    println!("=== register/start response ===");
    println!("{}", serde_json::to_string_pretty(&ccr)?);

    // Downgrade residence for the soft token (cannot store resident keys).
    let mut downgraded = serde_json::to_value(&ccr)?;
    downgraded["publicKey"]["authenticatorSelection"]["residentKey"] =
        serde_json::Value::from("discouraged");
    downgraded["publicKey"]["authenticatorSelection"]["requireResidentKey"] =
        serde_json::Value::from(false);
    let reg = token.do_registration(origin.clone(), serde_json::from_value(downgraded)?)?;
    println!("=== register/finish request (as the browser produces it) ===");
    println!("{}", serde_json::to_string_pretty(&reg)?);
    let (passkey, _handle) = auth.finish_registration(&reg_ceremony, &reg)?;

    let (rcr, auth_ceremony) = auth.start_authentication()?;
    println!("=== login/start response ===");
    println!("{}", serde_json::to_string_pretty(&rcr)?);

    let mut hinted = serde_json::to_value(&rcr)?;
    hinted["publicKey"]["allowCredentials"] = serde_json::json!([{
        "type": "public-key",
        "id": "x",
    }]);
    // Recompute the real credential id hint for the soft token.
    let reg2 = serde_json::to_value(&reg)?;
    let cred_id = reg2["rawId"].clone();
    hinted["publicKey"]["allowCredentials"][0]["id"] = cred_id;
    let assertion = token.do_authentication(origin, serde_json::from_value(hinted)?)?;
    println!("=== login/finish request (as the browser produces it) ===");
    println!("{}", serde_json::to_string_pretty(&assertion)?);
    let _ = auth.finish_authentication(&auth_ceremony, &assertion, &passkey)?;
    let _ = passkey;
    Ok(())
}
