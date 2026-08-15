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
//! - **No new dependency.** The POST is written directly over the (for `https://`,
//!   TLS-wrapped) `tokio` stream ([`deliver`]) rather than pulling in an HTTP client,
//!   keeping the binary small (docs/DESIGN.md §11). This mirrors
//!   `quality_on_demand::notifications`.
//! - **`http://` and `https://` sinks.** An `http://` sink is POSTed over a raw TCP
//!   stream; an `https://` sink is POSTed over a `rustls` TLS session (server
//!   certificate verified against the bundled Mozilla root store, `webpki-roots`;
//!   default port `443`). DESIGN §11 prefers `rustls` over OpenSSL to stay static and
//!   small; the `ClientConfig` is built once and cached ([`tls_connector`]). Any other
//!   scheme (or an unparseable sink) is a no-op success — not delivered to (a
//!   documented cut). Mirrors QoD / Session Insights / QoS Provisioning / QoS Booking
//!   / Geofencing / Click to Dial / Traffic Influence.
//! - **`sinkCredential` (ACCESSTOKEN / PLAIN) auth.** When a payment is created with
//!   a `sinkCredential`, its credential is applied to the notification's
//!   `Authorization` header ([`sink_authorization`]): a `credentialType: ACCESSTOKEN`
//!   → `Bearer <accessToken>` (RFC 6750), and a `credentialType: PLAIN` →
//!   `Basic <base64(identifier:secret)>` (RFC 7617 HTTP Basic — the standard
//!   application of a plain identifier/secret pair). The credential is used at
//!   delivery time and never persisted with the payment (it is a secret).
//!   `REFRESHTOKEN` is accepted for schema fidelity but not applied (it needs a
//!   token-exchange round trip) — a documented cut in the served spec.

use std::sync::{Arc, OnceLock};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

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

/// A parsed delivery target: scheme (TLS or not), host, port, and request path.
#[derive(Debug, PartialEq)]
struct SinkTarget {
    /// `true` for an `https://` sink (deliver over TLS), `false` for `http://`.
    tls: bool,
    host: String,
    port: u16,
    /// The request path, including any query string (defaults to `/`).
    path: String,
}

impl SinkTarget {
    /// The `Host` header value: the bare host when the port is the scheme default
    /// (`80` for http, `443` for https), otherwise `host:port`.
    fn host_header(&self) -> String {
        let default = if self.tls { 443 } else { 80 };
        if self.port == default {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// POST `event` to an `http://` or `https://` `sink` as
/// `application/cloudevents+json`.
///
/// An `http://` sink is written over a raw `tokio` TCP stream; an `https://` sink is
/// written over a `rustls` TLS session (server cert verified against the bundled
/// Mozilla roots). In both cases a minimal HTTP/1.1 request is sent and the call
/// returns once the body is flushed (`Connection: close` is advertised; the stream
/// is dropped on return so the receiver sees EOF). When `auth` is `Some`, it is sent
/// as the `Authorization` header (from the payment's `sinkCredential`). Any other
/// scheme (or an unparseable sink) is a no-op success (see the module docs'
/// documented cut). The response is not read — delivery is best-effort.
async fn deliver(sink: &str, event: &Value, auth: Option<&str>) -> std::io::Result<()> {
    let Some(target) = parse_sink(sink) else {
        return Ok(()); // unsupported scheme: not delivered (documented cut)
    };
    if target.tls {
        deliver_tls(tls_connector(), &target, event, auth).await
    } else {
        let mut stream = TcpStream::connect((target.host.as_str(), target.port)).await?;
        write_request(&mut stream, &target.host_header(), &target.path, event, auth).await
    }
}

/// Deliver to an `https://` `target` over a TLS session established with
/// `connector`. Factored out (and taking the connector explicitly) so the test can
/// drive it with a connector trusting a throwaway self-signed cert while production
/// uses the cached Mozilla-roots connector ([`tls_connector`]).
async fn deliver_tls(
    connector: TlsConnector,
    target: &SinkTarget,
    event: &Value,
    auth: Option<&str>,
) -> std::io::Result<()> {
    let server_name = ServerName::try_from(target.host.clone())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let tcp = TcpStream::connect((target.host.as_str(), target.port)).await?;
    let mut stream = connector.connect(server_name, tcp).await?;
    write_request(&mut stream, &target.host_header(), &target.path, event, auth).await?;
    // Send `close_notify` so the peer sees a clean TLS shutdown before EOF.
    stream.shutdown().await.ok();
    Ok(())
}

/// Write the CloudEvent HTTP/1.1 POST for `event` to any async stream (a plain TCP
/// stream or a TLS session). When `auth` is `Some`, it is sent as the
/// `Authorization` header. Pure over the stream, so it is unit-tested against an
/// in-memory buffer.
async fn write_request<W: AsyncWrite + Unpin>(
    stream: &mut W,
    host_header: &str,
    path: &str,
    event: &Value,
    auth: Option<&str>,
) -> std::io::Result<()> {
    let body = serde_json::to_vec(event).unwrap_or_default();
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
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.flush().await?;
    Ok(())
}

/// The shared `rustls` client connector, built once from the bundled Mozilla root
/// store and cached (building a `ClientConfig` parses every trust anchor, so it is
/// not repeated per notification).
fn tls_connector() -> TlsConnector {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    let config = CONFIG.get_or_init(|| Arc::new(build_client_config(webpki_root_store())));
    TlsConnector::from(config.clone())
}

/// A [`RootCertStore`] holding the bundled Mozilla server-auth roots.
fn webpki_root_store() -> RootCertStore {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    roots
}

/// Build a `rustls` `ClientConfig` for the given trust anchors, pinned to the `ring`
/// crypto provider (the one feature-selected in `Cargo.toml`) and the default safe
/// protocol versions (TLS 1.2 + 1.3). No client authentication — CAMARA sinks
/// authenticate the *caller* via the `sinkCredential`, not mTLS.
fn build_client_config(roots: RootCertStore) -> ClientConfig {
    ClientConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring provider supports the default TLS protocol versions")
    .with_root_certificates(roots)
    .with_no_client_auth()
}

/// Parse an `http://` or `https://` `host[:port][/path]` sink into a [`SinkTarget`].
///
/// Returns `None` for any other scheme (a documented cut — not delivered to), an
/// empty host, or an unparseable port. The default port is `80` for `http` and `443`
/// for `https`; the returned `path` includes any query string, defaulting to `/`.
fn parse_sink(sink: &str) -> Option<SinkTarget> {
    let (tls, rest) = if let Some(rest) = sink.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = sink.strip_prefix("http://") {
        (false, rest)
    } else {
        return None;
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    let default_port = if tls { 443 } else { 80 };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok()?),
        None => (authority, default_port),
    };
    if host.is_empty() {
        return None;
    }
    Some(SinkTarget {
        tls,
        host: host.to_string(),
        port,
        path,
    })
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

    fn target(tls: bool, host: &str, port: u16, path: &str) -> SinkTarget {
        SinkTarget {
            tls,
            host: host.to_string(),
            port,
            path: path.to_string(),
        }
    }

    #[test]
    fn parse_sink_handles_http_host_port_and_path() {
        assert_eq!(
            parse_sink("http://127.0.0.1:8080/notify?x=1"),
            Some(target(false, "127.0.0.1", 8080, "/notify?x=1"))
        );
        // Default port (80) and default path (/).
        assert_eq!(
            parse_sink("http://example.test"),
            Some(target(false, "example.test", 80, "/"))
        );
        assert_eq!(
            parse_sink("http://example.test/cb"),
            Some(target(false, "example.test", 80, "/cb"))
        );
    }

    #[test]
    fn parse_sink_handles_https_and_defaults_to_port_443() {
        // https → TLS target, default port 443.
        assert_eq!(
            parse_sink("https://example.test/cb"),
            Some(target(true, "example.test", 443, "/cb"))
        );
        // An explicit https port is honoured.
        assert_eq!(
            parse_sink("https://example.test:8443/cb?y=2"),
            Some(target(true, "example.test", 8443, "/cb?y=2"))
        );
        // The Host header omits the default port but keeps a non-default one.
        assert_eq!(parse_sink("https://h/x").unwrap().host_header(), "h");
        assert_eq!(parse_sink("https://h:8443/x").unwrap().host_header(), "h:8443");
        assert_eq!(parse_sink("http://h:8080/x").unwrap().host_header(), "h:8080");
    }

    #[test]
    fn parse_sink_rejects_unsupported_schemes_and_malformed_authority() {
        assert!(parse_sink("ftp://example.test").is_none());
        assert!(parse_sink("not-a-url").is_none());
        assert!(parse_sink("http://").is_none());
        assert!(parse_sink("https://").is_none());
        assert!(parse_sink("http://:80/x").is_none());
        assert!(parse_sink("http://host:notaport/x").is_none());
    }

    #[tokio::test]
    async fn write_request_formats_the_post_with_and_without_auth() {
        let event = payment_completed_event(
            "evt-w".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-payment",
            "done",
            "2024-01-01T00:00:00Z",
        );
        let mut buf = Vec::new();
        write_request(&mut buf, "h:8443", "/cb", &event, Some("Basic abc"))
            .await
            .unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        assert!(head.starts_with("POST /cb HTTP/1.1\r\n"));
        assert!(head.contains("Host: h:8443\r\n"));
        assert!(head.contains("Authorization: Basic abc\r\n"));
        assert!(head.contains("Content-Type: application/cloudevents+json\r\n"));
        assert!(head.contains(&format!("Content-Length: {}\r\n", body.len())));
        assert!(head.ends_with("Connection: close"));
        let parsed: Value = serde_json::from_str(body).unwrap();
        assert_eq!(parsed["data"]["paymentId"], "the-payment");

        // Without auth, no Authorization header is written.
        let mut buf = Vec::new();
        write_request(&mut buf, "h", "/", &event, None).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        assert!(!raw.contains("Authorization:"));
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
    async fn deliver_to_an_unsupported_scheme_is_a_noop_success() {
        // Only http:// and https:// are delivered to; any other scheme (or junk) is
        // parsed to None and silently not delivered — and no connection is attempted
        // — yet Ok (best-effort). `https://` is now delivered (see the TLS test).
        let event = payment_completed_event(
            "evt-4".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "pid",
            "done",
            "2024-01-01T00:00:00Z",
        );
        assert!(deliver("ftp://example.test/cb", &event, None).await.is_ok());
        assert!(deliver("not-a-url", &event, None).await.is_ok());
    }

    #[tokio::test]
    async fn deliver_tls_posts_a_cloudevent_over_a_verified_tls_session() {
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
        use tokio_rustls::rustls::ServerConfig;
        use tokio_rustls::TlsAcceptor;

        // Throwaway self-signed cert with a `127.0.0.1` IP SAN, so the client can
        // both connect to and verify the loopback server with no DNS (rcgen is a
        // dev-dependency — never in the release binary).
        let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
        let cert_der = CertificateDer::from(cert.cert.der().to_vec());
        let key_der = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

        // TLS server presenting that cert (ring provider, matching the runtime).
        let server_config = ServerConfig::builder_with_provider(Arc::new(
            tokio_rustls::rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der.into())
        .unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(server_config));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        // Client connector that trusts only the throwaway cert (real verification —
        // a wrong/untrusted cert would fail the handshake).
        let mut roots = RootCertStore::empty();
        roots.add(cert_der).unwrap();
        let connector = TlsConnector::from(Arc::new(build_client_config(roots)));

        let event = payment_completed_event(
            "evt-tls".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            "the-payment",
            "done",
            "2024-01-01T00:00:00Z",
        );
        let target = target(true, "127.0.0.1", port, "/notify");

        let send = tokio::spawn(async move {
            deliver_tls(connector, &target, &event, Some("Bearer sekret")).await
        });

        let (tcp, _) = listener.accept().await.unwrap();
        let mut tls = acceptor.accept(tcp).await.expect("server-side handshake");
        let mut buf = Vec::new();
        tls.read_to_end(&mut buf).await.unwrap();
        send.await.unwrap().expect("tls delivery succeeds");

        let raw = String::from_utf8(buf).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(head.starts_with("POST /notify HTTP/1.1\r\n"), "request line: {head}");
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        assert!(head.contains(&format!("Host: 127.0.0.1:{port}")));
        assert!(head.contains("Authorization: Bearer sekret\r\n"));
        let parsed: Value = serde_json::from_str(body).expect("body is JSON");
        assert_eq!(parsed["type"], EVENT_TYPE_PAYMENT_COMPLETED);
        assert_eq!(parsed["data"]["paymentId"], "the-payment");
    }
}
