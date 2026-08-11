//! Single source of truth for every mounted CAMARA API's identity and its
//! vendored OpenAPI spec.
//!
//! Before this module there were **two** hand-maintained lists that had to be
//! kept in lock-step by hand for every new API: the `/` service catalog
//! (`main::catalog`) and the served-spec table (`apis::openapi`). A test asserted
//! they agreed, but adding an API still meant editing both in exactly the same
//! way. This module collapses them into one list — [`APIS`] — that the catalog
//! and the spec-serving routes are both derived from, so they cannot drift.
//!
//! Each spec body is embedded at compile time with [`include_str!`]: serving one
//! is an in-memory `&'static str` copy (no filesystem read on the request path —
//! non-blocking, DESIGN §11) and the binary stays self-contained (it needs no
//! `specs/` directory at runtime). The bodies are the exact bytes of the vendored
//! `specs/…` files the agent keeps in lock-step with the code, so the served copy
//! can never drift from the maintained file.

/// One mounted CAMARA API: its canonical name/version and its vendored,
/// compile-time-embedded OpenAPI spec.
pub struct ApiSpec {
    /// Canonical API name — the first URL path segment (e.g. `number-verification`).
    pub name: &'static str,
    /// Version segment mounted in the URL (e.g. `v1`, `v0.3`, `vwip`).
    pub version: &'static str,
    /// The embedded spec body — the exact bytes of the vendored `specs/…` file.
    pub body: &'static str,
}

impl ApiSpec {
    /// The API's base path, `/{name}/{version}` (e.g. `/number-verification/v1`).
    pub fn base_path(&self) -> String {
        format!("/{}/{}", self.name, self.version)
    }

    /// The URL its vendored spec is served at, `{base_path}/openapi.yaml`.
    pub fn spec_url(&self) -> String {
        format!("/{}/{}/openapi.yaml", self.name, self.version)
    }
}

/// Every mounted CAMARA API, in mount order. The single source of truth the `/`
/// catalog (`crate::catalog`) and the spec-serving routes (`crate::apis::openapi`)
/// are both derived from.
pub const APIS: &[ApiSpec] = &[
    ApiSpec {
        name: "number-verification",
        version: "v1",
        body: include_str!("../specs/number-verification/v1/openapi.yaml"),
    },
    ApiSpec {
        name: "sim-swap",
        version: "v2",
        body: include_str!("../specs/sim-swap/v2/openapi.yaml"),
    },
    ApiSpec {
        name: "kyc-match",
        version: "v0.3",
        body: include_str!("../specs/kyc-match/v0.3/openapi.yaml"),
    },
    ApiSpec {
        name: "device-reachability-status",
        version: "v1",
        body: include_str!("../specs/device-reachability-status/v1/openapi.yaml"),
    },
    ApiSpec {
        name: "device-roaming-status",
        version: "v1",
        body: include_str!("../specs/device-roaming-status/v1/openapi.yaml"),
    },
    ApiSpec {
        name: "device-identifier",
        version: "v0.3",
        body: include_str!("../specs/device-identifier/v0.3/openapi.yaml"),
    },
    ApiSpec {
        name: "one-time-password-sms",
        version: "v1",
        body: include_str!("../specs/one-time-password-sms/v1/openapi.yaml"),
    },
    ApiSpec {
        name: "quality-on-demand",
        version: "v1",
        body: include_str!("../specs/quality-on-demand/v1/openapi.yaml"),
    },
    ApiSpec {
        name: "location-verification",
        version: "v3",
        body: include_str!("../specs/location-verification/v3/openapi.yaml"),
    },
    ApiSpec {
        name: "location-retrieval",
        version: "v0.4",
        body: include_str!("../specs/location-retrieval/v0.4/openapi.yaml"),
    },
    ApiSpec {
        name: "geofencing-subscriptions",
        version: "v0.4",
        body: include_str!("../specs/geofencing-subscriptions/v0.4/openapi.yaml"),
    },
    ApiSpec {
        name: "carrier-billing",
        version: "v0.5",
        body: include_str!("../specs/carrier-billing/v0.5/openapi.yaml"),
    },
    ApiSpec {
        name: "call-forwarding-signal",
        version: "v0.4",
        body: include_str!("../specs/call-forwarding-signal/v0.4/openapi.yaml"),
    },
    ApiSpec {
        name: "number-recycling",
        version: "v0.2",
        body: include_str!("../specs/number-recycling/v0.2/openapi.yaml"),
    },
    ApiSpec {
        name: "kyc-age-verification",
        version: "v0.1",
        body: include_str!("../specs/kyc-age-verification/v0.1/openapi.yaml"),
    },
    ApiSpec {
        name: "device-swap",
        version: "v1",
        body: include_str!("../specs/device-swap/v1/openapi.yaml"),
    },
    ApiSpec {
        name: "kyc-fill-in",
        version: "v0.3",
        body: include_str!("../specs/kyc-fill-in/v0.3/openapi.yaml"),
    },
    ApiSpec {
        name: "home-devices-qod",
        version: "v0.4",
        body: include_str!("../specs/home-devices-qod/v0.4/openapi.yaml"),
    },
    ApiSpec {
        name: "qos-profiles",
        version: "v1",
        body: include_str!("../specs/qos-profiles/v1/openapi.yaml"),
    },
    ApiSpec {
        name: "kyc-tenure",
        version: "v0.2",
        body: include_str!("../specs/kyc-tenure/v0.2/openapi.yaml"),
    },
    ApiSpec {
        name: "blockchain-public-address",
        version: "v0.3",
        body: include_str!("../specs/blockchain-public-address/v0.3/openapi.yaml"),
    },
    ApiSpec {
        name: "simple-edge-discovery",
        version: "v2",
        body: include_str!("../specs/simple-edge-discovery/v2/openapi.yaml"),
    },
    ApiSpec {
        name: "customer-insights",
        version: "v0.2",
        body: include_str!("../specs/customer-insights/v0.2/openapi.yaml"),
    },
    ApiSpec {
        name: "connected-network-type",
        version: "v0.2",
        body: include_str!("../specs/connected-network-type/v0.2/openapi.yaml"),
    },
    ApiSpec {
        name: "device-data-volume",
        version: "vwip",
        body: include_str!("../specs/device-data-volume/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "connectivity-insights",
        version: "v0.6",
        body: include_str!("../specs/connectivity-insights/v0.6/openapi.yaml"),
    },
    ApiSpec {
        name: "region-device-count",
        version: "v0.2",
        body: include_str!("../specs/region-device-count/v0.2/openapi.yaml"),
    },
    ApiSpec {
        name: "device-visit-location",
        version: "vwip",
        body: include_str!("../specs/device-visit-location/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "population-density-data",
        version: "vwip",
        body: include_str!("../specs/population-density-data/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "qos-provisioning",
        version: "v0.3",
        body: include_str!("../specs/qos-provisioning/v0.3/openapi.yaml"),
    },
    ApiSpec {
        name: "qos-booking",
        version: "vwip",
        body: include_str!("../specs/qos-booking/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "media-streaming-rate",
        version: "vwip",
        body: include_str!("../specs/media-streaming-rate/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "network-health-assessment",
        version: "vwip",
        body: include_str!("../specs/network-health-assessment/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "network-traffic-analysis",
        version: "vwip",
        body: include_str!("../specs/network-traffic-analysis/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "optimal-edge-discovery",
        version: "vwip",
        body: include_str!("../specs/optimal-edge-discovery/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "verified-caller",
        version: "vwip",
        body: include_str!("../specs/verified-caller/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "application-profiles",
        version: "vwip",
        body: include_str!("../specs/application-profiles/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "subscription-status",
        version: "vwip",
        body: include_str!("../specs/subscription-status/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "device-authenticity",
        version: "vwip",
        body: include_str!("../specs/device-authenticity/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "session-insights",
        version: "vwip",
        body: include_str!("../specs/session-insights/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "consent-info",
        version: "vwip",
        body: include_str!("../specs/consent-info/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "iot-sim-fraud-prevention",
        version: "vwip",
        body: include_str!("../specs/iot-sim-fraud-prevention/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "sponsored-data",
        version: "vwip",
        body: include_str!("../specs/sponsored-data/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "click-to-dial",
        version: "vwip",
        body: include_str!("../specs/click-to-dial/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "most-frequent-location",
        version: "vwip",
        body: include_str!("../specs/most-frequent-location/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "traffic-influence",
        version: "vwip",
        body: include_str!("../specs/traffic-influence/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "application-endpoint-discovery",
        version: "vwip",
        body: include_str!("../specs/application-endpoint-discovery/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "application-endpoint-registration",
        version: "vwip",
        body: include_str!("../specs/application-endpoint-registration/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "predictive-connectivity-data",
        version: "vwip",
        body: include_str!("../specs/predictive-connectivity-data/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "network-access-devices",
        version: "vwip",
        body: include_str!("../specs/network-access-devices/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "sms",
        version: "v0alpha1",
        body: include_str!("../specs/sms/v0alpha1/openapi.yaml"),
    },
    ApiSpec {
        name: "capabilities-and-restrictions",
        version: "vwip",
        body: include_str!("../specs/capabilities-and-restrictions/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "dedicated-network-profiles",
        version: "vwip",
        body: include_str!("../specs/dedicated-network-profiles/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "dedicated-network",
        version: "vwip",
        body: include_str!("../specs/dedicated-network/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "dedicated-network-accesses",
        version: "vwip",
        body: include_str!("../specs/dedicated-network-accesses/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "dedicated-network-areas",
        version: "vwip",
        body: include_str!("../specs/dedicated-network-areas/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "edge-application-management",
        version: "vwip",
        body: include_str!("../specs/edge-application-management/vwip/openapi.yaml"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Extract `info.version` from an embedded OpenAPI body without a YAML dep.
    ///
    /// Scans the top-level `info:` block (every line until the next unindented
    /// key) for its direct-child `version:` entry (2-space indent, the CAMARA
    /// convention) and returns the unquoted scalar. Scoping to the `info:` block
    /// keeps a coincidental `version:` line elsewhere (e.g. inside a description
    /// block scalar or a component schema property) from being mistaken for it.
    fn info_version(body: &str) -> Option<String> {
        let mut in_info = false;
        for line in body.lines() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_info = line.trim_end() == "info:";
                continue;
            }
            if in_info {
                // `strip_prefix` matches exactly the 2-space indent of an `info:`
                // direct child, so a deeper `version:` (e.g. a 4-space component
                // property) never matches here.
                if let Some(rest) = line.strip_prefix("  version:") {
                    let v = rest.trim().trim_matches('"').trim_matches('\'');
                    return Some(v.to_string());
                }
            }
        }
        None
    }

    /// Extract every `operationId` value declared in an embedded OpenAPI body,
    /// in document order, without a YAML dep.
    ///
    /// `operationId` is an OpenAPI *operation-object* field — the canonical,
    /// document-unique name of an operation, which the server keys each handler
    /// to. It appears once per operation, conventionally at the 6-space indent of
    /// an operation field (`paths:` → `/path:` → `<method>:` → `operationId:`). A
    /// line is taken as a declaration when — after trimming leading whitespace —
    /// it begins with the `operationId:` key; its unquoted scalar is returned.
    /// Prose that merely *mentions* `operationId` (e.g. "(operationId `foo`)"
    /// inside a description) never begins with the key, so it is not matched.
    fn operation_ids(body: &str) -> Vec<String> {
        body.lines()
            .filter_map(|line| {
                let rest = line.trim_start().strip_prefix("operationId:")?;
                let v = rest.trim().trim_matches('"').trim_matches('\'');
                (!v.is_empty()).then(|| v.to_string())
            })
            .collect()
    }

    /// Extract every `$ref` target string declared in an embedded OpenAPI body,
    /// in document order, without a YAML dep.
    ///
    /// A `$ref` is an OpenAPI reference object (`$ref: "<target>"`). A line is
    /// taken as a declaration when — after trimming leading whitespace — it begins
    /// with the `$ref:` key; the unquoted scalar target is returned. Prose that
    /// merely *mentions* `$ref` never begins with the key, so it is not matched.
    fn ref_targets(body: &str) -> Vec<String> {
        body.lines()
            .filter_map(|line| {
                // `$ref` appears both as a mapping key (`$ref: "…"`) and as a YAML
                // sequence item (`- $ref: "…"`, e.g. in a `parameters:` list), so
                // strip an optional leading `- ` sequence marker before the key.
                let trimmed = line.trim_start();
                let after_dash = trimmed.strip_prefix("- ").unwrap_or(trimmed);
                let rest = after_dash.strip_prefix("$ref:")?;
                let v = rest.trim().trim_matches('"').trim_matches('\'');
                (!v.is_empty()).then(|| v.to_string())
            })
            .collect()
    }

    /// Count the `x-camarasim-scenarios:` blocks declared in an embedded OpenAPI
    /// body, without a YAML dep.
    ///
    /// `x-camarasim-scenarios` is the simulator's OpenAPI vendor extension that
    /// documents an operation's parameter-driven functional cases in the spec
    /// (docs/DESIGN.md §7, §9) — the structured counterpart to the prose in the
    /// operation `description`. It is an operation-object field (`paths:` →
    /// `/path:` → `<method>:` → `x-camarasim-scenarios:`). A line is counted as a
    /// declaration when — after trimming leading whitespace — it begins with the
    /// `x-camarasim-scenarios:` key, so prose that merely mentions the name is not
    /// matched.
    fn scenario_blocks(body: &str) -> usize {
        body.lines()
            .filter(|line| line.trim_start().starts_with("x-camarasim-scenarios:"))
            .count()
    }

    /// Extract the set of component pointers a `components:` fragment *defines*,
    /// as `#/components/<section>/<Name>` strings, without a YAML dep.
    ///
    /// Scans the top-level `components:` block. A 2-space direct child key opens a
    /// section (`  schemas:`, `  responses:`, …), and each exact-4-space child key
    /// under it (`    CamaraError:`) is one defined component. Deeper lines (6-space+
    /// properties, `content`, `example`, …) sit *inside* a component, so they are
    /// ignored — only the component objects themselves are collected. Used to prove
    /// that every `$ref` a spec makes into the shared error model points at a
    /// component that fragment actually declares.
    fn component_pointers(body: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut in_components = false;
        let mut section: Option<String> = None;
        for line in body.lines() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_components = line.trim_end() == "components:";
                section = None;
                continue;
            }
            if !in_components {
                continue;
            }
            // A 2-space direct child of `components:` (non-space at column 3) opens
            // a section (`schemas`, `responses`, …). A 4-space line also begins with
            // two spaces, but its column-3 char *is* a space, so it falls through to
            // the component check below.
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) {
                    if let Some(name) = rest.trim_end().strip_suffix(':') {
                        if !name.is_empty() && !name.contains(char::is_whitespace) {
                            section = Some(name.to_string());
                        }
                    }
                    continue;
                }
            }
            // An exact-4-space child (non-space at column 5) under a section is a
            // component definition; a bare `Name:` with no trailing value.
            if let Some(section) = &section {
                if let Some(rest) = line.strip_prefix("    ") {
                    if !rest.starts_with(char::is_whitespace) {
                        if let Some(name) = rest.trim_end().strip_suffix(':') {
                            if !name.is_empty() && !name.contains(char::is_whitespace) {
                                out.insert(format!("#/components/{section}/{name}"));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// Extract the set of security schemes a spec *defines* under
    /// `components.securitySchemes:`, by their scheme name (e.g. `openId`).
    ///
    /// Reuses `component_pointers` (which already collects every
    /// `#/components/<section>/<Name>` a spec declares) and keeps only the
    /// `securitySchemes` section, stripping back to the bare scheme name. The
    /// scheme's *definition body* (a `$ref` into the shared `auth/openapi.yaml`,
    /// or inline fields) sits deeper and is not collected — only the scheme
    /// object's own key. Used to prove every `security` *requirement* a spec makes
    /// names a scheme the spec actually defines.
    fn defined_security_schemes(body: &str) -> HashSet<String> {
        component_pointers(body)
            .iter()
            .filter_map(|p| {
                p.strip_prefix("#/components/securitySchemes/")
                    .map(str::to_string)
            })
            .collect()
    }

    /// Extract every security-scheme name *referenced* by a `security` requirement
    /// in an embedded OpenAPI body, in document order, without a YAML dep.
    ///
    /// A `security` requirement is a list of `{ <schemeName>: [scopes] }` maps
    /// (`security:` → `- <schemeName>:` → `- <scope>`). This scans each `security:`
    /// block — tracked by indentation, so `- name:`/`- in:` items of a sibling
    /// `parameters:` list are never mistaken for scheme references — and, within
    /// it, collects each sequence item that is a *mapping key* (`- openId:` or
    /// `- openId: []`), i.e. where the first `:` ends the token or is followed by
    /// whitespace. A scope entry (`- number-verification:verify`) is a plain
    /// scalar — its `:` is followed by a non-space — so it is skipped, even though
    /// it sits inside the same block.
    fn security_requirement_schemes(body: &str) -> Vec<String> {
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        // `Some(n)` while inside a `security:` block whose key sits at indent `n`.
        let mut security_indent: Option<usize> = None;
        for line in body.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let ind = indent(line);
            if let Some(sec) = security_indent {
                if ind <= sec {
                    // Dedent to at-or-above the `security:` key ends the block; fall
                    // through so this same line can open a new `security:` block.
                    security_indent = None;
                } else {
                    if let Some(name) = requirement_scheme_name(line) {
                        out.push(name);
                    }
                    continue;
                }
            }
            if line.trim() == "security:" {
                security_indent = Some(ind);
            }
        }
        out
    }

    /// If `line` is a security-requirement sequence item naming a scheme
    /// (`- openId:` / `- openId: []`), return the scheme name; otherwise `None`.
    ///
    /// The discriminator is YAML mapping-key syntax: the first `:` must end the
    /// token or be followed by whitespace. That accepts `- openId:` (scheme) and
    /// rejects a scope scalar like `- number-verification:verify`, whose `:` is
    /// followed by a non-space character.
    fn requirement_scheme_name(line: &str) -> Option<String> {
        let rest = line.trim_start().strip_prefix("- ")?;
        let colon = rest.find(':')?;
        let after = &rest[colon + 1..];
        if after.is_empty() || after.starts_with(char::is_whitespace) {
            let name = rest[..colon].trim();
            if !name.is_empty() && !name.contains(char::is_whitespace) {
                return Some(name.to_string());
            }
        }
        None
    }

    /// Does the spec's declared `info.version` agree with the version segment the
    /// API is mounted at in the URL (DESIGN §9 canonical URL versioning)?
    ///
    /// - `vwip` ↔ `info.version == "wip"` (work-in-progress).
    /// - a pre-release URL segment (`…alpha…`/`…rc…`, e.g. `v0alpha1`) ↔ a
    ///   pre-release `info.version` (contains `alpha`/`rc`).
    /// - `v0.N` (initial 0.x scheme) ↔ `info.version` starts with `0.N.`.
    /// - `vN` (stable major, N≥1) ↔ `info.version` starts with `N.`.
    fn url_version_agrees(mounted: &str, info_version: &str) -> bool {
        if mounted == "vwip" {
            return info_version == "wip";
        }
        let rest = mounted.strip_prefix('v').unwrap_or(mounted);
        if rest.contains("alpha") || rest.contains("rc") {
            return info_version.contains("alpha") || info_version.contains("rc");
        }
        match rest.split_once('.') {
            // `v0.N` → the 0.x initial-version scheme.
            Some((major, minor)) => info_version.starts_with(&format!("{major}.{minor}.")),
            // `vN` → a stable major.
            None => info_version.starts_with(&format!("{rest}.")),
        }
    }

    #[test]
    fn registry_is_non_empty() {
        // Guards a broken/emptied list: both the catalog and the served specs are
        // derived from this, so an empty registry would silently serve nothing.
        assert!(APIS.len() >= 28, "expected the full API registry, got {}", APIS.len());
    }

    #[test]
    fn each_api_is_listed_once() {
        // No two entries share a base path — a duplicate would mount the same
        // route twice and double-list the API in the `/` catalog.
        let mut seen = HashSet::new();
        for api in APIS {
            assert!(
                seen.insert(api.base_path()),
                "duplicate API in registry: {}",
                api.base_path()
            );
        }
    }

    #[test]
    fn derived_paths_are_consistent() {
        // `base_path`/`spec_url` are the exact shapes the catalog advertises and
        // the specs are served at (canonical URL versioning, DESIGN §9).
        for api in APIS {
            assert_eq!(api.base_path(), format!("/{}/{}", api.name, api.version));
            assert_eq!(api.spec_url(), format!("{}/openapi.yaml", api.base_path()));
            assert!(!api.name.is_empty() && !api.version.is_empty());
        }
    }

    #[test]
    fn bodies_are_non_empty_openapi_docs() {
        // Each embedded body is the vendored spec file, not an empty include.
        for api in APIS {
            assert!(
                api.body.contains("openapi:"),
                "{} body is not an OpenAPI document",
                api.name
            );
        }
    }

    #[test]
    fn spec_server_url_matches_mounted_base_path() {
        // Contract-harness invariant (DESIGN §9): every vendored CAMARA spec
        // declares its base path in `servers[].url` as `{apiRoot}/{name}/{version}`,
        // and the router mounts the API at exactly that `base_path()`. A newly
        // vendored spec that kept the CAMARA template's original server url — or an
        // entry mounted at a version its spec doesn't declare — is a real drift the
        // existing catalog↔served-spec tests can't see (they check *which* specs are
        // served, not that a spec's advertised path matches where it's mounted).
        // This ties the two together: the served contract must name the served path.
        for api in APIS {
            let expected = format!("url: \"{{apiRoot}}{}\"", api.base_path());
            assert!(
                api.body.contains(&expected),
                "{} spec's servers url does not match its mounted base path {} \
                 (expected the body to contain `{}`)",
                api.name,
                api.base_path(),
                expected
            );
        }
    }

    #[test]
    fn spec_info_version_matches_mounted_url_version() {
        // Contract-harness invariant (DESIGN §9): the semantic version each
        // vendored spec declares in `info.version` must agree with the version
        // segment the API is mounted at in the URL. This catches a distinct
        // drift the `servers[].url` test can't: a spec vendored (or copy-pasted
        // from a sibling) with a stale/mismatched `info.version` — e.g. a spec
        // bumped upstream to a new major while still mounted at the old `v{n}`,
        // or an initial-version spec whose `0.N` minor disagrees with its mount.
        // The `servers[].url` check only proves the spec *names* its mount path;
        // this proves the spec's declared version *is* that version.
        for api in APIS {
            let iv = info_version(api.body).unwrap_or_else(|| {
                panic!("{} spec has no info.version", api.name)
            });
            assert!(
                url_version_agrees(api.version, &iv),
                "{} is mounted at `{}` but its spec declares info.version `{}` \
                 (URL version segment and spec version disagree — DESIGN §9)",
                api.name,
                api.version,
                iv
            );
        }
    }

    #[test]
    fn every_vendored_spec_on_disk_is_registered() {
        // Contract-harness invariant (DESIGN §9): the `specs/` tree and the `APIS`
        // registry must agree in BOTH directions. The compile-time `include_str!`
        // already fails the build if a *registered* API's spec file is missing
        // (registry → file), but nothing catches the reverse: a spec vendored on
        // disk under `specs/…` that was never added to `APIS` compiles fine and is
        // simply never mounted or served — an invisible, forgotten API. This walks
        // the on-disk tree and asserts the set of vendored `openapi.yaml` files
        // exactly equals the set of registered spec paths, so a forgotten
        // registration (or a stray extra version dir) fails CI.
        use std::path::Path;

        let specs_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("specs");

        // On disk: every `specs/<name>/<version>/openapi.yaml`, excluding the
        // shared building blocks (`shared/errors.yaml`, `auth/openapi.yaml`) that
        // are served for `$ref` resolution but are not mounted APIs.
        let mut on_disk: HashSet<String> = HashSet::new();
        for api_entry in std::fs::read_dir(&specs_root).expect("read specs/") {
            let api_dir = api_entry.expect("specs/ entry").path();
            if !api_dir.is_dir() {
                continue;
            }
            let name = api_dir.file_name().unwrap().to_string_lossy().into_owned();
            if name == "shared" || name == "auth" {
                continue;
            }
            for ver_entry in std::fs::read_dir(&api_dir).expect("read spec version dir") {
                let ver_dir = ver_entry.expect("version entry").path();
                if !ver_dir.is_dir() {
                    continue;
                }
                let version = ver_dir.file_name().unwrap().to_string_lossy().into_owned();
                if ver_dir.join("openapi.yaml").is_file() {
                    on_disk.insert(format!("specs/{name}/{version}/openapi.yaml"));
                }
            }
        }

        // Registered: the spec path each APIS entry embeds via `include_str!`.
        let registered: HashSet<String> = APIS
            .iter()
            .map(|a| format!("specs/{}/{}/openapi.yaml", a.name, a.version))
            .collect();

        let mut unregistered: Vec<&String> = on_disk.difference(&registered).collect();
        unregistered.sort();
        assert!(
            unregistered.is_empty(),
            "vendored spec(s) on disk not registered in APIS (they would never be \
             mounted or served): {unregistered:?}"
        );
        // The reverse direction, belt-and-braces with the compile-time
        // `include_str!`: a registered entry whose vendored file has vanished.
        let mut missing: Vec<&String> = registered.difference(&on_disk).collect();
        missing.sort();
        assert!(
            missing.is_empty(),
            "registered API(s) with no vendored spec file on disk: {missing:?}"
        );
    }

    #[test]
    fn operation_ids_are_unique_within_each_spec() {
        // Contract-harness invariant (OpenAPI structural + DESIGN §9): within a
        // single OpenAPI document every operation's `operationId` MUST be unique —
        // it is the operation's canonical name, and the simulator keys each
        // handler/scope narrative to it. A new endpoint's spec is usually drafted
        // by copy-pasting an operation from a sibling API, so it is easy to leave
        // the pasted `operationId` unchanged and end up with two operations
        // sharing an id — an invalid document that no existing contract test sees
        // (they check a spec's mount path, declared version, and registry parity,
        // never that its operation *names* are well-formed). This asserts every
        // mounted spec declares at least one operationId and none repeats within
        // it. Uniqueness is scoped per spec on purpose: the *same* operationId
        // (e.g. `createSubscription`, `retrieveSessionsByDevice`) legitimately
        // recurs across different APIs — only within one document is it a defect.
        for api in APIS {
            let ids = operation_ids(api.body);
            assert!(
                !ids.is_empty(),
                "{} spec declares no operationId (every mounted API has operations)",
                api.name
            );
            let mut seen = HashSet::new();
            for id in &ids {
                assert!(
                    seen.insert(id.as_str()),
                    "{} spec declares operationId `{}` more than once \
                     (operationIds must be unique within an OpenAPI document)",
                    api.name,
                    id
                );
            }
        }
    }

    /// The single `$ref` every mounted spec must use to define its `openId`
    /// security scheme: a pointer into the shared, centrally-maintained
    /// `auth/openapi.yaml` `camaraOAuth` definition. A spec sits two directories
    /// below `specs/` (`specs/<name>/<version>/openapi.yaml`), so the relative path
    /// up to the shared `auth/openapi.yaml` is identical for every API.
    const SHARED_OAUTH_REF: &str =
        "../../auth/openapi.yaml#/components/securitySchemes/camaraOAuth";

    #[test]
    fn every_spec_refs_the_shared_camara_oauth_scheme() {
        // Contract-harness invariant (CAMARA canonical auth + DESIGN §8/§9): the
        // OAuth/OIDC security scheme is defined once, in the shared
        // `auth/openapi.yaml` as `camaraOAuth` (its `openIdConnectUrl` points at
        // this server's discovery doc), and every mounted spec references *that*
        // single source of truth for its `openId` scheme via a `$ref`. A spec that
        // instead defines the scheme inline is a drift-prone duplicate: its
        // `openIdConnectUrl`, type, or description can silently diverge from the
        // shared definition (and from what the resource server actually enforces),
        // and no existing contract test sees it — the mount-path/version/parity/
        // operationId tests all check a spec's *identity*, never how it wires auth.
        // Every business endpoint is OAuth-protected, so every spec must carry the
        // shared reference. Verified true across all mounted specs before asserting.
        for api in APIS {
            assert!(
                api.body.contains(SHARED_OAUTH_REF),
                "{} spec does not reference the shared camaraOAuth security scheme \
                 (expected a `$ref` to `{}`); it likely defines `openId` inline, a \
                 drift-prone duplicate of the single source of truth in auth/openapi.yaml",
                api.name,
                SHARED_OAUTH_REF
            );
        }
    }

    #[test]
    fn every_spec_documents_functional_cases() {
        // Contract-harness invariant (DESIGN §7, §9): every mounted API is driven
        // by its input (the reserved-identifier convention + per-parameter control
        // planes), and that behaviour MUST be documented in the spec as a
        // structured `x-camarasim-scenarios` block, not prose alone — so the
        // vendored spec is a faithful, machine-readable record of what the server
        // does. A newly vendored spec drafted from a CAMARA template carries no
        // such block until the agent adds one; leaving it out lets spec and
        // behaviour drift silently (the mount-path/version/parity/operationId/oauth
        // tests all check a spec's identity or wiring, never that it documents its
        // functional cases). This asserts every mounted spec declares at least one
        // `x-camarasim-scenarios` block. Verified true across all mounted specs
        // before asserting.
        for api in APIS {
            assert!(
                scenario_blocks(api.body) >= 1,
                "{} spec declares no x-camarasim-scenarios block \
                 (every mounted API documents its parameter-driven functional \
                 cases in the spec — DESIGN §7, §9)",
                api.name
            );
        }
    }

    #[test]
    fn shared_fragment_refs_use_the_canonical_relative_path() {
        // Contract-harness invariant (DESIGN §8/§9 + `apis::openapi` serving): a
        // spec's cross-file `$ref`s to the two shared fragments — the error model
        // (`shared/errors.yaml`) and the auth scheme (`auth/openapi.yaml`) — MUST
        // use the canonical relative path from a spec's own location,
        // `specs/<name>/<version>/openapi.yaml`: `../../shared/errors.yaml` and
        // `../../auth/openapi.yaml`. That is the ONLY form that resolves, because
        // the server serves those fragments at `/shared/errors.yaml` and
        // `/auth/openapi.yaml`, and a `$ref` in a spec served at
        // `/{name}/{version}/openapi.yaml` is resolved *relative to that URL* — so
        // only `../../shared/errors.yaml` climbs back to `/shared/errors.yaml`.
        //
        // A bare `errors.yaml#…` (a copy-paste from a CAMARA template, where the
        // error file sits beside the spec) resolves to
        // `/{name}/{version}/errors.yaml`, which the server never serves → a 404
        // for any client (Redoc/Swagger/codegen) that follows the ref, leaving the
        // served spec unresolvable. No existing contract test sees this: the
        // camaraOAuth-scheme test only checks the `openId` *securityScheme* `$ref`,
        // and the mount-path/version/parity/operationId/functional-cases tests all
        // check a spec's identity or its documented behaviour, never that its
        // cross-file `$ref`s point at a path the server actually serves.
        //
        // Local intra-document refs (`#/components/…`) carry neither fragment name,
        // so a spec that legitimately inlines its own error responses is unaffected.
        for api in APIS {
            for target in ref_targets(api.body) {
                if target.contains("errors.yaml") {
                    assert!(
                        target.contains("../../shared/errors.yaml"),
                        "{} spec has a $ref to the shared error model by a \
                         non-canonical path `{}` — it will not resolve when the \
                         spec is served (expected `../../shared/errors.yaml#…`, the \
                         only path that reaches the served /shared/errors.yaml)",
                        api.name,
                        target
                    );
                }
                if target.contains("auth/openapi.yaml") {
                    assert!(
                        target.contains("../../auth/openapi.yaml"),
                        "{} spec has a $ref to the shared auth spec by a \
                         non-canonical path `{}` — it will not resolve when the \
                         spec is served (expected `../../auth/openapi.yaml#…`, the \
                         only path that reaches the served /auth/openapi.yaml)",
                        api.name,
                        target
                    );
                }
            }
        }
    }

    #[test]
    fn shared_error_refs_resolve_to_defined_components() {
        // Contract-harness invariant (DESIGN §8/§9 + `apis::openapi` serving): a
        // spec's cross-file `$ref`s into the shared error model
        // (`../../shared/errors.yaml#/components/…`) must point at a component that
        // fragment actually DEFINES. The sibling canonical-path test proves such a
        // ref uses the one relative path that reaches the served fragment; this
        // proves the JSON-pointer *into* that fragment names a real component, so a
        // client (Redoc/Swagger/codegen) dereferencing it gets the response/schema
        // rather than a dangling pointer.
        //
        // The break this catches: a spec drafted by copy-pasting a sibling's error
        // block can pick a response name that does not exist in the shared fragment
        // — a typo (`InvalidArguments`), a CAMARA-template name the shared model
        // never adopted (`Generic404`), or a name renamed in the shared file after
        // the copy. Both `$ref` halves then look right (correct file, plausible
        // pointer) yet resolve to nothing. No existing contract test sees this: the
        // canonical-path test checks only the *file* half of the ref, and the
        // identity/wiring tests never dereference a spec's cross-file pointers.
        //
        // The allowed set is extracted from the embedded shared fragment itself
        // (not hard-coded), so adding a new shared response automatically widens it
        // and this test never needs editing when the shared model grows.
        const SHARED_ERRORS: &str = include_str!("../specs/shared/errors.yaml");
        let defined = component_pointers(SHARED_ERRORS);
        // Non-vacuous floor: the fragment defines the CamaraError schema and the
        // canonical CAMARA response objects (9 statuses today).
        assert!(
            defined.contains("#/components/schemas/CamaraError"),
            "shared/errors.yaml is expected to define the CamaraError schema; \
             extracted {defined:?}"
        );
        assert!(
            defined.len() >= 10,
            "expected the shared error model to define ≥10 components \
             (CamaraError + the canonical responses), got {}: {defined:?}",
            defined.len()
        );

        for api in APIS {
            for target in ref_targets(api.body) {
                let Some((file, pointer)) = target.split_once('#') else {
                    continue;
                };
                // Only cross-file refs into the shared error model. (Local
                // intra-document refs have an empty `file` half and are resolved
                // within the spec itself, not against this fragment.)
                if !file.contains("shared/errors.yaml") {
                    continue;
                }
                let pointer = format!("#{pointer}");
                assert!(
                    defined.contains(&pointer),
                    "{} spec has a $ref into the shared error model at `{}`, but \
                     shared/errors.yaml defines no such component — it resolves to a \
                     dangling pointer when the spec is served. Defined components: {:?}",
                    api.name,
                    target,
                    {
                        let mut v: Vec<&String> = defined.iter().collect();
                        v.sort();
                        v
                    }
                );
            }
        }
    }

    #[test]
    fn every_security_requirement_references_a_defined_scheme() {
        // Contract-harness invariant (CAMARA canonical auth + DESIGN §8/§9): every
        // `security` requirement an operation declares MUST name a security scheme
        // the spec DEFINES under `components.securitySchemes`. In this simulator
        // that scheme is `openId` (a `$ref` to the shared `camaraOAuth`), and every
        // CAMARA business operation is OAuth-protected, so every spec both defines
        // `openId` and references it from each operation's `security` block.
        //
        // The break this catches: a new endpoint's spec is usually drafted by
        // copy-pasting an operation from a CAMARA template or sibling API, and the
        // pasted `security` requirement can keep a scheme name the spec never
        // defines — a template leftover (`oAuth2ClientCredentials`, `three_legged`),
        // a typo (`openID`), or a name renamed away from `openId`. The requirement
        // then dangles: an OpenAPI document where an operation demands a scheme its
        // own `securitySchemes` never declares, so a client cannot tell what auth
        // the operation needs and codegen breaks. No existing contract test sees it:
        // the camaraOAuth-scheme test checks only how `openId` is *defined* (the
        // shared `$ref`), never that operations *reference* a defined scheme; the
        // mount-path/version/parity/operationId/functional-cases tests all check a
        // spec's identity or documented behaviour. Verified true across all mounted
        // specs before asserting.
        for api in APIS {
            let defined = defined_security_schemes(api.body);
            // Non-vacuous floor: every business spec defines the shared `openId`
            // scheme, so an extractor that silently found none can't hide here.
            assert!(
                defined.contains("openId"),
                "{} spec defines no `openId` scheme under components.securitySchemes \
                 (defines {:?})",
                api.name,
                {
                    let mut v: Vec<&String> = defined.iter().collect();
                    v.sort();
                    v
                }
            );
            let requirements = security_requirement_schemes(api.body);
            // Non-vacuous floor: every business operation is OAuth-protected, so a
            // spec with zero `security` requirements would make the per-ref loop
            // below unreachable — that is itself a drift worth failing on.
            assert!(
                !requirements.is_empty(),
                "{} spec declares no `security` requirement (every CAMARA business \
                 operation is OAuth-protected — a missing requirement leaves an \
                 operation unsecured)",
                api.name
            );
            for scheme in requirements {
                assert!(
                    defined.contains(&scheme),
                    "{} spec has a `security` requirement referencing scheme `{}`, but \
                     its components.securitySchemes defines no such scheme (defines \
                     {:?}) — a dangling, unresolvable requirement (likely a \
                     copy-pasted CAMARA-template scheme name never renamed to `openId`)",
                    api.name,
                    scheme,
                    {
                        let mut v: Vec<&String> = defined.iter().collect();
                        v.sort();
                        v
                    }
                );
            }
        }
    }

    #[test]
    fn component_pointer_extraction_rules() {
        // Unit-cover the `component_pointers` extractor so the contract test above
        // can't pass vacuously (an extractor that found no components would make its
        // per-ref assertions unreachable) and so its section/component/property
        // discrimination is pinned.
        let body = "\
openapi: 3.0.3
components:
  schemas:
    CamaraError:
      type: object
      properties:
        status:
          type: integer
  responses:
    NotFound:
      description: not found
      content:
        application/json:
          schema:
            $ref: \"#/components/schemas/CamaraError\"
paths: {}
";
        let ptrs = component_pointers(body);
        assert!(ptrs.contains("#/components/schemas/CamaraError"));
        assert!(ptrs.contains("#/components/responses/NotFound"));
        // Only the components themselves — never their nested property/content keys.
        assert!(!ptrs.contains("#/components/schemas/properties"));
        assert!(!ptrs.contains("#/components/schemas/status"));
        assert!(!ptrs.contains("#/components/responses/content"));
        assert_eq!(ptrs.len(), 2, "extracted {ptrs:?}");
        // A body with no `components:` block yields nothing.
        assert!(component_pointers("openapi: 3.0.3\npaths: {}\n").is_empty());
        // The real shared fragment defines the CamaraError schema + 9 responses.
        let shared = component_pointers(include_str!("../specs/shared/errors.yaml"));
        assert_eq!(shared.len(), 10, "shared components: {shared:?}");
    }

    #[test]
    fn security_scheme_extraction_rules() {
        // Unit-cover both security helpers so the contract test above can't pass
        // vacuously (a requirement extractor that always returned nothing, or a
        // definition extractor that returned everything, would make its per-ref
        // assertions unreachable) and so their scheme-vs-scope and
        // block-vs-sibling-list discrimination is pinned.
        let body = "\
openapi: 3.0.3
paths:
  /verify:
    post:
      operationId: doVerify
      security:
        - openId:
            - number-verification:verify
      parameters:
        - name: x-correlator
          in: header
components:
  securitySchemes:
    openId:
      $ref: \"../../auth/openapi.yaml#/components/securitySchemes/camaraOAuth\"
";
        // The defined set is exactly the scheme object key — not its `$ref` body.
        let defined = defined_security_schemes(body);
        assert!(defined.contains("openId"));
        assert_eq!(defined.len(), 1, "defined {defined:?}");
        // The requirement names `openId` — never the scope scalar
        // (`number-verification:verify`, whose `:` is followed by a non-space) and
        // never the sibling `parameters:` list's `- name:` / `- in:` items (they
        // sit outside the `security:` block, which ends at the dedent to
        // `parameters:`).
        assert_eq!(security_requirement_schemes(body), vec!["openId"]);
        // The inline empty-scopes form is still a scheme reference.
        assert_eq!(
            security_requirement_schemes("      security:\n        - openId: []\n"),
            vec!["openId"]
        );
        // An undefined scheme name is surfaced verbatim (the contract test turns
        // that into a failure).
        let drift = "      security:\n        - three_legged:\n            - scope:read\n";
        assert_eq!(security_requirement_schemes(drift), vec!["three_legged"]);
        // A body with no `security:` block yields no requirements, and one with no
        // `components:` yields no defined schemes.
        assert!(security_requirement_schemes("openapi: 3.0.3\npaths: {}\n").is_empty());
        assert!(defined_security_schemes("openapi: 3.0.3\npaths: {}\n").is_empty());
        // The real vendored spec defines exactly the shared `openId` scheme and
        // references it from its operations.
        let real = include_str!("../specs/number-verification/v1/openapi.yaml");
        assert_eq!(defined_security_schemes(real).len(), 1);
        assert!(defined_security_schemes(real).contains("openId"));
        assert!(security_requirement_schemes(real)
            .iter()
            .all(|s| s == "openId"));
    }

    #[test]
    fn ref_target_extraction_rules() {
        // Unit-cover the `ref_targets` extractor so the contract test above can't
        // pass vacuously (an extractor that never found a ref would make the
        // per-ref assertions unreachable) and so its quote-stripping and
        // prose-skipping are pinned.
        let body = "responses:\n  '400':\n    $ref: \"../../shared/errors.yaml#/x\"\n\
                        '401':\n      $ref: '#/components/responses/Local'\n\
                    security:\n  - $ref: \"../../auth/openapi.yaml#/y\"\n";
        assert_eq!(
            ref_targets(body),
            vec![
                "../../shared/errors.yaml#/x",
                "#/components/responses/Local",
                "../../auth/openapi.yaml#/y",
            ]
        );
        // A description that merely mentions `$ref` is not a declaration (it does
        // not begin with the `$ref:` key after trimming).
        let prose = "      description: |\n        see the $ref above for details.\n";
        assert!(ref_targets(prose).is_empty());
        // The broken bare form the contract test rejects is still *extracted* (so
        // the assertion can catch it), and it fails the canonical-path check.
        let broken = "    $ref: \"errors.yaml#/components/responses/NotFound\"\n";
        assert_eq!(ref_targets(broken), vec!["errors.yaml#/components/responses/NotFound"]);
        assert!(!ref_targets(broken)[0].contains("../../shared/errors.yaml"));
    }

    #[test]
    fn scenario_block_extraction_rules() {
        // Unit-cover the `scenario_blocks` counter so the contract test above
        // can't pass vacuously (a counter that always returned 0 would make the
        // assertion unreachable) and so its key-vs-prose discrimination is pinned.
        let body = "paths:\n  /a:\n    post:\n      operationId: doA\n\
                    \x20     x-camarasim-scenarios:\n        cases: []\n\
                    \x20 /b:\n    post:\n      x-camarasim-scenarios:\n        cases: []\n";
        assert_eq!(scenario_blocks(body), 2);
        // A description that merely mentions the extension name is not a
        // declaration (it does not begin with the key after trimming).
        let prose = "      description: |\n        cases live in x-camarasim-scenarios.\n";
        assert_eq!(scenario_blocks(prose), 0);
        // A spec with no scenarios block counts zero (the contract test turns that
        // into a failure).
        assert_eq!(scenario_blocks("openapi: 3.0.3\npaths: {}\n"), 0);
    }

    #[test]
    fn operation_id_extraction_rules() {
        // Unit-cover the `operation_ids` extractor so the contract test above
        // can't pass vacuously (an extractor that never found an id would make
        // "no duplicates within a spec" trivially true) and so its quote-stripping
        // and prose-skipping are pinned.
        let body = "paths:\n  /a:\n    get:\n      operationId: doA\n\
                        post:\n      operationId: \"doB\"\n  /c:\n    get:\n\
                          operationId: 'doC'\n";
        assert_eq!(operation_ids(body), vec!["doA", "doB", "doC"]);
        // A description that merely mentions the word must not be picked up as a
        // declaration (it does not begin with the `operationId:` key after trim).
        let prose = "      description: |\n        Retrieve a status (operationId `getStatus`).\n";
        assert!(operation_ids(prose).is_empty());
        // A body with no operations yields an empty list.
        assert!(operation_ids("openapi: 3.0.3\npaths: {}\n").is_empty());
        // A duplicate id is surfaced verbatim (the contract test turns a repeat
        // within one spec into a failure).
        let dup = "    get:\n      operationId: same\n    post:\n      operationId: same\n";
        assert_eq!(operation_ids(dup), vec!["same", "same"]);
    }

    #[test]
    fn info_version_extraction_and_agreement_rules() {
        // Unit-cover the two pure helpers so the contract test above can't pass
        // vacuously (e.g. a broken extractor returning the same string for all).
        let body = "openapi: 3.0.3\n\
                    info:\n  title: X\n  description: |\n    a version: 9.9.9 line inside prose\n  version: \"1.2.3\"\n\
                    paths:\n  /x:\n    get: {}\n";
        assert_eq!(info_version(body).as_deref(), Some("1.2.3"));
        // A `version:` inside the description block scalar must not be picked up,
        // and a top-level-only body returns None.
        assert_eq!(info_version("openapi: 3.0.3\npaths: {}\n"), None);

        // Agreement rules across every URL-versioning shape in the registry.
        assert!(url_version_agrees("vwip", "wip"));
        assert!(!url_version_agrees("vwip", "1.0.0"));
        assert!(url_version_agrees("v1", "1.0.0"));
        assert!(url_version_agrees("v1", "1.1.1"));
        assert!(!url_version_agrees("v1", "2.0.0"));
        assert!(url_version_agrees("v2", "2.0.1"));
        assert!(url_version_agrees("v3", "3.0.0"));
        assert!(url_version_agrees("v0.3", "0.3.0"));
        assert!(!url_version_agrees("v0.3", "0.4.0"));
        assert!(!url_version_agrees("v0.3", "1.3.0"));
        assert!(url_version_agrees("v0alpha1", "0.1.0-alpha.1"));
        assert!(!url_version_agrees("v0alpha1", "1.0.0"));
    }
}
