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

    /// Whether an embedded OpenAPI body declares a **non-empty** `info.description`,
    /// without a YAML dep. Returns `None` when the `info` object carries no
    /// `description:` field at all, `Some(false)` when the field is present but
    /// empty (an empty inline scalar, or a block scalar with no indented body),
    /// and `Some(true)` when it has content — mirroring the missing/blank/present
    /// trichotomy `info_title` exposes for the title.
    ///
    /// `info.description` is the OpenAPI `info` object's RECOMMENDED overview
    /// field: the CommonMark prose every Redoc/Swagger client renders as the
    /// API's introduction, and where each CAMARA spec documents the API's
    /// purpose, its two/three-legged auth model, and (per docs/DESIGN §7) its
    /// parameter-driven functional cases in human-readable form. Every CamaraSim
    /// vendored spec writes it as a literal block scalar (`description: |`), so an
    /// empty one would render the served docs page with a blank overview.
    ///
    /// Scoping mirrors `info_title`: only a line at exactly the 2-space indent of
    /// an `info:` direct child is considered, so a deeper schema `description:`
    /// (a component-property field, always more than 2 spaces in) is never
    /// mistaken for it. Content-detection understands both an inline scalar
    /// (`description: text`) and a block scalar (`description: |`/`>`, whose body
    /// is any following non-blank line indented deeper than the key).
    fn info_description_present(body: &str) -> Option<bool> {
        let lines: Vec<&str> = body.lines().collect();
        let mut in_info = false;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_info = line.trim_end() == "info:";
                continue;
            }
            if !in_info {
                continue;
            }
            let rest = match line.strip_prefix("  description:") {
                Some(r) => r,
                None => continue,
            };
            let inline = rest.trim().trim_matches('"').trim_matches('\'');
            // A `|`/`>` (with any chomping/indent indicator) opens a block scalar;
            // otherwise the trimmed remainder is the inline value itself.
            let is_block = matches!(inline.chars().next(), Some('|') | Some('>'));
            if !is_block {
                return Some(!inline.is_empty());
            }
            // Block scalar: its body is indented deeper than the 2-space key. The
            // first non-blank line indented > 2 spaces is content; the first
            // non-blank line at indent <= 2 (a sibling `info` field) ends it.
            for l in &lines[i + 1..] {
                if l.trim().is_empty() {
                    continue;
                }
                let indent = l.len() - l.trim_start().len();
                return Some(indent > 2);
            }
            return Some(false);
        }
        None
    }

    /// Extract `info.license.name` from an embedded OpenAPI body, without a YAML
    /// dep, distinguishing a missing `license` from a present one with no `name`.
    ///
    /// The `license` field of the OpenAPI `info` object is OPTIONAL, but when
    /// present the License Object's `name` is its single REQUIRED field — the
    /// human licence label (`Apache-2.0`) every Redoc/Swagger/codegen client reads
    /// and the served `/{api}/v{n}/docs` page renders. Every CamaraSim vendored
    /// spec carries the CAMARA-template `license: { name: Apache-2.0, url: … }`.
    ///
    /// Returns the missing/no-name/present trichotomy — mirroring the shape
    /// `info_description_present` exposes for the description:
    ///   * `None` — the `info` object declares no `license:` field at all.
    ///   * `Some(None)` — `info.license` is present but declares no `name:` child
    ///     (an invalid License Object).
    ///   * `Some(Some(name))` — `info.license.name` is present; `name` is its
    ///     unquoted scalar (possibly empty, which the contract test rejects).
    ///
    /// Scoping mirrors `info_title`: only the top-level `info:` block is scanned,
    /// `license:` is matched at exactly its 2-space direct-child indent and `name:`
    /// at exactly the 4-space grandchild indent, so a deeper schema `license:`/
    /// `name:` (a component property, always more than 2/4 spaces in) is never
    /// mistaken for it. A following non-blank line at indent ≤ 2 (a sibling `info`
    /// field) ends the licence block, so a `name:` outside it is not credited.
    fn info_license_name(body: &str) -> Option<Option<String>> {
        let lines: Vec<&str> = body.lines().collect();
        let mut in_info = false;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_info = line.trim_end() == "info:";
                continue;
            }
            if !in_info {
                continue;
            }
            // `license:` is a direct child of `info:` at exactly 2 spaces.
            if line.strip_prefix("  license:").is_none() {
                continue;
            }
            // Scan the licence block for its `name:` grandchild (4-space indent),
            // stopping at the next non-blank line indented ≤ 2 (a sibling `info`
            // field), which ends the block.
            for l in &lines[i + 1..] {
                if l.trim().is_empty() {
                    continue;
                }
                let indent = l.len() - l.trim_start().len();
                if indent <= 2 {
                    break;
                }
                if let Some(rest) = l.strip_prefix("    name:") {
                    let v = rest.trim().trim_matches('"').trim_matches('\'');
                    return Some(Some(v.to_string()));
                }
            }
            return Some(None);
        }
        None
    }

    /// Extract every URL-template variable a spec's `servers[].url` references but
    /// does **not** back with a Server Variable Object carrying a non-empty
    /// `default:` — without a YAML dep.
    ///
    /// An OpenAPI Server Object's `url` MAY be a template containing `{name}`
    /// placeholders; each such placeholder MUST be declared in that server's
    /// `variables:` map, and a Server Variable Object's single REQUIRED field is
    /// `default` (the value substituted when a client supplies none). CamaraSim's
    /// every vendored spec templates its base path as `{apiRoot}/…` and declares
    /// `variables: { apiRoot: { default: http://localhost:8080, … } }`, so the
    /// served `/{api}/v{n}/docs` "try it" URL and every codegen client can build a
    /// concrete request URL. A `url` that names an undeclared — or `default`-less —
    /// variable is an invalid Server Object whose substituted URL keeps a literal
    /// `{var}`.
    ///
    /// Returns the (sorted, de-duplicated) set of offending variable names: those
    /// named in a `{…}` inside a server `url:` line yet not matched by a
    /// `variables:` entry declaring a non-empty `default:`. A well-formed spec
    /// returns an empty vec.
    ///
    /// Scoping: only the top-level `servers:` block is scanned (a line == `servers:`
    /// at column zero, through the next column-zero key), so a deeper `url:`/
    /// `variables:`/`default:` (a schema example, a description mention) — always
    /// more indented — is never mistaken for it. References and definitions are
    /// gathered set-wise across the whole block: CamaraSim specs each declare a
    /// single server, so a per-server association is unnecessary (documented
    /// simplification). Only `url:` lines (optionally under a `- ` sequence dash)
    /// contribute references, so a `{…}` in a sibling `description:` is not read as
    /// a template variable.
    fn server_url_undefined_variables(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();

        // Isolate the top-level `servers:` block.
        let start = match lines.iter().position(|l| *l == "servers:") {
            Some(s) => s,
            None => return Vec::new(),
        };
        let end = lines[start + 1..]
            .iter()
            .position(|l| !l.is_empty() && !l.starts_with(char::is_whitespace))
            .map(|off| start + 1 + off)
            .unwrap_or(lines.len());
        let block = &lines[start + 1..end];

        // Template variables named in `{…}` within any server `url:` line.
        fn brace_vars(s: &str) -> Vec<String> {
            let mut out = Vec::new();
            let mut rest = s;
            while let Some(open) = rest.find('{') {
                let after = &rest[open + 1..];
                match after.find('}') {
                    Some(close) => {
                        let name = &after[..close];
                        if !name.is_empty() {
                            out.push(name.to_string());
                        }
                        rest = &after[close + 1..];
                    }
                    None => break,
                }
            }
            out
        }
        let mut referenced: Vec<String> = Vec::new();
        for l in block {
            let t = l.trim_start();
            let t = t.strip_prefix("- ").unwrap_or(t);
            if let Some(url) = t.strip_prefix("url:") {
                referenced.extend(brace_vars(url));
            }
        }

        // Variable names declared with a non-empty `default:` under a `variables:`
        // mapping anywhere in the block.
        let mut defined: Vec<String> = Vec::new();
        let mut k = 0;
        while k < block.len() {
            if block[k].trim() != "variables:" {
                k += 1;
                continue;
            }
            let v_indent = block[k].len() - block[k].trim_start().len();
            let mut child_indent: Option<usize> = None;
            let mut m = k + 1;
            while m < block.len() {
                let l = block[m];
                if l.trim().is_empty() {
                    m += 1;
                    continue;
                }
                let indent = l.len() - l.trim_start().len();
                if indent <= v_indent {
                    break; // end of the `variables:` mapping
                }
                let ci = *child_indent.get_or_insert(indent);
                if indent == ci {
                    // A direct-child key of `variables:` is a variable name.
                    let name = l.trim().split_once(':').map(|(k, _)| k.trim()).unwrap_or("");
                    // Scan this variable's own sub-block for a non-empty `default:`.
                    let mut has_default = false;
                    let mut n = m + 1;
                    while n < block.len() {
                        let ll = block[n];
                        if ll.trim().is_empty() {
                            n += 1;
                            continue;
                        }
                        let ind = ll.len() - ll.trim_start().len();
                        if ind <= ci {
                            break;
                        }
                        if let Some(rest) = ll.trim().strip_prefix("default:") {
                            let val = rest.trim().trim_matches('"').trim_matches('\'');
                            if !val.is_empty() {
                                has_default = true;
                            }
                        }
                        n += 1;
                    }
                    if has_default && !name.is_empty() {
                        defined.push(name.to_string());
                    }
                }
                m += 1;
            }
            k = m;
        }

        referenced.retain(|r| !defined.contains(r));
        referenced.sort();
        referenced.dedup();
        referenced
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

    /// Return every `$ref` target in an embedded OpenAPI body that carries **no**
    /// `#/` JSON-pointer fragment, in document order, without a YAML dep.
    ///
    /// Every reference these specs make is a *pointer* into a component: a local
    /// `#/components/…` or a cross-file `<relative-path>#/components/…`. The `#/…`
    /// fragment is the half a client (Redoc/Swagger/codegen) dereferences to reach
    /// the schema/response/parameter; a target that lost it — a bare component name
    /// (`CamaraError`) or a whole-file path (`errors.yaml`) — points at a document
    /// root, never the intended component, so the served spec is unresolvable.
    ///
    /// Built on [`ref_targets`] (already unit-covered), keeping only the targets
    /// missing a `#/` fragment.
    fn refs_missing_fragment(body: &str) -> Vec<String> {
        ref_targets(body)
            .into_iter()
            .filter(|t| !t.contains("#/"))
            .collect()
    }

    /// Return every `$ref` reference object in an embedded OpenAPI body that carries
    /// a **sibling key** in its own mapping — reported as `"<target> (sibling:
    /// <key>)"` in document order, without a YAML dep.
    ///
    /// In OpenAPI 3.0.x a `$ref` is a Reference Object whose members "other than
    /// `$ref` SHALL be ignored". So a schema written as
    /// ```text
    ///   center:
    ///     $ref: "#/components/schemas/Point"
    ///     description: The centre of a CIRCLE area.
    /// ```
    /// silently drops the `description` — the annotation the author meant to attach
    /// never renders (Redoc/Swagger/codegen honour only the referenced schema), a
    /// lost-intent bug no sibling test sees: the fragment / canonical-path /
    /// resolution ref tests inspect a ref's *target* (its shape and what it points
    /// at), never whether the ref object stands alone. The canonical 3.0.x way to
    /// annotate a reference is to wrap it (`allOf:` with a single `- $ref` item, plus
    /// the sibling), which moves the `$ref` into its own item so it stands alone.
    ///
    /// For each `$ref` key — a bare mapping key (`$ref:`) or the first key of a `- `
    /// sequence item (`- $ref:`) — at effective indent `ind`, the enclosing mapping's
    /// other keys sit at indent exactly `ind`. The scan walks both directions,
    /// bounded by a dedent (`indent < ind`, out of the mapping) and by the next `- `
    /// sequence item, and flags the first sibling key it meets:
    /// - a bare `$ref:` may have siblings above **and** below, and — when it is a
    ///   non-first key of a `- ` item — the opener line at `ind - 2` carries this
    ///   same item's first key (a genuine sibling), so an upward scan reads it;
    /// - a sequence-form `- $ref:` is the first line of its item, so no key precedes
    ///   it *within* the item; only its downward keys are siblings, and a following
    ///   `- ` at `ind - 2` is the next item (a boundary, never a sibling).
    ///
    /// A key indented past `ind` is nested inside a sibling's subtree, not a sibling
    /// of the `$ref`, so it is never counted; the `$ref` itself is never mistaken for
    /// a sibling of another `$ref`.
    fn refs_with_sibling_keys(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The key name a line declares at effective indent `ind` — a bare `key:` at
        // column `ind`, or the first key of a `- ` sequence item whose content starts
        // at `ind` — if any, never a `$ref` (that is a reference, not a sibling).
        let key_at = |l: &str, ind: usize| -> Option<String> {
            let li = indent(l);
            let bare = l.trim_start();
            let (kcol, content) = match bare.strip_prefix("- ") {
                Some(rest) => (li + 2, rest),
                None => (li, bare),
            };
            if kcol != ind {
                return None;
            }
            let name = content.split(':').next().unwrap_or("").trim();
            (!name.is_empty() && name != "$ref").then(|| name.to_string())
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let bare = line.trim_start();
            let after_dash = bare.strip_prefix("- ").unwrap_or(bare);
            let is_seq = after_dash.len() != bare.len();
            let Some(rest) = after_dash.strip_prefix("$ref:") else {
                continue;
            };
            let target = rest.trim().trim_matches('"').trim_matches('\'');
            let ind = indent(line) + if is_seq { 2 } else { 0 };
            let mut sibling: Option<String> = None;
            'dir: for step in [-1i64, 1] {
                // A sequence-form `$ref` is the first line of its item, so nothing
                // above it belongs to the same mapping — skip the upward scan.
                if step == -1 && is_seq {
                    continue;
                }
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
                    let opener = l.trim_start().starts_with("- ");
                    if li < ind {
                        // Dedented out of the mapping. The one exception: scanning
                        // *up* from a bare `$ref`, a `- ` opener at exactly `ind - 2`
                        // is this item's own start and carries its first key at
                        // effective indent `ind` — a genuine sibling.
                        if step == -1 && !is_seq && li + 2 == ind && opener {
                            if let Some(k) = key_at(l, ind) {
                                sibling = Some(k);
                                break 'dir;
                            }
                        }
                        break;
                    }
                    // Going *down*, a `- ` opener at `ind` is the next sequence
                    // element — the current item's mapping has ended.
                    if step == 1 && li == ind && opener {
                        break;
                    }
                    if let Some(k) = key_at(l, ind) {
                        sibling = Some(k);
                        break 'dir;
                    }
                    // li > ind: nested inside a sibling's subtree — keep scanning.
                }
            }
            if let Some(s) = sibling {
                out.push(format!("{target} (sibling: {s})"));
            }
        }
        out
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

    /// Extract every component whose *key* — its name under a `components.<section>:`
    /// map — violates the OpenAPI 3 Components Object key rule, returned as
    /// `#/components/<section>/<name>` in document order.
    ///
    /// OpenAPI 3 requires every key of a `components` sub-object (schemas,
    /// responses, parameters, examples, requestBodies, headers, securitySchemes,
    /// links, callbacks) to match `^[a-zA-Z0-9._-]+$`; a key bearing any other
    /// character (a space, `/`, `#`) is an invalid document that can never be
    /// legally `$ref`'d, because a JSON Pointer built from it doesn't resolve.
    /// Scopes exactly like `component_pointers` (top-level `components:` → a
    /// 2-space section key → an exact-4-space component key), but — unlike it,
    /// which drops a whitespace-bearing name — keeps *every* component key so an
    /// invalid one is surfaced rather than silently ignored. Strips a matching
    /// pair of surrounding quotes before validating (a quoted key's logical name
    /// still must match).
    fn components_with_invalid_names(body: &str) -> Vec<String> {
        fn is_valid_component_name(name: &str) -> bool {
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        }
        let mut out = Vec::new();
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
            // A 2-space direct child of `components:` opens a section.
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
            // An exact-4-space child under a section is a component definition key.
            if let Some(section) = &section {
                if let Some(rest) = line.strip_prefix("    ") {
                    if !rest.starts_with(char::is_whitespace) {
                        if let Some(name) = rest.trim_end().strip_suffix(':') {
                            let unquoted = name
                                .strip_prefix('"')
                                .and_then(|n| n.strip_suffix('"'))
                                .or_else(|| {
                                    name.strip_prefix('\'')
                                        .and_then(|n| n.strip_suffix('\''))
                                })
                                .unwrap_or(name);
                            if !is_valid_component_name(unquoted) {
                                out.push(format!("#/components/{section}/{name}"));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// Extract every direct child *section* key of a top-level `components:` object,
    /// in document order (e.g. `schemas`, `responses`, `securitySchemes`).
    ///
    /// Scopes the scan exactly like `component_pointers` / `components_with_invalid_
    /// names` — a top-level `components:` block → its 2-space direct-child keys (a
    /// non-space at column 3; deeper 4-space+ lines are component definitions/bodies)
    /// — but returns the *section* names themselves rather than the component keys
    /// nested under them. A quoted key is unquoted; a 2-space child bearing an inline
    /// scalar (no trailing `:`) or internal whitespace opens no map section and is
    /// skipped, mirroring how the sibling extractors recognise a section.
    fn components_section_names(body: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut in_components = false;
        for line in body.lines() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_components = line.trim_end() == "components:";
                continue;
            }
            if !in_components {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) {
                    if let Some(name) = rest.trim_end().strip_suffix(':') {
                        let unquoted = name
                            .strip_prefix('"')
                            .and_then(|n| n.strip_suffix('"'))
                            .or_else(|| {
                                name.strip_prefix('\'')
                                    .and_then(|n| n.strip_suffix('\''))
                            })
                            .unwrap_or(name);
                        if !unquoted.is_empty() && !unquoted.contains(char::is_whitespace)
                        {
                            out.push(unquoted.to_string());
                        }
                    }
                }
            }
        }
        out
    }

    /// Extract every `components` section key whose name is not a valid OpenAPI 3
    /// Components Object field, returned in document order.
    ///
    /// The Components Object holds a fixed set of named maps — `schemas`,
    /// `responses`, `parameters`, `examples`, `requestBodies`, `headers`,
    /// `securitySchemes`, `links`, `callbacks` (plus `pathItems` in 3.1) — and, like
    /// every object in the document, permits `x-` Specification Extensions. A 2-space
    /// child of `components:` bearing any other name (a typo'd `shemas:`, a
    /// Swagger-2.0 `definitions:` pasted from an old template) is an invalid section:
    /// every component nested under it is unreachable, because a `$ref` addresses a
    /// component only through the canonical `#/components/<field>/<Name>` path.
    /// Filters `components_section_names` by the fixed field set.
    fn components_with_invalid_section_names(body: &str) -> Vec<String> {
        fn is_valid_section(name: &str) -> bool {
            matches!(
                name,
                "schemas"
                    | "responses"
                    | "parameters"
                    | "examples"
                    | "requestBodies"
                    | "headers"
                    | "securitySchemes"
                    | "links"
                    | "callbacks"
                    | "pathItems"
            ) || name.starts_with("x-")
        }
        components_section_names(body)
            .into_iter()
            .filter(|name| !is_valid_section(name))
            .collect()
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

    /// The `METHOD /path` label of every operation a spec declares whose
    /// `security` requirement names a scheme but lists **no scope** — the empty
    /// forms `- openId: []` (inline empty flow sequence) and a `- openId:` with no
    /// `- <scope>` items beneath it (empty block) — in document order, without a
    /// YAML dep.
    ///
    /// Mirrors [`operations_without_operation_id`]'s scoping (a 4-space HTTP-verb
    /// key under a 2-space `/…` path item beneath the top-level `paths:` block).
    /// Within an operation it finds the 6-space `security:` block and, for each
    /// requirement sequence item that is a scheme mapping key (via
    /// [`requirement_scheme_name`], so a scope scalar like
    /// `- number-verification:verify` is skipped), decides whether the scheme
    /// carries at least one scope: an inline value is a flow sequence (`[]` →
    /// empty, `[a]`/`[a, b]` → non-empty; any other inline scalar is treated as
    /// non-empty, defensively), while an empty inline value means the block form,
    /// whose scopes are the `- <scope>` items indented under the requirement item.
    /// An operation with any scopeless scheme requirement is flagged.
    fn operations_with_scopeless_security(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            if indent(line) != 4 {
                continue;
            }
            let key = line.trim_start();
            let Some(name) = key.strip_suffix(':') else { continue };
            if name.contains(char::is_whitespace) || !METHODS.contains(&name) {
                continue;
            }
            // Scan the operation's block (indent > 4) for a 6-space `security:` key,
            // then inspect each scheme requirement in it for a scope.
            let mut has_scopeless = false;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) <= 4 {
                    break; // dedented out of this operation
                }
                let is_security_key = indent(l) == 6
                    && l.trim_start().split_once(':').map(|(k, _)| k) == Some("security");
                if !is_security_key {
                    j += 1;
                    continue;
                }
                // Walk the `security:` block: lines indented past the 6-space key,
                // until a dedent to at-or-above it ends the block.
                let mut k = j + 1;
                while k < lines.len() {
                    let sl = lines[k];
                    if sl.trim().is_empty() {
                        k += 1;
                        continue;
                    }
                    let sind = indent(sl);
                    if sind <= 6 {
                        break;
                    }
                    if requirement_scheme_name(sl).is_some() {
                        let rest = sl.trim_start().strip_prefix("- ").unwrap_or(sl.trim_start());
                        let after = rest[rest.find(':').unwrap() + 1..].trim();
                        let scopeless = if after.is_empty() {
                            // Block form: any `- <scope>` item indented past this
                            // requirement item before the next dedent?
                            let mut m = k + 1;
                            let mut has_scope = false;
                            while m < lines.len() {
                                let ml = lines[m];
                                if ml.trim().is_empty() {
                                    m += 1;
                                    continue;
                                }
                                if indent(ml) <= sind {
                                    break;
                                }
                                if ml.trim_start().starts_with("- ") {
                                    has_scope = true;
                                    break;
                                }
                                m += 1;
                            }
                            !has_scope
                        } else {
                            // Inline flow sequence: empty only when `[]` (or `[ ]`).
                            match after.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                                Some(inner) => inner.trim().is_empty(),
                                None => false,
                            }
                        };
                        if scopeless {
                            has_scopeless = true;
                        }
                    }
                    k += 1;
                }
                break; // one `security:` block per operation
            }
            if has_scopeless {
                out.push(format!("{} {}", name.to_uppercase(), current_path));
            }
        }
        out
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

    /// Extract the key of every path item a spec declares under `paths:` — the
    /// full path-template string (e.g. `/sessions/{sessionId}`) — without a YAML
    /// dep.
    ///
    /// A path item is a 2-space *direct child* of the top-level `paths:` block
    /// (non-space at column 3); deeper lines (methods, parameters, responses)
    /// sit inside an item and are skipped, and a `/`-looking key elsewhere (a
    /// schema property, a description) is not under `paths:` so is never seen.
    /// OpenAPI also permits `x-` specification extensions as direct children of
    /// the Paths Object; such a key is not a Path Item (its value need not be a
    /// slash-prefixed template) and is excluded. A quoted key
    /// (`"/foo":`) is unquoted so the returned template is the bare path.
    /// Mirrors `path_template_params`' `paths:`-scoping.
    fn path_item_keys(body: &str) -> Vec<String> {
        let mut out = Vec::new();
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
            if let Some(rest) = line.strip_prefix("  ") {
                if rest.starts_with(char::is_whitespace) {
                    continue; // deeper than a direct child of `paths:`
                }
                let key = rest.trim_end();
                let key = key.strip_suffix(':').unwrap_or(key);
                let key = key.trim_matches('"').trim_matches('\'');
                // An `x-` Paths-Object extension is not a Path Item; exclude it.
                if key.is_empty() || key.starts_with("x-") {
                    continue;
                }
                out.push(key.to_string());
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

    /// Extract a label for every parameter object marked `in: path` that is
    /// **missing a `required: true`** declaration — without a YAML dep.
    ///
    /// OpenAPI makes `required` OPTIONAL on a Parameter Object in general, but for
    /// a path parameter it is REQUIRED and its value MUST be `true` (a path
    /// template variable is never omissible). A path parameter with no `required:`
    /// key, or one set to `false`, is therefore an invalid document. This mirrors
    /// `declared_path_parameter_names`' object scan: for each `in: path` key (a
    /// bare mapping key or the first key of a `- ` sequence item) at effective
    /// indentation `ind`, it scans that parameter object's own sibling keys (at
    /// indentation `ind`, bounded by a dedent out of the object) for a `required:`
    /// key and reads its value, and — for a useful label — the object's `name` the
    /// same way. A key indented past `ind` is a nested child (e.g. a `schema:`
    /// subtree), so a `required: true` inside a sub-schema never satisfies the
    /// parameter's own requirement.
    fn path_parameters_missing_required_true(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let bare = line.trim_start();
            let key = bare.strip_prefix("- ").unwrap_or(bare);
            if key.trim() != "in: path" {
                continue;
            }
            // Indentation of the `in:` key itself (past a `- ` opener, if any).
            let ind = indent(line) + if bare.len() != key.len() { 2 } else { 0 };
            let mut required_true = false;
            let mut name: Option<String> = None;
            for step in [-1i64, 1] {
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
                    if li < ind {
                        // Dedented out of this parameter object. A `- name: X`
                        // sequence-item opener sits at `ind`-2 and carries the
                        // object's name in its first key — capture it for the label
                        // before leaving.
                        if li + 2 == ind {
                            if let Some(rest) = l.trim_start().strip_prefix("- ") {
                                if let Some(v) = rest.strip_prefix("name:") {
                                    let v = v.trim().trim_matches('"').trim_matches('\'');
                                    if !v.is_empty() && name.is_none() {
                                        name = Some(v.to_string());
                                    }
                                }
                            }
                        }
                        break;
                    }
                    if li != ind {
                        continue; // a nested child (e.g. a `schema:` subtree)
                    }
                    let t = l.trim_start().strip_prefix("- ").unwrap_or(l.trim_start());
                    if let Some(v) = t.strip_prefix("required:") {
                        if v.trim() == "true" {
                            required_true = true;
                        }
                    }
                    if let Some(v) = t.strip_prefix("name:") {
                        let v = v.trim().trim_matches('"').trim_matches('\'');
                        if !v.is_empty() {
                            name = Some(v.to_string());
                        }
                    }
                }
            }
            if !required_true {
                out.push(name.unwrap_or_else(|| format!("<unnamed>@line {}", i + 1)));
            }
        }
        out
    }

    /// Enumerate every parameter `in:` value a spec declares that is **not** a
    /// valid OpenAPI 3 parameter location, as `"<value>@line N"` (document
    /// order, 1-based line), without a YAML dep.
    ///
    /// `in` is a REQUIRED field of an OpenAPI Parameter Object and its value
    /// MUST be one of the fixed enum `query` / `header` / `path` / `cookie` —
    /// where the parameter is carried. Anything else is an invalid document a
    /// Redoc/Swagger/codegen client rejects (it has no location to bind the
    /// parameter to). The break it catches is a migration / copy-paste hazard
    /// the sibling path-parameter tests can't see (they only ever look at
    /// `in: path`): a Swagger-2.0 parameter location removed in OpenAPI 3 —
    /// `in: body` / `in: formData` (a request body became `requestBody`, form
    /// fields became a `content` schema) — pasted from an old template, or a
    /// location scalar typo'd (`in: quiery`).
    ///
    /// Detection mirrors the trusted `in: path` scan in
    /// [`declared_path_parameter_names`]: a parameter `in` is a mapping key
    /// (`in: path`) or the first key of a `- ` sequence item (`- in: path`),
    /// and always carries its value inline. A line is taken as a
    /// parameter-location declaration when — after trimming leading whitespace
    /// and an optional `- ` opener — it begins with the exact `in:` key and has
    /// a non-empty inline scalar. An `in:` with no inline value opens a nested
    /// block (e.g. a schema property named `in`), which is not a parameter
    /// location, so it is skipped; `info:` and other keys sharing the `in`
    /// prefix don't match the exact `in:` key. Quotes around the value are
    /// stripped before the enum check.
    fn parameters_with_invalid_location(body: &str) -> Vec<String> {
        const LOCATIONS: [&str; 4] = ["query", "header", "path", "cookie"];
        let mut out = Vec::new();
        for (i, line) in body.lines().enumerate() {
            let bare = line.trim_start();
            let key = bare.strip_prefix("- ").unwrap_or(bare);
            let Some(rest) = key.strip_prefix("in:") else { continue };
            let v = rest.trim().trim_matches('"').trim_matches('\'');
            // An `in:` with no inline scalar opens a nested block — not a
            // parameter location.
            if v.is_empty() {
                continue;
            }
            if !LOCATIONS.contains(&v) {
                out.push(format!("{}@line {}", v, i + 1));
            }
        }
        out
    }

    /// Enumerate every parameter object a spec declares that is **missing a
    /// `name`** — labelled `"<in>@line N"` (document order, 1-based line of its
    /// `in:` anchor) — without a YAML dep.
    ///
    /// `name` and `in` are the two REQUIRED fields of an OpenAPI Parameter
    /// Object: `in` says where the parameter is carried, `name` says which one.
    /// The sibling [`parameters_with_invalid_location`] pins the `in` half (every
    /// parameter's location is a valid enum); this pins the other half (every
    /// located parameter also names itself). A parameter object with no `name` is
    /// an invalid document — a Redoc/Swagger/codegen client is handed a slot with
    /// a location but no identity, so it can't bind or generate it — and a live
    /// copy-paste hazard: a parameter block pasted from a sibling can lose or
    /// dedent its `name:` line while keeping its `in:`, a break no other contract
    /// test sees (the location test only checks the `in` value; the path-parameter
    /// tests only line up `in: path` variables by name they *assume* present; the
    /// responses/operationId/`$ref` tests never look at a parameter's identity).
    ///
    /// Detection anchors on a parameter's `in:` location line — a mapping key
    /// (`in: path`) or the first key of a `- ` sequence item (`- in: query`)
    /// whose inline scalar is one of `query`/`header`/`path`/`cookie` — then
    /// scans that same parameter object (its sibling keys at the `in:` key's own
    /// indentation `ind`, plus the `- name:` sequence opener at `ind`-2, bounded
    /// by a dedent out of the object) for a `name:` key, mirroring the object scan
    /// in [`path_parameters_missing_required_true`]. A `name:` indented past `ind`
    /// is a nested child (e.g. a `schema:` property literally named `name`), so it
    /// never satisfies the parameter's own requirement. A `$ref` parameter
    /// (`- $ref: …`) carries no inline `in`, so it is never anchored and is thus
    /// exempt — it inherits `name`/`in` from the referenced component.
    fn parameters_missing_name(body: &str) -> Vec<String> {
        const LOCATIONS: [&str; 4] = ["query", "header", "path", "cookie"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let bare = line.trim_start();
            let key = bare.strip_prefix("- ").unwrap_or(bare);
            let Some(rest) = key.strip_prefix("in:") else { continue };
            let loc = rest.trim().trim_matches('"').trim_matches('\'');
            if !LOCATIONS.contains(&loc) {
                continue;
            }
            // Indentation of the `in:` key itself (past a `- ` opener, if any).
            let is_seq_opener = bare.len() != key.len();
            let ind = indent(line) + if is_seq_opener { 2 } else { 0 };
            let mut has_name = false;
            // When the anchor line is itself the `- ` sequence opener (in-first,
            // `- in: query`), the object starts here — it has no sibling keys
            // *above* the anchor, and any same-indent lines above belong to the
            // previous sibling parameter — so scan downward only. For a mapping
            // key or a name-first sequence item, the object's opener sits above at
            // `ind`-2, so an upward scan reads the object's earlier keys (its
            // `name`, or the `- name:` opener) and stops at that dedent.
            let steps: &[i64] = if is_seq_opener { &[1] } else { &[-1, 1] };
            for &step in steps {
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
                    if li < ind {
                        // Dedented out of this parameter object. A `- name: X`
                        // sequence-item opener sits at `ind`-2 and carries the
                        // object's name in its first key — read it before leaving.
                        if li + 2 == ind {
                            if let Some(r) = l.trim_start().strip_prefix("- ") {
                                if let Some(v) = r.strip_prefix("name:") {
                                    if !v.trim().trim_matches('"').trim_matches('\'').is_empty() {
                                        has_name = true;
                                    }
                                }
                            }
                        }
                        break;
                    }
                    if li != ind {
                        continue; // a nested child (e.g. a `schema:` subtree)
                    }
                    let t = l.trim_start().strip_prefix("- ").unwrap_or(l.trim_start());
                    if let Some(v) = t.strip_prefix("name:") {
                        if !v.trim().trim_matches('"').trim_matches('\'').is_empty() {
                            has_name = true;
                        }
                    }
                }
            }
            if !has_name {
                out.push(format!("{}@line {}", loc, i + 1));
            }
        }
        out
    }

    /// The `location@line` label of every parameter a spec declares whose object
    /// carries neither a `schema:` nor a `content:` key — the value-type field of an
    /// OpenAPI Parameter Object.
    ///
    /// A Parameter Object MUST declare exactly one of `schema` (the common case: a
    /// typed value) or `content` (a value described by a media-type map). Alongside
    /// `in` (location) and `name` (identity), the value-type is REQUIRED: a located,
    /// named parameter with neither declares no type, so a client/codegen tool cannot
    /// bind or serialise it — the same class of invalid document the location/name
    /// tests catch on the other two fields.
    ///
    /// Mirrors [`parameters_missing_name`]'s object scan exactly — anchor on a
    /// parameter's `in:` location line (a mapping key or a `- ` sequence opener whose
    /// value is one of the four valid locations), then look for a `schema:`/`content:`
    /// sibling at the parameter object's own child indent (upward for a mapping or
    /// name-first form whose opener sits above, downward only for an in-first
    /// `- in: …` opener whose object starts at the anchor). A `schema:` nested inside
    /// a `content:` media type sits deeper than the parameter's own indent, so it
    /// never satisfies the check; a `$ref` parameter (no inline `in`) is never
    /// anchored, so it is exempt (it inherits its type from the referenced component).
    fn parameters_missing_schema_or_content(body: &str) -> Vec<String> {
        const LOCATIONS: [&str; 4] = ["query", "header", "path", "cookie"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // A `schema:`/`content:` mapping key at a parameter object's own indent,
        // whether written as a plain key or (defensively) a `- ` sequence opener.
        let is_type_key = |trimmed: &str| {
            let t = trimmed.strip_prefix("- ").unwrap_or(trimmed);
            t.starts_with("schema:") || t.starts_with("content:")
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let bare = line.trim_start();
            let key = bare.strip_prefix("- ").unwrap_or(bare);
            let Some(rest) = key.strip_prefix("in:") else { continue };
            let loc = rest.trim().trim_matches('"').trim_matches('\'');
            if !LOCATIONS.contains(&loc) {
                continue;
            }
            let is_seq_opener = bare.len() != key.len();
            let ind = indent(line) + if is_seq_opener { 2 } else { 0 };
            let mut has_type = false;
            // See `parameters_missing_name` for why an in-first `- in: …` opener
            // scans downward only while a mapping/name-first anchor scans both ways.
            let steps: &[i64] = if is_seq_opener { &[1] } else { &[-1, 1] };
            for &step in steps {
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
                    if li < ind {
                        // Dedented out of this parameter object. A `- schema:`/
                        // `- content:` sequence-item opener would sit at `ind`-2 —
                        // read it before leaving (mirrors the name test's opener read).
                        if li + 2 == ind && is_type_key(l.trim_start()) {
                            has_type = true;
                        }
                        break;
                    }
                    if li != ind {
                        continue; // a nested child (e.g. a `content:` media-type schema)
                    }
                    if is_type_key(l.trim_start()) {
                        has_type = true;
                    }
                }
            }
            if !has_type {
                out.push(format!("{}@line {}", loc, i + 1));
            }
        }
        out
    }

    /// The `parameters@line N: …` label of every `parameters:` array a spec
    /// declares that repeats a `(name, location)` pair — the OpenAPI uniqueness
    /// rule for the Parameter Object ("A unique parameter is defined by a
    /// combination of a name and location.").
    ///
    /// The key is the pair `(name, in)`, so the *same* name in two different
    /// locations (e.g. `id` in `path` and `id` in `query`) is legitimately
    /// distinct and never flagged; only a genuine repeat of both fields is. The
    /// scan is scoped to a single `parameters:` block on purpose: a `(name, in)`
    /// appearing once at the Path Item level and again at the Operation level is a
    /// legitimate *override* (the operation's wins), so comparing only within one
    /// array avoids that false positive while still catching the real hazard — a
    /// parameter block pasted twice into the same array.
    ///
    /// No YAML dep: anchor on a block-form `parameters:` opener (empty value),
    /// then walk its items. The first `- ` child fixes the item indent; each `- `
    /// at that indent opens a new parameter object, whose own `name:`/`in:` sit
    /// inline on the opener or at the item's child indent (opener indent + 2).
    /// Lines deeper than the child indent are a nested subtree (a `schema:` with
    /// its own `properties` named `name`/`in`) and are ignored, so only the
    /// parameter's own two fields are read. A `$ref` item carries neither inline,
    /// so it contributes no pair and is exempt.
    fn parameter_arrays_with_duplicate_name_location(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let unquote = |s: &str| s.trim().trim_matches('"').trim_matches('\'').to_string();
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(rest) = line.trim_start().strip_prefix("parameters:") else { continue };
            // Only a block-sequence `parameters:` opener (empty value). Skip a flow
            // list `parameters: [ … ]` (not used in these specs) and a `parameters`
            // key that is itself a schema property carrying an inline value.
            let rest = rest.trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                continue;
            }
            let params_ind = indent(line);
            let mut pairs: Vec<(String, String)> = Vec::new();
            let mut item_ind: Option<usize> = None;
            let mut cur: Option<(Option<String>, Option<String>)> = None;
            let flush = |cur: &mut Option<(Option<String>, Option<String>)>,
                         pairs: &mut Vec<(String, String)>| {
                if let Some((Some(n), Some(iv))) = cur.take() {
                    pairs.push((n, iv));
                }
            };
            for l in &lines[i + 1..] {
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li <= params_ind {
                    break; // dedented out of the parameters block
                }
                let bare = l.trim_start();
                let is_opener = bare.starts_with("- ");
                if is_opener && item_ind.is_none() {
                    item_ind = Some(li);
                }
                let Some(iind) = item_ind else { continue };
                if is_opener && li == iind {
                    flush(&mut cur, &mut pairs);
                    let after = &bare[2..];
                    let mut name = None;
                    let mut inv = None;
                    if let Some(v) = after.strip_prefix("name:") {
                        name = Some(unquote(v));
                    } else if let Some(v) = after.strip_prefix("in:") {
                        inv = Some(unquote(v));
                    }
                    cur = Some((name, inv));
                } else if li == iind + 2 {
                    if let Some((ref mut name, ref mut inv)) = cur {
                        if let Some(v) = bare.strip_prefix("name:") {
                            *name = Some(unquote(v));
                        } else if let Some(v) = bare.strip_prefix("in:") {
                            *inv = Some(unquote(v));
                        }
                    }
                }
                // Lines deeper than `iind + 2` are a nested subtree — ignored.
            }
            flush(&mut cur, &mut pairs);
            let mut seen: Vec<(String, String)> = Vec::new();
            for pair in &pairs {
                if seen.contains(pair) {
                    out.push(format!(
                        "parameters@line {}: duplicate parameter (name={}, in={})",
                        i + 1,
                        pair.0,
                        pair.1
                    ));
                    break;
                }
                seen.push(pair.clone());
            }
        }
        out
    }

    /// Extract the labels (`METHOD /path`) of every operation a spec declares
    /// that is **missing** a `responses:` object — without a YAML dep.
    ///
    /// `responses` is the single REQUIRED field of an OpenAPI Operation Object
    /// (a summary/operationId/parameters are all optional), so an operation with
    /// none is an invalid document: a client/codegen tool has no declared
    /// outcomes to bind. This scans the `paths:` section, treating a 2-space key
    /// beginning with `/` as a path item and a 4-space HTTP-method key
    /// (`get`/`put`/`post`/`delete`/`patch`/`options`/`head`/`trace`) under it as
    /// an operation, then looks within the operation's block (lines indented past
    /// the 4-space method key, until a dedent back to ≤4 spaces) for a 6-space
    /// `responses:` key. Method keys only count under a path item, so an HTTP verb
    /// appearing as a schema property name elsewhere is never mistaken for an
    /// operation.
    fn operations_without_responses(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            // A 2-space direct child of `paths:` beginning with `/` is a path item.
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            // A 4-space method key under a path item is an operation.
            if indent(line) != 4 {
                continue;
            }
            let key = line.trim_start();
            let Some(name) = key.strip_suffix(':') else { continue };
            if name.contains(char::is_whitespace) || !METHODS.contains(&name) {
                continue;
            }
            // Scan the operation's block for a 6-space `responses:` key.
            let mut has_responses = false;
            for l in &lines[i + 1..] {
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) <= 4 {
                    break; // dedented out of this operation
                }
                if indent(l) == 6 && l.trim_start().strip_suffix(':') == Some("responses") {
                    has_responses = true;
                    break;
                }
            }
            if !has_responses {
                out.push(format!("{} {}", name.to_uppercase(), current_path));
            }
        }
        out
    }

    /// The `METHOD /path` label of every operation a spec declares that carries no
    /// `operationId` key. Mirrors [`operations_without_responses`]'s scoping (a
    /// 4-space HTTP-verb key under a 2-space `/…` path item beneath the top-level
    /// `paths:` block), but scans each operation's block for a 6-space
    /// `operationId:` key. Unlike `responses:` (a mapping key whose value is the
    /// nested block on the following lines), `operationId:` is a scalar key with its
    /// value inline on the same line, so the block is matched on the `operationId`
    /// key name, not on the whole trimmed line.
    fn operations_without_operation_id(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            if indent(line) != 4 {
                continue;
            }
            let key = line.trim_start();
            let Some(name) = key.strip_suffix(':') else { continue };
            if name.contains(char::is_whitespace) || !METHODS.contains(&name) {
                continue;
            }
            // Scan the operation's block for a 6-space `operationId:` key (a scalar
            // key with an inline value, so match on the key name before the colon).
            let mut has_operation_id = false;
            for l in &lines[i + 1..] {
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) <= 4 {
                    break; // dedented out of this operation
                }
                if indent(l) == 6
                    && l.trim_start().split_once(':').map(|(k, _)| k) == Some("operationId")
                {
                    has_operation_id = true;
                    break;
                }
            }
            if !has_operation_id {
                out.push(format!("{} {}", name.to_uppercase(), current_path));
            }
        }
        out
    }

    /// The `METHOD /path` label of every operation a spec declares that carries no
    /// `summary` key. Mirrors [`operations_without_operation_id`]'s scoping (a
    /// 4-space HTTP-verb key under a 2-space `/…` path item beneath the top-level
    /// `paths:` block) and, like `operationId`, matches a 6-space `summary:` scalar
    /// key on its key name before the inline-value colon — so a `summary` nested
    /// deeper (an `examples` entry's `summary`, or a Path Item Object's own 4-space
    /// `summary`) never satisfies the operation.
    fn operations_without_summary(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            if indent(line) != 4 {
                continue;
            }
            let key = line.trim_start();
            let Some(name) = key.strip_suffix(':') else { continue };
            if name.contains(char::is_whitespace) || !METHODS.contains(&name) {
                continue;
            }
            // Scan the operation's block for a 6-space `summary:` key (a scalar key
            // with an inline value, so match on the key name before the colon).
            let mut has_summary = false;
            for l in &lines[i + 1..] {
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) <= 4 {
                    break; // dedented out of this operation
                }
                if indent(l) == 6
                    && l.trim_start().split_once(':').map(|(k, _)| k) == Some("summary")
                {
                    has_summary = true;
                    break;
                }
            }
            if !has_summary {
                out.push(format!("{} {}", name.to_uppercase(), current_path));
            }
        }
        out
    }

    /// The `METHOD /path <status>` label of every **response entry** a spec
    /// declares whose Response Object carries neither a `description` nor a
    /// `$ref` — without a YAML dep.
    ///
    /// `description` is the single REQUIRED field of an OpenAPI Response Object
    /// (everything else — `headers`/`content`/`links` — is optional), so an
    /// inline response missing it is an invalid document: a Redoc/Swagger/codegen
    /// client is handed an outcome with no human-readable summary to render. A
    /// response supplied as a `$ref` is exempt — it inherits its description from
    /// the referenced component (the shared `errors.yaml` responses are all
    /// `$ref`'d this way). Mirrors [`operations_without_responses`]'s
    /// path-item/method scoping (a 4-space HTTP-verb key under a 2-space `/…`
    /// path item beneath the top-level `paths:` block), then within an operation
    /// finds the 6-space `responses:` key and treats each 8-space status-code /
    /// `default` / `NXX`-range key under it as a response entry, scanning that
    /// entry's block (lines indented past 8, until a dedent to ≤8) for a 10-space
    /// `description:` or `$ref:` field. Matching the field at exactly the response
    /// object's own child indent (10) means a `description`/`$ref` nested deeper —
    /// inside a `content` media type's `schema`, or a `headers` entry, say — never
    /// satisfies the entry.
    fn responses_missing_description(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        // A Responses Object key that maps to a Response Object: an HTTP status
        // code, an `NXX` wildcard range (`1XX`..`5XX`), or `default`. Anything
        // else under `responses:` (an `x-` extension, say) is not a response.
        let is_status_key = |key: &str| -> bool {
            key == "default"
                || (key.len() == 3
                    && matches!(key.as_bytes()[0], b'1'..=b'5')
                    && key.as_bytes()[1..]
                        .iter()
                        .all(|&c| c.is_ascii_digit() || c == b'X'))
        };
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            if indent(line) != 4 {
                continue;
            }
            let key = line.trim_start();
            let Some(name) = key.strip_suffix(':') else { continue };
            if name.contains(char::is_whitespace) || !METHODS.contains(&name) {
                continue;
            }
            // Within this operation's block, find the 6-space `responses:` key,
            // then inspect each 8-space response entry under it.
            let mut in_responses = false;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                let li = indent(l);
                if li <= 4 {
                    break; // dedented out of this operation
                }
                if li == 6 {
                    // `responses:` opens the block; any other 6-space key (e.g. a
                    // trailing `security:`) closes it.
                    in_responses = l.trim_start().strip_suffix(':') == Some("responses");
                    j += 1;
                    continue;
                }
                if in_responses && li == 8 {
                    if let Some(k) = l.trim_start().strip_suffix(':') {
                        let status = k.trim_matches(|c| c == '"' || c == '\'');
                        if is_status_key(status) {
                            // Scan this response object's block for a 10-space
                            // `description:` or `$ref:` field.
                            let mut satisfied = false;
                            let mut m = j + 1;
                            while m < lines.len() {
                                let e = lines[m];
                                if e.trim().is_empty() {
                                    m += 1;
                                    continue;
                                }
                                if indent(e) <= 8 {
                                    break; // dedented out of this response entry
                                }
                                if indent(e) == 10 {
                                    let field =
                                        e.trim_start().split_once(':').map(|(f, _)| f);
                                    if field == Some("description") || field == Some("$ref") {
                                        satisfied = true;
                                        break;
                                    }
                                }
                                m += 1;
                            }
                            if !satisfied {
                                out.push(format!(
                                    "{} {} {}",
                                    name.to_uppercase(),
                                    current_path,
                                    status
                                ));
                            }
                        }
                    }
                }
                j += 1;
            }
        }
        out
    }

    /// Enumerate every key under an operation's `responses:` block that is **not** a
    /// valid Responses Object key, as `"<METHOD> <path> <key>"` (document order),
    /// without a YAML dep.
    ///
    /// A Responses Object maps keys to Response Objects, and OpenAPI restricts those
    /// keys to: an explicit HTTP status code (`"200"`), an `NXX` wildcard range
    /// (`"1XX"`..`"5XX"`), the `default` key, or an `x-` specification extension —
    /// nothing else. This mirrors [`responses_missing_description`]'s
    /// path-item/method scoping to reach each 8-space response-entry key, then flags
    /// any that is none of those forms — a status code typo'd into an invalid token
    /// (`"4O4"` with a letter O, an out-of-range `"600"`, a truncated `"20"`) from a
    /// copy-paste/edit. Quotes around a key are stripped before the check.
    ///
    /// Note the complement to [`responses_missing_description`]: *that* helper's
    /// `is_status_key` filter is used to *find* responses to description-check, so a
    /// key it rejects is silently skipped there; here the same rejection is the
    /// finding, so the two together cover both "is a response and lacks a
    /// description" and "is under `responses:` but is not a valid response key".
    fn responses_with_invalid_status_key(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        // Valid non-extension Responses Object keys: an explicit HTTP status code, an
        // `NXX` wildcard range (`1XX`..`5XX`), or `default` — the exact shape the
        // description test uses to *recognise* a response. (An `x-` specification
        // extension is a permitted key too; it is admitted separately below.)
        let is_status_key = |key: &str| -> bool {
            key == "default"
                || (key.len() == 3
                    && matches!(key.as_bytes()[0], b'1'..=b'5')
                    && key.as_bytes()[1..]
                        .iter()
                        .all(|&c| c.is_ascii_digit() || c == b'X'))
        };
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            if indent(line) != 4 {
                continue;
            }
            let key = line.trim_start();
            let Some(name) = key.strip_suffix(':') else { continue };
            if name.contains(char::is_whitespace) || !METHODS.contains(&name) {
                continue;
            }
            // Within this operation's block, find the 6-space `responses:` key, then
            // inspect each 8-space response-entry key under it.
            let mut in_responses = false;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                let li = indent(l);
                if li <= 4 {
                    break; // dedented out of this operation
                }
                if li == 6 {
                    in_responses = l.trim_start().strip_suffix(':') == Some("responses");
                    j += 1;
                    continue;
                }
                if in_responses && li == 8 {
                    if let Some(k) = l.trim_start().strip_suffix(':') {
                        let status = k.trim_matches(|c| c == '"' || c == '\'');
                        if !is_status_key(status) && !status.starts_with("x-") {
                            out.push(format!(
                                "{} {} {}",
                                name.to_uppercase(),
                                current_path,
                                status
                            ));
                        }
                    }
                }
                j += 1;
            }
        }
        out
    }

    /// The `METHOD /path` label of every operation a spec declares whose
    /// `responses:` object is present but documents no **success** (`2XX`)
    /// outcome — without a YAML dep.
    ///
    /// Every CAMARA business operation returns a concrete happy-path `2XX`
    /// (`200`/`201`/`202`/`204`); that entry is the return type a Redoc/Swagger/
    /// codegen client derives, so an operation declaring only its error branches
    /// (the shared `errors.yaml` `4XX`/`5XX` `$ref`s) is an incomplete contract. No
    /// sibling responses test sees the loss: `operations_without_responses` pins the
    /// *presence* of the `responses:` object, `responses_with_invalid_status_key`
    /// that each remaining key is a *well-formed* status, and
    /// `responses_missing_description` that each inline response *describes itself*
    /// (the `$ref`'d error responses are exempt) — none require a success outcome
    /// among them.
    ///
    /// Mirrors [`responses_with_invalid_status_key`]'s scoping exactly (a 4-space
    /// HTTP-verb key under a 2-space `/…` path item beneath the top-level `paths:`
    /// block, then the 8-space keys under that operation's 6-space `responses:`),
    /// but instead of judging each key it asks, per operation, whether *any* key is
    /// a success: a 3-char token beginning `2` whose other two chars are each a
    /// digit or the `X` wildcard — i.e. an explicit `2XX`-range code (`200`..`299`)
    /// or the `2XX` range itself. Only an operation that *declares* a `responses:`
    /// block is judged (one missing the object entirely is
    /// [`operations_without_responses`]' concern), so the two never double-flag the
    /// same operation.
    fn operations_without_success_response(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        let is_success_key = |key: &str| -> bool {
            key.len() == 3
                && key.as_bytes()[0] == b'2'
                && key.as_bytes()[1..].iter().all(|&c| c.is_ascii_digit() || c == b'X')
        };
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            if indent(line) != 4 {
                continue;
            }
            let key = line.trim_start();
            let Some(name) = key.strip_suffix(':') else { continue };
            if name.contains(char::is_whitespace) || !METHODS.contains(&name) {
                continue;
            }
            // Within this operation's block, find the 6-space `responses:` key, then
            // check each 8-space response-entry key under it for a success code.
            let mut in_responses = false;
            let mut saw_responses = false;
            let mut saw_success = false;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                let li = indent(l);
                if li <= 4 {
                    break; // dedented out of this operation
                }
                if li == 6 {
                    in_responses = l.trim_start().strip_suffix(':') == Some("responses");
                    if in_responses {
                        saw_responses = true;
                    }
                    j += 1;
                    continue;
                }
                if in_responses && li == 8 {
                    if let Some(k) = l.trim_start().strip_suffix(':') {
                        let status = k.trim_matches(|c| c == '"' || c == '\'');
                        if is_success_key(status) {
                            saw_success = true;
                        }
                    }
                }
                j += 1;
            }
            if saw_responses && !saw_success {
                out.push(format!("{} {}", name.to_uppercase(), current_path));
            }
        }
        out
    }

    /// The `METHOD /path` label of every operation a spec declares whose
    /// `requestBody` object carries neither a `content` field nor a `$ref` —
    /// without a YAML dep.
    ///
    /// `content` is the single REQUIRED field of an OpenAPI Request Body Object
    /// (`description`/`required` are optional), so a `requestBody:` block without
    /// it is an invalid document: a Redoc/Swagger/codegen client is handed an
    /// operation that takes a body of no declared media type or schema. A
    /// `requestBody` supplied as a `$ref` is exempt — it inherits its `content`
    /// from the referenced component. Only operations that *declare* a
    /// `requestBody` are inspected (a GET/DELETE with none is not flagged), the
    /// exact analogue of [`responses_missing_description`], which flags a declared
    /// response missing `description` without requiring every operation to have
    /// one. Mirrors [`operations_without_responses`]'s path-item/method scoping (a
    /// 4-space HTTP-verb key under a 2-space `/…` path item beneath the top-level
    /// `paths:` block), then within an operation finds the 6-space `requestBody:`
    /// key and scans its block (lines indented past 6, until a dedent to ≤6) for an
    /// 8-space `content:` or `$ref:` field. Matching at exactly the request body
    /// object's own child indent (8) means a `content`/`$ref` nested deeper — a
    /// `content` under an `application/json` media type's `schema`, say — never
    /// satisfies it. A `requestBody:` given inline as a `$ref` mapping (`{$ref:
    /// …}`) or a flow `$ref` on the key line is treated as satisfied.
    fn request_bodies_missing_content(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            if indent(line) != 4 {
                continue;
            }
            let key = line.trim_start();
            let Some(name) = key.strip_suffix(':') else { continue };
            if name.contains(char::is_whitespace) || !METHODS.contains(&name) {
                continue;
            }
            // Within this operation's block, find the 6-space `requestBody:` key,
            // then inspect its object for an 8-space `content:` or `$ref:` field.
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                let li = indent(l);
                if li <= 4 {
                    break; // dedented out of this operation
                }
                if li == 6 {
                    if let Some((k, v)) = l.trim_start().split_once(':') {
                        if k == "requestBody" {
                            // An inline `$ref` value on the key line satisfies it.
                            let inline = v.trim();
                            if inline.starts_with("$ref") || inline.starts_with('{') {
                                j += 1;
                                continue;
                            }
                            // Otherwise scan the request body object's block for an
                            // 8-space `content:` / `$ref:` field.
                            let mut satisfied = false;
                            let mut m = j + 1;
                            while m < lines.len() {
                                let e = lines[m];
                                if e.trim().is_empty() {
                                    m += 1;
                                    continue;
                                }
                                if indent(e) <= 6 {
                                    break; // dedented out of this request body
                                }
                                if indent(e) == 8 {
                                    let field =
                                        e.trim_start().split_once(':').map(|(f, _)| f);
                                    if field == Some("content") || field == Some("$ref") {
                                        satisfied = true;
                                        break;
                                    }
                                }
                                m += 1;
                            }
                            if !satisfied {
                                out.push(format!(
                                    "{} {}",
                                    name.to_uppercase(),
                                    current_path
                                ));
                            }
                        }
                    }
                }
                j += 1;
            }
        }
        out
    }

    /// Enumerate every Media Type Object under a `content:` mapping (in a request
    /// body, a response, or a parameter) that declares no `schema` — nor an inline
    /// `$ref` — as `"<path> <media-type>"` in document order, without a YAML dep.
    ///
    /// Scans within `paths:` only (like [`request_bodies_missing_content`] and
    /// [`responses_missing_description`]), tracking the current path item for the
    /// report. Each `content:` key opens a Content mapping; every child key two
    /// spaces deeper that names a media type — a key containing `/`, e.g.
    /// `application/json` or `application/problem+json` — opens a Media Type Object,
    /// which MUST carry a `schema` (or a `$ref` to a shared Schema / Media Type) so a
    /// Redoc/Swagger/codegen client can bind the payload's shape. Its object block is
    /// scanned for a `schema:`/`$ref:` field four spaces deeper; none → a finding.
    ///
    /// Two disambiguations keep it from mis-flagging:
    /// - A media type whose value is inline (a flow mapping `{...}` or an inline
    ///   `$ref`) is treated as satisfied — its shape can't be introspected line-wise.
    /// - A `content:` key that is actually a schema *property* named `content` (a
    ///   CloudEvent-style field, say) has no MIME-shaped children (its keys are
    ///   `type`/`description`/…, none containing `/`), so it contributes nothing.
    ///
    /// Only Content mappings reachable via `paths:` are covered; reusable
    /// `components.requestBodies`/`responses` blocks are out of scope, mirroring the
    /// sibling helpers.
    fn media_types_missing_schema(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            // A `content:` mapping opens here (a key with an empty value).
            if line.trim() != "content:" {
                continue;
            }
            let Some(current_path) = path.as_deref() else { continue };
            let c = indent(line);
            // Walk this Content object's block; each media-type child sits at c+2.
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break; // dedented out of this content object
                }
                if indent(l) == c + 2 {
                    if let Some((k, v)) = l.trim_start().split_once(':') {
                        // A media type key names a MIME type: it contains a '/'.
                        if k.contains('/') {
                            let inline = v.trim();
                            if inline.starts_with('{') || inline.starts_with("$ref") {
                                // Inline object / ref — can't introspect; satisfied.
                                j += 1;
                                continue;
                            }
                            // Scan this media type object for a c+4 `schema:`/`$ref:`.
                            let mut satisfied = false;
                            let mut m = j + 1;
                            while m < lines.len() {
                                let e = lines[m];
                                if e.trim().is_empty() {
                                    m += 1;
                                    continue;
                                }
                                if indent(e) <= c + 2 {
                                    break; // dedented out of this media type object
                                }
                                if indent(e) == c + 4 {
                                    let field =
                                        e.trim_start().split_once(':').map(|(f, _)| f);
                                    if field == Some("schema") || field == Some("$ref") {
                                        satisfied = true;
                                        break;
                                    }
                                }
                                m += 1;
                            }
                            if !satisfied {
                                out.push(format!("{current_path} {}", k.trim()));
                            }
                        }
                    }
                }
                j += 1;
            }
        }
        out
    }

    /// True when `key` names a syntactically valid media type: a `type/subtype`
    /// pair (RFC 6838 / RFC 2045), each half a non-empty restricted-name token or
    /// the `*` range wildcard, with any trailing `;`-introduced parameters ignored.
    ///
    /// A restricted-name token is an ASCII-alphanumeric first character followed
    /// by characters from the registered-name set `[A-Za-z0-9!#$&^_.+-]` — which
    /// admits the structured-suffix (`+json`), facet (`.`), and vendor (`-`) forms
    /// CAMARA uses (`application/cloudevents+json`, `application/merge-patch+json`,
    /// `application/x-www-form-urlencoded`). Exactly one `/` is required: a key with
    /// none (`applicationjson`), an empty half (`application/`, `/json`), or a
    /// second slash (`a/b/c`) is rejected.
    fn is_valid_media_type_key(key: &str) -> bool {
        // Drop any parameters (`; charset=…`); the media range is what we validate.
        let base = key.split(';').next().unwrap_or(key).trim();
        let mut halves = base.split('/');
        let (Some(ty), Some(sub), None) = (halves.next(), halves.next(), halves.next())
        else {
            return false; // not exactly one '/'
        };
        let is_token = |t: &str| -> bool {
            if t == "*" {
                return true;
            }
            let mut chars = t.chars();
            let Some(first) = chars.next() else {
                return false; // empty half
            };
            first.is_ascii_alphanumeric()
                && t.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "!#$&^_.+-".contains(c))
        };
        is_token(ty) && is_token(sub)
    }

    /// Enumerate every key a spec declares directly under a **Content Object** (a
    /// `content:` mapping within `paths:`) that does not name a valid media type —
    /// reported as `"<path> <key>"` in document order, without a YAML dep.
    ///
    /// Under an OpenAPI 3 Content Object every direct child key MUST be a media
    /// type; a client dispatches request/response bodies by matching that key, so a
    /// key that is not a well-formed MIME type (a slash dropped in a paste —
    /// `applicationjson:`; a garbled subtype — `application/:`; a stray second
    /// slash) names a media type no client selects, silently undocumenting the body.
    /// This is the complement of [`media_types_missing_schema`], whose scan only
    /// *acts on* content children that already contain a `/` (so it never sees a
    /// slash-less malformed key) and only checks that a media type carries a schema
    /// (never that its key is well-formed); and of the `content:`-media-type sweeps
    /// generally, none of which validate the key's MIME syntax.
    ///
    /// To avoid mistaking a schema **property** literally named `content` (whose
    /// children are schema fields like `type:`/`properties:`, never MIME-shaped) for
    /// a Content Object, a `content:` block qualifies only when at least one of its
    /// direct children is itself MIME-shaped (contains a `/`) — the same signal
    /// [`media_types_missing_schema`] relies on. Within a qualifying Content Object
    /// every direct child is then required to be a valid media type. (A hypothetical
    /// Content Object whose *only* child dropped its slash would not qualify and is
    /// left to the schema/description sweeps; that trade keeps the property-named-
    /// `content` false positive out, and is documented here rather than silently.)
    /// Scoping mirrors [`media_types_missing_schema`]: only within `paths:`, only a
    /// `c+2` direct child of a `content:` mapping at indent `c`.
    fn media_types_with_invalid_names(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            if line.trim() != "content:" {
                continue;
            }
            let Some(current_path) = path.as_deref() else { continue };
            let c = indent(line);
            // Collect this block's direct child keys (each media-type slot sits at
            // c+2), in document order, walking until the block dedents out.
            let mut children: Vec<&str> = Vec::new();
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break; // dedented out of this content object
                }
                if indent(l) == c + 2 {
                    let trimmed = l.trim_start();
                    // Skip comments; take a mapping key (the text before its ':').
                    if !trimmed.starts_with('#') {
                        if let Some((k, _)) = trimmed.split_once(':') {
                            children.push(k.trim());
                        }
                    }
                }
                j += 1;
            }
            // Qualify as a Content Object only if a child is MIME-shaped; otherwise
            // this is a schema property named `content`, not a media-type mapping.
            if children.iter().any(|k| k.contains('/')) {
                for k in children {
                    if !is_valid_media_type_key(k) {
                        out.push(format!("{current_path} {k}"));
                    }
                }
            }
        }
        out
    }

    /// Enumerate every key a spec declares directly under a Path Item Object (a
    /// 4-space child of a 2-space `/…` path item beneath the top-level `paths:`
    /// block) that is neither a valid HTTP method nor a permitted Path Item field —
    /// reported as `"<path> <key>"` in document order, without a YAML dep.
    ///
    /// Under an OpenAPI 3 Path Item Object the only valid keys are the fixed
    /// operation verbs (`get`/`put`/`post`/`delete`/`options`/`head`/`patch`/
    /// `trace`), the fixed non-operation fields (`$ref`/`summary`/`description`/
    /// `servers`/`parameters`), and `x-` specification extensions. Any other key —
    /// most commonly a mistyped verb (`psot:`, `pust:`, or an upper-case `POST:`) —
    /// silently defines a **phantom operation** that no HTTP client routes: every
    /// sibling operation test (`operations_without_responses`,
    /// `operations_without_operation_id`, `responses_missing_description`, and the
    /// operation-id/response tests) enumerates operations from the *valid* method
    /// set and `continue`s past anything else, so a malformed verb is invisible to
    /// all of them. This is their exact complement — it inspects the keys they skip.
    ///
    /// Scoping mirrors [`operations_without_responses`]: only within `paths:`, only
    /// a 4-space key under a 2-space `/…` path item. Comment lines and non-mapping
    /// lines (no `:`) are ignored and a key is unquoted before classification, so
    /// deeper structure (a `parameters:` list's 6-space `- name:` items, block
    /// scalars) never reaches the check.
    fn invalid_path_item_keys(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        // Fixed non-operation fields of an OpenAPI 3 Path Item Object.
        const FIELDS: [&str; 5] =
            ["$ref", "summary", "description", "servers", "parameters"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_paths = false;
        let mut path: Option<String> = None;
        for line in &lines {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_paths = line.trim_end() == "paths:";
                path = None;
                continue;
            }
            if !in_paths {
                continue;
            }
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) && rest.starts_with('/') {
                    let key = rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
                    path = Some(key.to_string());
                    continue;
                }
            }
            let Some(current_path) = path.as_deref() else { continue };
            // A path-item field / operation key sits exactly four spaces in.
            if indent(line) != 4 {
                continue;
            }
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                continue; // a comment, not a key
            }
            // The mapping key is the text before the first colon; a line with no
            // colon (a block-scalar continuation, a `- ` list item) is not a key.
            let Some((raw_key, _)) = trimmed.split_once(':') else { continue };
            let key = raw_key.trim().trim_matches(|c| c == '"' || c == '\'');
            if key.is_empty()
                || METHODS.contains(&key)
                || FIELDS.contains(&key)
                || key.starts_with("x-")
            {
                continue;
            }
            out.push(format!("{current_path} {key}"));
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
    fn every_spec_declares_a_non_empty_info_description() {
        // Contract-harness invariant (OpenAPI structural rule): every mounted
        // vendored spec MUST declare a non-empty `info.description`. Together
        // with `info.title` (asserted above) and `info.version` (pinned by
        // `spec_info_version_matches_mounted_url_version`), this completes the
        // `info`-object field coverage for the two REQUIRED fields plus this,
        // the one RECOMMENDED overview field CAMARA always populates.
        //
        // `info.description` is the CommonMark prose Redoc/Swagger renders as
        // the API's introduction on the served `/{api}/v{n}/docs` page — and
        // where each CamaraSim spec carries the API's purpose, its two/three-
        // legged auth model, and its parameter-driven functional cases in
        // human-readable form (docs/DESIGN §7, §9). A spec drafted from a CAMARA
        // template whose `description:` block scalar was dropped, or left with
        // its indented body deleted, still parses as a structurally valid
        // OpenAPI document — so the identity/wiring/scenario tests (which trust
        // the doc is complete) never see it — yet renders a blank overview.
        // `None` (no `description:` at all) and `Some(false)` (present but empty)
        // are reported distinctly so the failure names the exact drift. Verified
        // true across every mounted spec before asserting.
        for api in APIS {
            match info_description_present(api.body) {
                Some(true) => {}
                Some(false) => panic!(
                    "{} spec declares an empty `info.description` — the API \
                     overview its /docs page renders is blank",
                    api.name
                ),
                None => panic!(
                    "{} spec declares no `info.description` — the recommended \
                     API overview is absent from the `info` object",
                    api.name
                ),
            }
        }
    }

    #[test]
    fn every_spec_declares_a_valid_info_license() {
        // Contract-harness invariant (OpenAPI License Object rule + CAMARA-template
        // uniformity): every mounted vendored spec MUST declare an `info.license`
        // whose `name` is present and non-empty. The `license` field of the `info`
        // object is OPTIONAL, but when present its `name` is the License Object's
        // single REQUIRED field — a licence block with no `name` (or a blank one)
        // is an invalid License Object. Every CamaraSim spec carries the
        // CAMARA-template `license: { name: Apache-2.0, url: … }`, so this also
        // pins that uniformity: the served `/{api}/v{n}/docs` page and every
        // codegen client read the licence from here.
        //
        // Extends the `info`-object field series
        // (`every_spec_declares_a_non_empty_info_title` / `…_info_description`,
        // `spec_info_version_matches_mounted_url_version`) to the `license.name`
        // field. The break it catches: a spec drafted from a CAMARA template whose
        // `license:` block was dropped in an edit, or whose `name:` line was
        // deleted/blanked (leaving only the `url:`), still parses as a
        // structurally valid OpenAPI document — invisible to the identity/wiring/
        // scenario tests, which trust the doc is complete — yet ships an incomplete
        // `info` object with no licence label to render. `None` (no `license:` at
        // all), `Some(None)` (present but no `name:` child), and `Some(Some(""))`
        // (blank name) are reported distinctly so a failure names the exact drift.
        // Verified true across every mounted spec before asserting.
        for api in APIS {
            match info_license_name(api.body) {
                Some(Some(name)) => assert!(
                    !name.is_empty(),
                    "{} spec declares an empty `info.license.name` — the License \
                     Object's one REQUIRED field is blank",
                    api.name
                ),
                Some(None) => panic!(
                    "{} spec declares `info.license` with no `name:` child — an \
                     invalid License Object (`name` is its one REQUIRED field)",
                    api.name
                ),
                None => panic!(
                    "{} spec declares no `info.license` — the CAMARA-template \
                     licence block is absent from the `info` object",
                    api.name
                ),
            }
        }
    }

    #[test]
    fn every_server_url_variable_is_defined_with_a_default() {
        // Contract-harness invariant (OpenAPI Server Object / Server Variable
        // Object rule): every `{name}` a spec's `servers[].url` templates MUST be
        // declared in that server's `variables:` map, and a Server Variable
        // Object's one REQUIRED field is `default`. CamaraSim's every vendored spec
        // templates its base path as `{apiRoot}/…` (the exact text pinned to
        // `{apiRoot}{base_path()}` by `spec_server_url_matches_mounted_base_path`)
        // and must back `apiRoot` with a `variables.apiRoot.default` — the base URL
        // the served `/{api}/v{n}/docs` "try it" panel and every codegen client
        // substitute to build a concrete request URL.
        //
        // The break it catches: a spec whose `variables:` block (or its
        // `apiRoot.default`) was dropped in an edit still parses as a structurally
        // valid document — so the identity/wiring/scenario tests, which trust the
        // doc is complete, never see it — yet its substituted request URL renders
        // with a literal, unresolved `{apiRoot}`. This is invisible to
        // `spec_server_url_matches_mounted_base_path`, which proves only that the
        // *url text* names the mount path, never that the template variable it
        // names resolves. Verified true across every mounted spec before asserting.
        for api in APIS {
            let undefined = server_url_undefined_variables(api.body);
            assert!(
                undefined.is_empty(),
                "{} spec's servers url references template variable(s) {:?} not \
                 declared in `variables:` with a non-empty `default:` — an invalid \
                 Server Object whose substituted request URL keeps a literal \
                 `{{var}}`",
                api.name,
                undefined
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
    fn shared_auth_refs_resolve_to_defined_components() {
        // Contract-harness invariant (DESIGN §8/§9 + `apis::openapi` serving): a
        // spec's cross-file `$ref`s into the shared auth fragment
        // (`../../auth/openapi.yaml#/components/…`) must point at a component that
        // fragment actually DEFINES. This is the auth-fragment complement of
        // `shared_error_refs_resolve_to_defined_components`: that dereferences a
        // spec's cross-file pointers into `shared/errors.yaml`; this dereferences
        // the ones into `auth/openapi.yaml`. Together they prove that BOTH shared
        // fragments a spec `$ref`s resolve target-for-target, not just that the
        // *file* half uses the served path (the canonical-path test's job).
        //
        // The break this catches: `every_spec_refs_the_shared_camara_oauth_scheme`
        // proves each spec's `openId` securityScheme `$ref`s the shared fragment by
        // the canonical path, but it never dereferences the JSON-pointer *into* the
        // fragment. A spec that copied the scheme ref with a stale/typo'd pointer
        // (`…#/components/securitySchemes/camaraOauth`, or a component renamed in the
        // auth fragment after the copy) keeps a correct file half yet resolves to a
        // dangling pointer — a client following it never finds the security scheme.
        // No existing test sees this (the scheme test checks the file half + that a
        // scheme is referenced; the identity/wiring tests never dereference
        // cross-file pointers into the auth fragment).
        //
        // The allowed set is extracted from the embedded auth fragment itself (not
        // hard-coded), so adding a shared auth component automatically widens it and
        // this test never needs editing when the auth fragment grows.
        const SHARED_AUTH: &str = include_str!("../specs/auth/openapi.yaml");
        let defined = component_pointers(SHARED_AUTH);
        // Non-vacuous floor: the fragment defines the shared camaraOAuth scheme
        // every business spec references.
        assert!(
            defined.contains("#/components/securitySchemes/camaraOAuth"),
            "auth/openapi.yaml is expected to define the camaraOAuth securityScheme; \
             extracted {defined:?}"
        );

        let mut checked = 0usize;
        for api in APIS {
            for target in ref_targets(api.body) {
                let Some((file, pointer)) = target.split_once('#') else {
                    continue;
                };
                // Only cross-file refs into the shared auth fragment. (Local
                // intra-document refs have an empty `file` half and are resolved
                // within the spec itself; refs into shared/errors.yaml are the
                // sibling test's job.)
                if !file.contains("auth/openapi.yaml") {
                    continue;
                }
                let pointer = format!("#{pointer}");
                assert!(
                    defined.contains(&pointer),
                    "{} spec has a $ref into the shared auth fragment at `{}`, but \
                     auth/openapi.yaml defines no such component — it resolves to a \
                     dangling pointer when the spec is served. Defined components: {:?}",
                    api.name,
                    target,
                    {
                        let mut v: Vec<&String> = defined.iter().collect();
                        v.sort();
                        v
                    }
                );
                checked += 1;
            }
        }
        // Non-vacuous floor: every mounted business spec refs the shared scheme, so
        // the loop must actually have dereferenced auth-fragment pointers.
        assert!(
            checked >= APIS.len().saturating_sub(2),
            "expected almost every mounted spec to $ref the shared auth fragment, \
             but only {checked} auth-fragment refs were checked across {} specs",
            APIS.len()
        );
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
    fn every_security_requirement_declares_a_scope() {
        // Contract-harness invariant (CAMARA canonical auth + DESIGN §8/§9): every
        // operation a mounted spec declares carries a `security` requirement that
        // lists at least one **scope**. Every CamaraSim business endpoint is gated
        // on a specific purpose/technical scope (the resource-server
        // `verify::Claims::require_scope`), and the spec documents that scope as its
        // security requirement's scope list (`- openId:` → `- <scope>`).
        //
        // The break this catches: an operation's `security` block that names the
        // scheme but lists **no scope** — the inline `- openId: []` or a
        // `- openId:` whose scope line was dropped/dedented in a copy-paste. An
        // empty scope list is a real authorization drift: it tells a client (and
        // codegen, and the served "try it" panel) the endpoint needs only a valid
        // token, silently discarding the specific scope the endpoint actually
        // enforces. No existing contract test sees it — the scheme-name test
        // (`every_security_requirement_references_a_defined_scheme`) and its helper
        // `security_requirement_schemes` deliberately separate the scheme from its
        // scopes and check only that the *scheme* (`openId`) is defined, never that
        // the scope list is non-empty; the operationId/summary/responses/`$ref`
        // tests check a spec's identity, wiring, or a payload's presence. Verified
        // true across all mounted specs before asserting (every operation's `openId`
        // requirement carries a scope).
        for api in APIS {
            let scopeless = operations_with_scopeless_security(api.body);
            assert!(
                scopeless.is_empty(),
                "{} spec has operation(s) whose `security` requirement lists no scope \
                 (an empty scope list drops the authorization the endpoint enforces — \
                 likely a copy-pasted `- openId: []` or a dropped `- <scope>` line): {:?}",
                api.name,
                scopeless
            );
        }
    }

    #[test]
    fn scopeless_security_extraction_rules() {
        // Unit-cover the `operations_with_scopeless_security` extractor so the
        // contract test above can't pass vacuously (an extractor that returned an
        // empty Vec for every body would make its assertion meaningless) and so its
        // block-form / inline-flow / prose discrimination is pinned.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /ok:
    post:
      operationId: doOk
      security:
        - openId:
            - some-api:read
      responses:
        '200':
          description: ok
  /inline-empty:
    post:
      operationId: doInlineEmpty
      security:
        - openId: []
      responses:
        '200':
          description: ok
  /block-empty:
    get:
      operationId: doBlockEmpty
      security:
        - openId:
      responses:
        '200':
          description: ok
  /inline-full:
    get:
      operationId: doInlineFull
      security:
        - openId: [some-api:read]
      responses:
        '200':
          description: ok
components:
  schemas:
    Widget:
      type: object
      required:
        - openId
      properties:
        openId:
          type: string
";
        // `POST /inline-empty` (`- openId: []`) and `GET /block-empty` (`- openId:`
        // with no `- <scope>` beneath it) list no scope, so both are flagged, in
        // document order. `POST /ok` (block form with a scope) and `GET /inline-full`
        // (`[some-api:read]`) each carry a scope, and the `required: - openId` /
        // `openId:` property under `components.schemas.Widget` sit outside any
        // `security:` block, so none is flagged.
        assert_eq!(
            operations_with_scopeless_security(body),
            vec!["POST /inline-empty".to_string(), "GET /block-empty".to_string()]
        );

        // Non-vacuous floor: across every registered spec, no operation has a
        // scopeless `security` requirement (the invariant the contract test
        // asserts), and the corpus actually declares many scoped requirements, so a
        // broken extractor can't hide behind an empty scan.
        let mut total_requirements = 0usize;
        for api in APIS {
            assert!(
                operations_with_scopeless_security(api.body).is_empty(),
                "{}: every operation's `security` requirement must list a scope",
                api.name
            );
            total_requirements += security_requirement_schemes(api.body).len();
        }
        assert!(
            total_requirements >= 100,
            "expected many scoped security requirements across specs, got {total_requirements}"
        );
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
    fn info_description_extraction_rules() {
        // Unit-cover the `info_description_present` extractor so the contract
        // test above can't pass vacuously (an extractor that returned `Some(true)`
        // for every body would make its assertion meaningless), and so the
        // info-block scoping, inline-vs-block detection, and the
        // missing/blank/present trichotomy are pinned.

        // A non-empty inline scalar → Some(true).
        assert_eq!(
            info_description_present(
                "info:\n  title: X\n  description: A short overview\n  version: \"1\"\npaths: {}\n"
            ),
            Some(true)
        );
        // A literal block scalar with an indented body → Some(true); the CAMARA
        // form every vendored spec uses.
        assert_eq!(
            info_description_present(
                "info:\n  description: |\n    Real overview prose.\n    More prose.\n  version: \"1\"\n"
            ),
            Some(true)
        );
        // A block scalar whose body begins after a blank line is still content.
        assert_eq!(
            info_description_present(
                "info:\n  description: |\n\n    Prose after a blank line.\n  version: \"1\"\n"
            ),
            Some(true)
        );
        // A block scalar opened but immediately followed by a sibling `info`
        // field (no indented body) → Some(false).
        assert_eq!(
            info_description_present("info:\n  description: |\n  version: \"1\"\npaths: {}\n"),
            Some(false)
        );
        // A present-but-blank inline `description:` → Some(false), distinct from a
        // missing field (None).
        assert_eq!(
            info_description_present("info:\n  title: X\n  description:\npaths: {}\n"),
            Some(false)
        );
        // No `description:` under `info` at all → None.
        assert_eq!(
            info_description_present("info:\n  title: X\n  version: \"1\"\npaths: {}\n"),
            None
        );
        // A deeper `description:` inside a component schema (outside the `info:`
        // block, and more than 2 spaces in) is never mistaken for info's → None.
        assert_eq!(
            info_description_present(
                "info:\n  title: X\npaths: {}\ncomponents:\n  schemas:\n    Foo:\n      description: nope\n"
            ),
            None
        );

        // Non-vacuous floor: every registered spec declares a non-empty
        // info.description, so a broken extractor can't hide behind an empty loop.
        for api in APIS {
            assert_eq!(
                info_description_present(api.body),
                Some(true),
                "{} spec must declare a non-empty info.description",
                api.name
            );
        }
    }

    #[test]
    fn info_license_name_extraction_rules() {
        // Unit-cover the `info_license_name` extractor so the contract test above
        // can't pass vacuously and its scoping is pinned: the missing/no-name/
        // present trichotomy, name-first vs url-first child ordering, a blank name,
        // a sibling `info` field ending the block before a name, and a deeper
        // component `license:`/`name:` not being mistaken for the `info` one.

        // Present, name-first (the CAMARA-template form) → the name.
        assert_eq!(
            info_license_name(
                "info:\n  title: t\n  license:\n    name: Apache-2.0\n    url: https://x\n"
            ),
            Some(Some("Apache-2.0".to_string()))
        );
        // Present, url-first — the `name:` grandchild is still found.
        assert_eq!(
            info_license_name("info:\n  license:\n    url: https://x\n    name: MIT\n"),
            Some(Some("MIT".to_string()))
        );
        // Present but only a `url:` child (no `name:`) → an invalid License Object.
        assert_eq!(
            info_license_name("info:\n  license:\n    url: https://x\n  version: \"1\"\n"),
            Some(None)
        );
        // Present with a blank `name:` → Some(Some("")), distinct from no-name.
        assert_eq!(
            info_license_name("info:\n  license:\n    name:\n"),
            Some(Some(String::new()))
        );
        // No `license:` under `info` at all → None.
        assert_eq!(
            info_license_name("info:\n  title: t\n  version: \"1\"\npaths: {}\n"),
            None
        );
        // A sibling `info` field at ≤2-space indent ends the licence block before
        // any deeper `name:` line → Some(None) (the trailing name is not credited).
        assert_eq!(
            info_license_name("info:\n  license:\n  version: \"1\"\n    name: nope\n"),
            Some(None)
        );
        // A deeper `license:`/`name:` inside a component schema (outside `info:`,
        // more than 2 spaces in) is never mistaken for info's → None.
        assert_eq!(
            info_license_name(
                "info:\n  title: t\npaths: {}\ncomponents:\n  schemas:\n    S:\n      license:\n        name: X\n"
            ),
            None
        );

        // Non-vacuous floor: every registered spec declares a non-empty
        // info.license.name (all carry the CAMARA-template Apache-2.0 block), so
        // the contract test asserts over a real, non-empty population.
        for api in APIS {
            assert!(
                matches!(info_license_name(api.body), Some(Some(ref n)) if !n.is_empty()),
                "{} spec must declare a non-empty info.license.name",
                api.name
            );
        }
    }

    #[test]
    fn server_url_undefined_variables_extraction_rules() {
        // Unit-cover the `server_url_undefined_variables` extractor so the contract
        // test above can't pass vacuously and its scoping is pinned: a well-formed
        // server backs its `{apiRoot}` with a `default`; a missing `variables:`
        // block, a `variables:` entry with no `default:`, and one with a blank
        // `default:` are each flagged; only the *undefined* variable of several is
        // flagged; a `{…}` in a sibling `description:` and a deeper (post-block)
        // `url:` are not read as references.

        // Well-formed (the CAMARA-template form): apiRoot referenced and defined
        // with a non-empty default → nothing undefined.
        assert!(server_url_undefined_variables(
            "openapi: 3.0.3\nservers:\n  - url: \"{apiRoot}/x/v1\"\n    variables:\n      apiRoot:\n        default: http://localhost:8080\n        description: root\npaths: {}\n"
        )
        .is_empty());

        // Referenced but no `variables:` block at all → flagged.
        assert_eq!(
            server_url_undefined_variables("servers:\n  - url: \"{apiRoot}/x/v1\"\npaths: {}\n"),
            vec!["apiRoot".to_string()]
        );

        // Declared but with no `default:` child → flagged (default is REQUIRED).
        assert_eq!(
            server_url_undefined_variables(
                "servers:\n  - url: \"{apiRoot}/x/v1\"\n    variables:\n      apiRoot:\n        description: root\npaths: {}\n"
            ),
            vec!["apiRoot".to_string()]
        );

        // Declared with a blank `default:` → flagged (empty is not a value).
        assert_eq!(
            server_url_undefined_variables(
                "servers:\n  - url: \"{apiRoot}/x/v1\"\n    variables:\n      apiRoot:\n        default: \"\"\npaths: {}\n"
            ),
            vec!["apiRoot".to_string()]
        );

        // Two referenced, one undefined → only the undefined one is flagged.
        assert_eq!(
            server_url_undefined_variables(
                "servers:\n  - url: \"{scheme}://{apiRoot}/x/v1\"\n    variables:\n      apiRoot:\n        default: localhost\npaths: {}\n"
            ),
            vec!["scheme".to_string()]
        );

        // No `servers:` block at all → empty (no false positive).
        assert!(server_url_undefined_variables("openapi: 3.0.3\npaths: {}\n").is_empty());

        // A `{apiRoot}` mention in a sibling `description:` is not a url reference,
        // and a `{…}` in a deeper path `description:` after the servers block ends
        // (a column-zero `paths:` key) is outside the scanned block entirely.
        assert!(server_url_undefined_variables(
            "servers:\n  - url: \"{apiRoot}/x/v1\"\n    description: \"root is {unused}\"\n    variables:\n      apiRoot:\n        default: localhost\npaths:\n  /p:\n    get:\n      description: \"see {apiRoot}\"\n"
        )
        .is_empty());

        // Non-vacuous floor: every registered spec templates `{apiRoot}` in its
        // servers url (so the contract asserts over a real, non-empty reference
        // set) and defines it with a default (so nothing is undefined).
        for api in APIS {
            assert!(
                api.body.contains("url: \"{apiRoot}"),
                "{} spec must template {{apiRoot}} in its servers url",
                api.name
            );
            assert!(
                server_url_undefined_variables(api.body).is_empty(),
                "{} spec has an undefined server url variable",
                api.name
            );
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

    #[test]
    fn every_path_parameter_declares_required_true() {
        // Contract-harness invariant (OpenAPI structural rule): every `in: path`
        // parameter a mounted spec declares MUST carry `required: true`. OpenAPI
        // makes `required` OPTIONAL on a Parameter Object in general, but for a path
        // parameter it is REQUIRED and its value MUST be `true` — a path template
        // variable is not omissible, so a path parameter with no `required:` key (or
        // one set to `false`) is an invalid document a Redoc/Swagger/codegen client
        // rejects or mis-binds. This is a live copy-paste hazard: a new endpoint's
        // parameter block is drafted from a sibling, so a query parameter (whose
        // `required` defaults to/reads `false`) re-tagged `in: path`, or a path
        // parameter block that dropped its `required: true` line, slips past every
        // existing contract test — the path-templating test checks that path
        // variables and path parameters line up by *name*, never that each path
        // parameter is marked required; the responses/operationId/description tests
        // check operation and response fields, never parameter objects. Verified
        // true (all 42 `in: path` parameters across the 19 path-templating specs)
        // before asserting.
        for api in APIS {
            let missing = path_parameters_missing_required_true(api.body);
            assert!(
                missing.is_empty(),
                "{} spec declares `in: path` parameter(s) without `required: true`: \
                 {:?} — a path parameter MUST be `required: true` (OpenAPI)",
                api.name,
                missing
            );
        }
    }

    #[test]
    fn path_parameter_required_extraction_rules() {
        // Unit-cover `path_parameters_missing_required_true` so the contract test
        // above can't pass vacuously and its scoping is pinned: a path parameter
        // carrying `required: true` (sequence or mapping form, name-first or
        // in-first) is not flagged; one with `required: false`, or none at all, is
        // flagged; a `required: true` sitting inside a nested `schema:` (not the
        // parameter's own sibling of `in:`) does not satisfy it; and an `in: query`
        // parameter — whose `required` is genuinely optional — is never considered.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a/{good}:
    get:
      parameters:
        - name: good
          in: path
          required: true
  /b/{missing}:
    get:
      parameters:
        - name: missing
          in: path
          schema:
            type: string
  /c/{falsey}:
    get:
      parameters:
        - in: path
          name: falsey
          required: false
  /d/{nested}:
    get:
      parameters:
        - name: nested
          in: path
          schema:
            required: true
  /e:
    get:
      parameters:
        - name: q
          in: query
components:
  parameters:
    Legacy:
      name: legacyGood
      in: path
      required: true
";
        let flagged = path_parameters_missing_required_true(body);
        // `good` (sequence item, name-first) and `legacyGood` (bare mapping form)
        // carry `required: true` → not flagged. `missing` (no `required:` at all),
        // `falsey` (`required: false`, in-first sequence form), and `nested` (its
        // only `required: true` is nested inside `schema:`, not a sibling of `in:`)
        // → flagged. The `in: query` `q` is never a path parameter.
        assert!(flagged.contains(&"missing".to_string()), "flagged: {flagged:?}");
        assert!(flagged.contains(&"falsey".to_string()), "flagged: {flagged:?}");
        assert!(flagged.contains(&"nested".to_string()), "flagged: {flagged:?}");
        assert!(!flagged.contains(&"good".to_string()), "flagged: {flagged:?}");
        assert!(!flagged.contains(&"legacyGood".to_string()), "flagged: {flagged:?}");
        assert!(!flagged.contains(&"q".to_string()), "flagged: {flagged:?}");
        assert_eq!(
            flagged.len(),
            3,
            "exactly the three broken path parameters expected: {flagged:?}"
        );

        // Non-vacuous floor: every registered spec already satisfies the invariant
        // (no `in: path` parameter is missing `required: true`) — combined with the
        // positive cases above proving the extractor *does* flag real breaks, this
        // makes the contract test assert over a real, non-empty population rather
        // than an empty loop.
        for api in APIS {
            assert!(
                path_parameters_missing_required_true(api.body).is_empty(),
                "{}: every `in: path` parameter must declare `required: true`",
                api.name
            );
        }
    }

    #[test]
    fn every_operation_declares_a_responses_object() {
        // Contract-harness invariant (OpenAPI structural rule): every operation a
        // mounted spec declares MUST carry a `responses` object — it is the single
        // REQUIRED field of an Operation Object (summary/operationId/parameters are
        // all optional), so an operation without one is an invalid document: a
        // Redoc/Swagger/codegen client is handed an operation with no declared
        // outcome to render or bind. This is a live copy-paste hazard — a new
        // endpoint's spec is drafted from a sibling, so an operation block can be
        // pasted or edited with its `responses:` accidentally dropped or dedented
        // out of the operation — a drift no existing contract test sees: the
        // mount-path/version/parity/operationId/path-templating/`$ref` tests all
        // check a spec's identity, wiring, or path variables, never that each
        // operation declares its responses. Verified true (142 operations across
        // all mounted specs, none missing) before asserting.
        for api in APIS {
            let missing = operations_without_responses(api.body);
            assert!(
                missing.is_empty(),
                "{} spec has operation(s) with no `responses` object (the single \
                 REQUIRED field of an OpenAPI Operation Object): {:?}",
                api.name,
                missing
            );
        }
    }

    #[test]
    fn operations_without_responses_extraction_rules() {
        // Unit-cover the `operations_without_responses` extractor so the contract
        // test above can't pass vacuously (an extractor that returned an empty Vec
        // for every body would make its assertion meaningless) and so the
        // scoping/indentation rules are pinned: a `responses:` is credited only to
        // the operation whose block it sits in; an HTTP verb appearing as a schema
        // property name (not under a path item) is not an operation; and a bare
        // dedent ends an operation's block before a sibling path's `responses`.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
    post:
      operationId: postA
      summary: no responses here
  /b/{id}:
    delete:
      operationId: delB
      responses:
        '204':
          description: gone
components:
  schemas:
    Widget:
      type: object
      properties:
        get:
          type: string
";
        // `POST /a` is the only operation missing a `responses` block. `GET /a`
        // and `DELETE /b/{id}` each declare one; the `get` schema *property* under
        // `components.schemas.Widget` is not under `paths:`, so it is not an
        // operation and never counted.
        assert_eq!(operations_without_responses(body), vec!["POST /a".to_string()]);

        // Non-vacuous floor: across every registered spec, no operation is missing
        // its `responses` object (the invariant the contract test asserts) — and
        // the extractor sees a non-trivial number of operations overall, so a
        // broken extractor can't hide behind an empty scan.
        let mut total_ops = 0usize;
        for api in APIS {
            assert!(
                operations_without_responses(api.body).is_empty(),
                "{}: every operation must declare a `responses` object",
                api.name
            );
            total_ops += operation_ids(api.body).len();
        }
        assert!(total_ops >= 100, "expected many operations across specs, got {total_ops}");
    }

    #[test]
    fn every_operation_declares_an_operation_id() {
        // Contract-harness invariant (CAMARA API Design Guidelines + DESIGN §9):
        // every operation a mounted spec declares MUST carry an `operationId`. The
        // OpenAPI spec makes `operationId` optional, but CAMARA mandates it — it is
        // the operation's canonical name, the method name a codegen client derives,
        // and the key each simulator handler/scope narrative is written against.
        // The sibling `operation_ids_are_unique_within_each_spec` pins the *other*
        // half of the operationId contract (≥1 per spec, and none repeated *within*
        // a document) but never that *every* operation carries one: a spec with
        // three operations, two sharing an id and one with none, passes it (two
        // distinct ids, no duplicate). This closes that gap — a live copy-paste
        // hazard, since a new endpoint's spec is drafted from a sibling and an
        // operation block can be pasted or edited with its `operationId:` line
        // dropped, leaving an anonymous operation that codegen names arbitrarily.
        // No other contract test sees it: the responses test checks the one REQUIRED
        // Operation field, the path-templating/version/parity/`$ref` tests check a
        // spec's path variables, identity, or wiring. Verified true (every operation
        // across all mounted specs carries an operationId) before asserting.
        for api in APIS {
            let missing = operations_without_operation_id(api.body);
            assert!(
                missing.is_empty(),
                "{} spec has operation(s) with no `operationId` (CAMARA mandates an \
                 operationId on every operation): {:?}",
                api.name,
                missing
            );
        }
    }

    #[test]
    fn operations_without_operation_id_extraction_rules() {
        // Unit-cover the `operations_without_operation_id` extractor so the contract
        // test above can't pass vacuously (an extractor that returned an empty Vec
        // for every body would make its assertion meaningless) and so its
        // scoping/indentation rules are pinned: an `operationId:` is a scalar key
        // with an inline value (unlike `responses:`, whose block is the following
        // lines), credited only to the operation whose block it sits in; a longer
        // key such as `operationIdSuffix:` is not mistaken for it; and an HTTP verb
        // used as a schema property name (not under a path item) is not an operation.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
    post:
      summary: no operationId here
      responses:
        '201':
          description: created
  /b/{id}:
    delete:
      operationIdSuffix: notAnId
      responses:
        '204':
          description: gone
components:
  schemas:
    Widget:
      type: object
      properties:
        get:
          type: string
";
        // `POST /a` declares no operationId; `DELETE /b/{id}` has only a look-alike
        // `operationIdSuffix` key, so it is missing too. `GET /a` declares one; the
        // `get` schema *property* under `components.schemas.Widget` is not under
        // `paths:`, so it is not an operation and never counted.
        assert_eq!(
            operations_without_operation_id(body),
            vec!["POST /a".to_string(), "DELETE /b/{id}".to_string()]
        );

        // Non-vacuous floor: across every registered spec, no operation is missing
        // its `operationId` (the invariant the contract test asserts), and the
        // extractor sees a non-trivial number of operations overall, so a broken
        // extractor can't hide behind an empty scan.
        let mut total_ops = 0usize;
        for api in APIS {
            assert!(
                operations_without_operation_id(api.body).is_empty(),
                "{}: every operation must declare an `operationId`",
                api.name
            );
            total_ops += operation_ids(api.body).len();
        }
        assert!(total_ops >= 100, "expected many operations across specs, got {total_ops}");
    }

    #[test]
    fn every_operation_declares_a_summary() {
        // Contract-harness invariant (CAMARA API Design Guidelines + DESIGN §9):
        // every operation a mounted spec declares carries a `summary`. OpenAPI marks
        // it OPTIONAL, but it is the RECOMMENDED short label a Redoc/Swagger client
        // renders as the operation's name in the navigation sidebar and the codegen
        // hint many generators prefer over the operationId — every CAMARA operation
        // populates one. This is the RECOMMENDED-field member of the operation-field
        // series the sibling tests own: `every_operation_declares_a_responses_object`
        // pins the single REQUIRED field, `every_operation_declares_an_operation_id`
        // the CAMARA-mandated canonical name, and this the human-readable label. The
        // break it catches is a live copy-paste hazard invisible to both: a new
        // endpoint's spec is drafted from a sibling, so an operation block can be
        // pasted or edited with its `summary:` line dropped or dedented, leaving an
        // operation Redoc renders anonymously in its nav. No other contract test sees
        // it (the responses/operationId tests check the operation's other fields; the
        // path-templating/version/parity/`$ref` tests check a spec's path variables,
        // identity, or wiring, never an operation's label). Verified true (every
        // operation across all mounted specs carries a summary) before asserting.
        for api in APIS {
            let missing = operations_without_summary(api.body);
            assert!(
                missing.is_empty(),
                "{} spec has operation(s) with no `summary` (every CAMARA operation \
                 declares the short label Redoc renders in its nav): {:?}",
                api.name,
                missing
            );
        }
    }

    #[test]
    fn operations_without_summary_extraction_rules() {
        // Unit-cover the `operations_without_summary` extractor so the contract test
        // above can't pass vacuously and its scoping/indentation rules are pinned: a
        // `summary:` is a scalar key with an inline value, credited only to the
        // operation whose 6-space block it sits in; a Path Item Object's own 4-space
        // `summary`, and an `examples` entry's deeply-nested `summary`, do not count;
        // and an HTTP verb used as a schema property name is not an operation.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      summary: Get A
      responses:
        '200':
          description: ok
          content:
            application/json:
              examples:
                ok:
                  summary: a nested example label, not the operation's
                  value: {}
    post:
      operationId: postA
      responses:
        '201':
          description: created
  /b/{id}:
    summary: a path-item summary, not the operation's
    delete:
      operationId: deleteB
      responses:
        '204':
          description: gone
components:
  schemas:
    Widget:
      type: object
      properties:
        get:
          type: string
        summary:
          type: string
";
        // `GET /a` declares an operation-level summary; its nested example `summary`
        // is irrelevant. `POST /a` has none. `DELETE /b/{id}` has only the *path
        // item's* 4-space `summary`, not its own, so it is missing too. The `get`
        // and `summary` schema *properties* under `components` are not operations.
        assert_eq!(
            operations_without_summary(body),
            vec!["POST /a".to_string(), "DELETE /b/{id}".to_string()]
        );

        // Non-vacuous floor: across every registered spec, no operation is missing
        // its `summary` (the invariant the contract test asserts), and the extractor
        // sees a non-trivial number of operations overall, so a broken extractor
        // can't hide behind an empty scan.
        let mut total_ops = 0usize;
        for api in APIS {
            assert!(
                operations_without_summary(api.body).is_empty(),
                "{}: every operation must declare a `summary`",
                api.name
            );
            total_ops += operation_ids(api.body).len();
        }
        assert!(total_ops >= 100, "expected many operations across specs, got {total_ops}");
    }

    #[test]
    fn every_paths_object_declares_slash_prefixed_path_items() {
        // Contract-harness invariant (OpenAPI structural rule): a document's
        // `paths` object maps *path templates* to Path Item Objects, and every
        // such key MUST begin with a forward slash — it is a URL path resolved
        // relative to the API's server URL. A spec must also declare at least
        // one path item: a `paths:` block with none describes no operation, so
        // it is not a usable API document.
        //
        // Both halves are drifts no existing contract test sees. A new
        // endpoint's spec is drafted by copy-pasting a sibling's path block, so
        // a path key can be pasted or edited with its leading `/` dropped
        // (`sessions:` instead of `/sessions:`). Every operation-scoped test —
        // `operations_without_responses`, `operations_without_operation_id`,
        // `path_template_params_match_declared_path_parameters` — treats only a
        // 2-space key that *already* begins with `/` as a path item, so a
        // non-slash key contributes zero operations and every one of those tests
        // passes it *vacuously* (no operations found → nothing missing). And no
        // test asserts a spec declares any path at all — a spec whose only path
        // key lost its slash, or that carries an empty `paths:` block, would
        // otherwise sail through the whole harness describing nothing. Verified
        // true across all mounted specs before asserting.
        for api in APIS {
            let keys = path_item_keys(api.body);
            assert!(
                !keys.is_empty(),
                "{} spec declares no path items under `paths:` — a document \
                 that describes no operation is not a usable API spec",
                api.name
            );
            for key in &keys {
                assert!(
                    key.starts_with('/'),
                    "{} spec has a `paths:` key `{}` that does not begin with \
                     `/` — an OpenAPI path template must be slash-prefixed \
                     (resolved relative to the server URL); a client/codegen \
                     tool cannot bind a non-slash path, and every \
                     operation-scoped contract test skips it silently",
                    api.name,
                    key
                );
            }
        }
    }

    #[test]
    fn path_item_key_extraction_rules() {
        // Unit-cover the `path_item_keys` extractor so the contract test above
        // can't pass vacuously (an extractor returning an empty Vec for every
        // body would make its assertion meaningless) and so its scoping is
        // pinned: only 2-space direct children of the top-level `paths:` block
        // are path items; deeper method/parameter keys are not; an `x-`
        // Paths-Object extension is excluded; and a `/`-looking key elsewhere (a
        // schema property under `components:`) is not under `paths:`.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /sessions:
    post:
      operationId: create
      responses:
        '201':
          description: made
  /sessions/{id}:
    get:
      operationId: read
      responses:
        '200':
          description: ok
  x-paths-note: not a path item
components:
  schemas:
    Thing:
      type: object
      properties:
        /weird:
          type: string
";
        // The two `/…` path items are extracted; the `x-paths-note` Paths-Object
        // extension and the `/weird` schema *property* (a deeper child of
        // `components`, not under `paths:`) are excluded.
        let mut keys = path_item_keys(body);
        keys.sort();
        assert_eq!(
            keys,
            vec!["/sessions".to_string(), "/sessions/{id}".to_string()]
        );

        // Non-vacuous floor: across every registered spec, every path item key
        // is slash-prefixed and each spec declares at least one — the invariant
        // the contract test asserts — and the extractor sees many keys overall,
        // so a broken extractor can't hide behind an empty scan.
        let mut total = 0usize;
        for api in APIS {
            let ks = path_item_keys(api.body);
            assert!(!ks.is_empty(), "{}: expected ≥1 path item", api.name);
            for k in &ks {
                assert!(
                    k.starts_with('/'),
                    "{}: path key `{}` is not slash-prefixed",
                    api.name,
                    k
                );
            }
            total += ks.len();
        }
        assert!(total >= 100, "expected many path items across specs, got {total}");
    }

    #[test]
    fn every_declared_response_has_a_description() {
        // Contract-harness invariant (OpenAPI structural rule): every response a
        // mounted spec declares MUST carry a `description` — it is the single
        // REQUIRED field of a Response Object (`headers`/`content`/`links` are all
        // optional), so an inline response without one is an invalid document: a
        // Redoc/Swagger/codegen client is handed an outcome with no human-readable
        // summary to render. A response given as a `$ref` is exempt — it inherits
        // its description from the referenced component (the shared `errors.yaml`
        // error responses are all `$ref`'d this way). This closes the gap the
        // sibling `every_operation_declares_a_responses_object` leaves: that pins
        // the *presence* of the `responses` object, never that each response
        // *within* it is a valid Response Object. A live copy-paste hazard — a new
        // status branch is drafted by pasting a sibling response and can lose or
        // dedent its `description:` line — that no other contract test sees: the
        // responses/operationId tests check the operation's own required fields, the
        // path-templating/version/parity/`$ref` tests check a spec's path variables,
        // identity, or wiring, never that each declared response describes itself.
        // Verified true (1157 response entries across all mounted specs, none
        // missing) before asserting.
        for api in APIS {
            let missing = responses_missing_description(api.body);
            assert!(
                missing.is_empty(),
                "{} spec has response(s) with neither a `description` (the single \
                 REQUIRED field of an OpenAPI Response Object) nor a `$ref`: {:?}",
                api.name,
                missing
            );
        }
    }

    #[test]
    fn responses_missing_description_extraction_rules() {
        // Unit-cover the `responses_missing_description` extractor so the contract
        // test above can't pass vacuously (an extractor returning an empty Vec for
        // every body would make its assertion meaningless) and so its scoping is
        // pinned: a `description`/`$ref` counts only at the Response Object's own
        // child indent (10), so one nested deeper — inside a `content` schema or a
        // `headers` entry — never satisfies the response; a `$ref` response is
        // exempt; a `default` / `NXX` key is a response entry; and a non-status key
        // under `responses:` is not.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
        '400':
          $ref: \"../../shared/errors.yaml#/components/responses/BadRequest\"
    post:
      operationId: postA
      responses:
        '201':
          content:
            application/json:
              schema:
                type: object
                description: a schema description, not the response's own
        default:
          description: fallback
  /b:
    get:
      operationId: getB
      responses:
        '200':
          headers:
            x-correlator:
              description: a header description, not the response's own
components:
  schemas:
    Widget:
      type: object
      properties:
        get:
          type: string
";
        // Flagged: `POST /a 201` (its only `description` sits at indent 16 inside a
        // schema, not at the response's own child indent 10) and `GET /b 200` (its
        // `description` sits inside a `headers` entry). Not flagged: `GET /a 200`
        // (inline description), `GET /a 400` (a `$ref`, exempt), `POST /a default`
        // (a `default` response with a description). The `get` schema *property*
        // under `components.schemas.Widget` is not under `paths:`, so it is never an
        // operation.
        assert_eq!(
            responses_missing_description(body),
            vec!["POST /a 201".to_string(), "GET /b 200".to_string()]
        );

        // Non-vacuous floor: across every registered spec, no declared response is
        // missing its `description` (the invariant the contract test asserts) — and
        // the corpus carries many operations (hence many response entries), so a
        // broken extractor can't hide behind an empty scan.
        let mut total_ops = 0usize;
        for api in APIS {
            assert!(
                responses_missing_description(api.body).is_empty(),
                "{}: every declared response must carry a `description` or be a `$ref`",
                api.name
            );
            total_ops += operation_ids(api.body).len();
        }
        assert!(total_ops >= 100, "expected many operations across specs, got {total_ops}");
    }

    #[test]
    fn every_responses_object_key_is_a_valid_status() {
        // Contract-harness invariant (OpenAPI structural rule): every key of an
        // operation's `responses:` map MUST be an HTTP status code (`"200"`), an
        // `NXX` wildcard range (`"1XX"`..`"5XX"`), the `default` key, or an `x-`
        // specification extension — the Responses Object admits nothing else. A key
        // that is none of those is an invalid document: a Redoc/Swagger/codegen
        // client has no outcome to bind a non-status key to.
        //
        // This closes the gap the sibling `every_declared_response_has_a_description`
        // leaves. That test *finds* the responses it checks through the same
        // `is_status_key` filter, so a key it does not recognise is silently skipped
        // there — and it is exactly such an unrecognised key that this test catches: a
        // status code typo'd into an invalid token (`"4O4"` with a letter O, an
        // out-of-range `"600"`, a truncated `"20"`), all live copy-paste/edit hazards.
        // No other contract test sees it either: the responses/operationId/
        // path-templating/version/parity/`$ref` tests check the operation's own
        // required fields, path variables, identity, or wiring, never that each
        // `responses:` key is itself a well-formed status. Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let invalid = responses_with_invalid_status_key(api.body);
            assert!(
                invalid.is_empty(),
                "{} spec has `responses:` key(s) that are not a valid HTTP status \
                 code / `NXX` range / `default` / `x-` extension: {:?}",
                api.name,
                invalid
            );
        }
    }

    #[test]
    fn responses_invalid_status_key_extraction_rules() {
        // Unit-cover the `responses_with_invalid_status_key` extractor so the
        // contract test above can't pass vacuously and its scoping is pinned: only an
        // 8-space key directly under an operation's `responses:` is judged; a valid
        // status code, an `NXX` wildcard, `default`, and an `x-` extension are all
        // allowed; an invalid token (a letter, out-of-range, truncated) is flagged;
        // and a status-looking key outside `paths:` is never a response.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
        '2XX':
          description: range ok
        default:
          description: fallback
        x-vendor-note:
          description: an extension key, allowed
    post:
      operationId: postA
      responses:
        '4O4':
          description: typo, a letter O not a zero
        '600':
          description: out of range
        '20':
          description: truncated
components:
  schemas:
    Widget:
      type: object
      properties:
        '200':
          type: string
";
        // Flagged: the three invalid POST /a keys, in document order. Not flagged:
        // every GET /a key (a valid code, an `NXX` wildcard, `default`, an `x-`
        // extension). The `'200'` *property* under components.schemas.Widget is not
        // under `paths:`, so it is never a response key.
        assert_eq!(
            responses_with_invalid_status_key(body),
            vec![
                "POST /a 4O4".to_string(),
                "POST /a 600".to_string(),
                "POST /a 20".to_string()
            ]
        );

        // Non-vacuous floor: across every registered spec, no `responses:` key is
        // invalid (the invariant the contract test asserts), and the corpus carries
        // many operations, so a broken extractor can't hide behind an empty scan.
        let mut total_ops = 0usize;
        for api in APIS {
            assert!(
                responses_with_invalid_status_key(api.body).is_empty(),
                "{}: every `responses:` key must be a valid status code, `NXX`, \
                 `default`, or `x-` extension",
                api.name
            );
            total_ops += operation_ids(api.body).len();
        }
        assert!(total_ops >= 100, "expected many operations across specs, got {total_ops}");
    }

    #[test]
    fn every_operation_declares_a_success_response() {
        // Contract-harness invariant (CAMARA convention over the OpenAPI Responses
        // Object): every operation a mounted spec declares whose `responses:` object
        // is present MUST document at least one **success** outcome — a `2XX` status
        // code (or the `2XX` wildcard). Every CAMARA business operation returns a
        // concrete happy-path `2XX` (`200`/`201`/`202`/`204`); that entry is the
        // return type a Redoc/Swagger/codegen client derives, so an operation
        // declaring only its error branches (the shared `errors.yaml` `4XX`/`5XX`
        // `$ref`s) is an incomplete contract.
        //
        // This closes a gap the three sibling responses tests leave open together: a
        // happy-path `2XX` block lost or dedented in the paste/edit that drafts a new
        // operation still passes `every_operation_declares_a_responses_object` (the
        // object is present, full of error entries),
        // `every_responses_object_key_is_a_valid_status` (every remaining key is a
        // well-formed status), and `every_declared_response_has_a_description` (the
        // `$ref`'d error responses are exempt). None of them — nor the operationId/
        // path-templating/version/parity/`$ref` tests — requires a success outcome to
        // exist. An operation missing its `responses:` object entirely is
        // `every_operation_declares_a_responses_object`'s concern (via
        // `operations_without_responses`), so the two never double-flag. Verified
        // true across all mounted specs before asserting.
        for api in APIS {
            let missing = operations_without_success_response(api.body);
            assert!(
                missing.is_empty(),
                "{} spec has operation(s) whose `responses:` declares no success \
                 (`2XX`) outcome — an incomplete contract (only error responses \
                 documented): {:?}",
                api.name,
                missing
            );
        }
    }

    #[test]
    fn operations_without_success_response_extraction_rules() {
        // Unit-cover the `operations_without_success_response` extractor so the
        // contract test above can't pass vacuously (an extractor returning an empty
        // Vec for every body would make its assertion meaningless) and its scoping is
        // pinned: a success is a `2XX`-range code or the `2XX` wildcard among an
        // operation's 8-space `responses:` keys; an operation declaring only error
        // responses is flagged; a `content`/`schema` nested under a `requestBody`
        // is never a response key; a `2XX`-looking key outside `paths:` (a schema
        // property literally named `'200'`) is not a response; and an operation with
        // no `responses:` block is left to the responses-object test (not flagged).
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
        '404':
          $ref: \"../../shared/errors.yaml#/components/responses/NotFound\"
    post:
      operationId: postA
      responses:
        '400':
          $ref: \"../../shared/errors.yaml#/components/responses/BadRequest\"
        '404':
          $ref: \"../../shared/errors.yaml#/components/responses/NotFound\"
  /b:
    put:
      operationId: putB
      responses:
        '2XX':
          description: a wildcard success range
        default:
          description: fallback
    delete:
      operationId: deleteB
      requestBody:
        content:
          application/json:
            schema:
              type: object
      responses:
        '204':
          description: no content
components:
  schemas:
    Widget:
      type: object
      properties:
        '200':
          type: string
";
        // Flagged: only `POST /a` — its `responses:` declares `400`/`404` but no
        // `2XX`. Not flagged: `GET /a` (`200`), `PUT /b` (`2XX` wildcard),
        // `DELETE /b` (`204`, past a `requestBody` whose nested `content`/`schema`
        // keys are not response keys). The `'200'` *property* under
        // components.schemas.Widget is not under `paths:`, so it is never a response.
        assert_eq!(
            operations_without_success_response(body),
            vec!["POST /a".to_string()]
        );

        // An operation with no `responses:` block at all is not flagged here (that is
        // the responses-object test's concern), so the two never double-flag the
        // same operation.
        let no_responses = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /c:
    get:
      operationId: getC
      summary: no responses object at all
";
        assert!(operations_without_success_response(no_responses).is_empty());

        // Non-vacuous floor: across every registered spec, every operation with a
        // `responses:` object declares a success outcome (the invariant the contract
        // test asserts), and the corpus carries many operations, so a broken
        // extractor can't hide behind an empty scan.
        let mut total_ops = 0usize;
        for api in APIS {
            assert!(
                operations_without_success_response(api.body).is_empty(),
                "{}: every operation's `responses:` must declare a success (`2XX`) outcome",
                api.name
            );
            total_ops += operation_ids(api.body).len();
        }
        assert!(total_ops >= 100, "expected many operations across specs, got {total_ops}");
    }

    #[test]
    fn every_request_body_declares_content() {
        // Contract-harness invariant (OpenAPI structural rule): every operation a
        // mounted spec declares whose `requestBody` object is spelled out inline
        // MUST carry a `content` field — it is the single REQUIRED field of an
        // OpenAPI Request Body Object (`description`/`required` are optional), so a
        // `requestBody:` block without it is an invalid document: a Redoc/Swagger/
        // codegen client is handed an operation that consumes a body of no declared
        // media type or schema. A `requestBody` given as a `$ref` is exempt — it
        // inherits its `content` from the referenced component.
        //
        // This is the request-side analogue of the sibling
        // `every_declared_response_has_a_description` (the required field of a
        // *Response* Object), and no other contract test sees the break it catches:
        // the CAMARA business operations are almost all POSTs carrying a request
        // body, and a new one is routinely drafted by pasting a sibling operation —
        // so a `content:` line lost or dedented in that paste leaves a bodiless
        // `requestBody` the responses/operationId/path-templating/version/parity/
        // `$ref` tests never inspect (they check the operation's responses, id, path
        // variables, identity, or wiring, never its request body's shape). Only
        // operations that *declare* a `requestBody` are judged (a GET/DELETE with
        // none is fine). Verified true (all 95 request bodies across the mounted
        // specs carry `content`) before asserting.
        for api in APIS {
            let missing = request_bodies_missing_content(api.body);
            assert!(
                missing.is_empty(),
                "{} spec has operation(s) whose `requestBody` carries neither a \
                 `content` (the single REQUIRED field of an OpenAPI Request Body \
                 Object) nor a `$ref`: {:?}",
                api.name,
                missing
            );
        }
    }

    #[test]
    fn request_bodies_missing_content_extraction_rules() {
        // Unit-cover the `request_bodies_missing_content` extractor so the contract
        // test above can't pass vacuously (an extractor returning an empty Vec for
        // every body would make its assertion meaningless) and its scoping is
        // pinned: `content`/`$ref` counts only at the Request Body Object's own
        // child indent (8), so one nested deeper — inside a media type's `schema` —
        // never satisfies it; a `$ref` request body is exempt; an operation with no
        // `requestBody` is not flagged; and a `requestBody:` outside `paths:` is not
        // an operation's.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    post:
      operationId: postA
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
      responses:
        '200':
          description: ok
    put:
      operationId: putA
      requestBody:
        required: true
        description: a body whose only `content` sits inside the schema below
        x-note:
          content:
            application/json:
              schema:
                type: object
      responses:
        '200':
          description: ok
  /b:
    post:
      operationId: postB
      requestBody:
        $ref: \"#/components/requestBodies/Shared\"
      responses:
        '200':
          description: ok
    get:
      operationId: getB
      responses:
        '200':
          description: ok
components:
  requestBodies:
    Shared:
      required: true
      content:
        application/json:
          schema:
            type: object
";
        // Flagged: only `PUT /a` — its `requestBody` declares `required`/`description`
        // but its sole `content:` sits at indent 10 inside an `x-note` block, not at
        // the request body object's own child indent 8. Not flagged: `POST /a` (an
        // 8-space `content:`), `POST /b` (a `$ref` request body, exempt), `GET /b`
        // (no `requestBody`). The `Shared` request body under
        // `components.requestBodies` is not under `paths:`, so it is never an
        // operation's request body.
        assert_eq!(
            request_bodies_missing_content(body),
            vec!["PUT /a".to_string()]
        );

        // Non-vacuous floor: across every registered spec, no declared request body
        // is missing its `content` (the invariant the contract test asserts), and
        // the corpus carries many operations, so a broken extractor can't hide
        // behind an empty scan.
        let mut total_ops = 0usize;
        for api in APIS {
            assert!(
                request_bodies_missing_content(api.body).is_empty(),
                "{}: every declared request body must carry `content` or be a `$ref`",
                api.name
            );
            total_ops += operation_ids(api.body).len();
        }
        assert!(total_ops >= 100, "expected many operations across specs, got {total_ops}");
    }

    #[test]
    fn every_parameter_declares_a_valid_location() {
        // Contract-harness invariant (OpenAPI structural rule): every parameter a
        // mounted spec declares MUST carry an `in` whose value is one of the fixed
        // enum `query` / `header` / `path` / `cookie` — the location the parameter
        // is passed. Any other value is an invalid document: a Redoc/Swagger/codegen
        // client has no location to bind the parameter to.
        //
        // The break it catches is a migration / copy-paste hazard no sibling test
        // sees. The parameter tests that exist — `path_template_params_match_declared_
        // path_parameters` and `every_path_parameter_declares_required_true` — only
        // ever look at `in: path`, so a parameter whose location is one OpenAPI 3
        // removed (a Swagger-2.0 `in: body`/`in: formData`, pasted from an old
        // template) or simply typo'd (`in: quiery`) is invisible to them and to the
        // responses/operationId/version/parity/`$ref` tests (which check an
        // operation's outcomes, id, identity, or wiring, never a parameter's
        // location). Verified true across all mounted specs before asserting.
        for api in APIS {
            let invalid = parameters_with_invalid_location(api.body);
            assert!(
                invalid.is_empty(),
                "{} spec declares parameter(s) whose `in` is not a valid OpenAPI 3 \
                 location (`query`/`header`/`path`/`cookie`): {:?}",
                api.name,
                invalid
            );
        }
    }

    #[test]
    fn parameter_location_extraction_rules() {
        // Unit-cover the `parameters_with_invalid_location` extractor so the contract
        // test above can't pass vacuously and its detection is pinned: the four valid
        // locations pass in both the mapping (`in: path`) and sequence (`- in: query`)
        // forms and when quoted; the Swagger-2.0 `in: body`/`in: formData` and a
        // typo'd location are flagged in document order; an `in:` opening a nested
        // block (a schema property named `in`) is not a location; and `info:` (sharing
        // the `in` prefix) is never mistaken for one.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      parameters:
        - name: x-correlator
          in: header
        - name: id
          in: path
          required: true
        - name: filter
          in: \"query\"
      responses:
        '200':
          description: ok
    post:
      operationId: postA
      parameters:
        - name: legacyBody
          in: body
        - name: upload
          in: formData
        - name: where
          in: quiery
      responses:
        '200':
          description: ok
components:
  schemas:
    Widget:
      type: object
      properties:
        in:
          type: string
";
        // Flagged: exactly the three invalid POST /a locations, in document order.
        // Not flagged: the GET /a `header`/`path`/quoted `query`; the `in:` property
        // of components.schemas.Widget (it opens a nested `type:` block, so it has no
        // inline scalar and is not a parameter location); the `info:` key.
        assert_eq!(
            parameters_with_invalid_location(body),
            vec![
                "body@line 24".to_string(),
                "formData@line 26".to_string(),
                "quiery@line 28".to_string()
            ]
        );

        // Non-vacuous floor: across every registered spec, no parameter `in` is an
        // invalid location (the invariant the contract test asserts), and the corpus
        // actually declares many parameters, so a broken extractor can't hide behind
        // an empty scan. Count parameter `in:` lines with the same detection the
        // extractor uses, over the whole corpus.
        let mut total_params = 0usize;
        for api in APIS {
            assert!(
                parameters_with_invalid_location(api.body).is_empty(),
                "{}: every parameter `in` must be query/header/path/cookie",
                api.name
            );
            for line in api.body.lines() {
                let bare = line.trim_start();
                let key = bare.strip_prefix("- ").unwrap_or(bare);
                if let Some(rest) = key.strip_prefix("in:") {
                    if !rest.trim().is_empty() {
                        total_params += 1;
                    }
                }
            }
        }
        assert!(
            total_params >= 50,
            "expected many declared parameters across specs, got {total_params}"
        );
    }

    #[test]
    fn every_parameter_declares_a_name() {
        // Contract-harness invariant (OpenAPI structural rule): every parameter a
        // mounted spec declares MUST carry a `name` — the other REQUIRED field of a
        // Parameter Object alongside `in`. A located parameter with no name is an
        // invalid document: a Redoc/Swagger/codegen client is handed a slot with a
        // location but no identity, so it can't bind or generate it.
        //
        // This is the exact complement of the sibling
        // `every_parameter_declares_a_valid_location`: that pins the `in` half of
        // the two-field contract (every parameter's location is a valid enum), this
        // pins the `name` half (every located parameter names itself). The break it
        // catches is a live copy-paste hazard no other test sees — a parameter block
        // pasted from a sibling that loses or dedents its `name:` line while keeping
        // its `in:` — invisible to the location test (checks only the `in` value),
        // the path-parameter tests (line up `in: path` variables by a name they
        // assume present), and the responses/operationId/version/parity/`$ref` tests
        // (which check an operation's outcomes, id, identity, or wiring, never a
        // parameter's identity). Verified true across all mounted specs before
        // asserting.
        for api in APIS {
            let unnamed = parameters_missing_name(api.body);
            assert!(
                unnamed.is_empty(),
                "{} spec declares parameter(s) with a valid `in` location but no \
                 `name` (the other REQUIRED field of an OpenAPI Parameter Object): \
                 {:?}",
                api.name,
                unnamed
            );
        }
    }

    #[test]
    fn parameter_name_extraction_rules() {
        // Unit-cover the `parameters_missing_name` extractor so the contract test
        // above can't pass vacuously and its detection is pinned: a parameter is
        // flagged only when its object (anchored on a valid `in:` location) carries
        // no `name:` sibling — in either the name-first or in-first sequence form
        // and the mapping (components.parameters) form; a `name` nested inside the
        // parameter's own `schema:` does NOT satisfy it; a `$ref` parameter (no
        // inline `in`) is exempt; and an `in:` opening a nested block (a schema
        // property named `in`) is never anchored.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      parameters:
        - name: x-correlator
          in: header
        - in: path
          name: id
          required: true
        - in: query
          required: false
          schema:
            type: object
            properties:
              name:
                type: string
        - $ref: '#/components/parameters/Shared'
      responses:
        '200':
          description: ok
components:
  parameters:
    Shared:
      name: shared
      in: query
      schema:
        type: string
  schemas:
    Widget:
      type: object
      properties:
        in:
          type: string
";
        // Flagged: only the third GET /a parameter — an `in: query` whose object's
        // sole `name:` sits deeper inside its `schema.properties` (not the
        // parameter's own name). Not flagged: the name-first `header`, the in-first
        // `path` (its `name: id` sibling), the `$ref` parameter (no inline `in`, so
        // never anchored), the well-formed mapping `Shared` (`name: shared`), and
        // the `in:` property of components.schemas.Widget (opens a nested block, no
        // inline scalar).
        assert_eq!(parameters_missing_name(body), vec!["query@line 15".to_string()]);

        // Non-vacuous floor: across every registered spec, every located parameter
        // declares a name (the invariant the contract test asserts), and the corpus
        // actually declares many parameter objects, so a broken extractor can't hide
        // behind an empty scan.
        const LOCATIONS: [&str; 4] = ["query", "header", "path", "cookie"];
        let mut total_located = 0usize;
        for api in APIS {
            assert!(
                parameters_missing_name(api.body).is_empty(),
                "{}: every located parameter must declare a `name`",
                api.name
            );
            for line in api.body.lines() {
                let bare = line.trim_start();
                let key = bare.strip_prefix("- ").unwrap_or(bare);
                if let Some(rest) = key.strip_prefix("in:") {
                    let loc = rest.trim().trim_matches('"').trim_matches('\'');
                    if LOCATIONS.contains(&loc) {
                        total_located += 1;
                    }
                }
            }
        }
        assert!(
            total_located >= 50,
            "expected many located parameters across specs, got {total_located}"
        );
    }

    #[test]
    fn every_parameter_declares_a_schema_or_content() {
        // Contract-harness invariant (OpenAPI structural rule): every parameter a
        // mounted spec declares MUST carry exactly one of `schema` or `content` — the
        // field that types the parameter's value. Alongside `in` (location) and
        // `name` (identity), a Parameter Object's value-type is REQUIRED: a located,
        // named parameter with neither `schema` nor `content` declares no type at all,
        // so a Redoc/Swagger/codegen client cannot bind or serialise it.
        //
        // Completes the Parameter Object required-field trio the two sibling tests
        // begin — `every_parameter_declares_a_valid_location` (the `in` half) and
        // `every_parameter_declares_a_name` (the `name` half). The break it catches is
        // a live copy-paste hazard neither sees: a parameter block pasted from a
        // sibling that keeps `name:`/`in:` but loses or dedents its `schema:` line (or
        // whose `content:` media-type block was trimmed) — invisible to the
        // location/name tests (which check a parameter's identity, not its type) and
        // to the responses/operationId/version/parity/`$ref` tests. Verified true
        // across all mounted specs before asserting.
        for api in APIS {
            let untyped = parameters_missing_schema_or_content(api.body);
            assert!(
                untyped.is_empty(),
                "{} spec declares parameter(s) with a valid `in` location but neither \
                 a `schema` nor a `content` (an OpenAPI Parameter Object MUST declare \
                 one): {:?}",
                api.name,
                untyped
            );
        }
    }

    #[test]
    fn parameter_schema_or_content_extraction_rules() {
        // Unit-cover the `parameters_missing_schema_or_content` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a
        // parameter is flagged only when its object (anchored on a valid `in:`
        // location) carries neither a `schema:` nor a `content:` sibling — in either
        // sequence form and the mapping (components.parameters) form; a `schema:`
        // nested inside a `content:` media type does NOT count as the parameter's own
        // type, a `content`-typed parameter is accepted, and a `$ref` parameter (no
        // inline `in`) is exempt.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      parameters:
        - name: x-correlator
          in: header
          schema:
            type: string
        - in: query
          name: filter
          content:
            application/json:
              schema:
                type: object
        - name: bare
          in: query
          required: true
        - $ref: '#/components/parameters/Shared'
      responses:
        '200':
          description: ok
components:
  parameters:
    Shared:
      name: shared
      in: query
      schema:
        type: string
";
        // Flagged: only the third GET /a parameter — an `in: query` whose object
        // carries only `name`/`required` and no `schema:`/`content:`. Not flagged: the
        // name-first `header` (its `schema:` sibling), the in-first `query` (its
        // `content:` sibling, whose nested media-type `schema:` sits deeper and is not
        // the parameter's own), the `$ref` parameter (no inline `in`, never anchored),
        // and the well-formed mapping `Shared` (`schema:` sibling).
        assert_eq!(
            parameters_missing_schema_or_content(body),
            vec!["query@line 21".to_string()]
        );

        // Non-vacuous floor: across every registered spec, every located parameter
        // declares a `schema` or `content` (the invariant the contract test asserts),
        // and the corpus actually declares many parameter objects, so a broken
        // extractor can't hide behind an empty scan.
        const LOCATIONS: [&str; 4] = ["query", "header", "path", "cookie"];
        let mut total_located = 0usize;
        for api in APIS {
            assert!(
                parameters_missing_schema_or_content(api.body).is_empty(),
                "{}: every located parameter must declare a `schema` or `content`",
                api.name
            );
            for line in api.body.lines() {
                let bare = line.trim_start();
                let key = bare.strip_prefix("- ").unwrap_or(bare);
                if let Some(rest) = key.strip_prefix("in:") {
                    let loc = rest.trim().trim_matches('"').trim_matches('\'');
                    if LOCATIONS.contains(&loc) {
                        total_located += 1;
                    }
                }
            }
        }
        assert!(
            total_located >= 50,
            "expected many located parameters across specs, got {total_located}"
        );
    }

    #[test]
    fn every_component_key_is_a_valid_name() {
        // Contract-harness invariant (OpenAPI structural rule): every key of a
        // `components` sub-object a mounted spec declares — a schema, response,
        // parameter, requestBody, header, securityScheme, example, link or callback
        // name — MUST match `^[a-zA-Z0-9._-]+$`. A key bearing any other character
        // (a space, `/`, `#`) is an invalid document: it can never be legally
        // `$ref`'d, because a JSON Pointer built from it doesn't resolve, so the
        // component is unreachable however correctly its body is defined.
        //
        // This is the definition-side complement of the ref-resolution tests
        // (`shared_error_refs_resolve_to_defined_components`,
        // `local_component_refs_resolve_within_their_own_spec`): those check that a
        // spec's `$ref`s *point at* a defined component, never that the component
        // *definition's own key* is a legal name. The break it catches is a
        // vendoring / copy-paste hazard invisible to every sibling — an
        // invalidly-named component is unseen by the ref tests (one never referenced
        // is not dereferenced at all; one whose only illegal char is non-whitespace,
        // e.g. `/`, is even collected as "defined" by `component_pointers`, so a ref
        // to it resolves there), and its key's character set is checked by no
        // identity / wiring / parameter / response / operationId test. Verified true
        // across all mounted specs before asserting.
        for api in APIS {
            let invalid = components_with_invalid_names(api.body);
            assert!(
                invalid.is_empty(),
                "{} spec declares component(s) whose key is not a valid OpenAPI 3 \
                 component name (`^[A-Za-z0-9._-]+$`), so it can never be `$ref`'d: {:?}",
                api.name,
                invalid
            );
        }
    }

    #[test]
    fn component_name_validity_extraction_rules() {
        // Unit-cover the `components_with_invalid_names` extractor so the contract
        // test above can't pass vacuously and its detection is pinned: only an
        // exact-4-space section-child key (a component name) is judged, in document
        // order; a space- or slash-bearing name is flagged across sections; the
        // allowed `.`/`-`/`_`/digits pass; and no deeper property/field of a schema
        // (at indent >= 6) is ever mistaken for a component key.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
components:
  schemas:
    ValidName:
      type: object
      properties:
        bad key:
          type: string
    Pet Info:
      type: object
    Broken/Name:
      type: object
    dotted.name-ok_1:
      type: object
  responses:
    Also Bad:
      description: x
";
        // Flagged in document order: the space-bearing `Pet Info` and slash-bearing
        // `Broken/Name` under `schemas`, and the space-bearing `Also Bad` under
        // `responses`. Not flagged: `ValidName`, `dotted.name-ok_1` (`.`/`-`/`_`/
        // digits all allowed), and — crucially — the `bad key` *property* of
        // `ValidName` (at indent 8, inside `properties:`, not a component key).
        assert_eq!(
            components_with_invalid_names(body),
            vec![
                "#/components/schemas/Pet Info".to_string(),
                "#/components/schemas/Broken/Name".to_string(),
                "#/components/responses/Also Bad".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec, every component key is a
        // valid OpenAPI 3 name (the invariant the contract test asserts), and the
        // corpus actually declares many components, so a broken extractor can't hide
        // behind an empty scan.
        let mut total_components = 0usize;
        for api in APIS {
            assert!(
                components_with_invalid_names(api.body).is_empty(),
                "{}: every component key must match ^[A-Za-z0-9._-]+$",
                api.name
            );
            total_components += component_pointers(api.body).len();
        }
        assert!(
            total_components >= 50,
            "expected many components across specs, got {total_components}"
        );
    }

    #[test]
    fn every_components_section_is_a_valid_field() {
        // Contract-harness invariant (OpenAPI structural rule): every direct child
        // key of a mounted spec's top-level `components:` object MUST be one of the
        // fixed Components Object fields — `schemas`, `responses`, `parameters`,
        // `examples`, `requestBodies`, `headers`, `securitySchemes`, `links`,
        // `callbacks` (plus `pathItems` in 3.1) — or a `x-` Specification Extension.
        // A section under any other key (a typo'd `shemas:`, a Swagger-2.0
        // `definitions:` pasted from an old template) is an invalid document: every
        // component nested under it is unreachable, because a `$ref` addresses a
        // component only through the canonical `#/components/<field>/<Name>` path.
        //
        // This is the section-side complement of `every_component_key_is_a_valid_name`,
        // which validates the component *keys within* a section but never the section
        // key itself — and of the ref-resolution tests
        // (`shared_error_refs_resolve_to_defined_components`,
        // `local_component_refs_resolve_within_their_own_spec`), which dereference a
        // spec's `$ref`s: a ref into a mistyped section simply dangles, and the
        // components under it are still collected by `component_pointers` under the
        // wrong field, so no sibling notices the section name is wrong. Verified true
        // across all mounted specs before asserting.
        for api in APIS {
            let invalid = components_with_invalid_section_names(api.body);
            assert!(
                invalid.is_empty(),
                "{} spec declares a `components` section that is not a valid OpenAPI 3 \
                 Components Object field, so its components are unreachable: {:?}",
                api.name,
                invalid
            );
        }
    }

    #[test]
    fn component_section_name_validity_extraction_rules() {
        // Unit-cover the `components_section_names` / `components_with_invalid_section_
        // names` extractors so the contract test above can't pass vacuously and their
        // detection is pinned: only a 2-space direct child of the top-level
        // `components:` block is treated as a section (in document order); the fixed
        // Components Object fields and `x-` extensions pass; a typo'd or Swagger-2.0
        // section is flagged; and no deeper property/field (indent >= 6) — even one
        // literally named like a section — is ever mistaken for a section key.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
components:
  schemas:
    Foo:
      type: object
      properties:
        definitions:
          type: string
  definitions:
    Legacy:
      type: object
  shemas:
    Typo:
      type: object
  x-vendor-block:
    anything: here
  securitySchemes:
    openId:
      type: openIdConnect
";
        // Every recognised section, in document order — note the `definitions:`
        // *property* of `Foo` (indent 8, inside `properties:`) is NOT one.
        assert_eq!(
            components_section_names(body),
            vec![
                "schemas".to_string(),
                "definitions".to_string(),
                "shemas".to_string(),
                "x-vendor-block".to_string(),
                "securitySchemes".to_string(),
            ]
        );
        // Flagged: the Swagger-2.0 `definitions` and the typo'd `shemas`. Not
        // flagged: the standard `schemas`/`securitySchemes` and the `x-` extension.
        assert_eq!(
            components_with_invalid_section_names(body),
            vec!["definitions".to_string(), "shemas".to_string()]
        );

        // Non-vacuous floor: across every registered spec, every `components` section
        // is a valid Components Object field (the invariant the contract asserts), and
        // the corpus actually declares many sections, so a broken extractor can't hide
        // behind an empty scan.
        let mut total_sections = 0usize;
        for api in APIS {
            assert!(
                components_with_invalid_section_names(api.body).is_empty(),
                "{}: every `components` section must be a valid OpenAPI 3 field",
                api.name
            );
            total_sections += components_section_names(api.body).len();
        }
        assert!(
            total_sections >= 100,
            "expected many components sections across specs, got {total_sections}"
        );
    }

    #[test]
    fn every_media_type_declares_a_schema() {
        // Contract-harness invariant (OpenAPI structural rule): every Media Type
        // Object a mounted spec declares under a `content:` mapping — in a request
        // body, a response, or a parameter — MUST carry a `schema` (or a `$ref` to
        // one). A Media Type Object with no schema hands a Redoc/Swagger/codegen
        // client a payload slot with no shape to bind or generate, so the request or
        // response body is undocumented at exactly the point a caller needs it.
        //
        // This is the finer complement of two sibling tests. `request_bodies_missing
        // _content` only asserts a request body *has* a `content` object (never that
        // its media types carry schemas); `every_declared_response_has_a_description`
        // only asserts a response *describes itself* (never that a body it declares
        // is typed). A media type block pasted from a sibling that keeps
        // `application/json:` but loses or dedents its `schema:` line — a routine
        // copy-paste hazard when vendoring a new endpoint — is invisible to both, and
        // to the parameter/responses/operationId/version/parity/`$ref` tests (which
        // check a parameter's identity, a response's key/description, an operation's
        // id/outcomes, or a spec's identity/wiring, never a payload's type). Verified
        // true (340 media types, all schema-bearing) across all mounted specs before
        // asserting.
        for api in APIS {
            let untyped = media_types_missing_schema(api.body);
            assert!(
                untyped.is_empty(),
                "{} spec declares media type(s) under `content:` with no `schema` \
                 (a Media Type Object must type its payload): {:?}",
                api.name,
                untyped
            );
        }
    }

    #[test]
    fn media_types_missing_schema_extraction_rules() {
        // Unit-cover the `media_types_missing_schema` extractor so the contract test
        // above can't pass vacuously and its detection is pinned: within `paths:`, a
        // media type (a `content:` child whose key contains `/`) is flagged only when
        // its object carries no `schema`/`$ref`; this holds for request-body and
        // response content alike; an inline `{...}`/`$ref` value is satisfied; and a
        // schema *property* literally named `content` (whose children are not
        // MIME-shaped) is never mistaken for a Content mapping.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    post:
      operationId: postA
      requestBody:
        content:
          application/json:
            schema:
              $ref: \"#/components/schemas/Req\"
          application/merge-patch+json:
            example: {}
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema:
                type: object
                properties:
                  content:
                    type: string
        '400':
          description: bad
          content:
            application/problem+json:
              example:
                code: X
components:
  schemas:
    Req:
      type: object
      properties:
        content:
          type: string
";
        // Flagged, in document order: the request-body `application/merge-patch+json`
        // (only an `example`, no `schema`) and the `400` response's
        // `application/problem+json` (likewise). Not flagged: both `application/json`
        // media types (schema-bearing); the `content` *property* nested inside the
        // `200` response's inline schema (its child `type:` is not MIME-shaped, so it
        // opens no media type); and the `content` property under `components.schemas`
        // (outside `paths:` entirely).
        assert_eq!(
            media_types_missing_schema(body),
            vec![
                "/a application/merge-patch+json".to_string(),
                "/a application/problem+json".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec, every media type carries a
        // schema (the invariant the contract test asserts), and the corpus actually
        // declares many media types, so a broken extractor can't hide behind an empty
        // scan. Count MIME-shaped keys (`<type>/<subtype>:`, never a `/path:` item)
        // with a detection independent of the extractor.
        let mut total_media_types = 0usize;
        for api in APIS {
            assert!(
                media_types_missing_schema(api.body).is_empty(),
                "{}: every media type under `content:` must declare a schema",
                api.name
            );
            for line in api.body.lines() {
                let t = line.trim();
                if let Some(key) = t.strip_suffix(':') {
                    if !key.starts_with('/') && key.contains('/') && !key.contains(' ') {
                        total_media_types += 1;
                    }
                }
            }
        }
        assert!(
            total_media_types >= 100,
            "expected many media types across specs, got {total_media_types}"
        );
    }

    #[test]
    fn every_media_type_key_names_a_valid_mime_type() {
        // Contract-harness invariant (OpenAPI structural rule): every direct child
        // key of a `content:` Content Object a mounted spec declares MUST be a
        // well-formed media type (`type/subtype`). A client selects the request- or
        // response-body slot by matching that key against a MIME type, so a key that
        // isn't one — a slash dropped in a paste (`applicationjson`), a garbled half
        // (`application/`), a stray second slash — names a body no client dispatches,
        // leaving the payload effectively undocumented at that content type.
        //
        // This is the key-*validity* complement of the media-type sweeps.
        // `media_types_missing_schema` (and `every_media_type_declares_a_schema`)
        // only ever act on a content child that *already* contains a `/`, so a
        // slash-less malformed key is invisible to them, and even a slash-bearing key
        // is only checked for a schema, never for MIME syntax. It sits in the same
        // valid-key series as `every_responses_object_key_is_a_valid_status` and
        // `every_path_item_key_names_a_valid_operation_or_field`, which pin the shape
        // of response-status and path-item keys respectively; this pins content keys.
        // Verified true across all mounted specs before asserting (the corpus uses
        // only `application/json`, `application/cloudevents+json`,
        // `application/merge-patch+json`, `application/x-www-form-urlencoded`).
        for api in APIS {
            let invalid = media_types_with_invalid_names(api.body);
            assert!(
                invalid.is_empty(),
                "{} spec declares content key(s) that are not valid media types \
                 (a Content Object's keys must name MIME types): {:?}",
                api.name,
                invalid
            );
        }
    }

    #[test]
    fn media_type_key_validity_extraction_rules() {
        // Pin the media-type predicate so the contract test above can't drift: the
        // registered-name forms CAMARA uses pass, and the malformed shapes fail.
        for ok in [
            "application/json",
            "application/cloudevents+json",
            "application/merge-patch+json",
            "application/x-www-form-urlencoded",
            "application/problem+json",
            "text/plain",
            "*/*",
            "application/*",
            "application/json; charset=utf-8", // parameters ignored
        ] {
            assert!(is_valid_media_type_key(ok), "should accept {ok:?}");
        }
        for bad in [
            "applicationjson", // no slash
            "application/",    // empty subtype
            "/json",           // empty type
            "application/json/x", // stray second slash
            "",                // empty
            "type",            // schema field, not a media type
            "properties",      // schema field, not a media type
        ] {
            assert!(!is_valid_media_type_key(bad), "should reject {bad:?}");
        }

        // Unit-cover the `media_types_with_invalid_names` extractor: within `paths:`,
        // a Content Object's malformed child keys are flagged in document order,
        // valid ones are not, and a schema *property* literally named `content` (no
        // MIME-shaped child) is never mistaken for a Content Object.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    post:
      operationId: postA
      requestBody:
        content:
          application/json:
            schema:
              $ref: \"#/components/schemas/Req\"
          applicationjson:
            schema:
              type: string
      responses:
        '200':
          description: ok
          content:
            application/merge-patch+json:
              schema:
                type: object
                properties:
                  content:
                    type: string
        '400':
          description: bad
          content:
            application/:
              schema:
                type: string
components:
  schemas:
    Req:
      type: object
      properties:
        content:
          type: string
";
        // Flagged, in document order: the request-body `applicationjson` (no slash)
        // and the `400` response's `application/` (empty subtype). Not flagged: the
        // two well-formed media types; the `content` *property* nested inside the
        // `200` response's inline schema (no MIME-shaped child, so it opens no
        // Content Object); and the `content` property under `components.schemas`
        // (outside `paths:` entirely).
        assert_eq!(
            media_types_with_invalid_names(body),
            vec![
                "/a applicationjson".to_string(),
                "/a application/".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec, every content key is a
        // valid media type (the invariant the contract test asserts), and the corpus
        // actually declares many content objects, so a broken extractor can't hide
        // behind an empty scan.
        let mut total_content_objects = 0usize;
        for api in APIS {
            assert!(
                media_types_with_invalid_names(api.body).is_empty(),
                "{}: every content key must be a valid media type",
                api.name
            );
            for line in api.body.lines() {
                if line.trim() == "content:" {
                    total_content_objects += 1;
                }
            }
        }
        assert!(
            total_content_objects >= 50,
            "expected many content objects across specs, got {total_content_objects}"
        );
    }

    #[test]
    fn every_path_item_key_names_a_valid_operation_or_field() {
        // Contract-harness invariant (OpenAPI structural rule): every key a mounted
        // spec declares directly under a Path Item Object MUST be either a valid HTTP
        // method (`get`/`put`/`post`/`delete`/`options`/`head`/`patch`/`trace`), a
        // permitted Path Item field (`$ref`/`summary`/`description`/`servers`/
        // `parameters`), or an `x-` extension. Any other key — nearly always a
        // mistyped verb (`psot:`, `pust:`, an upper-case `POST:`) — defines a phantom
        // operation that no HTTP client routes and no tool renders.
        //
        // This is the exact complement of the operation tests. `operations_without
        // _responses`, `operations_without_operation_id`, `responses_missing
        // _description`, and the operation-id/response-key tests each enumerate
        // operations from the *valid* method set and `continue` past everything else —
        // so a malformed verb is invisible to all of them: it is silently skipped, its
        // (real, dangling) operation never checked for a responses object, an
        // operationId, or typed responses. This test inspects precisely the keys they
        // skip. Verified true across all mounted specs before asserting (147 operation
        // keys + `parameters`, no malformed verbs).
        for api in APIS {
            let invalid = invalid_path_item_keys(api.body);
            assert!(
                invalid.is_empty(),
                "{} spec declares path-item key(s) that are neither a valid HTTP \
                 method nor a permitted Path Item field (a phantom operation no \
                 client routes): {:?}",
                api.name,
                invalid
            );
        }
    }

    #[test]
    fn path_item_key_validity_extraction_rules() {
        // Unit-cover the `invalid_path_item_keys` extractor so the contract test above
        // can't pass vacuously and its detection is pinned: within `paths:`, a 4-space
        // path-item key is flagged only when it is neither a valid verb nor a permitted
        // field nor an `x-` extension; the fixed fields (`summary`/`description`/
        // `parameters`) and extensions pass; a comment line and a `parameters:` list's
        // deeper `- name:` item are never mistaken for keys; and both a lower-case
        // typo (`psot`) and an upper-case verb (`POST`) are caught, in document order.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    summary: A path
    description: desc
    # a stray comment at operation indent
    parameters:
      - name: x-correlator
        in: header
    get:
      operationId: getA
      responses:
        '200':
          description: ok
    psot:
      operationId: typoVerb
      responses:
        '200':
          description: ok
    x-internal: true
  /b:
    POST:
      operationId: upperVerb
      responses:
        '200':
          description: ok
";
        // Flagged, in document order: `/a psot` (a lower-case typo of `post`) and
        // `/b POST` (an upper-case verb — OpenAPI method keys are lower-case). Not
        // flagged: `summary`/`description`/`parameters` (fixed fields), `get` (a valid
        // verb), the `# …` comment, the `x-internal` extension, and the `parameters:`
        // list's 6-space `- name:` item (deeper than the path-item indent).
        assert_eq!(
            invalid_path_item_keys(body),
            vec!["/a psot".to_string(), "/b POST".to_string()]
        );

        // Non-vacuous floor: across every registered spec, no path-item key is invalid
        // (the invariant the contract test asserts), and the corpus actually declares
        // many operations, so a broken extractor can't hide behind an empty scan. Count
        // valid method keys with a detection independent of the extractor.
        let mut total_ops = 0usize;
        for api in APIS {
            assert!(
                invalid_path_item_keys(api.body).is_empty(),
                "{}: every path-item key must be a valid operation or field",
                api.name
            );
            for line in api.body.lines() {
                if matches!(
                    line.trim(),
                    "get:" | "put:" | "post:" | "delete:" | "patch:" | "options:"
                        | "head:" | "trace:"
                ) {
                    total_ops += 1;
                }
            }
        }
        assert!(
            total_ops >= 100,
            "expected many operations across specs, got {total_ops}"
        );
    }

    #[test]
    fn every_ref_target_is_a_fragment_pointer() {
        // Contract-harness invariant (OpenAPI reference rule, as these specs use it):
        // every `$ref` a mounted spec declares MUST carry a `#/` JSON-pointer
        // fragment — a local `#/components/…` or a cross-file
        // `<relative-path>#/components/…`. The fragment is the half a client
        // (Redoc/Swagger/codegen) dereferences to reach the actual
        // schema/response/parameter; a target that lost it points at a document root
        // (`errors.yaml`) or nothing (a bare `CamaraError`), so the ref never resolves
        // to the intended component and the served spec is unusable.
        //
        // This is the shape-level complement of the two ref-*resolution* tests. Both
        // `shared_error_refs_resolve_to_defined_components` and
        // `local_component_refs_resolve_within_their_own_spec` begin with
        // `target.split_once('#')` and `continue` when there is no `#` — so a
        // fragmentless ref is silently skipped by *both*, its target never checked
        // against any defined component. The canonical-path test
        // (`shared_fragment_refs_use_the_canonical_relative_path`) only inspects refs
        // that already name the shared file, so it skips it too. This test inspects
        // precisely the malformed targets they all fall through: a `$ref` whose
        // fragment was dropped in a copy-paste (`$ref: "errors.yaml"`) or typo'd away
        // (`$ref: "#components/schemas/Foo"`, missing the `/`). Verified true across
        // all mounted specs before asserting (every declared `$ref` is
        // fragment-bearing).
        for api in APIS {
            let missing = refs_missing_fragment(api.body);
            assert!(
                missing.is_empty(),
                "{} spec declares $ref target(s) with no `#/` JSON-pointer fragment \
                 (they resolve to a document root or nothing, not the intended \
                 component): {:?}",
                api.name,
                missing
            );
        }
    }

    #[test]
    fn every_ref_object_stands_alone() {
        // Contract-harness invariant (OpenAPI 3.0.x Reference Object rule): a `$ref`
        // object's members "other than `$ref` SHALL be ignored", so a reference
        // written with a neighbouring key silently drops that key. The break it
        // catches: a schema property (or a response/parameter) given as a bare `$ref`
        // plus a `description:` / `example:` / `nullable:` sibling — the natural way
        // to *try* to annotate a reference — renders only the referenced component,
        // the annotation lost, with no error any tool reports. Invisible to the three
        // ref-*target* tests (`every_ref_target_is_a_fragment_pointer`,
        // `shared_fragment_refs_use_the_canonical_relative_path`, the
        // resolve-to-defined-component tests): each inspects what a `$ref` points at,
        // never whether it stands alone in its mapping. The canonical 3.0.x way to
        // annotate a reference is to wrap it (`allOf:` with a single `- $ref` item
        // plus the sibling), which the extractor treats as sibling-free (the `$ref`
        // is then the lone key of its own sequence item). Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let bad = refs_with_sibling_keys(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares $ref object(s) carrying a sibling key — OpenAPI \
                 3.0.x ignores a `$ref`'s siblings, so the neighbouring key is \
                 silently dropped; wrap the reference in `allOf` to annotate it: {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn ref_sibling_extraction_rules() {
        // Unit-cover `refs_with_sibling_keys` so the contract test above can't pass
        // vacuously and its scoping is pinned: a bare `$ref` with a following or
        // opener-line sibling is flagged; a reference nested under `items:` whose
        // `description` sits on the array schema, a lone `$ref`, a `$ref` wrapped in
        // `allOf`, and a `- $ref` sequence item whose following `- name:` is the next
        // element are all left alone.
        let body = "\
components:
  schemas:
    Area:
      type: object
      properties:
        center:
          $ref: '#/components/schemas/Point'
          description: The centre of a CIRCLE area.
        boundary:
          type: array
          items:
            $ref: '#/components/schemas/Point'
          description: The vertices (array-level sibling of items, not of $ref).
        good:
          $ref: '#/components/schemas/Point'
        wrapped:
          description: Annotated the canonical 3.0.x way.
          allOf:
            - $ref: '#/components/schemas/Point'
      required:
        - center
    Params:
      parameters:
        - $ref: '#/components/parameters/A'
        - name: b
          in: query
        - description: a leading sibling within one sequence item
          $ref: '#/components/schemas/Point'
";
        // `center`'s $ref has a following `description` sibling → flagged. The last
        // `parameters` item pairs a `description` with a `$ref` in one mapping, so
        // the `$ref` (scanning up to its opener line) has that sibling → flagged.
        // Not flagged: `boundary` (its `description` is a sibling of `items`/the
        // array schema, one indent shallower than the nested `$ref`); `good` (a lone
        // reference); `wrapped` (the `$ref` is the sole key of its `allOf` item — the
        // `description` sits on the property, outside the reference); the first
        // `- $ref: …/A` (the following `- name: b` is the next parameter, a separate
        // sequence item, never a sibling).
        assert_eq!(
            refs_with_sibling_keys(body),
            vec![
                "#/components/schemas/Point (sibling: description)".to_string(),
                "#/components/schemas/Point (sibling: description)".to_string(),
            ]
        );

        // Non-vacuous floor: every registered spec already satisfies the invariant
        // (no `$ref` carries a sibling), and the corpus declares many refs, so the
        // contract test asserts over a real, non-empty population rather than an
        // empty loop.
        let mut total_refs = 0usize;
        for api in APIS {
            assert!(
                refs_with_sibling_keys(api.body).is_empty(),
                "{}: every $ref object must stand alone",
                api.name
            );
            total_refs += ref_targets(api.body).len();
        }
        assert!(
            total_refs >= 100,
            "expected many $refs across specs, got {total_refs}"
        );
    }

    #[test]
    fn ref_fragment_extraction_rules() {
        // Unit-cover the `refs_missing_fragment` extractor so the contract test above
        // can't pass vacuously and its detection is pinned: a local `#/…` pointer and
        // a cross-file `<path>#/…` pointer are fragment-bearing (not flagged), while a
        // whole-file ref (`errors.yaml`) and a fragment missing its leading slash
        // (`#components/…`) carry no `#/` and are flagged, in document order.
        let body = "\
responses:
  '400':
    $ref: '#/components/responses/Generic400'
  '404':
    $ref: '../../shared/errors.yaml#/components/responses/Generic404'
  '409':
    $ref: 'errors.yaml'
  '422':
    $ref: '#components/responses/BadPointer'
";
        assert_eq!(
            refs_missing_fragment(body),
            vec!["errors.yaml".to_string(), "#components/responses/BadPointer".to_string()]
        );

        // Non-vacuous floor: across every registered spec no `$ref` is fragmentless
        // (the invariant the contract test asserts), and the corpus actually declares
        // many refs, so a broken extractor can't hide behind an empty scan.
        let mut total_refs = 0usize;
        for api in APIS {
            assert!(
                refs_missing_fragment(api.body).is_empty(),
                "{}: every $ref must carry a `#/` fragment",
                api.name
            );
            total_refs += ref_targets(api.body).len();
        }
        assert!(
            total_refs >= 100,
            "expected many $refs across specs, got {total_refs}"
        );
    }

    /// Enumerate every `enum:` a spec declares whose value list is **empty** or
    /// contains a **duplicate** value — reported as `"[<values>] (<reason>)"` in
    /// document order, without a YAML dep.
    ///
    /// An OpenAPI / JSON-Schema `enum` fixes the closed set of values a field may
    /// take: a codegen client emits one variant per value and a validator admits
    /// only those, so a **duplicate** value makes two variants collide (the second
    /// silently shadows the first) and an **empty** list admits nothing — no payload
    /// can ever satisfy it. Both are copy-paste hazards in these scenario-table-heavy
    /// specs (a status/enum block pasted from a sibling and half-edited keeps a stale
    /// value, or is left `[]`), invisible to every sibling test — which check a
    /// field's identity, a payload's presence, a component key's shape, or a `$ref`'s
    /// target, never the values an enum enumerates.
    ///
    /// Both YAML forms are handled: a flow list (`enum: [A, B]`, gathered across
    /// lines to its `]`) and a block list (`enum:` then `- A` children at a deeper
    /// indent). To avoid mistaking a schema *property literally named* `enum` (whose
    /// value is a mapping like `type: string`, not a list) for an enum, a block
    /// `enum:` is treated as a list only when its first non-blank child is a `-`
    /// item; anything else is skipped. Values are unquoted and a trailing ` #`
    /// comment trimmed before comparison. Whole-document scan (enums live under
    /// `components.schemas` as well as inline), mirroring the reference tests.
    fn enums_with_no_values_or_duplicates(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // Unquote a scalar and trim a trailing ` # comment`.
        let norm = |raw: &str| -> String {
            let mut v = raw.trim();
            if let Some(pos) = v.find(" #") {
                v = v[..pos].trim_end();
            }
            let v = v.trim();
            let unq = v
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(v);
            unq.trim().to_string()
        };
        // Given one enum's ordered values, return its problem descriptor, if any.
        let problem = |values: &[String]| -> Option<String> {
            if values.is_empty() {
                return Some("[] (empty enum)".to_string());
            }
            let mut seen = std::collections::HashSet::new();
            for v in values {
                if !seen.insert(v.as_str()) {
                    return Some(format!(
                        "[{}] (duplicate value: {})",
                        values.join(", "),
                        v
                    ));
                }
            }
            None
        };
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let t = line.trim_start();
            if !t.starts_with("enum:") {
                i += 1;
                continue;
            }
            let rest = t["enum:".len()..].trim_start();
            if rest.starts_with('[') {
                // Flow list — gather across lines until the closing `]`.
                let mut buf = rest.to_string();
                let mut k = i;
                while !buf.contains(']') && k + 1 < lines.len() {
                    k += 1;
                    buf.push(' ');
                    buf.push_str(lines[k].trim());
                }
                let open = buf.find('[').map(|x| x + 1).unwrap_or(0);
                let close = buf.rfind(']').unwrap_or(buf.len());
                let inner = if close >= open { &buf[open..close] } else { "" };
                let values: Vec<String> = if inner.trim().is_empty() {
                    Vec::new()
                } else {
                    inner.split(',').map(|s| norm(s)).filter(|v| !v.is_empty()).collect()
                };
                if let Some(p) = problem(&values) {
                    out.push(p);
                }
                i = k + 1;
                continue;
            }
            // Block form (empty value or only a trailing comment): collect `- ` items
            // at a deeper indent — but only when this is genuinely an enum list (its
            // first child is a `-` item, not a property named `enum` whose value is a
            // mapping).
            if rest.is_empty() || rest.starts_with('#') {
                let base = indent(line);
                let mut values: Vec<String> = Vec::new();
                let mut first_child_seen = false;
                let mut is_list = false;
                let mut j = i + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() || l.trim_start().starts_with('#') {
                        j += 1;
                        continue;
                    }
                    if indent(l) <= base {
                        break; // dedented out of the enum block
                    }
                    let item = l.trim_start();
                    if !first_child_seen {
                        first_child_seen = true;
                        is_list = item.starts_with('-');
                        if !is_list {
                            break; // a property named `enum`, not an enum list
                        }
                    }
                    if !item.starts_with('-') {
                        break; // end of the contiguous list
                    }
                    let val = norm(item[1..].trim_start());
                    if !val.is_empty() {
                        values.push(val);
                    }
                    j += 1;
                }
                if is_list {
                    if let Some(p) = problem(&values) {
                        out.push(p);
                    }
                }
                i = j;
                continue;
            }
            i += 1;
        }
        out
    }

    #[test]
    fn every_enum_lists_unique_non_empty_values() {
        // Contract-harness invariant (OpenAPI / JSON-Schema structural rule): every
        // `enum:` a mounted spec declares MUST list at least one value and MUST NOT
        // repeat a value. An `enum` fixes the closed set a field may take — a
        // codegen/validation client emits one variant per value and admits only those
        // — so a duplicate value makes two variants collide (the second silently
        // shadows the first) and an empty list admits nothing, so no payload can ever
        // validate against it.
        //
        // No sibling test looks *inside* an enum: the parameter/response/media-type/
        // component/ref tests check a field's identity, a payload's presence, a
        // component key's shape, or a `$ref`'s target — never the values an enum
        // enumerates. In these scenario-table-heavy specs (status enums, network-type
        // enums, credential-type enums, event-type enums) a value block pasted from a
        // sibling and half-edited is a live copy-paste hazard: a stale value left in
        // place duplicates one already listed, or an in-progress block is left `[]`.
        // Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = enums_with_no_values_or_duplicates(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares enum(s) that are empty or list a duplicate value \
                 (an enum must enumerate a non-empty set of distinct values): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn enum_values_extraction_rules() {
        // Unit-cover the `enums_with_no_values_or_duplicates` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a
        // block enum with a repeated item and a flow enum with a repeated value are
        // both flagged (with the offending value), an `enum: []` is flagged empty, a
        // clean block/flow enum passes, and a schema *property literally named* `enum`
        // (whose value is a mapping, not a list) is never mistaken for an enum. All in
        // document order.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
components:
  schemas:
    NetType:
      type: string
      enum:
        - 2G
        - 3G
        - 3G
    Credential:
      type: string
      enum: [PLAIN, ACCESSTOKEN, PLAIN]
    EmptyOne:
      type: string
      enum: []
    Clean:
      type: string
      enum:
        - A
        - B
    Media:
      type: string
      enum: [\"application/json\"]
    PropNamedEnum:
      type: object
      properties:
        enum:
          type: string
";
        // Flagged, in document order: `NetType` (block enum repeats `3G`),
        // `Credential` (flow enum repeats `PLAIN`), and `EmptyOne` (`enum: []`). Not
        // flagged: `Clean`/`Media` (distinct non-empty values), and the `enum`
        // *property* under `PropNamedEnum.properties` (its value is a mapping, not a
        // list, so it opens no enum).
        assert_eq!(
            enums_with_no_values_or_duplicates(body),
            vec![
                "[2G, 3G, 3G] (duplicate value: 3G)".to_string(),
                "[PLAIN, ACCESSTOKEN, PLAIN] (duplicate value: PLAIN)".to_string(),
                "[] (empty enum)".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec no enum is empty or has a
        // duplicate value (the invariant the contract test asserts), and the corpus
        // actually declares many enums, so a broken extractor can't hide behind an
        // empty scan. Count `enum:` declarations with a detection independent of the
        // extractor.
        let mut total_enums = 0usize;
        for api in APIS {
            assert!(
                enums_with_no_values_or_duplicates(api.body).is_empty(),
                "{}: every enum must list a non-empty set of distinct values",
                api.name
            );
            for line in api.body.lines() {
                if line.trim_start().starts_with("enum:") {
                    total_enums += 1;
                }
            }
        }
        assert!(
            total_enums >= 100,
            "expected many enums across specs, got {total_enums}"
        );
    }

    /// Enumerate every object-schema `required:` array a spec declares that repeats
    /// a property name — reported as `"[<entries>] (duplicate entry: <name>)"` in
    /// document order, without a YAML dep.
    ///
    /// An OpenAPI / JSON-Schema object schema's `required` array names the
    /// properties an instance MUST carry, and JSON Schema fixes that the array's
    /// "elements … MUST be unique". A repeated name is therefore an invalid schema:
    /// a codegen client that emits one presence constraint per required entry gets a
    /// redundant, colliding duplicate, and the redundant name usually marks a real
    /// mistake — a sibling property mistyped or since-renamed, so the schema now
    /// *requires the same field twice and silently no longer requires the one that
    /// was meant*. It is a live copy-paste hazard in these specs: a `required:`
    /// block pasted from a sibling schema and only half-edited keeps a stale name
    /// that duplicates one already listed — invisible to every sibling test, which
    /// check a field's identity, a payload's presence, a component key's shape, an
    /// enum's values, or a `$ref`'s target, never the names a `required` array
    /// lists.
    ///
    /// The scalar `required: true` / `required: false` boolean (a parameter or
    /// requestBody flag, *not* a schema's property list — already pinned for `path`
    /// params by `every_path_parameter_declares_required_true`) is skipped: only a
    /// flow list (`required: [a, b]`, gathered across lines to its `]`) or a block
    /// list (`required:` then `- name` children at a deeper indent, recognised only
    /// when its first non-blank child is a `-` item) is read as an array. Names are
    /// unquoted and a trailing ` #` comment trimmed before comparison. Whole-document
    /// scan (required arrays live under `components.schemas` as well as inline
    /// request/response schemas), mirroring the enum / reference tests.
    fn required_arrays_with_duplicate_entries(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // Unquote a scalar and trim a trailing ` # comment`.
        let norm = |raw: &str| -> String {
            let mut v = raw.trim();
            if let Some(pos) = v.find(" #") {
                v = v[..pos].trim_end();
            }
            let v = v.trim();
            let unq = v
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(v);
            unq.trim().to_string()
        };
        // Given one required array's ordered entries, return its duplicate
        // descriptor, if any. (Empty `required: []` is legal under OpenAPI 3.1 /
        // JSON-Schema 2020-12, so only the always-invalid duplicate is flagged.)
        let problem = |values: &[String]| -> Option<String> {
            let mut seen = std::collections::HashSet::new();
            for v in values {
                if !seen.insert(v.as_str()) {
                    return Some(format!("[{}] (duplicate entry: {})", values.join(", "), v));
                }
            }
            None
        };
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let t = line.trim_start();
            if !t.starts_with("required:") {
                i += 1;
                continue;
            }
            let rest = t["required:".len()..].trim_start();
            if rest.starts_with('[') {
                // Flow list — gather across lines until the closing `]`.
                let mut buf = rest.to_string();
                let mut k = i;
                while !buf.contains(']') && k + 1 < lines.len() {
                    k += 1;
                    buf.push(' ');
                    buf.push_str(lines[k].trim());
                }
                let open = buf.find('[').map(|x| x + 1).unwrap_or(0);
                let close = buf.rfind(']').unwrap_or(buf.len());
                let inner = if close >= open { &buf[open..close] } else { "" };
                let values: Vec<String> = if inner.trim().is_empty() {
                    Vec::new()
                } else {
                    inner.split(',').map(|s| norm(s)).filter(|v| !v.is_empty()).collect()
                };
                if let Some(p) = problem(&values) {
                    out.push(p);
                }
                i = k + 1;
                continue;
            }
            // Block form (empty value or only a trailing comment): collect `- ` items
            // at a deeper indent — but only when this is genuinely a `required` list
            // (its first child is a `-` item), never a scalar `required: true`/`false`
            // (handled below by falling through) or a mapping.
            if rest.is_empty() || rest.starts_with('#') {
                let base = indent(line);
                let mut values: Vec<String> = Vec::new();
                let mut first_child_seen = false;
                let mut is_list = false;
                let mut j = i + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() || l.trim_start().starts_with('#') {
                        j += 1;
                        continue;
                    }
                    if indent(l) <= base {
                        break; // dedented out of the required block
                    }
                    let item = l.trim_start();
                    if !first_child_seen {
                        first_child_seen = true;
                        is_list = item.starts_with('-');
                        if !is_list {
                            break; // not a required array (e.g. a mapping child)
                        }
                    }
                    if !item.starts_with('-') {
                        break; // end of the contiguous list
                    }
                    let val = norm(item[1..].trim_start());
                    if !val.is_empty() {
                        values.push(val);
                    }
                    j += 1;
                }
                if is_list {
                    if let Some(p) = problem(&values) {
                        out.push(p);
                    }
                }
                i = j;
                continue;
            }
            // Scalar `required: true` / `required: false` — a boolean flag, not an
            // array; nothing to check.
            i += 1;
        }
        out
    }

    #[test]
    fn every_required_array_lists_distinct_entries() {
        // Contract-harness invariant (OpenAPI / JSON-Schema structural rule): every
        // object-schema `required:` array a mounted spec declares MUST NOT repeat a
        // property name — JSON Schema fixes that the array's elements are unique. A
        // duplicate is an invalid schema whose redundant name almost always marks a
        // real slip: a sibling property mistyped or since-renamed, so the schema now
        // requires one field twice and silently no longer requires the intended one.
        //
        // No sibling test looks *inside* a `required` array: the parameter/response/
        // media-type/component/enum/ref tests check a field's identity, a payload's
        // presence, a component key's shape, an enum's values, or a `$ref`'s target —
        // never the names a `required` array lists. In these scenario-table-heavy
        // specs a `required:` block pasted from a sibling schema and half-edited is a
        // live copy-paste hazard. Verified true across all mounted specs before
        // asserting.
        for api in APIS {
            let bad = required_arrays_with_duplicate_entries(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares `required` array(s) that repeat a property name \
                 (a schema's required entries must be distinct): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn required_array_entries_extraction_rules() {
        // Unit-cover the `required_arrays_with_duplicate_entries` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a
        // block required array with a repeated entry and a flow required array with a
        // repeated entry are both flagged (with the offending name), a scalar
        // `required: true` boolean is never mistaken for an array, and a clean
        // block/flow required array passes. All in document order.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /x:
    get:
      parameters:
        - name: q
          in: query
          required: true
components:
  schemas:
    DupBlock:
      type: object
      required:
        - phoneNumber
        - amount
        - phoneNumber
      properties:
        phoneNumber:
          type: string
    DupFlow:
      type: object
      required: [device, device]
    Clean:
      type: object
      required:
        - a
        - b
    CleanFlow:
      type: object
      required: [x, y]
";
        // Flagged, in document order: `DupBlock` (block array repeats `phoneNumber`)
        // and `DupFlow` (flow array repeats `device`). Not flagged: the parameter's
        // scalar `required: true` (a boolean flag, not an array), and `Clean` /
        // `CleanFlow` (distinct entries).
        assert_eq!(
            required_arrays_with_duplicate_entries(body),
            vec![
                "[phoneNumber, amount, phoneNumber] (duplicate entry: phoneNumber)".to_string(),
                "[device, device] (duplicate entry: device)".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec no `required` array repeats
        // an entry (the invariant the contract test asserts), and the corpus actually
        // declares many array-form `required` blocks, so a broken extractor can't hide
        // behind an empty scan. Count array-form `required:` declarations (flow `[…]`
        // or a block whose next non-blank line is a `- ` item) independently of the
        // extractor.
        let mut array_required = 0usize;
        for api in APIS {
            assert!(
                required_arrays_with_duplicate_entries(api.body).is_empty(),
                "{}: every `required` array must list distinct entries",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            for (idx, line) in lines.iter().enumerate() {
                let t = line.trim_start();
                let Some(rest) = t.strip_prefix("required:") else { continue };
                let rest = rest.trim_start();
                if rest.starts_with('[') {
                    array_required += 1;
                } else if rest.is_empty() || rest.starts_with('#') {
                    // Block form: array only if the next non-blank child is a `- ` item.
                    if lines[idx + 1..]
                        .iter()
                        .map(|l| l.trim())
                        .find(|l| !l.is_empty() && !l.starts_with('#'))
                        .is_some_and(|l| l.starts_with('-'))
                    {
                        array_required += 1;
                    }
                }
            }
        }
        assert!(
            array_required >= 100,
            "expected many array-form `required` blocks across specs, got {array_required}"
        );
    }

    #[test]
    fn every_parameter_array_lists_distinct_name_location_pairs() {
        // Contract-harness invariant (OpenAPI structural rule): within a
        // `parameters:` array a mounted spec declares, no two parameters MUST share
        // the same `(name, location)` pair — the Parameter Object's identity rule
        // ("A unique parameter is defined by a combination of a name and
        // location."). A repeat is an invalid document whose second entry is
        // ignored by a Redoc/Swagger/codegen client, so an intended distinct
        // parameter silently vanishes.
        //
        // Extends the active uniqueness family — `every_enum_lists_unique_non_empty_
        // values` (an enum's values) and `every_required_array_lists_distinct_
        // entries` (a `required` array's names) — to the third collection whose
        // members must be distinct: an operation's parameter list. The key is the
        // *pair*, so the same name in two locations (path vs query) stays legal;
        // only a genuine both-field repeat — a parameter block pasted twice into
        // one array — is flagged. No sibling test compares parameters to each
        // other: the name/location/schema tests check a single parameter's three
        // required fields, never two parameters' identity. Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let dups = parameter_arrays_with_duplicate_name_location(api.body);
            assert!(
                dups.is_empty(),
                "{} spec declares a `parameters` array that repeats a (name, \
                 location) pair (an operation's parameters must be unique by name + \
                 location): {:?}",
                api.name,
                dups
            );
        }
    }

    #[test]
    fn parameter_name_location_duplicate_extraction_rules() {
        // Unit-cover the `parameter_arrays_with_duplicate_name_location` extractor so
        // the contract test above can't pass vacuously and its detection is pinned: a
        // both-field repeat (name-first and in-first forms mixed) is flagged; the same
        // name in two different locations is NOT; a name repeated across two separate
        // `parameters:` arrays is NOT (scoped per array); a `schema` property named
        // `name`/`in` nested inside a parameter is never mistaken for the parameter's
        // own fields; and a `$ref` item contributes no pair.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      parameters:
        - name: x-correlator
          in: header
        - in: header
          name: x-correlator
        - name: id
          in: path
          required: true
          schema:
            type: object
            properties:
              in:
                type: string
        - $ref: '#/components/parameters/Shared'
      responses:
        '200':
          description: ok
  /b:
    get:
      operationId: getB
      parameters:
        - name: filter
          in: query
        - name: filter
          in: header
  /c:
    get:
      operationId: getC
      parameters:
        - name: page
          in: query
components:
  parameters:
    Shared:
      name: shared
      in: query
      schema:
        type: string
";
        // Flagged: only GET /a's array — `x-correlator`/`header` appears twice (once
        // name-first, once in-first). Not flagged inside /a: the `id`/`path` param
        // (its nested schema property literally named `in` sits deeper than the
        // item's child indent, so it is ignored) and the `$ref` item (no inline
        // name/in). Not flagged in /b: `filter` repeats but in different locations
        // (query vs header) — a distinct pair. Not flagged in /c: single param. The
        // `filter` name shared across /a…/b lives in separate arrays, so cross-array
        // repeats are never compared.
        assert_eq!(
            parameter_arrays_with_duplicate_name_location(body),
            vec![
                "parameters@line 9: duplicate parameter (name=x-correlator, in=header)".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec no `parameters` array
        // repeats a (name, location) pair (the invariant the contract test asserts),
        // and the corpus actually declares many block-form `parameters:` arrays, so a
        // broken extractor can't hide behind an empty scan.
        let mut param_arrays = 0usize;
        for api in APIS {
            assert!(
                parameter_arrays_with_duplicate_name_location(api.body).is_empty(),
                "{}: every `parameters` array must list distinct (name, location) pairs",
                api.name
            );
            for line in api.body.lines() {
                if line.trim_start().strip_prefix("parameters:").is_some_and(|r| {
                    let r = r.trim();
                    r.is_empty() || r.starts_with('#')
                }) {
                    param_arrays += 1;
                }
            }
        }
        assert!(
            param_arrays >= 30,
            "expected many block-form `parameters` arrays across specs, got {param_arrays}"
        );
    }

    /// Extract the 1-based line number of every `type: array` Schema Object a spec
    /// declares that lacks an `items` sibling — without a YAML dep.
    ///
    /// In OpenAPI 3.0.x a Schema Object typed `array` MUST declare `items` (the
    /// schema each element validates against); an array with no `items` is an
    /// invalid, under-specified schema whose elements are untyped. `type: array`
    /// occurs only inside a Schema Object, so no context-scoping is needed. `items`
    /// is a sibling key of `type` in the same mapping — at the same indentation `C`.
    /// For each `type: array` line this scans that object's block for an `items:`
    /// sibling at indent exactly `C`, walking down and then up from the `type` line,
    /// each direction bounded by the first non-blank line that dedents below `C` (the
    /// enclosing property key that opened the object, or a shallower following key).
    /// Lines indented deeper than `C` are the object's nested values — including any
    /// block-scalar `description:` content, which YAML always indents past its key —
    /// so scoping the sibling scan to exactly `C` sidesteps them, and an `items:`
    /// mentioned inside such prose is never miscredited. A `type:` whose value is not
    /// exactly `array` (`object`, `string`, or an empty value on a property literally
    /// named `type`) opens no obligation and is skipped.
    fn array_schemas_missing_items(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The scalar value of a `type:` key, inline comment and quotes stripped.
        fn type_value(l: &str) -> Option<&str> {
            l.trim_start().strip_prefix("type:").map(|v| {
                v.split('#')
                    .next()
                    .unwrap_or(v)
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
            })
        }
        // True when `l` is a sibling key `items:` at indentation exactly `c`.
        let is_items_sibling = |l: &str, c: usize| -> bool {
            indent(l) == c
                && l.trim_start()
                    .split_once(':')
                    .is_some_and(|(k, _)| k.trim() == "items")
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if type_value(line) != Some("array") {
                continue;
            }
            let c = indent(line);
            let mut found = false;
            // Scan down through this object's block for an `items:` sibling.
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break; // dedented out of this object
                }
                if is_items_sibling(l, c) {
                    found = true;
                    break;
                }
                j += 1;
            }
            // `items` may be declared before `type`; scan up the same block.
            if !found {
                let mut k = i;
                while k > 0 {
                    k -= 1;
                    let l = lines[k];
                    if l.trim().is_empty() {
                        continue;
                    }
                    if indent(l) < c {
                        break; // reached the key that opened this object
                    }
                    if is_items_sibling(l, c) {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_array_schema_declares_items() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every Schema
        // Object a mounted spec types as `array` MUST declare `items` — the schema
        // its elements validate against. In 3.0.x `items` is REQUIRED for an array
        // schema; an array with no `items` is invalid, and a Redoc/Swagger/codegen
        // client handed one has no element shape to render or generate, so the list
        // payload is untyped at exactly the point a caller reads or builds it.
        //
        // A routine hazard in these vendored, scenario-table-heavy specs: an array
        // schema pasted from a sibling that keeps `type: array` but loses or dedents
        // its `items:` line, or a refactor that lifts the element schema out and
        // forgets to leave the `items` ref behind. It is invisible to every existing
        // test — `every_media_type_declares_a_schema` checks that a payload *has* a
        // schema, never that an array schema is *complete*; the enum/required/
        // parameter/`$ref` tests check a value list's members, a required list's
        // entries, a parameter's identity, or a ref's target, never an array schema's
        // element type. Verified true across all mounted specs before asserting.
        for api in APIS {
            let untyped = array_schemas_missing_items(api.body);
            assert!(
                untyped.is_empty(),
                "{} spec declares `type: array` schema(s) with no `items` sibling \
                 (an array Schema Object must type its elements) at line(s): {:?}",
                api.name,
                untyped
            );
        }
    }

    #[test]
    fn array_schema_items_extraction_rules() {
        // Unit-cover the `array_schemas_missing_items` extractor so the contract test
        // above can't pass vacuously and its detection is pinned: an array is flagged
        // only when its object carries no `items` sibling; `items` declared after
        // *or* before `type` satisfies it; a non-`array` `type:` (and a property
        // literally named `type`) opens no obligation; a nested array-of-arrays needs
        // `items` at both levels; a following sibling property's `items` never leaks
        // to the array above it; and an `items:` mentioned inside a block-scalar
        // `description:` (indented past the key) is not miscredited.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
components:
  schemas:
    Good1:
      type: array
      items:
        type: string
    Good2:
      description: items listed here
      items:
        $ref: '#/components/schemas/Good1'
      type: array
    Bad1:
      type: array
      minItems: 1
    Nested:
      type: array
      items:
        type: array
        items:
          type: string
    Obj:
      type: object
      properties:
        type:
          type: string
        list:
          type: array
          items:
            type: integer
    Prose:
      type: array
      description: |
        This mentions
        items: still just prose
    Pair:
      type: object
      properties:
        a:
          type: array
        b:
          items:
            type: string
          type: array
";
        // Flagged, in document order: `Bad1` (only a `minItems` sibling, no `items`),
        // `Prose` (its `description` block scalar mentions `items:` but only as
        // deeper-indented prose, so it is not a sibling), and `Pair.a` (the `items`
        // beneath `Pair.b` belongs to a *following* property, never the array above).
        // Not flagged: `Good1`/`Good2` (items after / before `type`), both levels of
        // `Nested`, `Obj.list` (the property literally named `type` and the outer
        // `type: object` open no obligation), and `Pair.b` (items before `type`).
        assert_eq!(array_schemas_missing_items(body), vec![24, 42, 50]);

        // Non-vacuous floor: across every registered spec every `type: array` schema
        // declares `items` (the invariant the contract test asserts), and the corpus
        // actually declares many array schemas, so a broken extractor can't hide
        // behind an empty scan. Count `type: array` lines with a detection independent
        // of the extractor.
        let mut arrays = 0usize;
        for api in APIS {
            assert!(
                array_schemas_missing_items(api.body).is_empty(),
                "{}: every `type: array` schema must declare `items`",
                api.name
            );
            for line in api.body.lines() {
                let v = line.trim_start().strip_prefix("type:").map(|v| {
                    v.split('#')
                        .next()
                        .unwrap_or(v)
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                });
                if v == Some("array") {
                    arrays += 1;
                }
            }
        }
        assert!(
            arrays >= 50,
            "expected many array schemas across specs, got {arrays}"
        );
    }

    /// Returns the 1-based line numbers of a schema's *lower*-bound keyword
    /// (`minimum`/`minLength`/`minItems`/`minProperties`) whose paired
    /// *upper*-bound keyword (`maximum`/`maxLength`/`maxItems`/`maxProperties`),
    /// declared as a sibling in the same schema object, holds a strictly smaller
    /// numeric value — an inverted, unsatisfiable range (`minimum: 100` beside
    /// `maximum: 1`, so no value can validate).
    ///
    /// Pure and YAML-dep-free: for each lower-bound key with an inline numeric
    /// value at indent `c`, scan its object's block both directions (down then
    /// up), each bounded by the first line indented *below* `c` (the dedent that
    /// closes the object), for the paired upper-bound key at *exactly* `c`. The
    /// exact-indent, dedent-bounded match keeps a bound nested in a sub-schema
    /// (a deeper `properties:` entry) or belonging to a following sibling schema
    /// from being mistaken for the pair. A bound whose value is non-numeric (a
    /// `{template}` or an unparsable scalar) or that opens a block rather than an
    /// inline value is skipped — there is nothing to compare. `min == max` (a
    /// single-value range) is valid; only `min > max` is flagged.
    fn schema_bounds_inverted(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar value of a `name:` key as f64 — inline comment and
        // quotes stripped. `None` when the line is a different key, opens a block
        // (no inline value), or the value isn't a number.
        let num_val = |l: &str, name: &str| -> Option<f64> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != name {
                return None;
            }
            let v = v
                .split('#')
                .next()
                .unwrap_or(v)
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            if v.is_empty() {
                return None;
            }
            v.parse::<f64>().ok()
        };
        // The (lower, upper) bound keyword pairs whose values must be ordered.
        const PAIRS: [(&str, &str); 4] = [
            ("minimum", "maximum"),
            ("minLength", "maxLength"),
            ("minItems", "maxItems"),
            ("minProperties", "maxProperties"),
        ];
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            for (lo, hi) in PAIRS {
                let Some(lv) = num_val(line, lo) else { continue };
                let c = indent(line);
                let mut hv = None;
                // Scan down through this object's block for the upper-bound sibling.
                let mut j = i + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() {
                        j += 1;
                        continue;
                    }
                    if indent(l) < c {
                        break; // dedented out of this object
                    }
                    if indent(l) == c {
                        if let Some(v) = num_val(l, hi) {
                            hv = Some(v);
                            break;
                        }
                    }
                    j += 1;
                }
                // The upper bound may be declared before the lower; scan up too.
                if hv.is_none() {
                    let mut k = i;
                    while k > 0 {
                        k -= 1;
                        let l = lines[k];
                        if l.trim().is_empty() {
                            continue;
                        }
                        if indent(l) < c {
                            break; // reached the key that opened this object
                        }
                        if indent(l) == c {
                            if let Some(v) = num_val(l, hi) {
                                hv = Some(v);
                                break;
                            }
                        }
                    }
                }
                if let Some(hv) = hv {
                    if lv > hv {
                        out.push(i + 1);
                    }
                }
            }
        }
        out
    }

    #[test]
    fn every_numeric_bound_is_ordered_low_to_high() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares both a lower and an upper bound of the same
        // family — `minimum`/`maximum`, `minLength`/`maxLength`, `minItems`/
        // `maxItems`, `minProperties`/`maxProperties` — the lower MUST NOT exceed the
        // upper. An inverted pair (`minimum: 100` beside `maximum: 1`) is an
        // unsatisfiable schema: no value validates, so a Redoc/Swagger/codegen client
        // is handed a field nothing can ever fill and a validator rejects every
        // payload at exactly the point a caller reads or builds it.
        //
        // A routine hazard in these scenario-table-heavy specs, where numeric ranges
        // are hand-tuned per API (a `maxAge`, a `radius`, an array-size cap): a bound
        // pasted from a sibling and only half-edited, or a lower/upper pair typed in
        // the wrong order. It is invisible to every existing test — the enum test
        // checks a value list's members, the required/parameter/array/`$ref` tests
        // check required entries, a parameter's identity, an array's element type, or
        // a ref's target; none ever compares two numeric keywords. Verified true
        // across all mounted specs before asserting.
        for api in APIS {
            let inverted = schema_bounds_inverted(api.body);
            assert!(
                inverted.is_empty(),
                "{} spec declares an inverted numeric bound pair (lower bound exceeds \
                 its upper-bound sibling — an unsatisfiable range) at line(s): {:?}",
                api.name,
                inverted
            );
        }
    }

    #[test]
    fn numeric_bound_ordering_extraction_rules() {
        // Unit-cover the `schema_bounds_inverted` extractor so the contract test
        // above can't pass vacuously and its detection is pinned: a lower bound is
        // flagged only when its same-family upper-bound *sibling* (same object, same
        // indent) holds a strictly smaller value; the upper bound declared before
        // *or* after the lower is paired; `min == max` (a single-value range) is
        // valid; a non-numeric value is skipped (nothing to compare); and a bound in
        // a different object — a following sibling schema, or a nested sub-schema at
        // another indent — is never mistaken for the pair.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
components:
  schemas:
    Good:
      type: integer
      minimum: 1
      maximum: 10
    BadNum:
      type: integer
      minimum: 100
      maximum: 1
    BadLen:
      type: string
      maxLength: 3
      minLength: 9
    Equal:
      type: integer
      minimum: 5
      maximum: 5
    Split:
      type: object
      properties:
        a:
          type: integer
          minimum: 50
        b:
          type: integer
          maximum: 1
    Nested:
      type: object
      minProperties: 1
      properties:
        inner:
          type: integer
          minimum: 2
          maximum: 100
      maxProperties: 3
    Weird:
      type: string
      minLength: notanumber
      maxLength: 5
";
        // Flagged, in document order: line 20 (`BadNum.minimum: 100` > its
        // `maximum: 1` sibling below) and line 25 (`BadLen.minLength: 9` > its
        // `maxLength: 3` sibling above — upper declared first). Not flagged: `Good`
        // and `Equal` (ordered / single-value ranges); `Split.a.minimum: 50`, whose
        // only candidate `maximum: 1` sits in the *following* property `Split.b` past
        // a dedent, so the two never pair; `Nested` (`minProperties: 1` pairs across
        // the nested `inner` sub-schema with `maxProperties: 3`, and `inner`'s own
        // `2`/`100` is ordered); and `Weird` (a non-numeric `minLength` is skipped).
        assert_eq!(schema_bounds_inverted(body), vec![20, 25]);

        // Non-vacuous floor: across every registered spec no bound pair is inverted
        // (the invariant the contract test asserts), and the corpus actually declares
        // many *ordered* bound pairs — so the value-comparison path runs on real data
        // and a broken (always-empty) extractor can't hide behind a corpus that never
        // pairs bounds. Count pairs with a presence-only detector independent of the
        // extractor's value comparison.
        let mut pairs = 0usize;
        for api in APIS {
            assert!(
                schema_bounds_inverted(api.body).is_empty(),
                "{}: every numeric bound pair must order lower <= upper",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let is_key = |l: &str, name: &str| {
                l.trim_start()
                    .split_once(':')
                    .is_some_and(|(k, _)| k.trim() == name)
            };
            for (lo, hi) in [
                ("minimum", "maximum"),
                ("minLength", "maxLength"),
                ("minItems", "maxItems"),
                ("minProperties", "maxProperties"),
            ] {
                for (i, l) in lines.iter().enumerate() {
                    if !is_key(l, lo) {
                        continue;
                    }
                    let c = indent(l);
                    let mut has = false;
                    let mut j = i + 1;
                    while j < lines.len() {
                        let x = lines[j];
                        if x.trim().is_empty() {
                            j += 1;
                            continue;
                        }
                        if indent(x) < c {
                            break;
                        }
                        if indent(x) == c && is_key(x, hi) {
                            has = true;
                            break;
                        }
                        j += 1;
                    }
                    if !has {
                        let mut k = i;
                        while k > 0 {
                            k -= 1;
                            let x = lines[k];
                            if x.trim().is_empty() {
                                continue;
                            }
                            if indent(x) < c {
                                break;
                            }
                            if indent(x) == c && is_key(x, hi) {
                                has = true;
                                break;
                            }
                        }
                    }
                    if has {
                        pairs += 1;
                    }
                }
            }
        }
        assert!(
            pairs >= 100,
            "expected many ordered bound pairs across specs, got {pairs}"
        );
    }

    /// Returns the 1-based line numbers of `example:` keys that share their
    /// object — same parent block, at the same indentation — with an `examples:`
    /// sibling. That pairing is the OpenAPI 3.0.x "the `example` field is
    /// mutually exclusive of the `examples` field" violation (Media Type Object
    /// and Parameter Object). Each conflicting object is reported once, at its
    /// `example:` line.
    ///
    /// Pure and YAML-dep-free: for each `example:` key at indent `c`, scan its
    /// object's block both directions (down then up), each bounded by the first
    /// line indented *below* `c` (the dedent that closes the object), and flag it
    /// when an `examples:` key appears at *exactly* `c`. The exact-indent match
    /// keeps a schema's own singular `example:` (3.0.x has no schema `examples:`)
    /// and a deeper-nested `examples:` inside the example payload from being
    /// mistaken for a sibling, and the dedent bound keeps a following media
    /// type's `examples:` from leaking across object boundaries.
    fn objects_declaring_both_example_and_examples(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let key_is = |l: &str, name: &str| -> bool {
            l.trim_start()
                .split_once(':')
                .is_some_and(|(k, _)| k.trim() == name)
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if !key_is(line, "example") {
                continue;
            }
            let c = indent(line);
            let mut conflict = false;
            // Scan down through this object's block for an `examples:` sibling.
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break; // dedented out of this object
                }
                if indent(l) == c && key_is(l, "examples") {
                    conflict = true;
                    break;
                }
                j += 1;
            }
            // `examples:` may be declared before `example:`; scan up the block.
            if !conflict {
                let mut k = i;
                while k > 0 {
                    k -= 1;
                    let l = lines[k];
                    if l.trim().is_empty() {
                        continue;
                    }
                    if indent(l) < c {
                        break; // reached the key that opened this object
                    }
                    if indent(l) == c && key_is(l, "examples") {
                        conflict = true;
                        break;
                    }
                }
            }
            if conflict {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn no_object_declares_both_example_and_examples() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): in a Media
        // Type Object and a Parameter Object the `example` field is *mutually
        // exclusive* of the `examples` field — a single object MUST NOT declare
        // both. A document that does is invalid, and a Redoc/Swagger/codegen
        // client is left to guess which sample to render or generate from, so the
        // documented example silently depends on the tool.
        //
        // A routine hazard in these vendored specs: a Media Type / Parameter block
        // drafted with a singular `example:` (or pasted from a sibling that used
        // one) later grows a richer `examples:` map, and the original `example:`
        // is left behind — both now sit as siblings. It is invisible to every
        // existing test: `every_media_type_declares_a_schema` checks that a
        // payload *has* a schema, never how its sample is expressed, and the
        // enum/required/array/`$ref` tests check value lists, required entries,
        // element types, or ref targets, never the example/examples pair.
        // Verified true across all mounted specs before asserting.
        for api in APIS {
            let both = objects_declaring_both_example_and_examples(api.body);
            assert!(
                both.is_empty(),
                "{} spec declares both `example` and `examples` in one object \
                 (they are mutually exclusive) at `example:` line(s): {:?}",
                api.name,
                both
            );
        }
    }

    #[test]
    fn example_examples_exclusivity_extraction_rules() {
        // Unit-cover the `objects_declaring_both_example_and_examples` extractor
        // so the contract test above can't pass vacuously and its detection is
        // pinned: an `example:` is flagged only when an `examples:` sits at the
        // same indent in the same object; `examples:` declared after *or* before
        // `example:` triggers it; a lone `example:` (parameter or schema) is left
        // alone; a following media type's `examples:` never leaks to the object
        // above it; and an `examples:` nested deeper than the `example:` (inside
        // the example payload, or in a sibling sub-schema) is not a sibling.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      parameters:
        - name: bad
          in: query
          example: 2
          examples:
            e1:
              value: 3
        - name: good
          in: query
          example: 1
      responses:
        '200':
          description: ok
          content:
            application/json:
              examples:
                e2:
                  value: baz
              example:
                foo: bar
            application/xml:
              example: solo
components:
  schemas:
    S:
      type: object
      properties:
        p:
          type: string
          example: nested
";
        // Flagged, in document order: the `example:` at line 12 (the parameter
        // named `bad`, with an `examples:` sibling directly below) and the
        // `example:` at line 27 (the `application/json` media type, whose
        // `examples:` sibling sits above it). Not flagged: line 18 (parameter
        // `good`, a lone `example:`), line 30 (`application/xml`, a lone
        // `example:` — the `examples:` above belongs to a different media type),
        // and line 38 (a schema's singular `example:`, no `examples:` in 3.0.x).
        assert_eq!(
            objects_declaring_both_example_and_examples(body),
            vec![12, 27]
        );

        // Non-vacuous floor: across every registered spec no object declares both
        // (the invariant the contract test asserts), and the corpus actually uses
        // both keys heavily — so the mutual-exclusivity surface is real and a
        // broken extractor can't hide behind an empty scan. Count each key with a
        // detection independent of the extractor.
        let (mut examples_plural, mut example_singular) = (0usize, 0usize);
        for api in APIS {
            assert!(
                objects_declaring_both_example_and_examples(api.body).is_empty(),
                "{}: no object may declare both `example` and `examples`",
                api.name
            );
            for line in api.body.lines() {
                match line.trim_start().split_once(':').map(|(k, _)| k.trim()) {
                    Some("examples") => examples_plural += 1,
                    Some("example") => example_singular += 1,
                    _ => {}
                }
            }
        }
        assert!(
            example_singular >= 100,
            "expected many `example:` keys across specs, got {example_singular}"
        );
        assert!(
            examples_plural >= 20,
            "expected many `examples:` keys across specs, got {examples_plural}"
        );
    }

    /// Extract the 1-based line number of every Discriminator Object a spec
    /// declares that is **missing its `propertyName`** — without a YAML dep.
    ///
    /// In OpenAPI 3.0.x a Discriminator Object has exactly one REQUIRED field,
    /// `propertyName` — the name of the payload property whose value selects the
    /// concrete schema (`mapping` is optional). A `discriminator:` block with no
    /// `propertyName` is invalid: a Redoc/Swagger/codegen client handed a
    /// polymorphic schema (CAMARA uses discriminators for the `Area`/`Device`/
    /// `SinkCredential` family) has no property to switch on, so it cannot pick a
    /// variant to deserialize or generate. It is invisible to every existing test
    /// — the array/enum/required/`$ref`/example tests check element types, value
    /// lists, required entries, ref targets, or example expression, never a
    /// discriminator's completeness.
    ///
    /// `discriminator:` names a Discriminator Object only as a block mapping key
    /// (its value is an object, never an inline scalar), and `propertyName` is one
    /// of its *children* — indented past the `discriminator:` key, not a sibling.
    /// For each block-form `discriminator:` at indent `C` this scans the object's
    /// block (following lines, skipping blanks, bounded by the first non-blank line
    /// that dedents to `C` or shallower — the sibling key or enclosing dedent that
    /// closes it) for a `propertyName:` key at any deeper indent. A
    /// `discriminator:` carrying an inline value opens no object (it would be a
    /// property literally named `discriminator`, not a Discriminator Object) and is
    /// skipped; a `propertyName:` never appears anywhere but a discriminator's
    /// children in these specs, so a deeper-indent match is unambiguous.
    fn discriminators_missing_property_name(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let key_is = |l: &str, name: &str| -> bool {
            l.trim_start()
                .split_once(':')
                .is_some_and(|(k, _)| k.trim() == name)
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            // A Discriminator Object is a block mapping key: `discriminator:` with
            // no inline scalar (an inline value would be a property named so).
            let Some((k, v)) = line.trim_start().split_once(':') else {
                continue;
            };
            if k.trim() != "discriminator"
                || !v.split('#').next().unwrap_or(v).trim().is_empty()
            {
                continue;
            }
            let c = indent(line);
            let mut has_property_name = false;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break; // dedented out of this discriminator object
                }
                if key_is(l, "propertyName") {
                    has_property_name = true;
                    break;
                }
                j += 1;
            }
            if !has_property_name {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_discriminator_declares_a_property_name() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every
        // Discriminator Object a mounted spec declares MUST carry `propertyName`,
        // its one REQUIRED field — the payload property whose value chooses the
        // concrete schema. A `discriminator:` with no `propertyName` is an invalid
        // document, and a Redoc/Swagger/codegen client handed a polymorphic schema
        // (CamaraSim uses discriminators for the `Area`/`Device` family) has
        // nothing to switch on, so it can't pick a variant to deserialize or
        // generate — the polymorphism silently breaks at the point a caller reads
        // or builds the payload.
        //
        // A routine hazard in these vendored specs: a discriminator block pasted
        // from a sibling that keeps `discriminator:` (and its optional `mapping:`)
        // but loses or dedents the `propertyName:` line. It is invisible to every
        // existing test — the array/enum/required/`$ref`/example tests check
        // element types, value lists, required entries, ref targets, or example
        // expression, never a discriminator's completeness. Verified true across
        // all mounted specs before asserting.
        for api in APIS {
            let incomplete = discriminators_missing_property_name(api.body);
            assert!(
                incomplete.is_empty(),
                "{} spec declares `discriminator` object(s) with no `propertyName` \
                 (its one REQUIRED field) at line(s): {:?}",
                api.name,
                incomplete
            );
        }
    }

    #[test]
    fn discriminator_property_name_extraction_rules() {
        // Unit-cover the `discriminators_missing_property_name` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a
        // discriminator is flagged only when its block carries no `propertyName`
        // child; a `propertyName` alongside an optional `mapping` satisfies it; a
        // discriminator whose only child is `mapping` (or that opens an empty block
        // before its sibling key) is flagged; and a discriminator's children are
        // bounded by the dedent that closes it.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
components:
  schemas:
    Good:
      type: object
      discriminator:
        propertyName: kind
      properties:
        kind:
          type: string
    GoodMapping:
      type: object
      discriminator:
        propertyName: areaType
        mapping:
          CIRCLE: '#/components/schemas/Circle'
      properties:
        areaType:
          type: string
    BadOnlyMapping:
      type: object
      discriminator:
        mapping:
          CIRCLE: '#/components/schemas/Circle'
      properties:
        areaType:
          type: string
    BadEmpty:
      type: object
      discriminator:
      properties:
        x:
          type: string
";
        // Flagged, in document order: `BadOnlyMapping`'s discriminator at line 32
        // (only a `mapping:` child, no `propertyName`) and `BadEmpty`'s at line 40
        // (its block dedents straight to the `properties:` sibling — no children at
        // all). Not flagged: `Good` (a lone `propertyName` child) and `GoodMapping`
        // (a `propertyName` child ahead of its optional `mapping`).
        assert_eq!(
            discriminators_missing_property_name(body),
            vec![32, 40]
        );

        // Non-vacuous floor: across every registered spec every discriminator
        // declares `propertyName` (the invariant the contract test asserts), and
        // the corpus actually declares discriminators (the `Area`/`Device` family),
        // so a broken extractor can't hide behind an empty scan. Count block-form
        // `discriminator:` keys with a detection independent of the extractor.
        let mut discriminators = 0usize;
        for api in APIS {
            assert!(
                discriminators_missing_property_name(api.body).is_empty(),
                "{}: every discriminator must declare `propertyName`",
                api.name
            );
            for line in api.body.lines() {
                if line.trim() == "discriminator:" {
                    discriminators += 1;
                }
            }
        }
        assert!(
            discriminators >= 5,
            "expected several discriminators across specs, got {discriminators}"
        );
    }

    /// Extract the 1-based line number of every `oneOf`/`anyOf`/`allOf` keyword a
    /// spec declares whose value is **not a sequence** (an array of schemas) —
    /// without a YAML dep.
    ///
    /// In OpenAPI 3.0.x the three schema-composition keywords `oneOf`, `anyOf`
    /// and `allOf` MUST each be an *array* of Schema Objects (`not`, by contrast,
    /// is a single schema, so it is deliberately excluded here). A composer whose
    /// value is a mapping (`allOf:` followed straight by `type: object` children)
    /// or a scalar is an invalid document: a Redoc/Swagger/codegen client expects
    /// a *list* of member schemas to compose — CamaraSim uses `allOf` to extend
    /// the shared `CamaraError` with each API's own `code` enum, and for the
    /// `Area`/`Device` polymorphic families — so it is handed a single object it
    /// can't iterate and the composition breaks. It is invisible to every existing
    /// test: the array-items / discriminator / enum / `$ref` tests check an
    /// `items` schema, a discriminator's completeness, a value list, or a ref
    /// target, never that a composer opens a sequence.
    ///
    /// For each `oneOf`/`anyOf`/`allOf` key: an inline flow-sequence value
    /// (`allOf: [ … ]`, first non-space char `[`) is a valid array and accepted;
    /// any other inline scalar is flagged. A block-form key (its value is empty
    /// once a trailing `# comment` is stripped) opens a block whose first
    /// non-blank following line decides it — a YAML sequence item (`-` first
    /// char), at the key's own indent or deeper, is a sequence and accepted; a
    /// mapping key or scalar indented deeper (the value is a mapping), or an
    /// immediate dedent to a shallower/sibling line (an empty value), is flagged.
    /// A line literally naming one of these keys inside prose is excluded by the
    /// exact key-name match before the value colon.
    fn composers_not_a_sequence(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some((k, v)) = line.trim_start().split_once(':') else {
                continue;
            };
            let k = k.trim();
            if k != "oneOf" && k != "anyOf" && k != "allOf" {
                continue;
            }
            // Strip a trailing `# comment` from the value; what remains, trimmed,
            // is the inline value (empty ⇒ the key opens a block).
            let inline = v.split('#').next().unwrap_or(v).trim();
            if !inline.is_empty() {
                // Inline value: only a flow sequence `[ … ]` is a valid array.
                if !inline.starts_with('[') {
                    out.push(i + 1);
                }
                continue;
            }
            // Block form: the first non-blank following line decides it.
            let c = indent(line);
            let mut opens_sequence = false;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                let li = indent(l);
                if li < c {
                    break; // dedented to an ancestor: the composer's value is empty
                }
                // A sequence item may sit at the key's own indent (`li == c`) or
                // deeper (`li > c`); a `-` first char is a YAML sequence entry.
                // Anything else — a sibling key at `li == c`, or a mapping key /
                // scalar at `li > c` — means the value is not a sequence.
                opens_sequence = l.trim_start().starts_with('-');
                break;
            }
            if !opens_sequence {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_composer_keyword_declares_a_sequence() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every
        // `oneOf`/`anyOf`/`allOf` a mounted spec declares MUST be a *sequence* —
        // an array of Schema Objects to compose. (`not` is a single schema and is
        // excluded.) A composer whose value is a mapping (`allOf:` straight to
        // `type: object` children) or a scalar is an invalid document: a
        // Redoc/Swagger/codegen client expecting a list of member schemas is
        // handed one object it can't iterate, so the composition silently breaks
        // where a caller reads or builds the payload.
        //
        // A routine hazard in these vendored specs, which lean on `allOf` to
        // extend the shared `CamaraError` with each API's own `code` enum and for
        // the `Area`/`Device` polymorphic families: a composer block pasted from a
        // sibling whose `- ` sequence markers are dropped/dedented in the edit,
        // collapsing the array into a bare mapping. It is invisible to every
        // existing test — the array-items / discriminator / enum / `$ref` tests
        // check an `items` schema, a discriminator's completeness, a value list,
        // or a ref target, never that a composer opens a sequence. Verified true
        // across all mounted specs before asserting.
        for api in APIS {
            let bad = composers_not_a_sequence(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares `oneOf`/`anyOf`/`allOf` whose value is not a \
                 sequence (an array of schemas) at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn composer_sequence_extraction_rules() {
        // Unit-cover the `composers_not_a_sequence` extractor so the contract test
        // above can't pass vacuously and its detection is pinned: a block composer
        // whose first child is a `- ` sequence item (at the key's own indent or
        // deeper) and an inline `[ … ]` flow sequence pass; a block composer whose
        // value is a mapping, an inline scalar, or empty (its block dedents
        // straight to a sibling key) is flagged, in document order.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      responses:
        '200':
          description: ok
components:
  schemas:
    GoodBlock:
      allOf:
        - $ref: '#/components/schemas/Base'
        - type: object
    GoodInline:
      oneOf: [ { type: string }, { type: integer } ]
    GoodSameIndent:
      anyOf:
      - type: string
      - type: integer
    BadMapping:
      allOf:
        type: object
        properties:
          x:
            type: string
    BadScalar:
      anyOf: nonsense
    BadEmpty:
      oneOf:
      properties:
        x:
          type: string
";
        // Flagged, in document order: `BadMapping`'s `allOf` at line 25 (its first
        // child is a mapping key, not a `- ` item), `BadScalar`'s `anyOf` at line
        // 31 (an inline scalar, not a flow sequence), and `BadEmpty`'s `oneOf` at
        // line 33 (its block dedents straight to the `properties:` sibling — an
        // empty value). Not flagged: `GoodBlock` (a deeper `- ` child),
        // `GoodInline` (an inline `[ … ]`) and `GoodSameIndent` (a `- ` child at
        // the key's own indent).
        assert_eq!(composers_not_a_sequence(body), vec![25, 31, 33]);

        // Non-vacuous floor: across every registered spec every composer opens a
        // sequence (the invariant the contract test asserts), and the corpus
        // actually declares many composers (`allOf` over `CamaraError` + the
        // polymorphic families), so a broken extractor can't hide behind an empty
        // scan. Count block-form composer keys with a detection independent of the
        // extractor.
        let mut composers = 0usize;
        for api in APIS {
            assert!(
                composers_not_a_sequence(api.body).is_empty(),
                "{}: every `oneOf`/`anyOf`/`allOf` must declare a sequence",
                api.name
            );
            for line in api.body.lines() {
                let t = line.trim();
                if t == "oneOf:" || t == "anyOf:" || t == "allOf:" {
                    composers += 1;
                }
            }
        }
        assert!(
            composers >= 50,
            "expected many composer keywords across specs, got {composers}"
        );
    }
}
