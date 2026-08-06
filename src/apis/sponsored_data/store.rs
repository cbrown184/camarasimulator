//! In-memory Sponsored Data **session store** (docs/DESIGN.md §4, §5 —
//! "in-memory stores", single node).
//!
//! Sponsored Data becomes **stateful** the moment a caller can read a session
//! back: `POST /sponsorship` starts a session and mints a `sessionId`, and
//! `GET /sponsorship/{sponsorId}/{campaignId}/{sessionId}/session-status`
//! (`getSessionStatus`) addresses that session by its id. This module is the
//! state that bridges those requests. It mirrors
//! [`crate::apis::carrier_billing::store`]: a process-global `HashMap` guarded by
//! a `std::sync::Mutex`, the lock held only for the map read/write and never
//! across an `.await`, so it never blocks the async runtime.
//!
//! Unlike Carrier Billing (which stores rendered payment JSON), the session
//! store keeps a small typed [`SponsorshipRecord`] — the granted window plus the
//! `sponsorId` / `campaignId` / `phoneNumber` needed to render a `session-status`
//! read. `getSessionStatus` derives the *live* status (active / inactive, data
//! consumed / available) from that record at read time, so the stored value is
//! the immutable grant, not a snapshot of a mutable status.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A started sponsorship session — the immutable grant, keyed in the store by its
/// minted `sessionId`. The live `session-status` view is derived from this at
/// read time (see `super::vwip::session_status_body`).
#[derive(Clone)]
pub struct SponsorshipRecord {
    /// The sponsoring company (`local@domain.tld`), as supplied at start.
    pub sponsor_id: String,
    /// The campaign (`UUID@domain.tld`), as supplied at start.
    pub campaign_id: String,
    /// The sponsored subscriber's E.164 phone number — also a control plane for
    /// the derived data-consumption figures (docs/DESIGN.md §7).
    pub phone_number: String,
    /// When the session was granted (Unix seconds, UTC).
    pub start_time: i64,
    /// When the session ends (`start_time + duration` minutes; Unix seconds, UTC).
    pub end_time: i64,
    /// The granted data volume (MB) — echoes the request's `dataVolume`, else the
    /// onboarding default.
    pub data_volume_mb: i64,
}

/// The process-global session store: `sessionId` → [`SponsorshipRecord`].
/// In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, SponsorshipRecord>> {
    static STORE: OnceLock<Mutex<HashMap<String, SponsorshipRecord>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `record` under `id`. Called by `startSponsorship` once the session is
/// granted, so it can be read back by `getSessionStatus`.
pub fn insert(id: String, record: SponsorshipRecord) {
    store()
        .lock()
        .expect("sponsored-data session store not poisoned")
        .insert(id, record);
}

/// Fetch the session stored under `id`, or `None` if no such session exists
/// (never created, or created in a different process). `getSessionStatus` uses
/// the distinction to answer `200` (found) vs `404 NOT_FOUND` (unknown id).
pub fn get(id: &str) -> Option<SponsorshipRecord> {
    store()
        .lock()
        .expect("sponsored-data session store not poisoned")
        .get(id)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> SponsorshipRecord {
        SponsorshipRecord {
            sponsor_id: "acme@sponsor.example.com".to_string(),
            campaign_id: "123e4567-e89b-12d3-a456-426614174000@sponsor.example.com".to_string(),
            phone_number: "+123456789012".to_string(),
            start_time: 1_717_200_000,
            end_time: 1_717_200_600,
            data_volume_mb: 50,
        }
    }

    #[test]
    fn stored_session_can_be_read_back_and_unknown_is_none() {
        // Uniquely-keyed so this shares the process-global store with nothing else.
        let id = "sd-store-unit-0001".to_string();
        assert!(get(&id).is_none(), "not stored yet → None");
        insert(id.clone(), record());
        let got = get(&id).expect("stored → Some");
        assert_eq!(got.sponsor_id, "acme@sponsor.example.com");
        assert_eq!(got.phone_number, "+123456789012");
        assert_eq!(got.data_volume_mb, 50);
        assert!(get("sd-store-unit-no-such").is_none());
    }
}
