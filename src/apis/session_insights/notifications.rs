//! Session Insights **CloudEvents notifications** delivered to a session's `sink`
//! (CAMARA SessionInsights, wip).
//!
//! When an application submits observed metrics for a session
//! (`sendSessionMetrics`), CAMARA acknowledges with `204` and delivers the
//! resulting network-quality score **later**, as a CloudEvent POSTed to the
//! consumer-supplied `sink` URL — not in the `204` response. This module builds
//! that `network-quality-score` event and delivers it. It mirrors
//! [`crate::apis::qos_provisioning::notifications`], the sibling API's version.
//!
//! ## Simulator model & documented cuts
//!
//! - **Synthetic, deterministic score.** CamaraSim does not measure a real
//!   network, so the delivered `qualityScore` (0–100) is derived deterministically
//!   from the submitted `MetricsPayload` ([`quality_score`]) — higher packet loss,
//!   delay, and jitter lower the score. The metrics payload is therefore a genuine
//!   control plane over the notified score (docs/DESIGN.md §7).
//! - **Non-blocking, fire-and-forget.** Delivery is spawned onto the async runtime
//!   ([`spawn_delivery`]) so it never sits on the API request path — a slow or
//!   unreachable `sink` cannot delay the `sendSessionMetrics` response. Delivery is
//!   best-effort: any transport error is dropped (CAMARA defines no retry contract
//!   the simulator must honour).
//! - **No new dependency.** The POST is written directly over a `tokio` TCP stream
//!   ([`deliver`]) rather than pulling in an HTTP client, keeping the binary small
//!   (docs/DESIGN.md §11).
//! - **`http://` sinks only.** A raw TCP POST cannot do TLS, and CamaraSim adds no
//!   TLS client, so an `https://` (or otherwise non-`http`) `sink` is parsed and
//!   **not delivered to** — a deliberate cut for the simulator (test receivers run
//!   on `http://` loopback), matching QoD / QoS Provisioning / Geofencing.
//! - **`sinkCredential` (ACCESSTOKEN) auth.** When a session is created with a
//!   `sinkCredential` of `credentialType: ACCESSTOKEN`, its bearer token is applied
//!   to the `network-quality-score` callback as an `Authorization: Bearer <token>`
//!   header ([`sink_authorization`] derives it). The secret is stashed in a
//!   `sessionId`-keyed credential side-store at creation (never in the returned
//!   `SessionInfo`, so `GET` never echoes it) and *peeked* on each delivery, since
//!   metrics may be submitted repeatedly. A `PLAIN`/`REFRESHTOKEN` credential (or
//!   none) is a documented cut → the callback is sent unauthenticated.

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// The CloudEvent `type` for a session network-quality score (CAMARA
/// session-insights v0, canonical `org.camaraproject.<api>.v<n>.<event>` shape).
pub const EVENT_TYPE: &str = "org.camaraproject.session-insights.v0.network-quality-score";

/// The CloudEvent `type` for a session ending (CAMARA session-insights v0,
/// canonical `org.camaraproject.<api>.v<n>.<event>` shape). It is the session's
/// **terminal** notification — CAMARA delivers it once, when the session ends for
/// any reason (deletion, expiry, or network-initiated termination), and no further
/// notifications follow.
pub const SESSION_ENDED_EVENT_TYPE: &str = "org.camaraproject.session-insights.v0.session-ended";

/// The CloudEvent `source` — a uri-reference identifying the simulator's Session
/// Insights provider context (CloudEvents requires `id` to be unique within
/// `source`).
pub const SOURCE: &str = "//camarasimulator/session-insights";

/// Derive the synthetic 0–100 network-quality score from a session's submitted
/// metrics.
///
/// Pure and deterministic so the notified score is a testable control plane
/// (docs/DESIGN.md §7). `loss_exponent` is the CAMARA `packetLossErrorRate`
/// (the exponent of a `10^-n` loss ratio, 1..=10 — a **higher** exponent means
/// **less** loss, so a better link); `delay_ms`/`jitter_ms` are the `packetDelay`
/// / `jitter` `value`s in milliseconds (`0` when the caller sent only a unit). The
/// score starts at 100 and subtracts a loss penalty (`(10 − exponent) × 8`, so a
/// pristine `exponent 10` costs nothing and a lossy `exponent 1` costs 72) plus a
/// delay and a jitter penalty (each `value / 20`), clamped to `0..=100`.
pub fn quality_score(loss_exponent: i64, delay_ms: i64, jitter_ms: i64) -> u8 {
    let loss_penalty = (10 - loss_exponent.clamp(0, 10)) * 8;
    let delay_penalty = delay_ms.max(0) / 20;
    let jitter_penalty = jitter_ms.max(0) / 20;
    (100 - loss_penalty - delay_penalty - jitter_penalty).clamp(0, 100) as u8
}

/// Build the `network-quality-score` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure: the caller supplies the unique `event_id`
/// ([`super::store::new_event_id`]) and the RFC 3339 `time`, so this is
/// deterministic and directly testable. `data` carries the `sessionId` and the
/// derived `qualityScore` (0–100).
pub fn quality_score_event(event_id: String, time: String, session_id: &str, score: u8) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "sessionId": session_id,
            "qualityScore": score,
        },
    })
}

/// Build the `session-ended` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure: the caller supplies the unique `event_id` ([`super::store::new_event_id`])
/// and the RFC 3339 `time`, so this is deterministic and directly testable. `data`
/// is the CAMARA `SessionEndedData` — the `sessionId` and the `terminationReason`,
/// one of `NETWORK_TERMINATED` / `SESSION_EXPIRED` / `ACCESS_TOKEN_EXPIRED` /
/// `SESSION_DELETED` (CamaraSim fires `SESSION_DELETED` from `deleteSession`).
pub fn session_ended_event(
    event_id: String,
    time: String,
    session_id: &str,
    termination_reason: &str,
) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": SESSION_ENDED_EVENT_TYPE,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "sessionId": session_id,
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
/// the notification is sent unauthenticated (documented cut). Pure and directly
/// testable (mirrors QoD / QoS Provisioning's `sink_authorization`).
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
/// `auth`, when present, is sent as the `Authorization` header (RFC 6750 bearer,
/// derived from the session's `sinkCredential` by [`sink_authorization`]).
pub fn spawn_delivery(sink: String, event: Value, auth: Option<String>) {
    tokio::spawn(async move {
        let _ = deliver(&sink, &event, auth.as_deref()).await;
    });
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
    fn quality_score_is_driven_by_the_metrics() {
        // A pristine link (no loss, no delay, no jitter) scores 100.
        assert_eq!(quality_score(10, 0, 0), 100);
        // A lossy link (exponent 1 → 72 penalty) drops sharply.
        assert_eq!(quality_score(1, 0, 0), 28);
        // Delay/jitter each cost value/20.
        assert_eq!(quality_score(10, 200, 100), 100 - 10 - 5);
        // Everything bad clamps at 0 (never negative).
        assert_eq!(quality_score(1, 500, 500), 0);
        // A missing (unit-only) figure contributes 0.
        assert_eq!(quality_score(10, 0, 40), 100 - 2);
    }

    #[test]
    fn event_has_the_camara_cloudevent_shape() {
        let e = quality_score_event(
            "evt-1".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "11111111-2222-4333-8444-555555555555",
            87,
        );
        assert_eq!(e["id"], "evt-1");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["sessionId"], "11111111-2222-4333-8444-555555555555");
        assert_eq!(e["data"]["qualityScore"], 87);
    }

    #[test]
    fn session_ended_event_has_the_camara_cloudevent_shape() {
        let e = session_ended_event(
            "evt-end".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "11111111-2222-4333-8444-555555555555",
            "SESSION_DELETED",
        );
        assert_eq!(e["id"], "evt-end");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], SESSION_ENDED_EVENT_TYPE);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["sessionId"], "11111111-2222-4333-8444-555555555555");
        assert_eq!(e["data"]["terminationReason"], "SESSION_DELETED");
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

    #[tokio::test]
    async fn deliver_posts_a_cloudevent_to_an_http_sink() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let event = quality_score_event(
            "evt-3".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-session",
            64,
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
        // No sinkCredential → no Authorization header (auth deferred).
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");
        let parsed: Value = serde_json::from_str(body).expect("body is JSON");
        assert_eq!(parsed["type"], EVENT_TYPE);
        assert_eq!(parsed["data"]["sessionId"], "the-session");
        assert_eq!(parsed["data"]["qualityScore"], 64);
    }

    #[test]
    fn sink_authorization_derives_a_bearer_header_only_for_accesstoken() {
        assert_eq!(
            sink_authorization(&json!({
                "credentialType": "ACCESSTOKEN",
                "accessToken": "abc123",
                "accessTokenType": "bearer",
            })),
            Some("Bearer abc123".to_string())
        );
        // No token, empty token, or a non-ACCESSTOKEN credential → unauthenticated.
        assert_eq!(sink_authorization(&json!({ "credentialType": "ACCESSTOKEN" })), None);
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "ACCESSTOKEN", "accessToken": "" })),
            None
        );
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "PLAIN", "identifier": "u", "secret": "s" })),
            None
        );
    }

    #[tokio::test]
    async fn deliver_authenticated_posts_the_bearer_header() {
        use tokio::io::AsyncReadExt;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let event = quality_score_event(
            "evt-auth".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "sid",
            80,
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
        let event = quality_score_event(
            "evt-4".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "sid",
            50,
        );
        assert!(deliver("https://example.test/cb", &event, None).await.is_ok());
        assert!(deliver("not-a-url", &event, None).await.is_ok());
    }
}
