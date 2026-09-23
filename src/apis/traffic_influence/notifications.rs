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
//! ## Simulator constraints & documented cuts (shared with QoD / Session Insights)
//!
//! - **Non-blocking, fire-and-forget.** Delivery is spawned onto the async runtime
//!   ([`spawn_delivery`]) so it never sits on the API request path — a slow or
//!   unreachable `sink` cannot delay the `201`. Best-effort: any transport error is
//!   dropped (CAMARA defines no retry contract the simulator must honour).
//! - **No new dependency.** The POST is written directly over the (for `https://`,
//!   TLS-wrapped) `tokio` stream ([`deliver`]) rather than pulling in an HTTP client,
//!   keeping the binary small (docs/DESIGN.md §11).
//! - **`http://` and `https://` sinks.** An `http://` sink is POSTed over a raw TCP
//!   stream; an `https://` sink is POSTed over a `rustls` TLS session (server
//!   certificate verified against the bundled Mozilla root store, `webpki-roots`;
//!   default port `443`). DESIGN §11 prefers `rustls` over OpenSSL to stay static and
//!   small; the `ClientConfig` is built once and cached ([`tls_connector`]). Any other
//!   scheme (or an unparseable sink) is a no-op success — not delivered to (a
//!   documented cut). Mirrors QoD / Session Insights / QoS Booking / QoS Provisioning
//!   / Geofencing / Click to Dial.
//! - **`sinkCredential` (ACCESSTOKEN / PLAIN) auth.** When the `subscriptionRequest`
//!   carries a `sinkCredential`, its credential is applied to the notification's
//!   `Authorization` header ([`sink_authorization`]): `credentialType: ACCESSTOKEN`
//!   → `Bearer <accessToken>` (RFC 6750); `credentialType: PLAIN` →
//!   `Basic base64(identifier:secret)` (RFC 7617 HTTP Basic). The credential is kept
//!   **in memory only** and never echoed in the resource (it is a secret).
//!   `REFRESHTOKEN` is accepted for schema fidelity but not applied — it needs a
//!   token-exchange round trip, a documented cut.

use std::sync::{Arc, OnceLock};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde_json::{json, Value};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

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
/// as the `Authorization` header. Any other scheme (or an unparseable sink) is a
/// no-op success (see the module docs' documented cut). The response is not read —
/// delivery is best-effort.
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
        // Explicit https port is honoured.
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
        let event = traffic_influence_change_event(
            "evt-w".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            &a_resource(),
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
        assert_eq!(parsed["data"]["state"], "active");

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
    async fn deliver_to_an_unsupported_scheme_is_a_noop_success() {
        // Only http:// and https:// are delivered to; any other scheme (or junk) is
        // parsed to None and silently not delivered — and no connection is attempted
        // — yet Ok (best-effort). `https://` is now delivered (see the TLS test).
        let event = traffic_influence_change_event(
            "evt-4".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            &a_resource(),
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

        let event = traffic_influence_change_event(
            "evt-tls".to_string(),
            "2024-01-01T00:00:00Z".to_string(),
            &a_resource(),
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
        assert_eq!(parsed["type"], EVENT_TYPE);
        assert_eq!(parsed["data"]["state"], "active");
    }
}
