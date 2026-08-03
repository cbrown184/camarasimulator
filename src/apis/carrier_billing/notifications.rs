//! Carrier Billing **CloudEvents charging notifications** delivered to a
//! payment's `sink` (CAMARA Carrier Billing 0.5.0, release r3.2).
//!
//! When a payment reaches a terminal charging outcome, CAMARA has the API
//! *provider* POST a CloudEvent to the consumer-supplied `sink` URL (declared in
//! the `createPayment` / `preparePayment` request). This slice delivers the
//! **`payment-completed`** event: the body is a CloudEvents 1.0 envelope whose
//! `data` carries the `paymentId`, the `status` (`succeeded`), a human-readable
//! `description`, and the `paymentDate`. The one-step `createPayment` fires it on
//! a successful charge.
//!
//! ## Simulator constraints & documented cuts
//!
//! - **Non-blocking, fire-and-forget.** Delivery is spawned onto the async runtime
//!   ([`spawn_delivery`]) so it never sits on the API request path — a slow or
//!   unreachable `sink` cannot delay the `createPayment` response. Delivery is
//!   best-effort: any transport error is dropped (CAMARA does not define a retry
//!   contract the simulator must honour).
//! - **No new dependency.** The POST is written directly over a `tokio` TCP
//!   stream ([`deliver`]) rather than pulling in an HTTP client, keeping the binary
//!   small (docs/DESIGN.md §11). This mirrors `quality_on_demand::notifications`.
//! - **`http://` sinks only.** A raw TCP POST cannot do TLS, and CamaraSim adds no
//!   TLS client, so an `https://` (or otherwise non-`http`) `sink` is parsed and
//!   **not delivered to** — a deliberate cut for the simulator (test receivers run
//!   on `http://` loopback). Documented in the served spec.
//! - **`sinkCredential` (ACCESSTOKEN) auth.** When a payment is created with a
//!   `sinkCredential` of `credentialType: ACCESSTOKEN`, its bearer token is applied
//!   to the notification as an `Authorization: Bearer <token>` header
//!   ([`sink_authorization`]) — matching RFC 6750. The credential is used at
//!   delivery time and never persisted with the payment (it is a secret). The other
//!   `credentialType`s (`PLAIN` HTTP Basic, `REFRESHTOKEN`) are accepted for schema
//!   fidelity but not applied — a documented cut in the served spec.

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// The CloudEvent `type` for a completed (charged) payment (CAMARA
/// carrier-billing v0).
pub const EVENT_TYPE_PAYMENT_COMPLETED: &str =
    "org.camaraproject.carrier-billing.v0.payment-completed";

/// The CloudEvent `source` — a uri-reference identifying the simulator's Carrier
/// Billing provider context (CloudEvents requires `id` to be unique in `source`).
pub const SOURCE: &str = "//camarasimulator/carrier-billing";

/// Build the `payment-completed` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure: the caller supplies the unique `event_id` and the RFC 3339 `time`, so
/// this is deterministic and directly testable. The `data` payload is the CAMARA
/// `BasicEvent` for a completed payment — `status` is always `succeeded` here (a
/// completed one-step charge), with the required `description` and `paymentDate`.
pub fn payment_completed_event(
    event_id: String,
    time: String,
    payment_id: &str,
    description: &str,
    payment_date: &str,
) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE_PAYMENT_COMPLETED,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "paymentId": payment_id,
            "status": "succeeded",
            "description": description,
            "paymentDate": payment_date,
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
/// testable (mirrors `quality_on_demand::notifications::sink_authorization`).
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
/// is sent as the `Authorization` header (RFC 6750 bearer, from the payment's
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
        let e = payment_completed_event(
            "evt-1".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "The payment has been completed successfully.",
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(e["id"], "evt-1");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE_PAYMENT_COMPLETED);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["paymentId"], "3fa85f64-5717-4562-b3fc-2c963f66afa6");
        assert_eq!(e["data"]["status"], "succeeded");
        assert_eq!(
            e["data"]["description"],
            "The payment has been completed successfully."
        );
        assert_eq!(e["data"]["paymentDate"], "2024-01-01T00:00:00Z");
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
        let event = payment_completed_event(
            "evt-3".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-payment",
            "done",
            "2024-01-01T00:00:00Z",
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
        assert_eq!(parsed["type"], EVENT_TYPE_PAYMENT_COMPLETED);
        assert_eq!(parsed["data"]["paymentId"], "the-payment");
        assert_eq!(parsed["data"]["status"], "succeeded");
    }

    #[tokio::test]
    async fn deliver_sends_the_authorization_header_when_auth_is_present() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let event = payment_completed_event(
            "evt-auth".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-payment",
            "done",
            "2024-01-01T00:00:00Z",
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
        // No TLS client, so an https sink is silently not delivered — yet Ok.
        let event = payment_completed_event(
            "evt-4".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "pid",
            "done",
            "2024-01-01T00:00:00Z",
        );
        assert!(deliver("https://example.test/cb", &event, None).await.is_ok());
        assert!(deliver("not-a-url", &event, None).await.is_ok());
    }
}
