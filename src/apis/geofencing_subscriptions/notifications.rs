//! Geofencing Subscriptions **CloudEvents notifications** delivered to a
//! subscription's `sink` (CAMARA geofencing-subscriptions 0.4.0, release r3.2).
//!
//! When a subscribed device enters or leaves the watched area, CAMARA has the
//! API *provider* POST a CloudEvent to the consumer-supplied `sink` URL. The
//! event body is a CloudEvents 1.0 envelope whose `type` is
//! `…area-entered` / `…area-left` and whose `data` carries the `subscriptionId`,
//! the `device`, and the `area`. This module builds that event and delivers it.
//!
//! ## What fires in the simulator (docs/DESIGN.md §7)
//!
//! There is no real device movement in a headless simulator, so CamaraSim
//! derives its geofencing events deterministically from the identifier:
//!
//! - The **initial event** (`config.initialEvent: true`): when a subscription is
//!   created and becomes `ACTIVE`, CAMARA reports the device's *current* position
//!   relative to the area. CamaraSim derives that from the identifier's trailing
//!   three digits — **even → inside** (`area-entered`), **odd → outside**
//!   (`area-left`). See [`initial_event_type`].
//! - A **movement event** (a simulated boundary crossing): two reserved
//!   identifier tails instruct the simulator to report a crossing shortly after
//!   creation — **`…001` → the device enters** (`area-entered`), **`…002` → it
//!   leaves** (`area-left`) — mirroring QoD's `…001` `NETWORK_TERMINATED`
//!   transition. See [`movement_event_type`].
//!
//! Both fire only for an `ACTIVE` subscription and only when the resulting event
//! type is among the subscription's `types` (a consumer only receives events it
//! subscribed to).
//!
//! ## Simulator constraints & documented cuts
//!
//! - **Non-blocking, fire-and-forget.** Delivery is spawned onto the async
//!   runtime ([`spawn_delivery`]) so it never sits on the API request path — a
//!   slow or unreachable `sink` cannot delay the `createSubscription` response.
//!   Delivery is best-effort; any transport error is dropped.
//! - **No new dependency.** The POST is written directly over a `tokio` TCP
//!   stream ([`deliver`]) rather than pulling in an HTTP client, keeping the
//!   binary small (docs/DESIGN.md §11). This mirrors
//!   [`crate::apis::quality_on_demand::notifications`].
//! - **`http://` sinks only.** A raw TCP POST cannot do TLS, and CamaraSim adds
//!   no TLS client, so an `https://` (or otherwise non-`http`) `sink` is parsed
//!   and **not delivered to** — a deliberate cut (test receivers run on `http://`
//!   loopback). Documented in the served spec.
//! - **`sinkCredential` (ACCESSTOKEN) auth.** When a subscription is created with a
//!   `sinkCredential` of `credentialType: ACCESSTOKEN`, its bearer token is applied
//!   to the initial-event callback as an `Authorization: Bearer <token>` header
//!   ([`sink_authorization`]) — matching RFC 6750, the same scheme CAMARA uses on
//!   the resource server, and mirroring
//!   [`crate::apis::quality_on_demand::notifications`]. The credential is kept **in
//!   memory only** (single node, DESIGN §4) and is never echoed back in a
//!   `SubscriptionInfo` (it is a secret). The other `credentialType`s (`PLAIN` HTTP
//!   Basic, `REFRESHTOKEN`) are accepted for schema fidelity but not applied — a
//!   documented cut in the served spec.

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// The CloudEvent `type` for a device entering the watched area.
pub const EVENT_TYPE_ENTERED: &str =
    "org.camaraproject.geofencing-subscriptions.v0.area-entered";
/// The CloudEvent `type` for a device leaving the watched area.
pub const EVENT_TYPE_LEFT: &str = "org.camaraproject.geofencing-subscriptions.v0.area-left";
/// The CloudEvent `type` reported when a subscription ends (CAMARA
/// event-subscription-template `subscription-ended`).
pub const EVENT_TYPE_SUBSCRIPTION_ENDED: &str =
    "org.camaraproject.geofencing-subscriptions.v0.subscription-ended";

/// The CloudEvent `source` — a uri-reference identifying the simulator's
/// geofencing provider context (CloudEvents requires `id` unique within `source`).
pub const SOURCE: &str = "//camarasimulator/geofencing-subscriptions";

/// Decide which initial CloudEvent (if any) `createSubscription` should deliver.
///
/// Returns the event `type` to send, or `None` when no initial event should
/// fire. An initial event fires only when **all** of these hold:
/// - the caller set `config.initialEvent: true`;
/// - the subscription became `ACTIVE` (an `ACTIVATION_REQUESTED` subscription is
///   not yet active, so it reports no state);
/// - the device's current position — `area-entered` if the identifier's trailing
///   three digits are even (inside), `area-left` if odd (outside) — corresponds
///   to an event type the subscription actually subscribed to (`types`).
///
/// Pure and directly testable.
pub fn initial_event_type(
    initial_event: Option<bool>,
    status: &str,
    digits: Option<u16>,
    types: &[String],
) -> Option<&'static str> {
    if initial_event != Some(true) || status != "ACTIVE" {
        return None;
    }
    // ACTIVE always carries a non-zero digit tail (…000 / no digits →
    // ACTIVATION_REQUESTED), but be defensive: no digits → no position to report.
    let d = digits?;
    let event_type = if d % 2 == 0 {
        EVENT_TYPE_ENTERED
    } else {
        EVENT_TYPE_LEFT
    };
    if types.iter().any(|t| t == event_type) {
        Some(event_type)
    } else {
        None
    }
}

/// Decide which movement CloudEvent (if any) a subscription should deliver as a
/// simulated boundary crossing shortly after creation.
///
/// A headless simulator has no real device movement, so — mirroring QoD's `…001`
/// `NETWORK_TERMINATED` transition (docs/DESIGN.md §7) — CamaraSim treats two
/// reserved identifier tails as an instruction to simulate a crossing:
/// - **`…001`** → the device *enters* the area → `area-entered`;
/// - **`…002`** → the device *leaves* the area → `area-left`.
///
/// Returns the event `type` to send, or `None` when no movement should fire. A
/// movement event fires only when **all** of these hold:
/// - the subscription became `ACTIVE` (an `ACTIVATION_REQUESTED` subscription is
///   not yet active, so it reports no transitions);
/// - the identifier's trailing three digits are exactly `001` (enter) or `002`
///   (leave) — any other tail simulates no movement;
/// - the crossing's event type is among the subscription's `types` (a consumer
///   receives only the events it subscribed to).
///
/// The movement marker is independent of the initial-event even/odd position: a
/// `…001` device (odd → currently outside) entering, and a `…002` device (even →
/// currently inside) leaving, are each a coherent crossing. Pure and directly
/// testable.
pub fn movement_event_type(
    status: &str,
    digits: Option<u16>,
    types: &[String],
) -> Option<&'static str> {
    if status != "ACTIVE" {
        return None;
    }
    let event_type = match digits? {
        1 => EVENT_TYPE_ENTERED,
        2 => EVENT_TYPE_LEFT,
        _ => return None,
    };
    if types.iter().any(|t| t == event_type) {
        Some(event_type)
    } else {
        None
    }
}

/// Build a geofencing `area-entered` / `area-left` CloudEvent (CloudEvents 1.0
/// envelope).
///
/// Pure: the caller supplies the unique `event_id` ([`super::store::new_event_id`])
/// and the RFC 3339 `time`, so this is deterministic and directly testable. The
/// `device` is included in `data` only when the subscription echoed one (a
/// three-legged subscription carries no `device`).
pub fn geofencing_event(
    event_id: String,
    time: String,
    event_type: &str,
    subscription_id: &str,
    device: Option<&Value>,
    area: &Value,
) -> Value {
    let mut data = json!({
        "subscriptionId": subscription_id,
        "area": area,
    });
    if let Some(device) = device {
        data["device"] = device.clone();
    }
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": event_type,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": data,
    })
}

/// Build a `subscription-ended` CloudEvent (CloudEvents 1.0 envelope).
///
/// CAMARA's event-subscription-template ends a subscription with a
/// `subscription-ended` event whose `data` carries the `subscriptionId` and a
/// `terminationReason` (here `SUBSCRIPTION_EXPIRED`, when the subscription reaches
/// its `config.subscriptionExpireTime`). Pure: the caller supplies the unique
/// `event_id` ([`super::store::new_event_id`]) and the RFC 3339 `time`, so this is
/// deterministic and directly testable. There is no `area` in an ended event.
pub fn subscription_ended_event(
    event_id: String,
    time: String,
    subscription_id: &str,
    termination_reason: &str,
) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE_SUBSCRIPTION_ENDED,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "subscriptionId": subscription_id,
            "terminationReason": termination_reason,
        },
    })
}

/// Derive the `Authorization` header value from a CAMARA `SinkCredential`.
///
/// Returns `Some("Bearer <token>")` for a `credentialType: ACCESSTOKEN` credential
/// carrying a non-empty `accessToken` (CAMARA's `accessTokenType` enum only permits
/// `bearer`, so RFC 6750 `Bearer` is always the scheme). Every other shape — a
/// missing/empty token, or a `PLAIN`/`REFRESHTOKEN` credential — returns `None`, so
/// the notification is sent unauthenticated (documented cut). Mirrors QoD's
/// [`crate::apis::quality_on_demand::notifications::sink_authorization`]. Pure and
/// directly testable.
pub fn sink_authorization(cred: &Value) -> Option<String> {
    if cred.get("credentialType").and_then(Value::as_str) != Some("ACCESSTOKEN") {
        return None;
    }
    let token = cred.get("accessToken").and_then(Value::as_str)?;
    if token.is_empty() {
        return None;
    }
    Some(format!("Bearer {token}"))
}

/// Fire-and-forget delivery of `event` to `sink`: spawn [`deliver`] onto the
/// runtime and drop its result. Never blocks the caller (the API request path).
/// `auth`, when present, is sent as the `Authorization` header (see
/// [`sink_authorization`]).
pub fn spawn_delivery(sink: String, event: Value, auth: Option<String>) {
    tokio::spawn(async move {
        let _ = deliver(&sink, &event, auth.as_deref()).await;
    });
}

/// Fire-and-forget **in-order** delivery of several `events` to `sink`, each over
/// its own connection. Used when a domain event must be immediately followed by a
/// terminal `subscription-ended` event (e.g. `subscriptionMaxEvents` reached) so the
/// two arrive in a deterministic order — a single [`spawn_delivery`] per event races.
/// `auth`, when present, is applied to every request. Never blocks the caller (the
/// API request path); each transport error is dropped (best-effort).
pub fn spawn_delivery_seq(sink: String, events: Vec<Value>, auth: Option<String>) {
    tokio::spawn(async move {
        for event in &events {
            let _ = deliver(&sink, event, auth.as_deref()).await;
        }
    });
}

/// POST `event` to an `http://` `sink` as `application/cloudevents+json`.
///
/// Writes a minimal HTTP/1.1 request over a `tokio` TCP stream and returns once
/// the body is flushed (the stream is dropped on return, closing the connection
/// so the receiver sees EOF; `Connection: close` is advertised). When `auth` is
/// `Some`, it is sent as the `Authorization` header (RFC 6750 bearer, from the
/// subscription's `sinkCredential`). A non-`http` sink is a no-op success (see the
/// module docs' documented cut). The response is not read — delivery is best-effort.
async fn deliver(sink: &str, event: &Value, auth: Option<&str>) -> std::io::Result<()> {
    let Some((host, port, path)) = parse_http_sink(sink) else {
        return Ok(()); // non-http sink: not delivered (documented cut)
    };
    let body = serde_json::to_vec(event).unwrap_or_default();
    let host_header = if port == 80 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    let auth_header = match auth {
        Some(a) => format!("Authorization: {a}\r\n"),
        None => String::new(),
    };
    let head = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host_header}\r\n\
         {auth_header}\
         Content-Type: application/cloudevents+json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    let mut stream = TcpStream::connect((host.as_str(), port)).await?;
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.flush().await?;
    Ok(())
}

/// Parse an `http://host[:port][/path]` sink into `(host, port, path)`.
///
/// Returns `None` for a non-`http` scheme (e.g. `https://` — no TLS client, a
/// documented cut), an empty host, or an unparseable port. The default port is
/// `80`; the returned `path` includes any query string, defaulting to `/`.
fn parse_http_sink(sink: &str) -> Option<(String, u16, String)> {
    let rest = sink.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok()?),
        None => (authority, 80),
    };
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port, path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    fn types(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn initial_event_fires_only_when_true_active_and_subscribed() {
        let both = types(&[EVENT_TYPE_ENTERED, EVENT_TYPE_LEFT]);

        // initialEvent absent / false → nothing.
        assert_eq!(initial_event_type(None, "ACTIVE", Some(12), &both), None);
        assert_eq!(
            initial_event_type(Some(false), "ACTIVE", Some(12), &both),
            None
        );
        // ACTIVATION_REQUESTED (not yet active) → nothing, even with initialEvent.
        assert_eq!(
            initial_event_type(Some(true), "ACTIVATION_REQUESTED", Some(12), &both),
            None
        );
        // Even tail → inside → area-entered; odd tail → outside → area-left.
        assert_eq!(
            initial_event_type(Some(true), "ACTIVE", Some(12), &both),
            Some(EVENT_TYPE_ENTERED)
        );
        assert_eq!(
            initial_event_type(Some(true), "ACTIVE", Some(13), &both),
            Some(EVENT_TYPE_LEFT)
        );
        // No digits → no position to report.
        assert_eq!(initial_event_type(Some(true), "ACTIVE", None, &both), None);
    }

    #[test]
    fn initial_event_is_filtered_to_subscribed_types() {
        let entered_only = types(&[EVENT_TYPE_ENTERED]);
        let left_only = types(&[EVENT_TYPE_LEFT]);

        // Even tail wants area-entered: delivered iff subscribed.
        assert_eq!(
            initial_event_type(Some(true), "ACTIVE", Some(12), &entered_only),
            Some(EVENT_TYPE_ENTERED)
        );
        assert_eq!(
            initial_event_type(Some(true), "ACTIVE", Some(12), &left_only),
            None
        );
        // Odd tail wants area-left: delivered iff subscribed.
        assert_eq!(
            initial_event_type(Some(true), "ACTIVE", Some(13), &left_only),
            Some(EVENT_TYPE_LEFT)
        );
        assert_eq!(
            initial_event_type(Some(true), "ACTIVE", Some(13), &entered_only),
            None
        );
    }

    #[test]
    fn movement_fires_only_for_the_001_002_tails_when_active() {
        let both = types(&[EVENT_TYPE_ENTERED, EVENT_TYPE_LEFT]);

        // …001 → enter, …002 → leave (an ACTIVE subscription).
        assert_eq!(
            movement_event_type("ACTIVE", Some(1), &both),
            Some(EVENT_TYPE_ENTERED)
        );
        assert_eq!(
            movement_event_type("ACTIVE", Some(2), &both),
            Some(EVENT_TYPE_LEFT)
        );
        // Any other tail simulates no movement.
        assert_eq!(movement_event_type("ACTIVE", Some(12), &both), None);
        assert_eq!(movement_event_type("ACTIVE", Some(101), &both), None);
        assert_eq!(movement_event_type("ACTIVE", None, &both), None);
        // ACTIVATION_REQUESTED reports no transitions, even with a marker tail.
        assert_eq!(
            movement_event_type("ACTIVATION_REQUESTED", Some(1), &both),
            None
        );
    }

    #[test]
    fn movement_is_filtered_to_subscribed_types() {
        let entered_only = types(&[EVENT_TYPE_ENTERED]);
        let left_only = types(&[EVENT_TYPE_LEFT]);

        // …001 wants area-entered: delivered iff subscribed.
        assert_eq!(
            movement_event_type("ACTIVE", Some(1), &entered_only),
            Some(EVENT_TYPE_ENTERED)
        );
        assert_eq!(movement_event_type("ACTIVE", Some(1), &left_only), None);
        // …002 wants area-left: delivered iff subscribed.
        assert_eq!(
            movement_event_type("ACTIVE", Some(2), &left_only),
            Some(EVENT_TYPE_LEFT)
        );
        assert_eq!(movement_event_type("ACTIVE", Some(2), &entered_only), None);
    }

    #[test]
    fn event_has_the_camara_cloudevent_shape() {
        let area = json!({
            "areaType": "CIRCLE",
            "center": { "latitude": 51.5, "longitude": -0.12 },
            "radius": 5000,
        });
        let device = json!({ "phoneNumber": "+123456789012" });
        let e = geofencing_event(
            "evt-1".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            EVENT_TYPE_ENTERED,
            "11111111-2222-4333-8444-555555555555",
            Some(&device),
            &area,
        );
        assert_eq!(e["id"], "evt-1");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE_ENTERED);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(
            e["data"]["subscriptionId"],
            "11111111-2222-4333-8444-555555555555"
        );
        assert_eq!(e["data"]["device"]["phoneNumber"], "+123456789012");
        assert_eq!(e["data"]["area"]["radius"], 5000);
    }

    #[test]
    fn subscription_ended_event_has_the_camara_shape() {
        let e = subscription_ended_event(
            "evt-end".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "11111111-2222-4333-8444-555555555555",
            "SUBSCRIPTION_EXPIRED",
        );
        assert_eq!(e["id"], "evt-end");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE_SUBSCRIPTION_ENDED);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(
            e["data"]["subscriptionId"],
            "11111111-2222-4333-8444-555555555555"
        );
        assert_eq!(e["data"]["terminationReason"], "SUBSCRIPTION_EXPIRED");
        // An ended event carries no area.
        assert!(e["data"].get("area").is_none());
    }

    #[test]
    fn subscription_ended_event_carries_the_max_events_reason() {
        // The same builder renders the MAX_EVENTS_REACHED termination (subscription
        // reached its subscriptionMaxEvents), differing only in terminationReason.
        let e = subscription_ended_event(
            "evt-max".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "11111111-2222-4333-8444-555555555555",
            "MAX_EVENTS_REACHED",
        );
        assert_eq!(e["type"], EVENT_TYPE_SUBSCRIPTION_ENDED);
        assert_eq!(e["data"]["terminationReason"], "MAX_EVENTS_REACHED");
        assert!(e["data"].get("area").is_none());
    }

    #[test]
    fn event_omits_device_when_none() {
        let area = json!({ "areaType": "CIRCLE" });
        let e = geofencing_event(
            "evt-2".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            EVENT_TYPE_LEFT,
            "sid",
            None,
            &area,
        );
        assert_eq!(e["type"], EVENT_TYPE_LEFT);
        assert!(e["data"].get("device").is_none());
    }

    #[test]
    fn parse_http_sink_handles_host_port_and_path_and_rejects_non_http() {
        assert_eq!(
            parse_http_sink("http://127.0.0.1:8080/notify?x=1"),
            Some(("127.0.0.1".to_string(), 8080, "/notify?x=1".to_string()))
        );
        assert_eq!(
            parse_http_sink("http://example.test"),
            Some(("example.test".to_string(), 80, "/".to_string()))
        );
        // https → not delivered (no TLS client); other junk → None.
        assert!(parse_http_sink("https://example.test/cb").is_none());
        assert!(parse_http_sink("ftp://example.test").is_none());
        assert!(parse_http_sink("http://").is_none());
        assert!(parse_http_sink("http://host:notaport/x").is_none());
    }

    #[tokio::test]
    async fn deliver_posts_a_cloudevent_to_an_http_sink() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let area = json!({ "areaType": "CIRCLE", "radius": 5000 });
        let event = geofencing_event(
            "evt-3".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            EVENT_TYPE_ENTERED,
            "the-subscription",
            None,
            &area,
        );
        let sink = format!("http://{addr}/geo-notify");

        // deliver() awaits the connection; run it concurrently with accept().
        let send = tokio::spawn(async move { deliver(&sink, &event, None).await });

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        send.await.unwrap().expect("delivery succeeds");

        let raw = String::from_utf8(buf).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /geo-notify HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        // No sinkCredential → no Authorization header.
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");
        let parsed: Value = serde_json::from_str(body).expect("body is JSON");
        assert_eq!(parsed["type"], EVENT_TYPE_ENTERED);
        assert_eq!(parsed["data"]["subscriptionId"], "the-subscription");
    }

    #[tokio::test]
    async fn deliver_sends_the_authorization_header_when_auth_is_present() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let area = json!({ "areaType": "CIRCLE", "radius": 5000 });
        let event = geofencing_event(
            "evt-auth".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            EVENT_TYPE_ENTERED,
            "the-subscription",
            None,
            &area,
        );
        let sink = format!("http://{addr}/geo-notify");

        let send =
            tokio::spawn(async move { deliver(&sink, &event, Some("Bearer sekret")).await });

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        send.await.unwrap().expect("delivery succeeds");

        let raw = String::from_utf8(buf).unwrap();
        let (head, _) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.contains("Authorization: Bearer sekret\r\n"),
            "authorization header present: {head}"
        );
    }

    #[test]
    fn sink_authorization_derives_a_bearer_header_only_for_accesstoken() {
        // ACCESSTOKEN with a token → RFC 6750 Bearer header.
        assert_eq!(
            sink_authorization(&json!({
                "credentialType": "ACCESSTOKEN",
                "accessToken": "abc123",
                "accessTokenType": "bearer",
            })),
            Some("Bearer abc123".to_string())
        );
        // ACCESSTOKEN missing/empty token → None (nothing to send).
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "ACCESSTOKEN" })),
            None
        );
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "ACCESSTOKEN", "accessToken": "" })),
            None
        );
        // Other credential types are a documented cut → None.
        assert_eq!(
            sink_authorization(&json!({
                "credentialType": "PLAIN",
                "identifier": "u",
                "secret": "p",
            })),
            None
        );
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "REFRESHTOKEN", "refreshToken": "r" })),
            None
        );
        assert_eq!(sink_authorization(&json!({})), None);
    }

    #[tokio::test]
    async fn deliver_to_a_non_http_sink_is_a_noop_success() {
        let area = json!({ "areaType": "CIRCLE" });
        let event = geofencing_event(
            "evt-4".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            EVENT_TYPE_LEFT,
            "sid",
            None,
            &area,
        );
        assert!(deliver("https://example.test/cb", &event, None).await.is_ok());
        assert!(deliver("not-a-url", &event, None).await.is_ok());
    }
}
