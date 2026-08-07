//! Traffic Influence **CloudEvents notifications** delivered to a subscription's
//! `sink` (CAMARA Traffic Influence, work-in-progress).
//!
//! When an API consumer creates a `TrafficInfluence` resource, the upstream
//! contract lets it embed a `subscriptionRequest` (a CAMARA event subscription)
//! so the operator POSTs a CloudEvent to the consumer-supplied `sink` when the
//! resource changes. The event body is a CloudEvents 1.0 envelope whose `data`
//! carries the `TrafficInfluence` resource (its `trafficInfluenceID`, `appId`,
//! `state`, and echoed placement). This module builds that event and delivers it.
//!
//! ## Scope this pass — the initial event only
//!
//! CamaraSim models the **initial event** (`config.initialEvent: true`): a single
//! `traffic-influence-change` CloudEvent fired as soon as a resource is created,
//! reflecting the created resource's current `state`. The ongoing state-change
//! stream, `subscriptionExpireTime` / `subscriptionMaxEvents` lifecycle, and the
//! `TrafficInfluenceNotification` extras (`selected_appInstanceId` /
//! `deviceResponse`) are documented cuts (no background provisioning worker;
//! DESIGN §7, §11), mirroring how QoD / QoS Provisioning first shipped one leg.
//!
//! ## Simulator constraints & documented cuts (shared with QoD / QoS Provisioning)
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
//!   loopback). The upstream schema mandates `https://`; CamaraSim additionally
//!   accepts `http://` for loopback receivers. Documented in the served spec.
//! - **`sinkCredential` (ACCESSTOKEN / PLAIN) auth.** When the `subscriptionRequest`
//!   carries a `sinkCredential`, its credential is applied to the notification's
//!   `Authorization` header ([`sink_authorization`]): `credentialType: ACCESSTOKEN`
//!   → `Bearer <accessToken>` (RFC 6750); `credentialType: PLAIN` →
//!   `Basic base64(identifier:secret)` (RFC 7617 HTTP Basic). The credential is kept
//!   **in memory only** and never echoed in the resource (it is a secret).
//!   `REFRESHTOKEN` is accepted for schema fidelity but not applied — it needs a
//!   token-exchange round trip, a documented cut.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// The CloudEvent `type` for a Traffic Influence change (CAMARA
/// traffic-influence). Also the single permitted `subscriptionRequest.types`
/// value (`SubscriptionEventType`).
pub const EVENT_TYPE: &str = "org.camaraproject.traffic-influence.v1.traffic-influence-change";

/// The CloudEvent `source` — a uri-reference identifying the simulator's Traffic
/// Influence provider context (CloudEvents requires `id` to be unique within
/// `source`).
pub const SOURCE: &str = "//camarasimulator/traffic-influence";

/// Build the `traffic-influence-change` CloudEvent (CloudEvents 1.0 envelope).
///
/// Pure: the caller supplies the unique `event_id` and the RFC 3339 `time`, so
/// this is deterministic and directly testable. The `data` is the created
/// `TrafficInfluence` resource verbatim (its `trafficInfluenceID` / `appId` /
/// `state` / echoed placement) — faithful to `TrafficInfluenceNotification`, which
/// inherits the resource fields (the `selected_appInstanceId` / `deviceResponse`
/// extras are a documented cut).
pub fn traffic_influence_change_event(event_id: String, time: String, resource: &Value) -> Value {
    json!({
        "id": event_id,
        "source": SOURCE,
        "type": EVENT_TYPE,
        "specversion": "1.0",
        "datacontenttype": "application/json",
        "time": time,
        "data": resource,
    })
}

/// Derive the `Authorization` header value from a CAMARA `SinkCredential`.
///
/// Two credential shapes are applied (mirrors QoD / Carrier Billing / Click to
/// Dial's `sink_authorization`):
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
/// is sent as the `Authorization` header (RFC 6750 bearer). A non-`http` sink is a
/// no-op success (see the module docs' documented cut). The response is not read —
/// delivery is best-effort.
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

    fn a_resource() -> Value {
        json!({
            "trafficInfluenceID": "11111111-2222-4333-8444-555555555555",
            "apiConsumerId": "consumer-42",
            "appId": "123e4567-e89b-12d3-a456-426614174002",
            "state": "active",
        })
    }

    #[test]
    fn event_has_the_camara_cloudevent_shape_carrying_the_resource() {
        let e = traffic_influence_change_event(
            "evt-1".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            &a_resource(),
        );
        assert_eq!(e["id"], "evt-1");
        assert_eq!(e["source"], SOURCE);
        assert_eq!(e["type"], EVENT_TYPE);
        assert_eq!(e["specversion"], "1.0");
        assert_eq!(e["datacontenttype"], "application/json");
        assert_eq!(e["time"], "2024-01-01T00:00:00Z");
        // data is the resource verbatim.
        assert_eq!(e["data"]["trafficInfluenceID"], "11111111-2222-4333-8444-555555555555");
        assert_eq!(e["data"]["appId"], "123e4567-e89b-12d3-a456-426614174002");
        assert_eq!(e["data"]["state"], "active");
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
        let event = traffic_influence_change_event(
            "evt-3".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            &a_resource(),
        );
        let sink = format!("http://{addr}/notify");

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
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");
        let parsed: Value = serde_json::from_str(body).expect("body is JSON");
        assert_eq!(parsed["type"], EVENT_TYPE);
        assert_eq!(parsed["data"]["state"], "active");
    }

    #[tokio::test]
    async fn deliver_sends_the_authorization_header_when_auth_is_present() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let event = traffic_influence_change_event(
            "evt-auth".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            &a_resource(),
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
    async fn deliver_to_a_non_http_sink_is_a_noop_success() {
        let event = traffic_influence_change_event(
            "evt-4".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            &a_resource(),
        );
        assert!(deliver("https://example.test/cb", &event, None).await.is_ok());
        assert!(deliver("not-a-url", &event, None).await.is_ok());
    }
}
