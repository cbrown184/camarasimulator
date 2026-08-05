//! QoS Provisioning **CloudEvents notifications** delivered to an assignment's
//! `sink` (CAMARA qos-provisioning 0.3.0, release r3.2).
//!
//! When a QoS assignment's status changes, CAMARA has the API *provider* POST a
//! CloudEvent to the consumer-supplied `sink` URL. The event body is a
//! CloudEvents 1.0 envelope whose `data` carries the `assignmentId`, the new
//! `status`, and — for an `UNAVAILABLE` transition — a `statusInfo` reason. This
//! module builds that event and delivers it. It mirrors
//! [`crate::apis::quality_on_demand::notifications`], the sibling API's version.
//!
//! ## Simulator constraints & documented cuts
//!
//! - **Non-blocking, fire-and-forget.** Delivery is spawned onto the async runtime
//!   ([`spawn_delivery`]) so it never sits on the API request path — a slow or
//!   unreachable `sink` cannot delay the `revokeQosAssignment` response. Delivery
//!   is best-effort: any transport error is dropped (CAMARA defines no retry
//!   contract the simulator must honour).
//! - **No new dependency.** The POST is written directly over a `tokio` TCP
//!   stream ([`deliver`]) rather than pulling in an HTTP client, keeping the binary
//!   small (docs/DESIGN.md §11).
//! - **`http://` sinks only.** A raw TCP POST cannot do TLS, and CamaraSim adds no
//!   TLS client, so an `https://` (or otherwise non-`http`) `sink` is parsed and
//!   **not delivered to** — a deliberate cut for the simulator (test receivers run
//!   on `http://` loopback), matching QoD / Geofencing / Carrier Billing.
//!   Documented in the served spec.
//! - **`sinkCredential` (ACCESSTOKEN) auth.** When an assignment is created with a
//!   `sinkCredential` of `credentialType: ACCESSTOKEN`, its bearer token is applied
//!   to the notification as an `Authorization: Bearer <token>` header
//!   ([`sink_authorization`]) — matching RFC 6750, the same scheme CAMARA uses on
//!   the resource server. The credential is kept **in memory only** (single node,
//!   DESIGN §4) and is never echoed back in an `AssignmentInfo` (it is a secret).
//!   The other `credentialType`s (`PLAIN`, `REFRESHTOKEN`) are accepted for schema
//!   fidelity but not applied — a documented cut in the served spec.

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// The CloudEvent `type` for a QoS-assignment status change (CAMARA
/// qos-provisioning v0).
pub const EVENT_TYPE: &str = "org.camaraproject.qos-provisioning.v0.status-changed";

/// The CloudEvent `source` — a uri-reference identifying the simulator's QoS
/// Provisioning provider context (CloudEvents requires `id` to be unique within
/// `source`).
pub const SOURCE: &str = "//camarasimulator/qos-provisioning";

/// Build the `status-changed` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure: the caller supplies the unique `event_id` ([`super::store::new_event_id`])
/// and the RFC 3339 `time`, so this is deterministic and directly testable. Per the
/// CAMARA schema `statusInfo` is only applicable to an `UNAVAILABLE` transition, so
/// it is omitted when `status_info` is `None`.
pub fn status_changed_event(
    event_id: String,
    time: String,
    assignment_id: &str,
    status: &str,
    status_info: Option<&str>,
) -> Value {
    let mut data = json!({
        "assignmentId": assignment_id,
        "status": status,
    });
    if let Some(info) = status_info {
        data["statusInfo"] = json!(info);
    }
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": data,
    })
}

/// Derive the `Authorization` header value from a CAMARA `SinkCredential`.
///
/// Returns `Some("Bearer <token>")` for a `credentialType: ACCESSTOKEN` credential
/// carrying a non-empty `accessToken` (CAMARA's `accessTokenType` enum only permits
/// `bearer`, so RFC 6750 `Bearer` is always the scheme). Every other shape — a
/// missing/empty token, or a `PLAIN`/`REFRESHTOKEN` credential — returns `None`, so
/// the notification is sent unauthenticated (documented cut). Pure and directly
/// testable (mirrors QoD's `sink_authorization`).
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

/// POST `event` to an `http://` `sink` as `application/cloudevents+json`.
///
/// Writes a minimal HTTP/1.1 request over a `tokio` TCP stream and returns once the
/// body is flushed (the stream is dropped on return, closing the connection so the
/// receiver sees EOF; `Connection: close` is advertised). When `auth` is `Some`, it
/// is sent as the `Authorization` header (RFC 6750 bearer, from the assignment's
/// `sinkCredential`). A non-`http` sink is a no-op success (see the module docs'
/// documented cut). The response is not read — delivery is best-effort.
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
    fn event_has_the_camara_cloudevent_shape() {
        let e = status_changed_event(
            "evt-1".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "11111111-2222-4333-8444-555555555555",
            "UNAVAILABLE",
            Some("DELETE_REQUESTED"),
        );
        assert_eq!(e["id"], "evt-1");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["assignmentId"], "11111111-2222-4333-8444-555555555555");
        assert_eq!(e["data"]["status"], "UNAVAILABLE");
        assert_eq!(e["data"]["statusInfo"], "DELETE_REQUESTED");
    }

    #[test]
    fn status_info_is_omitted_when_none() {
        let e = status_changed_event(
            "evt-2".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "aid",
            "AVAILABLE",
            None,
        );
        assert_eq!(e["data"]["status"], "AVAILABLE");
        assert!(e["data"].get("statusInfo").is_none());
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
        assert_eq!(
            parse_http_sink("http://example.test/cb"),
            Some(("example.test".to_string(), 80, "/cb".to_string()))
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
        let event = status_changed_event(
            "evt-3".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-assignment",
            "UNAVAILABLE",
            Some("DELETE_REQUESTED"),
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
        assert_eq!(parsed["data"]["assignmentId"], "the-assignment");
        assert_eq!(parsed["data"]["statusInfo"], "DELETE_REQUESTED");
    }

    #[tokio::test]
    async fn deliver_sends_the_authorization_header_when_auth_is_present() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let event = status_changed_event(
            "evt-auth".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-assignment",
            "UNAVAILABLE",
            Some("DELETE_REQUESTED"),
        );
        let sink = format!("http://{addr}/notify");

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
        assert_eq!(
            sink_authorization(&json!({
                "credentialType": "ACCESSTOKEN",
                "accessToken": "abc123",
                "accessTokenType": "bearer",
            })),
            Some("Bearer abc123".to_string())
        );
        assert_eq!(sink_authorization(&json!({ "credentialType": "ACCESSTOKEN" })), None);
        assert_eq!(
            sink_authorization(&json!({ "credentialType": "ACCESSTOKEN", "accessToken": "" })),
            None
        );
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
        // No TLS client, so an https sink is silently not delivered — and no
        // connection is attempted — yet Ok (documented cut).
        let event = status_changed_event(
            "evt-4".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "aid",
            "UNAVAILABLE",
            Some("DELETE_REQUESTED"),
        );
        assert!(deliver("https://example.test/cb", &event, None).await.is_ok());
        assert!(deliver("not-a-url", &event, None).await.is_ok());
    }
}
