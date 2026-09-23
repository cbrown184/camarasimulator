//! CAMARA **QoS Booking** API.
//!
//! QoS Booking lets an application **reserve** a QoS profile for a device over a
//! bounded time window in a given service area, *ahead of time* — the network
//! schedules the quality for the requested `startTime`/`duration` rather than
//! applying it immediately. It is the *booking* sibling of Quality on Demand
//! (immediate, bounded QoS sessions) and QoS Provisioning (open-ended
//! provisioning): all three live in the CAMARA connectivity-quality-management
//! sub-project. Like them it is a **stateful, resource-oriented** API:
//! `POST /device-qos-bookings` creates a booking and returns a `bookingId`, and
//! later requests address that booking by id.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/qos-booking/vwip` (CAMARA qos-booking, `wip` — no
//!   released version yet, so mounted at its canonical `vwip` base path like the
//!   other work-in-progress APIs, DESIGN §9). First slice: `POST /device-qos-bookings`
//!   (`createBooking`).

pub mod notifications;
pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted major version of QoS Booking.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
