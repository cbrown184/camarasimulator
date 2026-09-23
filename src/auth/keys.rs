//! Signing-key management for the auth surface.
//!
//! The simulator holds a single RSA signing key used to sign JWTs (RS256) and
//! publishes its public half as a JWK at `GET /oauth2/jwks`. Later passes reuse
//! [`signing_key`] to sign tokens at `/oauth2/token`.
//!
//! ## Why a bundled, fixed key
//!
//! CamaraSim is a test double, not a real IdP: the key is a **fixed, public,
//! simulator-only** PKCS#8 key bundled in the binary. This keeps the JWKS
//! deterministic across boots and processes (so integration tests and external
//! clients can pin it), and avoids paying RSA key-generation cost at startup.
//! It is emphatically **not secret** and must never be used to protect anything
//! real.
//!
//! The JWK is derived from the same parsed key at runtime, so the published
//! `jwks_uri` document can never drift from the key actually used to sign.

use std::sync::OnceLock;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{SignatureEncoding, Signer, Verifier};
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde_json::{json, Value};
use sha2::Sha256;

/// The bundled simulator signing key (PKCS#8 PEM). Fixed and public by design.
const SIGNING_KEY_PEM: &str = include_str!("assets/signing_key.pem");

/// Stable key id advertised in the JWK and (later) in signed-token headers.
///
/// A fixed `kid` is sufficient for a single-key simulator; clients select the
/// verification key by matching this value from the token header against the
/// JWKS.
pub const SIGNING_KID: &str = "camarasim-rs256-1";

/// The signing algorithm advertised for this key (RS256 per CAMARA, DESIGN §6).
pub const SIGNING_ALG: &str = "RS256";

/// The process-wide RSA signing key, parsed once from the bundled PEM.
///
/// Panics on first use only if the bundled PEM is malformed — a build-time
/// constant, so a failure here is a bug caught by the test suite, never a
/// request-path error.
pub fn signing_key() -> &'static RsaPrivateKey {
    static KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
    KEY.get_or_init(|| {
        RsaPrivateKey::from_pkcs8_pem(SIGNING_KEY_PEM).expect("bundled signing key is valid PKCS#8")
    })
}

/// Sign `message` with the bundled key using RSASSA-PKCS1-v1_5 over SHA-256 —
/// the JWS `RS256` algorithm (RFC 7518 §3.3). Returns the raw signature bytes,
/// which the token endpoint base64url-encodes as the JWT's third segment.
///
/// The `SigningKey` is built once (cloning the parsed private key) and cached,
/// so per-token signing does no key parsing. Signing a valid key never fails;
/// `Signer::sign` panics only on an internal RSA error, which for this fixed,
/// test-suite-checked key cannot occur on the request path.
pub fn sign_rs256(message: &[u8]) -> Vec<u8> {
    static SIGNER: OnceLock<SigningKey<Sha256>> = OnceLock::new();
    let signer = SIGNER.get_or_init(|| SigningKey::<Sha256>::new(signing_key().clone()));
    signer.sign(message).to_vec()
}

/// Verify an `RS256` signature (`SignatureEncoding` big-endian bytes) over
/// `message` using the bundled key's public half — the counterpart of
/// [`sign_rs256`], used by the token-verification middleware to authenticate
/// JWTs it issued. Returns `false` for a malformed signature or a mismatch;
/// never panics.
///
/// The `VerifyingKey` is derived once from the same parsed key that signs and
/// that the JWKS publishes, so verification can never drift from issuance.
// Consumed by the verify middleware (and its tests); not yet reached from a
// product route, so allow dead code until Phase 1 mounts a protected endpoint.
#[allow(dead_code)]
pub fn verify_rs256(message: &[u8], signature: &[u8]) -> bool {
    static VERIFIER: OnceLock<VerifyingKey<Sha256>> = OnceLock::new();
    let verifier =
        VERIFIER.get_or_init(|| VerifyingKey::<Sha256>::new(RsaPublicKey::from(signing_key())));
    match Signature::try_from(signature) {
        Ok(sig) => verifier.verify(message, &sig).is_ok(),
        Err(_) => false,
    }
}

/// base64url (no padding) of a big-endian byte slice, as required for JWK
/// `n`/`e` members (RFC 7517 / RFC 7518 §6.3).
fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The public JWK for the signing key: an RSA key marked for signature
/// verification (`use: "sig"`, `alg: "RS256"`) with base64url-encoded modulus
/// and exponent.
pub fn jwk() -> Value {
    let public = RsaPublicKey::from(signing_key());
    json!({
        "kty": "RSA",
        "use": "sig",
        "alg": SIGNING_ALG,
        "kid": SIGNING_KID,
        "n": b64url(&public.n().to_bytes_be()),
        "e": b64url(&public.e().to_bytes_be()),
    })
}

/// The JWK Set served at `/oauth2/jwks`: `{ "keys": [ <jwk> ] }`.
pub fn jwks() -> Value {
    json!({ "keys": [jwk()] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_key_parses() {
        // 2048-bit modulus => 256 bytes.
        let public = RsaPublicKey::from(signing_key());
        assert_eq!(public.n().to_bytes_be().len(), 256);
    }

    #[test]
    fn jwk_advertises_rsa_signing_metadata() {
        let k = jwk();
        assert_eq!(k["kty"], "RSA");
        assert_eq!(k["use"], "sig");
        assert_eq!(k["alg"], "RS256");
        assert_eq!(k["kid"], SIGNING_KID);
    }

    #[test]
    fn jwk_members_are_base64url_no_pad() {
        let k = jwk();
        for member in ["n", "e"] {
            let v = k[member].as_str().unwrap();
            assert!(!v.is_empty(), "{member} must be present");
            // base64url alphabet only, and no '+', '/', or '=' padding.
            assert!(
                v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "{member} must be base64url: {v}"
            );
            // Round-trips as base64url.
            URL_SAFE_NO_PAD.decode(v).expect("member decodes as base64url");
        }
    }

    #[test]
    fn jwk_matches_the_signing_key_modulus() {
        // The published modulus must equal the actual signing key's modulus,
        // so verification against the JWKS succeeds. This is the anti-drift
        // guarantee.
        let public = RsaPublicKey::from(signing_key());
        let expected_n = b64url(&public.n().to_bytes_be());
        let expected_e = b64url(&public.e().to_bytes_be());
        let k = jwk();
        assert_eq!(k["n"], expected_n);
        assert_eq!(k["e"], expected_e);
        // RSA F4 public exponent 65537 => bytes [0x01, 0x00, 0x01] => "AQAB".
        assert_eq!(k["e"], "AQAB");
    }

    #[test]
    fn sign_rs256_verifies_against_the_public_key() {
        // A signature from `sign_rs256` must verify under the published key, so
        // clients that fetch the JWKS can validate issued tokens.
        use rsa::pkcs1v15::{Signature, VerifyingKey};
        use rsa::signature::Verifier;

        let message = b"eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJjbGllbnQtMSJ9";
        let sig_bytes = sign_rs256(message);

        let verifying_key = VerifyingKey::<Sha256>::new(RsaPublicKey::from(signing_key()));
        let signature = Signature::try_from(sig_bytes.as_slice()).expect("signature bytes");
        verifying_key
            .verify(message, &signature)
            .expect("signature verifies under the JWKS public key");

        // A tampered message must fail verification.
        assert!(verifying_key
            .verify(b"tampered", &signature)
            .is_err());
    }

    #[test]
    fn verify_rs256_accepts_own_signature_and_rejects_tampering() {
        let message = b"header.payload";
        let sig = sign_rs256(message);
        assert!(verify_rs256(message, &sig), "own signature must verify");
        assert!(
            !verify_rs256(b"other.payload", &sig),
            "signature over a different message must be rejected"
        );
        assert!(
            !verify_rs256(message, b"not-a-signature"),
            "a malformed signature must be rejected, not panic"
        );
    }

    #[test]
    fn jwks_wraps_the_jwk_in_a_keys_array() {
        let set = jwks();
        let keys = set["keys"].as_array().unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0]["kid"], SIGNING_KID);
    }
}
