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
//! - **`sinkCredential` (ACCESSTOKEN / PLAIN) auth.** When a payment is created with
//!   a `sinkCredential`, its credential is applied to the notification's
//!   `Authorization` header ([`sink_authorization`]): a `credentialType: ACCESSTOKEN`
//!   → `Bearer <accessToken>` (RFC 6750), and a `credentialType: PLAIN` →
//!   `Basic <base64(identifier:secret)>` (RFC 7617 HTTP Basic — the standard
//!   application of a plain identifier/secret pair). The credential is used at
//!   delivery time and never persisted with the payment (it is a secret).
//!   `REFRESHTOKEN` is accepted for schema fidelity but not applied (it needs a
//!   token-exchange round trip) — a documented cut in the served spec.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// The CloudEvent `type` for a completed (charged) payment (CAMARA
/// carrier-billing v0).
pub const EVENT_TYPE_PAYMENT_COMPLETED: &str =
    "org.camaraproject.carrier-billing.v0.payment-completed";

/// The CloudEvent `type` for a **reserved** payment (CAMARA carrier-billing v0).
/// Fired by the two-step `preparePayment` when a reservation is created (the
/// amount is held but not yet charged, so — unlike `payment-completed` — the
/// event carries no `paymentDate`).
pub const EVENT_TYPE_PAYMENT_RESERVED: &str =
    "org.camaraproject.carrier-billing.v0.payment-reserved";

/// The CloudEvent `type` for a payment awaiting OTP validation (CAMARA
/// carrier-billing v0). Fired by the two-step `preparePayment` when a reservation
/// lands in `pending_validation` (the amount is neither charged nor reserved until
/// the OTP is validated), so — like `payment-reserved` — the event carries no
/// `paymentDate`.
pub const EVENT_TYPE_PAYMENT_PENDING_VALIDATION: &str =
    "org.camaraproject.carrier-billing.v0.payment-pending-validation";

/// The CloudEvent `type` for a **cancelled** (released) payment (CAMARA
/// carrier-billing v0). Fired by the two-step `cancelPayment` when a `reserved`
/// payment is released without ever being charged, so — like `payment-reserved`
/// — the event carries no `paymentDate`. The flow ends without a charge, so its
/// `data.status` is `failed` (not `succeeded`).
pub const EVENT_TYPE_PAYMENT_CANCELLED: &str =
    "org.camaraproject.carrier-billing.v0.payment-cancelled";

/// The CloudEvent `type` for a **denied** payment (CAMARA carrier-billing v0).
/// Fired by the two-step `validatePayment` when a `pending_validation`
/// reservation exhausts its OTP-attempt budget and is denied without ever being
/// charged, so — like `payment-cancelled` — the flow ends without a charge: the
/// event's `data.status` is `failed` and there is no `paymentDate`.
pub const EVENT_TYPE_PAYMENT_DENIED: &str =
    "org.camaraproject.carrier-billing.v0.payment-denied";

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

/// Build the `payment-reserved` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure and deterministic (the caller supplies `event_id` and the RFC 3339
/// `time`), mirroring [`payment_completed_event`]. The `data` payload is the
/// CAMARA `BasicEvent` for a reserved payment — `status` is always `succeeded`
/// here (the reservation was created), with the required `description`. A
/// reservation has charged nothing, so — unlike `payment-completed` — there is
/// no `paymentDate` field.
pub fn payment_reserved_event(
    event_id: String,
    time: String,
    payment_id: &str,
    description: &str,
) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE_PAYMENT_RESERVED,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "paymentId": payment_id,
            "status": "succeeded",
            "description": description,
        },
    })
}

/// Build the `payment-pending-validation` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure and deterministic (the caller supplies `event_id` and the RFC 3339
/// `time`), mirroring [`payment_reserved_event`]. The `data` payload is the CAMARA
/// `BasicEvent` for a payment awaiting validation — `status` is always `succeeded`
/// here (the reservation was accepted into `pending_validation`), with the
/// required `description`. Nothing is charged or reserved yet, so — like
/// `payment-reserved`, and unlike `payment-completed` — there is no `paymentDate`
/// field, and (per the CAMARA schema) no `validationInfo` in the notification (the
/// `authorizationId` is delivered only in the synchronous `preparePayment` body).
pub fn payment_pending_validation_event(
    event_id: String,
    time: String,
    payment_id: &str,
    description: &str,
) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE_PAYMENT_PENDING_VALIDATION,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "paymentId": payment_id,
            "status": "succeeded",
            "description": description,
        },
    })
}

/// Build the `payment-cancelled` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure and deterministic (the caller supplies `event_id` and the RFC 3339
/// `time`), mirroring [`payment_reserved_event`]. The `data` payload is the CAMARA
/// `BasicEvent` for a cancelled payment — a `cancelPayment` releases a `reserved`
/// payment without charging it, so the flow ended without a charge: `status` is
/// `failed` and (like `payment-reserved`) there is no `paymentDate`.
pub fn payment_cancelled_event(
    event_id: String,
    time: String,
    payment_id: &str,
    description: &str,
) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE_PAYMENT_CANCELLED,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "paymentId": payment_id,
            "status": "failed",
            "description": description,
        },
    })
}

/// Build the `payment-denied` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure and deterministic (the caller supplies `event_id` and the RFC 3339
/// `time`), mirroring [`payment_cancelled_event`]. The `data` payload is the
/// CAMARA `BasicEvent` for a denied payment — a `validatePayment` that exhausts
/// its OTP-attempt budget denies a `pending_validation` reservation without ever
/// charging it, so the flow ended without a charge: `status` is `failed` and
/// (like `payment-cancelled`) there is no `paymentDate`.
pub fn payment_denied_event(
    event_id: String,
    time: String,
    payment_id: &str,
    description: &str,
) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE_PAYMENT_DENIED,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": {
            "paymentId": payment_id,
            "status": "failed",
            "description": description,
        },
    })
}

/// Derive the `Authorization` header value from a CAMARA `SinkCredential`.
///
/// - `credentialType: ACCESSTOKEN` with a non-empty `accessToken` →
///   `Some("Bearer <token>")` (CAMARA's `accessTokenType` enum only permits `bearer`,
///   so RFC 6750 `Bearer` is always the scheme).
/// - `credentialType: PLAIN` with a non-empty `identifier` (and a `secret` field, whose
///   value may be empty — the schema requires the field, not a value) →
///   `Some("Basic <base64(identifier:secret)>")` (RFC 7617 HTTP Basic — the standard
///   application of a plain identifier/secret pair).
///
/// Every other shape — a missing/empty token, a `PLAIN` missing `identifier`/`secret`
/// or with an empty `identifier`, or a `REFRESHTOKEN` credential — returns `None`, so
/// the notification is sent unauthenticated (documented cut). Pure and directly
/// testable (mirrors `geofencing_subscriptions::notifications::sink_authorization`).
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
    fn reserved_event_has_the_camara_cloudevent_shape_without_a_payment_date() {
        let e = payment_reserved_event(
            "evt-r".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "The payment has been reserved successfully.",
        );
        assert_eq!(e["id"], "evt-r");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE_PAYMENT_RESERVED);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["paymentId"], "3fa85f64-5717-4562-b3fc-2c963f66afa6");
        assert_eq!(e["data"]["status"], "succeeded");
        assert_eq!(
            e["data"]["description"],
            "The payment has been reserved successfully."
        );
        // A reservation charges nothing → no paymentDate (unlike payment-completed).
        assert!(e["data"].get("paymentDate").is_none(), "no paymentDate: {e}");
    }

    #[test]
    fn pending_validation_event_has_the_camara_cloudevent_shape_without_a_payment_date() {
        let e = payment_pending_validation_event(
            "evt-pv".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "The payment is pending validation.",
        );
        assert_eq!(e["id"], "evt-pv");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE_PAYMENT_PENDING_VALIDATION);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["paymentId"], "3fa85f64-5717-4562-b3fc-2c963f66afa6");
        assert_eq!(e["data"]["status"], "succeeded");
        assert_eq!(e["data"]["description"], "The payment is pending validation.");
        // Nothing charged/reserved yet → no paymentDate; the notification also
        // carries no validationInfo (that is only in the synchronous body).
        assert!(e["data"].get("paymentDate").is_none(), "no paymentDate: {e}");
        assert!(e["data"].get("validationInfo").is_none(), "no validationInfo: {e}");
    }

    #[test]
    fn cancelled_event_has_the_camara_cloudevent_shape_with_a_failed_status_and_no_date() {
        let e = payment_cancelled_event(
            "evt-c".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "The payment has been cancelled.",
        );
        assert_eq!(e["id"], "evt-c");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE_PAYMENT_CANCELLED);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["paymentId"], "3fa85f64-5717-4562-b3fc-2c963f66afa6");
        // A cancellation ends the flow without a charge → status `failed`.
        assert_eq!(e["data"]["status"], "failed");
        assert_eq!(e["data"]["description"], "The payment has been cancelled.");
        // Nothing charged → no paymentDate (like payment-reserved).
        assert!(e["data"].get("paymentDate").is_none(), "no paymentDate: {e}");
    }

    #[test]
    fn denied_event_has_the_camara_cloudevent_shape_with_a_failed_status_and_no_date() {
        let e = payment_denied_event(
            "evt-d".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "The payment has been denied.",
        );
        assert_eq!(e["id"], "evt-d");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE_PAYMENT_DENIED);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        assert_eq!(e["data"]["paymentId"], "3fa85f64-5717-4562-b3fc-2c963f66afa6");
        // A denied validation ends the flow without a charge → status `failed`.
        assert_eq!(e["data"]["status"], "failed");
        assert_eq!(e["data"]["description"], "The payment has been denied.");
        // Nothing charged → no paymentDate (like payment-cancelled).
        assert!(e["data"].get("paymentDate").is_none(), "no paymentDate: {e}");
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
            sink_authorization(&json!({
                "credentialType": "PLAIN",
                "identifier": "",
                "secret": "p",
            })),
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
