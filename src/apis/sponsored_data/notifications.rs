//! Sponsored Data **end-of-session webhook** delivered to a session's
//! consumer-supplied `webhookUrl` (CAMARA Sponsored Data, work-in-progress).
//!
//! When an API consumer starts a sponsorship session it supplies a `webhookUrl`
//! (and a `callbackToken`) so the operator can notify it of the session's status
//! changes. CamaraSim models the **end-of-session** notification on
//! `revokeSponsorship`: when a session is revoked, a single `SessionEndedNotification`
//! is POSTed to the recorded `webhookUrl`, carrying `endReason: "session_revoked"`.
//! This makes the `session_revoked` end reason — otherwise unreachable through a
//! status read, because revoke evicts the session — observable to the consumer.
//!
//! ## Scope this pass — the revoke callback only
//!
//! Only the revoke-time end-of-session webhook is modelled. Notifications for a
//! natural end (`validity_expired` / `data_exhausted`) would need a background
//! expiry worker (there is none — the live status is derived at read time), so
//! they remain a documented cut, mirroring how the other stateful APIs first
//! shipped one notification leg (docs/DESIGN.md §7, §11).
//!
//! ## Simulator constraints & documented cuts (shared with the CloudEvents legs)
//!
//! - **Non-blocking, fire-and-forget.** Delivery is spawned onto the async runtime
//!   ([`spawn_delivery`]) so it never sits on the API request path — a slow or
//!   unreachable `webhookUrl` cannot delay the `200`. Best-effort: any transport
//!   error is dropped (CAMARA defines no retry contract the simulator must honour).
//! - **No new dependency.** The POST is written directly over a `tokio` TCP stream
//!   ([`deliver`]) rather than pulling in an HTTP client, keeping the binary small
//!   (docs/DESIGN.md §11), mirroring every other CamaraSim notification leg.
//! - **`http://` sinks only.** A raw TCP POST cannot do TLS and CamaraSim adds no
//!   TLS client, so an `https://` (or otherwise non-`http`) `webhookUrl` is parsed
//!   and **not delivered to** — a deliberate cut (test receivers run on `http://`
//!   loopback). The upstream schema mandates `https://`; CamaraSim additionally
//!   accepts `http://` for loopback receivers. Documented in the served spec.
//! - **`callbackToken` auth.** The session's `callbackToken` (a v4 UUID) is applied
//!   to the webhook's `Authorization` header as `Bearer <callbackToken>` (RFC 6750),
//!   the standard way to present the "security token for authenticating webhook
//!   notifications". The token is held in memory only and never echoed in a
//!   response (it is a secret).

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Build the `SessionEndedNotification` payload delivered to the `webhookUrl`.
///
/// Pure over its inputs (the caller supplies the RFC 3339 `event_time`), so it is
/// deterministic and directly testable. The payload mirrors the session-status
/// view's identity + terminal fields: it reports the session `inactive` with the
/// supplied `end_reason` (`session_revoked` for a revoke).
pub fn session_ended_notification(
    sponsor_id: &str,
    campaign_id: &str,
    session_id: &str,
    phone_number: &str,
    end_reason: &str,
    event_time: String,
) -> Value {
    json!({
        "sponsorId": sponsor_id,
        "campaignId": campaign_id,
        "sessionId": session_id,
        "phoneNumber": phone_number,
        "sessionStatus": "inactive",
        "endReason": end_reason,
        "eventTime": event_time,
    })
}

/// Derive the webhook's `Authorization` header value from a session's
/// `callbackToken`: a non-empty token → `Some("Bearer <token>")` (RFC 6750). An
/// empty token → `None` (nothing to send). Pure and directly testable.
pub fn callback_authorization(callback_token: &str) -> Option<String> {
    if callback_token.is_empty() {
        return None;
    }
    Some(format!("Bearer {callback_token}"))
}

/// Fire-and-forget delivery of `body` to `webhook_url`: spawn [`deliver`] onto the
/// runtime and drop its result. Never blocks the caller (the API request path).
/// `auth`, when present, is sent as the `Authorization` header.
pub fn spawn_delivery(webhook_url: String, body: Value, auth: Option<String>) {
    tokio::spawn(async move {
        let _ = deliver(&webhook_url, &body, auth.as_deref()).await;
    });
}

/// POST `body` to an `http://` `webhook_url` as `application/json`.
///
/// Writes a minimal HTTP/1.1 request over a `tokio` TCP stream and returns once the
/// body is flushed (the stream is dropped on return, closing the connection so the
/// receiver sees EOF; `Connection: close` is advertised). When `auth` is `Some`, it
/// is sent as the `Authorization` header (RFC 6750 bearer). A non-`http` webhook is
/// a no-op success (see the module docs' documented cut). The response is not read —
/// delivery is best-effort.
async fn deliver(webhook_url: &str, body: &Value, auth: Option<&str>) -> std::io::Result<()> {
    let Some((host, port, path)) = parse_http_url(webhook_url) else {
        return Ok(()); // non-http webhook: not delivered (documented cut)
    };
    let payload = serde_json::to_vec(body).unwrap_or_default();
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
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        payload.len()
    );
    let mut stream = TcpStream::connect((host.as_str(), port)).await?;
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;
    Ok(())
}

/// Parse an `http://host[:port][/path]` URL into `(host, port, path)`.
///
/// Returns `None` for a non-`http` scheme (e.g. `https://` — no TLS client, a
/// documented cut), an empty host, or an unparseable port. The default port is
/// `80`; the returned `path` includes any query string, defaulting to `/`.
fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
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
    fn notification_carries_the_session_identity_and_end_reason() {
        let n = session_ended_notification(
            "acme@sponsor.example.com",
            "123e4567-e89b-12d3-a456-426614174000@sponsor.example.com",
            "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70",
            "+123456789012",
            "session_revoked",
            "2024-06-01T00:05:00Z".to_string(),
        );
        assert_eq!(n["sponsorId"], "acme@sponsor.example.com");
        assert_eq!(n["campaignId"], "123e4567-e89b-12d3-a456-426614174000@sponsor.example.com");
        assert_eq!(n["sessionId"], "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70");
        assert_eq!(n["phoneNumber"], "+123456789012");
        assert_eq!(n["sessionStatus"], "inactive");
        assert_eq!(n["endReason"], "session_revoked");
        assert_eq!(n["eventTime"], "2024-06-01T00:05:00Z");
    }

    #[test]
    fn callback_authorization_derives_a_bearer_header_or_none() {
        assert_eq!(
            callback_authorization("550e8400-e29b-41d4-a716-446655440000"),
            Some("Bearer 550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(callback_authorization(""), None);
    }

    #[test]
    fn parse_http_url_handles_host_port_and_path_and_rejects_non_http() {
        assert_eq!(
            parse_http_url("http://127.0.0.1:8080/notify?x=1"),
            Some(("127.0.0.1".to_string(), 8080, "/notify?x=1".to_string()))
        );
        assert_eq!(
            parse_http_url("http://example.test"),
            Some(("example.test".to_string(), 80, "/".to_string()))
        );
        // https → not delivered (no TLS client); other junk → None.
        assert!(parse_http_url("https://example.test/cb").is_none());
        assert!(parse_http_url("ftp://example.test").is_none());
        assert!(parse_http_url("http://").is_none());
        assert!(parse_http_url("http://:80/x").is_none());
        assert!(parse_http_url("http://host:notaport/x").is_none());
    }

    #[tokio::test]
    async fn deliver_posts_the_notification_with_the_bearer_callback_token() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let body = session_ended_notification(
            "acme@sponsor.example.com",
            "123e4567-e89b-12d3-a456-426614174000@sponsor.example.com",
            "sid-1",
            "+123456789012",
            "session_revoked",
            "2024-06-01T00:05:00Z".to_string(),
        );
        let url = format!("http://{addr}/webhook");

        let auth = callback_authorization("550e8400-e29b-41d4-a716-446655440000");
        let send = tokio::spawn(async move { deliver(&url, &body, auth.as_deref()).await });

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        send.await.unwrap().expect("delivery succeeds");

        let raw = String::from_utf8(buf).unwrap();
        let (head, payload) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(head.starts_with("POST /webhook HTTP/1.1\r\n"), "request line: {head}");
        assert!(head.contains("Content-Type: application/json"));
        assert!(head.contains(&format!("Host: {addr}")));
        assert!(
            head.contains("Authorization: Bearer 550e8400-e29b-41d4-a716-446655440000\r\n"),
            "bearer callbackToken present: {head}"
        );
        let parsed: Value = serde_json::from_str(payload).expect("body is JSON");
        assert_eq!(parsed["endReason"], "session_revoked");
        assert_eq!(parsed["sessionStatus"], "inactive");
        assert_eq!(parsed["sessionId"], "sid-1");
    }

    #[tokio::test]
    async fn deliver_to_a_non_http_webhook_is_a_noop_success() {
        let body = session_ended_notification(
            "acme@sponsor.example.com",
            "c@d.example.com",
            "sid-2",
            "+123456789012",
            "session_revoked",
            "2024-06-01T00:05:00Z".to_string(),
        );
        // https → no TLS client → not delivered, but a success (best-effort cut).
        assert!(deliver("https://sponsor.example.com/webhook", &body, None).await.is_ok());
        assert!(deliver("not-a-url", &body, None).await.is_ok());
    }
}
