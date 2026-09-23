//! CAMARA purpose-scope validation.
//!
//! CAMARA expresses the **purpose** a client requests access for as an OAuth2
//! scope, using the W3C Data Privacy Vocabulary (DPV): a purpose scope is
//! `dpv:<Purpose>#<action>`, e.g. `dpv:FraudPreventionAndDetection#check-sim-swap`
//! (docs/DESIGN.md §6, §7). The `dpv:` prefix marks the token as a DPV purpose,
//! `<Purpose>` is the DPV purpose value, and `<action>` is the technical scope the
//! purpose applies to.
//!
//! This module gates the *grammar* of requested purpose scopes at the point a
//! client asks for them — the token endpoint's `client_credentials` grant, the
//! `/oauth2/authorize` request, and `/bc-authorize`. A token whose requested
//! scope contains a malformed `dpv:` entry is rejected with the OAuth2
//! `invalid_scope` error (RFC 6749 §5.2 / §4.1.2.1) before any token or code is
//! minted, so a client discovers a bad purpose scope up front rather than at the
//! protected endpoint.
//!
//! ## What is validated
//!
//! Scope requests are space-delimited (RFC 6749 §3.3); each token is checked
//! independently and empty tokens (extra spaces) are ignored:
//!
//! - A token **not** starting with `dpv:` is a technical scope (`openid`,
//!   `sim-swap:check`, …) and passes untouched — the simulator does not maintain a
//!   fixed catalog of technical scopes, so it does not reject unknown ones.
//! - A token starting with `dpv:` **must** be a well-formed purpose scope:
//!   exactly one `#`, a non-empty alphanumeric DPV purpose before it, and a
//!   non-empty action after it (`[A-Za-z0-9._:-]`). Anything else — `dpv:`,
//!   `dpv:Purpose` (no action), `dpv:#action` (no purpose), `dpv:A#b#c` — is
//!   invalid.
//!
//! The check is intentionally structural, not a lookup against the full DPV
//! purpose vocabulary: the simulator accepts any syntactically valid purpose so
//! integrators can exercise their own purpose values deterministically (§7).

/// The DPV purpose-scope prefix. A scope token beginning with this must satisfy
/// the CAMARA purpose-scope grammar `dpv:<Purpose>#<action>`.
const DPV_PREFIX: &str = "dpv:";

/// Whether a single scope token is acceptable.
///
/// Non-`dpv:` tokens are always acceptable (see the module docs); a `dpv:` token
/// must be a well-formed `dpv:<Purpose>#<action>`.
fn is_valid_token(token: &str) -> bool {
    let body = match token.strip_prefix(DPV_PREFIX) {
        Some(body) => body,
        // Not a purpose scope: a technical scope the simulator does not gate.
        None => return true,
    };

    // Exactly one `#` separates the DPV purpose from the technical action.
    if body.bytes().filter(|&b| b == b'#').count() != 1 {
        return false;
    }
    let (purpose, action) = body.split_once('#').expect("exactly one '#' present");

    // Purpose: a non-empty DPV purpose value (PascalCase alphanumerics).
    let purpose_ok = !purpose.is_empty() && purpose.bytes().all(|b| b.is_ascii_alphanumeric());
    // Action: a non-empty technical scope (letters/digits and `-` `_` `.` `:`,
    // covering both `check-sim-swap` and `sim-swap:check` styles).
    let action_ok = !action.is_empty()
        && action
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'));

    purpose_ok && action_ok
}

/// Validate a space-delimited `scope` request.
///
/// Returns `Ok(())` when every token is acceptable, or `Err(bad)` naming the first
/// malformed `dpv:` token so the caller can surface it in the `invalid_scope`
/// error description. An empty (or whitespace-only) scope is valid — it requests
/// no scope.
pub fn validate_scope(scope: &str) -> Result<(), String> {
    for token in scope.split(' ').filter(|t| !t.is_empty()) {
        if !is_valid_token(token) {
            return Err(token.to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scope_is_valid() {
        assert!(validate_scope("").is_ok());
        assert!(validate_scope("   ").is_ok());
    }

    #[test]
    fn non_dpv_tokens_are_not_gated() {
        // The simulator keeps no technical-scope catalog, so any non-`dpv:` token
        // passes — including ones that would be odd as a real scope.
        assert!(validate_scope("openid").is_ok());
        assert!(validate_scope("sim-swap:check").is_ok());
        assert!(validate_scope("openid profile foo:bar").is_ok());
        assert!(validate_scope("weird#token").is_ok());
    }

    #[test]
    fn well_formed_purpose_scopes_pass() {
        assert!(validate_scope("dpv:FraudPreventionAndDetection#check-sim-swap").is_ok());
        assert!(validate_scope("dpv:IdentityVerification#kyc-match").is_ok());
        // Colon-style technical action.
        assert!(validate_scope("dpv:FraudPreventionAndDetection#sim-swap:check").is_ok());
        // Mixed with a technical scope.
        assert!(validate_scope("openid dpv:FraudPreventionAndDetection#check-sim-swap").is_ok());
        // Extra spaces between tokens are ignored.
        assert!(validate_scope("openid   dpv:Foo#bar").is_ok());
    }

    #[test]
    fn malformed_purpose_scopes_are_rejected() {
        // No action / no `#`.
        assert_eq!(validate_scope("dpv:FraudPreventionAndDetection"), Err("dpv:FraudPreventionAndDetection".into()));
        // Empty purpose.
        assert_eq!(validate_scope("dpv:#check-sim-swap"), Err("dpv:#check-sim-swap".into()));
        // Empty action.
        assert_eq!(validate_scope("dpv:Foo#"), Err("dpv:Foo#".into()));
        // Nothing after the prefix.
        assert_eq!(validate_scope("dpv:"), Err("dpv:".into()));
        // More than one `#`.
        assert_eq!(validate_scope("dpv:Foo#bar#baz"), Err("dpv:Foo#bar#baz".into()));
        // Non-alphanumeric purpose.
        assert_eq!(validate_scope("dpv:Fraud-Prevention#check"), Err("dpv:Fraud-Prevention#check".into()));
        // Illegal action character.
        assert_eq!(validate_scope("dpv:Foo#a/b"), Err("dpv:Foo#a/b".into()));
    }

    #[test]
    fn the_first_offending_token_is_reported() {
        // A valid token before the bad one does not mask it, and the returned
        // string is exactly the offending token (for the error description).
        assert_eq!(
            validate_scope("openid dpv:Bad dpv:Foo#ok"),
            Err("dpv:Bad".to_string())
        );
    }
}
