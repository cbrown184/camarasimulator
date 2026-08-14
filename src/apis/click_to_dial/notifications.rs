//! Click to Dial **CloudEvents notifications** delivered to a call's `sink`
//! (CAMARA Click to Dial, `wip`).
//!
//! When an application creates a call (`createCall`) and supplies a `sink` URL,
//! the platform POSTs a `status-changed` CloudEvent to that sink each time the
//! call changes state (`initiating` → `callingCaller`/`callingCallee` →
//! `connected` → `disconnected`/`failed`). This module builds that event and
//! delivers it. It mirrors [`crate::apis::session_insights::notifications`] and
//! [`crate::apis::traffic_influence::notifications`], the sibling APIs' versions.
//!
//! ## Scope so far — the create-time and terminate events
//!
//! CamaraSim has no real call engine, so it models the two **request-triggered**
//! `status-changed` events (no background progression):
//!
//! - The **create-time** event: a single CloudEvent fired as soon as a call is
//!   created, reflecting the call's initial `status.state` (`initiating`, matching
//!   the `201` `Call` body) — see [`status_changed_event`].
//! - The **terminate** event: when a call created with a `sink` is terminated
//!   (`terminateCall`), a single CloudEvent reflecting the terminal `disconnected`
//!   state, carrying a `reason` — see [`terminated_event`]. This mirrors QoD's
//!   `deleteSession` → `DELETE_REQUESTED` event (request-triggered, not from a
//!   live engine).
//!
//! A **simulated successful progression** is also modelled: a `…001` `callee` line
//! drives an ordered `callingCallee` → `connected` sequence after the create-time
//! event, delivered off the request path by `super::vwip`'s `spawn_call_progression`
//! via the awaited [`send`] (so the two steps keep their order on the wire). The
//! remaining intermediate transitions (`callingCaller`, a spontaneous `failed`), the
//! `callDuration`, and the `recordingResult` remain documented cuts (no live call
//! engine; DESIGN §7, §11).
//!
//! ## Simulator constraints & documented cuts (shared with QoD / Session Insights)
//!
//! - **Non-blocking, fire-and-forget.** Delivery is spawned onto the async runtime
//!   ([`spawn_delivery`]) so it never sits on the API request path — a slow or
//!   unreachable `sink` cannot delay the `201`. Best-effort: any transport error is
//!   dropped (CAMARA defines no retry contract the simulator must honour).
//! - **No new dependency.** The POST is written directly over a `tokio` TCP stream
//!   ([`deliver`]) rather than pulling in an HTTP client, keeping the binary small
//!   (docs/DESIGN.md §11).
//! - **`http://` sinks only.** A raw TCP POST cannot do TLS and CamaraSim adds no
//!   TLS client, so an `https://` (or otherwise non-`http`) `sink` is parsed and
//!   **not delivered to** — a deliberate cut (test receivers run on `http://`
//!   loopback), matching QoD / Session Insights / Traffic Influence.
//! - **`sinkCredential` (ACCESSTOKEN / PLAIN) auth.** When the create carries a
//!   `sinkCredential`, its credential is applied to the `status-changed` callback's
//!   `Authorization` header ([`sink_authorization`]): a `credentialType: ACCESSTOKEN`
//!   → `Bearer <accessToken>` (RFC 6750), a `credentialType: PLAIN` →
//!   `Basic base64(identifier:secret)` (RFC 7617 HTTP Basic). The credential is used
//!   only to sign the callback and is never echoed in the returned `Call` (it is a
//!   secret). A `REFRESHTOKEN` credential (needs a token-exchange round trip) or none
//!   is a documented cut → the callback is sent unauthenticated.

use std::sync::atomic::{AtomicU64, Ordering};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// The CloudEvent `type` for a call status change (CAMARA click-to-dial v0,
/// canonical `org.camaraproject.<api>.v<n>.<event>` shape).
pub const EVENT_TYPE: &str = "org.camaraproject.click-to-dial.v0.status-changed";

/// The CloudEvent `source` — a uri-reference identifying the simulator's Click to
/// Dial provider context (CloudEvents requires `id` to be unique within `source`).
pub const SOURCE: &str = "//camarasimulator/click-to-dial";

/// Mint a unique, UUID-shaped CloudEvent `id` (unique within [`SOURCE`], as
/// CloudEvents 1.0 requires). Derived from a process-global monotonic counter
/// hashed to a v4-shaped UUID (SHA-256; no `uuid`/`rand` dependency, mirroring
/// `vwip::call_id`). Two events never share an id, so a delivered event is
/// individually addressable.
pub fn new_event_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(b"click-to-dial-event");
    hasher.update(n.to_le_bytes());
    let d = hasher.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Build the `status-changed` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure: the caller supplies the unique `event_id` ([`new_event_id`]) and the
/// RFC 3339 `time`, so this is deterministic and directly testable. `data` carries
/// the CAMARA `EventCallStatus` fields the simulator can populate at this point in
/// the call's life — the `callId`, the two participants, and the call's `status`
/// (an object `{ state }`, per the notification schema; note the resource's own
/// `status` is a plain string). The transition `reason`, `callDuration`, and
/// `recordingResult` are only meaningful once the call has progressed, a documented
/// cut for the create-time event.
pub fn status_changed_event(
    event_id: String,
    time: String,
    call_id: &str,
    caller: &str,
    callee: &str,
    state: &str,
) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "callId": call_id,
            "caller": { "number": caller },
            "callee": { "number": callee },
            "status": { "state": state },
        },
    })
}

/// Build the **terminal** `status-changed` CloudEvent for a call that was ended by
/// `terminateCall`. Same CloudEvents envelope as [`status_changed_event`], with the
/// terminal `state` (`disconnected`) and a `reason` added to `data.status` — the
/// `reason` is meaningful on a terminal state (unlike the create-time event, where
/// it is a documented cut). Pure and directly testable.
pub fn terminated_event(
    event_id: String,
    time: String,
    call_id: &str,
    caller: &str,
    callee: &str,
    state: &str,
    reason: &str,
) -> Value {
    let mut event = status_changed_event(event_id, time, call_id, caller, callee, state);
    event["data"]["status"]["reason"] = json!(reason);
    event
}

/// Derive the `Authorization` header value from a CAMARA `SinkCredential`.
///
/// Two credential shapes are applied (mirrors Session Insights / QoD /
/// Carrier Billing's `sink_authorization`):
///
/// - `credentialType: ACCESSTOKEN` with a non-empty `accessToken` →
///   `Some("Bearer <token>")` (CAMARA's `accessTokenType` enum only permits
///   `bearer`, so RFC 6750 `Bearer` is always the scheme).
/// - `credentialType: PLAIN` with a non-empty `identifier` and a present `secret` →
///   `Some("Basic <base64(identifier:secret)>")` (RFC 7617 HTTP Basic — the standard
///   application of a plain identifier/secret pair). An empty `secret` is permitted
///   (the schema requires the field, not a value); a missing `identifier`/`secret`
///   field, or an empty `identifier`, is unusable → `None`.
///
/// Every other shape — a missing/empty ACCESSTOKEN token, a `REFRESHTOKEN` (needs a
/// token-exchange round trip — a documented cut), or an unknown/absent
/// `credentialType` — returns `None`, so the notification is sent unauthenticated.
/// Pure and directly testable.
pub fn sink_authorization(cred: &Value) -> Option<String> {
    match cred.get("credentialType").and_then(Value::as_str)? {
        "ACCESSTOKEN" => {
            let token = cred.get("accessToken").and_then(Value::as_str)?;
            if token.is_empty() {
                return None;
            }
            Some(format!("Bearer {token}"))
        }
        "PLAIN" => {
            let identifier = cred.get("identifier").and_then(Value::as_str)?;
            let secret = cred.get("secret").and_then(Value::as_str)?;
            if identifier.is_empty() {
                return None;
            }
            let encoded = BASE64.encode(format!("{identifier}:{secret}"));
            Some(format!("Basic {encoded}"))
        }
        _ => None,
    }
}

/// Fire-and-forget delivery of `event` to `sink`: spawn [`deliver`] onto the
/// runtime and drop its result. Never blocks the caller (the API request path).
/// `auth`, when present, is sent as the `Authorization` header (RFC 6750 bearer,
/// derived from the call's `sinkCredential` by [`sink_authorization`]).
pub fn spawn_delivery(sink: String, event: Value, auth: Option<String>) {
    tokio::spawn(async move {
        let _ = deliver(&sink, &event, auth.as_deref()).await;
    });
}

/// Await a single best-effort delivery of `event` to `sink`, dropping any
/// transport error. Unlike [`spawn_delivery`] (which spawns an independent task),
/// this is awaited, so a caller delivering an **ordered sequence** of events from
/// one task keeps them in order on the wire — used by the simulated call
/// progression ([`super::vwip`]'s `spawn_call_progression`), which must send
/// `callingCallee` before `connected`.
pub async fn send(sink: &str, event: &Value, auth: Option<&str>) {
    let _ = deliver(sink, event, auth).await;
}

/// POST `event` to an `http://` `sink` as `application/cloudevents+json`.
///
/// Writes a minimal HTTP/1.1 request over a `tokio` TCP stream and returns once the
/// body is flushed (the stream is dropped on return, closing the connection so the
/// receiver sees EOF; `Connection: close` is advertised). When `auth` is `Some`, it
/// is sent as the `Authorization` header. A non-`http` sink is a no-op success (see
/// the module docs' documented cut). The response is not read — delivery is
/// best-effort.
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

    #[test]
    fn new_event_id_is_uuid_v4_shaped_and_unique() {
        let a = new_event_id();
        let b = new_event_id();
        assert_ne!(a, b, "the counter advances → distinct ids");
        for id in [&a, &b] {
            let parts: Vec<&str> = id.split('-').collect();
            assert_eq!(
                parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
                vec![8, 4, 4, 4, 12]
            );
            assert!(id.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-'));
            assert_eq!(parts[2].as_bytes()[0], b'4', "version 4");
            assert!(matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
        }
    }

    #[test]
    fn event_has_the_camara_cloudevent_shape() {
        let e = status_changed_event(
            "evt-1".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "11111111-2222-4333-8444-555555555555",
            "+123456789111",
            "+123456789012",
            "initiating",
        );
        assert_eq!(e["id"], "evt-1");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["callId"], "11111111-2222-4333-8444-555555555555");
        assert_eq!(e["data"]["caller"]["number"], "+123456789111");
        assert_eq!(e["data"]["callee"]["number"], "+123456789012");
        // The notification's `status` is an object `{ state }` (unlike the
        // resource's plain-string `status`).
        assert_eq!(e["data"]["status"]["state"], "initiating");
    }

    #[test]
    fn terminated_event_adds_the_disconnect_state_and_reason() {
        let e = terminated_event(
            "evt-term".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-call",
            "+123456789111",
            "+123456789012",
            "disconnected",
            "The call was terminated by the application.",
        );
        // Same envelope as the create-time event…
        assert_eq!(e["type"], EVENT_TYPE);
        assert_eq!(e["data"]["callId"], "the-call");
        assert_eq!(e["data"]["caller"]["number"], "+123456789111");
        assert_eq!(e["data"]["callee"]["number"], "+123456789012");
        // …but with the terminal state and a populated reason.
        assert_eq!(e["data"]["status"]["state"], "disconnected");
        assert_eq!(
            e["data"]["status"]["reason"],
            "The call was terminated by the application."
        );
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
        assert!(parse_http_sink("http://:80/x").is_none());
        assert!(parse_http_sink("http://host:notaport/x").is_none());
    }

    #[test]
    fn sink_authorization_derives_a_bearer_header_for_accesstoken() {
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
        assert_eq!(sink_authorization(&json!({ "credentialType": "ACCESSTOKEN" })), None);
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "ACCESSTOKEN", "accessToken": "" })),
            None
        );
    }

    #[test]
    fn sink_authorization_derives_a_basic_header_for_plain() {
        // PLAIN with identifier + secret → RFC 7617 Basic header.
        // base64("aladdin:opensesame") == "YWxhZGRpbjpvcGVuc2VzYW1l".
        assert_eq!(
            sink_authorization(&json!({
                "credentialType": "PLAIN",
                "identifier": "aladdin",
                "secret": "opensesame",
            })),
            Some("Basic YWxhZGRpbjpvcGVuc2VzYW1l".to_string())
        );
        // An empty secret is permitted (the schema requires the field, not a value):
        // base64("user:") == "dXNlcjo=".
        assert_eq!(
            sink_authorization(&json!({
                "credentialType": "PLAIN",
                "identifier": "user",
                "secret": "",
            })),
            Some("Basic dXNlcjo=".to_string())
        );
        // A missing identifier/secret field, or an empty identifier, is unusable → None.
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "PLAIN", "identifier": "user" })),
            None
        );
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "PLAIN", "secret": "p" })),
            None
        );
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "PLAIN", "identifier": "", "secret": "p" })),
            None
        );
    }

    #[test]
    fn sink_authorization_is_none_for_refreshtoken_and_junk() {
        // REFRESHTOKEN needs a token-exchange round trip → documented cut → None.
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "REFRESHTOKEN", "refreshToken": "r" })),
            None
        );
        // Unknown/missing credentialType → None.
        assert_eq!(sink_authorization(&json!({ "credentialType": "OTHER" })), None);
        assert_eq!(sink_authorization(&json!({})), None);
    }

    #[tokio::test]
    async fn deliver_posts_a_cloudevent_to_an_http_sink() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let event = status_changed_event(
            "evt-3".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-call",
            "+123456789111",
            "+123456789012",
            "initiating",
        );
        let sink = format!("http://{addr}/notify");

        // deliver() awaits the connection; run it concurrently with accept().
        let send = tokio::spawn(async move { deliver(&sink, &event, None).await });

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        send.await.unwrap().expect("delivery succeeds");

        let raw = String::from_utf8(buf).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(head.starts_with("POST /notify HTTP/1.1\r\n"), "request line: {head}");
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        assert!(head.contains(&format!("Host: {addr}")));
        // No sinkCredential → no Authorization header.
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");
        let parsed: Value = serde_json::from_str(body).expect("body is JSON");
        assert_eq!(parsed["type"], EVENT_TYPE);
        assert_eq!(parsed["data"]["callId"], "the-call");
        assert_eq!(parsed["data"]["status"]["state"], "initiating");
    }

    #[tokio::test]
    async fn deliver_authenticated_posts_the_bearer_header() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let event = status_changed_event(
            "evt-auth".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "cid",
            "+123456789111",
            "+123456789012",
            "initiating",
        );
        let sink = format!("http://{addr}/notify");
        let send =
            tokio::spawn(async move { deliver(&sink, &event, Some("Bearer sekret")).await });

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        send.await.unwrap().expect("delivery succeeds");

        let raw = String::from_utf8(buf).unwrap();
        assert!(
            raw.contains("Authorization: Bearer sekret\r\n"),
            "bearer header present: {raw}"
        );
    }

    #[tokio::test]
    async fn deliver_to_a_non_http_sink_is_a_noop_success() {
        // No TLS client, so an https sink is silently not delivered — and no
        // connection is attempted — yet Ok (documented cut).
        let event = status_changed_event(
            "evt-4".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "cid",
            "+123456789111",
            "+123456789012",
            "initiating",
        );
        assert!(deliver("https://example.test/cb", &event, None).await.is_ok());
        assert!(deliver("not-a-url", &event, None).await.is_ok());
    }
}
