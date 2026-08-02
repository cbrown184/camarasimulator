//! Shared reserved-identifier scenario convention (docs/DESIGN.md §7).
//!
//! **The input is the control plane.** Every identifier-keyed CAMARA endpoint in
//! the simulator lets the *caller* pick which functional case to exercise from
//! the identifier alone (a phone number, device id, …) — success variants *and*
//! every error case are reachable deterministically, with no hidden config.
//!
//! This module owns the one piece of that convention that is **shared across all
//! identifier-keyed APIs**: the reserved *error* suffixes. Defined once here so
//! the same suffix means the same thing everywhere, and documented in each
//! endpoint's OpenAPI so users discover cases from the spec alone.
//!
//! ## The convention
//!
//! Take the identifier's trailing **three digits** (ignoring any non-digit
//! formatting such as `+`, spaces, or dashes). If they equal a reserved HTTP
//! status from the CAMARA standard error set —
//!
//! `400`, `401`, `403`, `404`, `409`, `422`, `429`, `500`, `503`
//!
//! — the endpoint answers with that canonical CAMARA error (see
//! [`crate::errors`]) instead of a success. Every other identifier is a normal
//! ("happy path") input, handled by the API's own logic. Per-API *success*
//! variants (e.g. SIM Swap's "recently swapped" numbers) are chosen by each API
//! on top of this shared error floor.
//!
//! Example — a `phoneNumber` of `+123456789404` selects a `404 NOT_FOUND`;
//! `+123456789429` selects `429 TOO_MANY_REQUESTS`; `+123456789012` is a normal
//! input.
//!
//! No CAMARA business API consumes this yet (Phase 1), so it is
//! `dead_code`-allowed for now.
#![allow(dead_code)]

use crate::errors::CamaraError;

/// The reserved trailing-three-digit suffixes, each naming the HTTP status it
/// selects. This is exactly the standard CAMARA error set of
/// [`CamaraError::for_status`].
pub const RESERVED_ERROR_SUFFIXES: [u16; 9] =
    [400, 401, 403, 404, 409, 422, 429, 500, 503];

/// Inspect an identifier for a reserved error suffix.
///
/// Returns `Some(error)` when the identifier's trailing three digits name a
/// reserved CAMARA error status, or `None` for a normal input — in which case
/// the caller runs its own (happy-path or API-specific) logic.
pub fn reserved_error(identifier: &str) -> Option<CamaraError> {
    last_three_digits(identifier).and_then(CamaraError::for_status)
}

/// Whether an identifier selects a reserved error case.
pub fn is_reserved_error(identifier: &str) -> bool {
    reserved_error(identifier).is_some()
}

/// The trailing three ASCII digits of an identifier as a number (000..=999),
/// skipping any non-digit characters, or `None` if it has fewer than three
/// digits.
fn last_three_digits(identifier: &str) -> Option<u16> {
    let digits: Vec<u8> = identifier
        .bytes()
        .filter(u8::is_ascii_digit)
        .collect();
    if digits.len() < 3 {
        return None;
    }
    let tail = &digits[digits.len() - 3..];
    // Safe: `tail` is exactly three ASCII digits.
    std::str::from_utf8(tail).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reserved_suffix_maps_to_its_status() {
        for status in RESERVED_ERROR_SUFFIXES {
            let identifier = format!("+123456789{status:03}");
            let err = reserved_error(&identifier).expect("reserved suffix selects an error");
            assert_eq!(err.status.as_u16(), status, "identifier {identifier}");
        }
    }

    #[test]
    fn reserved_set_matches_the_error_catalog() {
        // The convention's set and the error catalog's set must not drift.
        for status in RESERVED_ERROR_SUFFIXES {
            assert!(CamaraError::for_status(status).is_some());
        }
    }

    #[test]
    fn normal_identifiers_are_happy_path() {
        for identifier in ["+123456789012", "+441234567890", "device-abc-200", "0000"] {
            assert!(
                reserved_error(identifier).is_none(),
                "{identifier} should be a normal input"
            );
        }
    }

    #[test]
    fn formatting_is_ignored_when_reading_the_suffix() {
        // Spaces / dashes / punctuation between the significant digits.
        let err = reserved_error("+1 (234) 567-8-404").expect("digits are 12345678404 → 404");
        assert_eq!(err.status.as_u16(), 404);
    }

    #[test]
    fn a_reserved_number_anywhere_but_the_tail_is_not_triggered() {
        // 404 appears mid-string; the trailing three digits are 012 → not reserved.
        assert!(reserved_error("+404123456012").is_none());
    }

    #[test]
    fn fewer_than_three_digits_is_happy_path() {
        for identifier in ["", "+", "ab", "4-4", "x9"] {
            assert!(reserved_error(identifier).is_none(), "{identifier}");
        }
    }

    #[test]
    fn is_reserved_error_agrees_with_reserved_error() {
        assert!(is_reserved_error("+123500"));
        assert!(!is_reserved_error("+123501"));
    }
}
