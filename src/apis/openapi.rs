//! Serve the **vendored OpenAPI specs** over HTTP (docs/DESIGN.md §9).
//!
//! Each mounted CAMARA API's annotated spec (and the shared fragments it
//! `$ref`s) is served at a stable, canonical URL so an integrator can fetch the
//! exact contract CamaraSim implements — the same file that lives under
//! `specs/…`, which is kept in lock-step with the code (spec and code
//! must not drift). A published, machine-readable spec is what
//! makes the served surface self-describing (Swagger/Redoc/codegen can consume
//! it directly).
//!
//! ## URLs
//!
//! - `/{api}/v{n}/openapi.yaml` — one per mounted API/version, mirroring the
//!   API's own base path (e.g. `/number-verification/v1/openapi.yaml`).
//! - `/{api}/v{n}/docs` — the **human-readable docs** page for that version
//!   (DESIGN §9's third discovery endpoint), one per full spec above.
//! - `/auth/openapi.yaml` — the authored authorization-server spec.
//! - `/shared/errors.yaml` — the shared CAMARA error-model / scenario fragment.
//!
//! The last two are served because every API spec `$ref`s them with the
//! relative paths `../../auth/openapi.yaml` and `../../shared/errors.yaml`;
//! resolved against an API's `…/v{n}/openapi.yaml` URL those land exactly on the
//! two URLs above, so a client that follows the `$ref`s finds them and every
//! served spec is fully resolvable.
//!
//! ## Docs pages
//!
//! Each `…/docs` page is a tiny static HTML shell that renders the sibling
//! `openapi.yaml` with [Redoc](https://redocly.com/redoc) (loaded from its CDN
//! in the viewer's browser). The page itself is an in-memory `String` built at
//! startup — no filesystem read or network call on the *server's* request path
//! (non-blocking, DESIGN §11) — and adds no Rust dependency. A `<noscript>`
//! fallback links straight to the raw spec, so the machine-readable contract is
//! reachable even with JavaScript disabled.
//!
//! ## Simulator constraints
//!
//! - **Single binary, no runtime I/O.** Every spec is embedded at compile time
//!   with [`include_str!`], so serving one is an in-memory `&'static str` copy —
//!   no filesystem read on the request path (non-blocking, DESIGN §11) and the
//!   binary stays self-contained (no `specs/` dir needed at runtime).
//! - **No new dependency.** Pure `axum` routing + a static body.
//!
//! These are simulator meta-endpoints (they publish the contracts), not a CAMARA
//! business API, so there is no upstream CAMARA spec for them to vendor.

use axum::{http::header, routing::get, Router};

/// The media type OpenAPI documents are served with. OpenAPI 3.x itself is
/// content-type-agnostic; `application/yaml` is the widely-understood generic
/// YAML type (Swagger UI / Redoc / most codegen accept it).
const YAML_CONTENT_TYPE: &str = "application/yaml";

/// The media type the human-readable `…/docs` pages are served with.
const HTML_CONTENT_TYPE: &str = "text/html; charset=utf-8";

/// The shared `$ref` fragment specs every API spec references by relative path
/// (`../../auth/openapi.yaml`, `../../shared/errors.yaml`). Served alongside the
/// API specs so those `$ref`s resolve, but they are serving infrastructure — not
/// catalogued CAMARA business APIs — so [`api_spec_urls`] excludes them and the
/// `/` catalog does not list them.
///
/// Bodies are embedded from `specs/…` at compile time so the served copy can
/// never drift from the maintained spec file. The mounted **APIs** themselves
/// live in the shared [`crate::registry::APIS`] source of truth, from which both
/// this module's routes and the `/` catalog are derived.
const FRAGMENTS: &[(&str, &str)] = &[
    (
        "/auth/openapi.yaml",
        include_str!("../../specs/auth/openapi.yaml"),
    ),
    (
        "/shared/errors.yaml",
        include_str!("../../specs/shared/errors.yaml"),
    ),
];

/// The URL paths of every **API** OpenAPI spec served — one per entry in the
/// shared [`crate::registry::APIS`] source of truth (the `/auth` and `/shared`
/// `$ref` fragments are serving infrastructure, not catalogued business APIs, so
/// they are excluded).
///
/// Exposed so the `/` catalog can be checked against the specs actually served:
/// both are now derived from the same registry, and a test asserts the sets are
/// equal (docs/DESIGN.md §9 — every mounted API is catalogued and its `spec_url`
/// resolves). Test-support only — it has no role on the request path, so it is
/// compiled only under `cfg(test)`.
#[cfg(test)]
pub fn api_spec_urls() -> impl Iterator<Item = String> {
    crate::registry::APIS.iter().map(|api| api.spec_url())
}

/// Build the human-readable docs page for the spec served at `spec_url`.
///
/// A minimal HTML shell that renders the spec with Redoc; `title` names the
/// browser tab (the API's base path, e.g. `number-verification/v1`). See the
/// module docs for why this is a static string with a `<noscript>` fallback.
fn docs_page(title: &str, spec_url: &str) -> String {
    format!(
        "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\"/>\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>\n\
<title>{title} — CamaraSim API docs</title>\n\
<style>body {{ margin: 0; padding: 0; }}</style>\n\
</head>\n\
<body>\n\
<redoc spec-url=\"{spec_url}\"></redoc>\n\
<noscript>These docs render the OpenAPI spec at <a href=\"{spec_url}\">{spec_url}</a>.</noscript>\n\
<script src=\"https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js\"></script>\n\
</body>\n\
</html>\n"
    )
}

/// A `GET` route for every vendored spec (embedded YAML, `application/yaml`) and,
/// for every full spec, a sibling `…/docs` page (static HTML, `text/html`) —
/// DESIGN §9's `/{api}/v{n}/openapi.yaml` + `/{api}/v{n}/docs` discovery pair.
pub fn routes() -> Router {
    let mut router = Router::new();
    // Every mounted API's vendored spec comes from the shared registry; the two
    // shared `$ref` fragments (auth full spec + errors fragment) follow.
    let api_specs = crate::registry::APIS
        .iter()
        .map(|api| (api.spec_url(), api.body));
    let fragments = FRAGMENTS.iter().map(|&(path, body)| (path.to_string(), body));
    for (path, body) in api_specs.chain(fragments) {
        router = router.route(
            &path,
            get(move || async move { ([(header::CONTENT_TYPE, YAML_CONTENT_TYPE)], body) }),
        );
        // Every full API/auth spec (`…/openapi.yaml`) gets a docs page at its
        // `…/docs` sibling. The `/shared/errors.yaml` fragment is not a
        // standalone document, so it has no docs page.
        if let Some(base) = path.strip_suffix("/openapi.yaml") {
            let docs_path = format!("{base}/docs");
            let title = base.trim_start_matches('/').to_string();
            let page = docs_page(&title, &path);
            router = router.route(
                &docs_path,
                get(move || async move { ([(header::CONTENT_TYPE, HTML_CONTENT_TYPE)], page) }),
            );
        }
    }
    router
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    async fn fetch(path: &str) -> (StatusCode, String, String) {
        let response = routes()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, content_type, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn serves_an_api_spec_as_yaml() {
        let (status, content_type, body) = fetch("/number-verification/v1/openapi.yaml").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, YAML_CONTENT_TYPE);
        // The served body is byte-for-byte the vendored file.
        assert_eq!(body, include_str!("../../specs/number-verification/v1/openapi.yaml"));
        assert!(body.contains("openapi: 3.0.3"));
        assert!(body.contains("number-verification/v1"));
    }

    #[tokio::test]
    async fn serves_every_mounted_api_spec() {
        // Driven by [`api_spec_urls`] (the single source of truth) rather than a
        // duplicated path list, so a newly served API can never be silently
        // omitted from this check. Each API spec URL resolves and looks like an
        // OpenAPI doc.
        let mut count = 0;
        for path in api_spec_urls() {
            count += 1;
            let (status, content_type, body) = fetch(&path).await;
            assert_eq!(status, StatusCode::OK, "spec {path} should be served");
            assert_eq!(content_type, YAML_CONTENT_TYPE, "spec {path} content-type");
            assert!(body.starts_with("#") || body.contains("openapi:"), "spec {path} is YAML");
        }
        // Sanity: the source of truth is non-empty (guards a broken filter).
        assert!(count >= 28, "expected the full API-spec catalog, got {count}");
    }

    #[tokio::test]
    async fn serves_every_spec_body_verbatim() {
        // The **byte-equality complement** of `serves_every_mounted_api_spec`, which
        // proves each mounted spec URL resolves (200), is YAML content-typed, and
        // *looks* like an OpenAPI doc (`starts_with("#")` / `contains("openapi:")`) —
        // a loose heuristic a truncated, stale, or cross-wired body could still
        // satisfy. Only `serves_an_api_spec_as_yaml` pins a served body to its
        // vendored file byte-for-byte, and only for `number-verification`. Here every
        // mounted API's served body MUST equal its registered `ApiSpec.body` verbatim,
        // so the serving route can never hand a caller a spec differing — even by a
        // trailing byte — from the single source of truth the contract tests validate
        // and the sibling `/{api}/v{n}/docs` Redoc page renders against. Driven by the
        // registry (the single source of truth), so a newly mounted API is covered
        // automatically.
        let mut count = 0;
        for api in crate::registry::APIS {
            count += 1;
            let path = api.spec_url();
            let (status, content_type, body) = fetch(&path).await;
            assert_eq!(status, StatusCode::OK, "spec {path} should be served");
            assert_eq!(content_type, YAML_CONTENT_TYPE, "spec {path} content-type");
            assert_eq!(
                body, api.body,
                "served body for {path} must be its registered spec verbatim"
            );
        }
        // Non-vacuity: every registered API was exercised, and the catalog is full.
        assert_eq!(
            count,
            crate::registry::APIS.len(),
            "every registered API must be exercised"
        );
        assert!(count >= 28, "expected the full API-spec catalog, got {count}");
    }

    #[tokio::test]
    async fn serves_the_shared_ref_targets_so_specs_resolve() {
        // Every API spec `$ref`s these two by relative path; they must be served
        // at the resolved URLs or the served specs are not resolvable.
        let (status, ct, errors) = fetch("/shared/errors.yaml").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ct, YAML_CONTENT_TYPE);
        assert!(errors.contains("components:"));

        let (status, ct, auth) = fetch("/auth/openapi.yaml").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ct, YAML_CONTENT_TYPE);
        assert!(auth.contains("camaraOAuth"));
    }

    #[tokio::test]
    async fn unknown_spec_path_is_404() {
        let (status, _, _) = fetch("/no-such-api/v9/openapi.yaml").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn serves_a_docs_page_as_html() {
        let (status, content_type, body) = fetch("/number-verification/v1/docs").await;
        assert_eq!(status, StatusCode::OK);
        assert!(content_type.starts_with("text/html"), "docs content-type: {content_type}");
        // The page renders with Redoc and points it at the spec this same app
        // serves at the sibling URL.
        assert!(body.contains("<redoc"));
        assert!(body.contains("spec-url=\"/number-verification/v1/openapi.yaml\""));
        // The no-JS fallback still surfaces the raw spec URL.
        assert!(body.contains("<noscript>"));
        assert!(body.contains("href=\"/number-verification/v1/openapi.yaml\""));
    }

    #[tokio::test]
    async fn serves_docs_for_every_mounted_api() {
        // Driven by [`api_spec_urls`] (the single source of truth), so a newly
        // served API automatically gets its docs page checked here too — the
        // `…/docs` sibling of each `…/openapi.yaml` resolves as HTML and names
        // its own spec.
        let mut count = 0;
        for spec in api_spec_urls() {
            count += 1;
            let docs = format!("{}/docs", spec.strip_suffix("/openapi.yaml").unwrap());
            let (status, content_type, body) = fetch(&docs).await;
            assert_eq!(status, StatusCode::OK, "docs {docs} should be served");
            assert!(content_type.starts_with("text/html"), "docs {docs} content-type");
            assert!(body.contains(&spec), "docs {docs} should reference its spec {spec}");
        }
        assert!(count >= 28, "expected the full API docs catalog, got {count}");
    }

    #[tokio::test]
    async fn serves_a_docs_page_for_the_auth_spec_too() {
        // The auth spec is a full document, so it gets a docs page like the APIs;
        // the `/shared/errors.yaml` fragment does not (below).
        let (status, content_type, body) = fetch("/auth/docs").await;
        assert_eq!(status, StatusCode::OK);
        assert!(content_type.starts_with("text/html"));
        assert!(body.contains("spec-url=\"/auth/openapi.yaml\""));
    }

    #[tokio::test]
    async fn shared_fragment_has_no_docs_page() {
        // `/shared/errors.yaml` is a `$ref` target, not a standalone spec, so
        // there is no `/shared/docs`.
        let (status, _, _) = fetch("/shared/docs").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_docs_path_is_404() {
        let (status, _, _) = fetch("/no-such-api/v9/docs").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
