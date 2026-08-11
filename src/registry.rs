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

    /// Extract `info.title` from an embedded OpenAPI body without a YAML dep.
    ///
    /// `title` and `version` are the two REQUIRED fields of an OpenAPI
    /// document's `info` object — `title` is the human name every
    /// Redoc/Swagger/codegen client renders as the document's heading (a
    /// document without it renders "Untitled") and the label the `/` catalog
    /// shows. Mirrors `info_version`: scans the top-level `info:` block (every
    /// line until the next unindented key) for its direct-child `title:` entry
    /// (2-space indent, the CAMARA convention) and returns the unquoted scalar.
    /// Scoping to the `info:` block keeps a coincidental `title:` line elsewhere
    /// (e.g. a JSON-Schema `title:` inside a component schema) from being
    /// mistaken for it. A present-but-blank `title:` returns `Some("")` — a
    /// distinct case from a missing line (`None`) the contract test rejects.
    fn info_title(body: &str) -> Option<String> {
        let mut in_info = false;
        for line in body.lines() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_info = line.trim_end() == "info:";
                continue;
            }
            if in_info {
                // `strip_prefix` matches exactly the 2-space indent of an
                // `info:` direct child, so a deeper `title:` (a 6-space schema
                // property) never matches here.
                if let Some(rest) = line.strip_prefix("  title:") {
                    let v = rest.trim().trim_matches('"').trim_matches('\'');
                    return Some(v.to_string());
                }
            }
        }
        None
    }

    /// Extract the root `openapi:` version string from an embedded OpenAPI body,
    /// without a YAML dep.
    ///
    /// `openapi` is the single REQUIRED *root* field of every OpenAPI document —
    /// the semantic version of the OpenAPI Specification the document follows
    /// (e.g. `3.0.3`), which every Redoc/Swagger/codegen tool reads first to
    /// decide how to interpret the rest (3.0 and 3.1 differ in `nullable`/`type`
    /// handling). It is a top-level key (zero indent). A line is taken as the
    /// declaration when it is an unindented `openapi:` key; its unquoted scalar is
    /// returned. An indented `openapi:` (e.g. inside a description block scalar or
    /// an example) is never at column zero, so a prose mention is not matched.
    fn openapi_version(body: &str) -> Option<String> {
        for line in body.lines() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                if let Some(rest) = line.strip_prefix("openapi:") {
                    let v = rest.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    }

    /// Whether a version string is a valid OpenAPI **3** version — exactly
    /// `3.MINOR.PATCH`, all three parts numeric — the family this simulator
    /// vendors. Rejects a Swagger `2.x` document (a different, incompatible
    /// specification), a truncated two-part `3.0`, an over-long `3.0.3.1`, and any
    /// non-numeric or malformed value.
    fn is_openapi_3_version(v: &str) -> bool {
        let mut parts = v.split('.');
        let (major, minor, patch) =
            match (parts.next(), parts.next(), parts.next(), parts.next()) {
                (Some(a), Some(b), Some(c), None) => (a, b, c),
                _ => return false,
            };
        major == "3"
            && !minor.is_empty()
            && !patch.is_empty()
            && minor.bytes().all(|b| b.is_ascii_digit())
            && patch.bytes().all(|b| b.is_ascii_digit())
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

    /// Extract every path-template parameter name a spec declares in its `paths:`
    /// keys — the `{name}` tokens of a templated path like `/sessions/{sessionId}`
    /// — without a YAML dep.
    ///
    /// Scans the top-level `paths:` block. A direct 2-space child key that begins
    /// with `/` is a path item (`  /sessions/{sessionId}:`); each `{…}` span in
    /// that key is one path-template parameter. Scoping to `paths:` keeps a `{…}`
    /// that appears elsewhere (a description, an example) from being mistaken for a
    /// path parameter.
    fn path_template_params(body: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut in_paths = false;
        for line in body.lines() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                continue;
            }
            if !in_paths {
                continue;
            }
            // A 2-space direct child of `paths:` (non-space at column 3) whose key
            // begins with `/` is a path item; deeper lines (methods, parameters,
            // responses) sit inside an item and are skipped.
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end();
                    let key = key.strip_suffix(':').unwrap_or(key);
                    let mut s = key;
                    while let Some(open) = s.find('{') {
                        let Some(close) = s[open + 1..].find('}') else { break };
                        let name = &s[open + 1..open + 1 + close];
                        if !name.is_empty() {
                            out.insert(name.to_string());
                        }
                        s = &s[open + 1 + close + 1..];
                    }
                }
            }
        }
        out
    }

    /// Extract the set of path-parameter names a spec *declares* — the `name` of
    /// every parameter object marked `in: path` — without a YAML dep.
    ///
    /// A path parameter is an OpenAPI parameter object carrying `in: path`. Its
    /// `name` is a sibling key of that `in:` within the same object: a bare
    /// `name: X` at the same indentation, or — when the object is a `parameters:`
    /// sequence item whose first key is `name` — a `- name: X` opener two columns
    /// shallower. For each `in: path` line this scans the object it belongs to (up
    /// first, since CAMARA specs list `name` before `in`, then down), bounded by
    /// the object's edges — a dedent out of it, or the `- ` opener of a *different*
    /// sequence item — so an adjacent sibling parameter's `name` is never
    /// miscredited. Both the mapping form (a `components.parameters` object,
    /// `$ref`-able) and the inline sequence form are handled.
    fn declared_path_parameter_names(body: &str) -> HashSet<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // If `l` is the parameter object's `name` key relative to an `in:` key at
        // indentation `ind` — a bare `name: X` at `ind`, or a `- name: X` sequence
        // opener at `ind`-2 — return the unquoted name.
        let name_key = |l: &str, ind: usize| -> Option<String> {
            let li = indent(l);
            if li != ind && li + 2 != ind {
                return None;
            }
            let t = l.trim_start();
            let t = t.strip_prefix("- ").unwrap_or(t);
            let v = t
                .strip_prefix("name:")?
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            (!v.is_empty() && !v.contains(char::is_whitespace)).then(|| v.to_string())
        };
        let mut out = HashSet::new();
        for (i, line) in lines.iter().enumerate() {
            // An `in: path` key, whether a bare mapping key (`in: path`) or the
            // first key of a sequence item (`- in: path`).
            let bare = line.trim_start();
            let key = bare.strip_prefix("- ").unwrap_or(bare);
            if key.trim() != "in: path" {
                continue;
            }
            // Indentation of the `in:` key itself (past a `- ` opener, if any).
            let ind = indent(line) + if bare.len() != key.len() { 2 } else { 0 };
            let mut found: Option<String> = None;
            'dir: for step in [-1i64, 1] {
                let mut j = i as i64;
                loop {
                    j += step;
                    if j < 0 || j as usize >= lines.len() {
                        break;
                    }
                    let l = lines[j as usize];
                    if l.trim().is_empty() {
                        break;
                    }
                    let li = indent(l);
                    if li + 2 < ind {
                        break; // dedented out of this parameter object
                    }
                    let is_item_opener =
                        li + 2 == ind && l.trim_start().starts_with("- ");
                    // Going *down*, a `- ` opener is always the next (different)
                    // sequence item, so it bounds the object before any name check.
                    if step == 1 && is_item_opener {
                        break;
                    }
                    if let Some(n) = name_key(l, ind) {
                        found = Some(n);
                        break 'dir;
                    }
                    // Going *up*, a `- ` opener that was not this object's own
                    // `name` marks the object's start — stop before leaving it.
                    if step == -1 && is_item_opener {
                        break;
                    }
                }
            }
            if let Some(n) = found {
                out.insert(n);
            }
        }
        out
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
    fn every_spec_declares_a_valid_openapi_3_version() {
        // Contract-harness invariant (OpenAPI structural rule): every mounted
        // vendored spec MUST declare, at its root, a valid `openapi:` version of
        // the OpenAPI 3 family (`3.MINOR.PATCH`, all numeric). `openapi` is the
        // single REQUIRED root field of an OpenAPI document — the first thing every
        // Redoc/Swagger/codegen client reads to decide how to interpret the rest
        // (3.0 and 3.1 differ in `nullable`/`type` handling) — so a document
        // without it, or one declaring a Swagger `2.x` version, is not a spec this
        // simulator serves.
        //
        // This hardens the weak `bodies_are_non_empty_openapi_docs` smoke check,
        // which only asserts the body *contains* the substring `openapi:`
        // anywhere — satisfied by a prose mention inside a description, a stale
        // Swagger `2.0` header, or a malformed/truncated `openapi: 3.0` — none of
        // which is a valid served document. A newly vendored spec drafted from a
        // CAMARA template can lose or mangle its root `openapi:` line (dropped in
        // an edit, indented into a block, or copied from a 2.x source), a drift the
        // identity/wiring tests (mount-path/version/parity/operationId/oauth/
        // scenarios) never look for — they all trust the document is structurally
        // an OpenAPI 3 doc to begin with. Verified true (all `3.0.3`) across every
        // mounted spec before asserting.
        for api in APIS {
            let v = openapi_version(api.body).unwrap_or_else(|| {
                panic!(
                    "{} spec declares no root `openapi:` version — not a valid \
                     OpenAPI document (the `openapi` field is REQUIRED at the root)",
                    api.name
                )
            });
            assert!(
                is_openapi_3_version(&v),
                "{} spec declares root `openapi: {}`, which is not a valid OpenAPI 3 \
                 version (`3.MINOR.PATCH`, all numeric) — a Swagger 2.x header or a \
                 malformed/truncated version the simulator does not serve",
                api.name,
                v
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
    fn every_spec_declares_a_non_empty_info_title() {
        // Contract-harness invariant (OpenAPI structural rule): every mounted
        // vendored spec MUST declare a non-empty `info.title`. Together with
        // `info.version`, `title` is one of the two REQUIRED fields of the
        // OpenAPI `info` object — the human name every Redoc/Swagger/codegen
        // client renders as the document's heading (a spec without it renders
        // "Untitled") and the label the `/` catalog and per-spec docs pages
        // show. This completes the required-`info`-field coverage:
        // `spec_info_version_matches_mounted_url_version` pins `info.version`
        // and `every_spec_declares_a_valid_openapi_3_version` pins the root
        // `openapi:` field, but nothing yet asserts the required `info.title`.
        //
        // A newly vendored spec drafted from a CAMARA template can lose or
        // blank its `title:` (dropped in an edit, or left as an empty scalar) —
        // a drift the identity/wiring tests (mount-path/version/parity/
        // operationId/oauth/scenarios) never look for, since they all trust the
        // document is a structurally complete OpenAPI doc to begin with.
        // Verified true across every mounted spec before asserting.
        for api in APIS {
            let title = info_title(api.body).unwrap_or_else(|| {
                panic!(
                    "{} spec declares no `info.title` — not a structurally \
                     complete OpenAPI document (`title` is a REQUIRED field of \
                     the `info` object)",
                    api.name
                )
            });
            assert!(
                !title.is_empty(),
                "{} spec declares an empty `info.title` — the required document \
                 heading is blank (Redoc/Swagger would render it as \"Untitled\")",
                api.name
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
    fn local_component_refs_resolve_within_their_own_spec() {
        // Contract-harness invariant (OpenAPI 3 structural rule): every
        // *intra-document* `$ref` a mounted spec makes — one whose target is a
        // local JSON-pointer `#/components/<section>/<Name>` (empty file half) —
        // MUST point at a component that same document DEFINES. This is the
        // complementary half of `shared_error_refs_resolve_to_defined_components`:
        // that test dereferences a spec's *cross-file* pointers into the shared
        // error fragment; this one dereferences a spec's *own* local pointers
        // against its own `components:` block.
        //
        // The break this catches: a new endpoint's spec is usually drafted by
        // copy-pasting an operation (with its `$ref`s) from a sibling API, so a
        // pasted `$ref: '#/components/schemas/Foo'` can name a schema/response/
        // parameter/header this document never declares — a component renamed after
        // the copy, or one that only ever existed in the sibling. Both halves then
        // look right (correct local form, plausible name) yet resolve to nothing, so
        // any client (Redoc/Swagger/codegen) that follows the ref gets a dangling
        // pointer and the served spec is unresolvable. No existing contract test
        // sees this: the shared-error-ref test only dereferences cross-file pointers
        // (it skips local refs, whose file half is empty), and the mount-path/
        // version/parity/operationId/security tests all check a spec's identity or
        // wiring, never that its own local pointers resolve.
        //
        // Only exact `#/components/<section>/<Name>` targets are checked — every
        // local `$ref` these specs make has that 2-segment shape (a top-level
        // component object), which is exactly what `component_pointers` collects. A
        // deeper pointer into a component's internals (e.g. `.../Foo/properties/bar`)
        // would sit below that granularity, so it is skipped rather than
        // false-flagged; none exist today, and the sibling extraction-rule unit tests
        // pin the helpers so this can't pass vacuously.
        for api in APIS {
            let defined = component_pointers(api.body);
            // Non-vacuous floor: every business spec defines at least its shared
            // `x-correlator`/`XCorrelator` header or a handful of schemas, so a spec
            // that references local components must define some.
            for target in ref_targets(api.body) {
                let Some((file, pointer)) = target.split_once('#') else {
                    continue;
                };
                // Only local intra-document refs (empty file half). Cross-file refs
                // into shared/auth fragments are covered by their own tests.
                if !file.is_empty() {
                    continue;
                }
                let pointer = format!("#{pointer}");
                // Restrict to top-level component pointers `#/components/<sec>/<name>`
                // (exactly 4 slash-separated segments incl. the leading empty one),
                // the granularity `component_pointers` resolves.
                if pointer.split('/').count() != 4
                    || !pointer.starts_with("#/components/")
                {
                    continue;
                }
                assert!(
                    defined.contains(&pointer),
                    "{} spec has a local $ref `{}`, but the document defines no such \
                     component — it resolves to a dangling pointer when the spec is \
                     served. Defined components: {:?}",
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

    #[test]
    fn info_title_extraction_rules() {
        // Unit-cover the `info_title` extractor so the contract test above
        // can't pass vacuously (a broken extractor returning the same string
        // for every body would make its assertions meaningless), and so the
        // info-block scoping, quote-stripping, and blank/missing distinction
        // are pinned.

        // The direct child of `info:` is taken; a JSON-Schema `title:` inside a
        // component schema (deeper indent, outside the `info:` block) is not.
        let body = "openapi: 3.0.3\n\
                    info:\n  title: Number Verification\n  version: \"1.0.0\"\n\
                    paths: {}\n\
                    components:\n  schemas:\n    Foo:\n      title: not the info title\n";
        assert_eq!(info_title(body).as_deref(), Some("Number Verification"));
        // A quoted value is unquoted.
        assert_eq!(
            info_title("info:\n  title: \"Quoted API\"\npaths: {}\n").as_deref(),
            Some("Quoted API")
        );
        // A body whose only `title:` sits outside the `info:` block → None.
        assert_eq!(
            info_title(
                "openapi: 3.0.3\ncomponents:\n  schemas:\n    Foo:\n      title: X\n"
            ),
            None
        );
        // A present-but-blank `title:` returns Some("") (the contract test
        // rejects it), a distinct case from a missing line (None).
        assert_eq!(info_title("info:\n  title:\npaths: {}\n").as_deref(), Some(""));

        // Non-vacuous floor: every registered spec declares a non-empty
        // info.title, so a broken extractor can't hide behind an empty loop.
        for api in APIS {
            let t = info_title(api.body).expect("registered spec has an info.title");
            assert!(!t.is_empty(), "{}", api.name);
        }
    }

    #[test]
    fn openapi_version_extraction_and_validation_rules() {
        // Unit-cover the `openapi_version` extractor and the `is_openapi_3_version`
        // validator so the contract test above can't pass vacuously (an extractor
        // that returned the same string for every body, or a validator that
        // accepted everything, would make its assertions meaningless), and so the
        // top-level-key / prose discrimination and the version-shape rules are pinned.

        // Extraction: the root `openapi:` key is taken; an indented `openapi:`
        // mention in a description block scalar (non-zero column) is not.
        let body = "openapi: 3.0.3\n\
                    info:\n  title: X\n  description: |\n    an openapi: 2.0 mention inside prose\n  version: \"1.0.0\"\n\
                    paths: {}\n";
        assert_eq!(openapi_version(body).as_deref(), Some("3.0.3"));
        // A quoted value is unquoted.
        assert_eq!(
            openapi_version("openapi: \"3.1.0\"\npaths: {}\n").as_deref(),
            Some("3.1.0")
        );
        // No *root* `openapi:` line (only an indented child of `info:`) → None.
        assert_eq!(openapi_version("info:\n  openapi: 3.0.3\npaths: {}\n"), None);

        // Validation: the OpenAPI 3 family is accepted; a Swagger 2.x header, a
        // truncated two-part version, an over-long version, and non-numeric or
        // malformed values are rejected.
        assert!(is_openapi_3_version("3.0.3"));
        assert!(is_openapi_3_version("3.1.0"));
        assert!(!is_openapi_3_version("2.0"));
        assert!(!is_openapi_3_version("2.0.0"));
        assert!(!is_openapi_3_version("3.0"));
        assert!(!is_openapi_3_version("3.0.3.1"));
        assert!(!is_openapi_3_version("3.0.x"));
        assert!(!is_openapi_3_version("3..0"));
        assert!(!is_openapi_3_version(""));

        // Non-vacuous floor: every registered spec declares a valid OpenAPI 3 root
        // version, so a broken extractor/validator can't hide behind an empty loop.
        for api in APIS {
            let v = openapi_version(api.body)
                .expect("registered spec has a root openapi version");
            assert!(is_openapi_3_version(&v), "{}: {}", api.name, v);
        }
    }

    #[test]
    fn path_template_params_match_declared_path_parameters() {
        // Contract-harness invariant (OpenAPI path templating): every `{name}` a
        // spec puts in a `paths:` key MUST be declared as an `in: path` parameter,
        // and — conversely — every `in: path` parameter a spec declares MUST appear
        // in some path template. Both halves are OpenAPI structural rules: an
        // undeclared path template variable, or a path parameter that templates no
        // path, is an invalid document (a client/codegen tool can't bind the URL
        // variable to a parameter, or is handed a parameter with nowhere to go).
        // This is a live copy-paste hazard — a new endpoint's spec is drafted from
        // a sibling, so a pasted path block can keep the sibling's `{sessionId}`
        // template while its operation declares a `paymentId` path parameter (or a
        // path is renamed but its parameter is not) — a mismatch no existing
        // contract test sees: the mount-path/version/parity/operationId/`$ref`
        // tests all check a spec's identity or wiring, never that its path
        // *variables* line up with its path *parameters*. Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let templated = path_template_params(api.body);
            let declared = declared_path_parameter_names(api.body);
            for name in &templated {
                assert!(
                    declared.contains(name),
                    "{} spec templates path variable `{{{}}}` but declares no \
                     matching `in: path` parameter (an undeclared path variable)",
                    api.name,
                    name
                );
            }
            for name in &declared {
                assert!(
                    templated.contains(name),
                    "{} spec declares an `in: path` parameter `{}` that appears in \
                     no path template (a path parameter templating nothing)",
                    api.name,
                    name
                );
            }
        }
    }

    #[test]
    fn path_parameter_extraction_rules() {
        // Unit-cover the `path_template_params` and `declared_path_parameter_names`
        // extractors so the contract test above can't pass vacuously (extractors
        // that returned the same set for every body, or empty sets, would make its
        // assertions meaningless), and so the scoping/indentation rules are pinned:
        // a `{…}` in prose is not a path variable; `in: path` is credited its own
        // object's `name` (name-first and in-first order, dash-sequence and bare
        // mapping forms) and never an adjacent sibling's; an `in: query` parameter
        // is not a path parameter.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
  description: >
    A path like /foo/{notAParam} mentioned in prose must be ignored.
paths:
  /sessions:
    post:
      operationId: create
  /sessions/{sessionId}:
    get:
      operationId: get
      parameters:
        - name: sessionId
          in: path
          required: true
          schema:
            type: string
        - name: fields
          in: query
  /items/{itemId}/tags/{tagId}:
    get:
      operationId: getTag
      parameters:
        - in: path
          name: itemId
        - name: tagId
          in: path
components:
  parameters:
    Legacy:
      name: legacyId
      in: path
";
        let want = |names: &[&str]| {
            names.iter().map(|s| s.to_string()).collect::<HashSet<_>>()
        };

        // Templated variables come only from `paths:` keys, across single- and
        // multi-variable paths; the prose `{notAParam}` under `info:` is excluded.
        let tp = path_template_params(body);
        assert_eq!(tp, want(&["sessionId", "itemId", "tagId"]));
        assert!(!tp.contains("notAParam"));

        // Declared path parameters: `sessionId` (dash item, name-first), `itemId`
        // (dash item, in-first), `tagId` (dash item, name-first), `legacyId` (a
        // bare `components.parameters` mapping object) — all `in: path`. `fields`
        // is `in: query`, so it is not collected; the schema property depth under
        // `sessionId` never leaks a stray name.
        let dp = declared_path_parameter_names(body);
        assert_eq!(dp, want(&["sessionId", "itemId", "tagId", "legacyId"]));
        assert!(!dp.contains("fields"));

        // Non-vacuous floor: across every registered spec, each templated variable
        // is a declared path parameter and vice versa (the invariant the contract
        // test asserts), so a broken extractor can't hide behind an empty loop.
        for api in APIS {
            let templated = path_template_params(api.body);
            let declared = declared_path_parameter_names(api.body);
            assert_eq!(
                templated, declared,
                "{}: path template variables and declared path parameters must match",
                api.name
            );
        }
    }
}
