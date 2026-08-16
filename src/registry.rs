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
    ApiSpec {
        name: "esim-remote-management",
        version: "vwip",
        body: include_str!("../specs/esim-remote-management/vwip/openapi.yaml"),
    },
    ApiSpec {
        name: "in-home-device-management",
        version: "v1",
        body: include_str!("../specs/in-home-device-management/v1/openapi.yaml"),
    },
    ApiSpec {
        name: "network-access-domains",
        version: "vwip",
        body: include_str!("../specs/network-access-domains/vwip/openapi.yaml"),
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

    /// Whether an `operationId` token is a codegen-safe identifier: it MUST begin
    /// with an ASCII letter and thereafter contain only ASCII alphanumerics, `_`,
    /// or `-`.
    ///
    /// An `operationId` is the canonical machine name of an operation — client
    /// generators (OpenAPI Generator, Redocly, …) turn it into a method/function
    /// name. A token carrying whitespace, a leading digit, or punctuation a code
    /// identifier can't hold (`.`/`/`/`:`/`(`) is one no generator can render
    /// verbatim, so a caller reads or calls a mangled or dropped method exactly
    /// where the operationId is meant to name it. The `-` (used by CAMARA's own
    /// `send-sms` / `KYC_Fill-in`) is tolerated: it is not a bare-identifier
    /// character but every generator normalises it to a word boundary
    /// (`send-sms` → `sendSms`), so it stays deterministically resolvable — unlike
    /// whitespace or a leading digit. No regex dep: a hand-rolled ASCII scan.
    fn operation_id_is_well_formed(id: &str) -> bool {
        let mut chars = id.chars();
        match chars.next() {
            Some(c) if c.is_ascii_alphabetic() => {}
            _ => return false,
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
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

    /// The two shared fragments the server serves alongside every spec, named by
    /// the canonical relative path a spec at `specs/<name>/<version>/openapi.yaml`
    /// must use to reach each once served: `/shared/errors.yaml` via
    /// `../../shared/errors.yaml`, `/auth/openapi.yaml` via `../../auth/openapi.yaml`.
    const SERVED_SHARED_FRAGMENTS: [&str; 2] =
        ["../../shared/errors.yaml", "../../auth/openapi.yaml"];

    /// Return every **cross-file** `$ref` target in an embedded OpenAPI body whose
    /// file half is not one of the two served shared fragments, in document order,
    /// without a YAML dep.
    ///
    /// A cross-file `$ref` is `<relative-path>#/…` — a target with a non-empty path
    /// *before* the `#`. When a spec is served at `/{name}/{version}/openapi.yaml` a
    /// client resolves that path relative to that URL, and the server serves only
    /// three documents across files: the spec itself (reached by a local `#/…` ref,
    /// whose file half is empty), `/shared/errors.yaml` (reached by
    /// `../../shared/errors.yaml`) and `/auth/openapi.yaml` (reached by
    /// `../../auth/openapi.yaml`). A cross-file ref to any *other* file half — a
    /// CAMARA-template leftover (`../CAMARA_common.yaml`), a sibling API's spec, or a
    /// mistyped shared path — resolves to a URL the server never serves, so a client
    /// (Redoc/Swagger/codegen) that follows it gets a 404 and the served spec is
    /// unresolvable.
    ///
    /// Built on [`ref_targets`] (already unit-covered). A target with no `#` is
    /// skipped — that fragmentless shape is `refs_missing_fragment`'s contract; a
    /// target with an empty file half is a local ref (its own resolution test); a
    /// cross-file target whose file half is a served fragment is allowed.
    fn cross_file_refs_to_unserved_files(body: &str) -> Vec<String> {
        ref_targets(body)
            .into_iter()
            .filter(|t| match t.split_once('#') {
                // No `#` fragment at all — a different contract's concern.
                None => false,
                // Empty file half → a local `#/…` ref, resolved within the spec.
                Some((file, _pointer)) => {
                    !file.is_empty() && !SERVED_SHARED_FRAGMENTS.contains(&file)
                }
            })
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

    /// Inspect the *internal* well-formedness of every `x-camarasim-scenarios`
    /// block and return a one-line reason per malformed block (in document order),
    /// without a YAML dep.
    ///
    /// `scenario_blocks` (above) only counts that a block *exists*; a block that
    /// exists but documents nothing — a `cases:` sequence that was lost/dedented in
    /// a copy-paste, an empty `cases:` with no `- input:` item, or a case missing
    /// its `result:` — records no functional case yet passes the existence check.
    /// This closes that gap. A block is well-formed when:
    ///   * it declares a `cases:` key as a direct child (indent = block + 2), and
    ///   * that `cases:` holds ≥1 `- input:` case, and
    ///   * every `- input:` case has a `result:` sibling.
    /// (Every case in these specs is a `{ input, result }` mapping — DESIGN §7, §9.)
    ///
    /// The block body is the run of following lines indented deeper than the
    /// `x-camarasim-scenarios:` key (blank lines skipped). Within it, a `result:`
    /// is credited to the most recently opened `- input:` case, so a case with two
    /// results never covers for a later case with none.
    fn malformed_scenario_blocks(body: &str) -> Vec<String> {
        let indent = |l: &str| l.len() - l.trim_start().len();
        let lines: Vec<&str> = body.lines().collect();
        let mut out = Vec::new();
        let mut ordinal = 0usize;
        for (i, line) in lines.iter().enumerate() {
            if !line.trim_start().starts_with("x-camarasim-scenarios:") {
                continue;
            }
            let block_indent = indent(line);
            ordinal += 1;
            let mut has_cases = false;
            let mut case_has_result: Vec<bool> = Vec::new();
            for l in &lines[i + 1..] {
                if l.trim().is_empty() {
                    continue;
                }
                let ind = indent(l);
                if ind <= block_indent {
                    break; // dedented out of the block
                }
                let t = l.trim_start();
                if ind == block_indent + 2 && t == "cases:" {
                    has_cases = true;
                } else if t.starts_with("- input:") {
                    case_has_result.push(false);
                } else if t.starts_with("result:") {
                    if let Some(last) = case_has_result.last_mut() {
                        *last = true;
                    }
                }
            }
            let reason = if !has_cases {
                Some("no `cases:` sequence")
            } else if case_has_result.is_empty() {
                Some("`cases:` holds no `- input:` case")
            } else if case_has_result.iter().any(|&r| !r) {
                Some("a `- input:` case has no `result:`")
            } else {
                None
            };
            if let Some(r) = reason {
                out.push(format!("scenarios block #{ordinal}: {r}"));
            }
        }
        out
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

    /// The `#/components/headers/<name>@line N` label of every Header Object a spec
    /// defines under `components.headers:` whose object carries neither a `schema:`
    /// nor a `content:` key — the value-type field of an OpenAPI Header Object.
    ///
    /// A Header Object "follows the structure of the Parameter Object" (OpenAPI
    /// 3.0.x), so — exactly like a parameter — it MUST declare one of `schema` (a
    /// typed value, the common case) or `content` (a media-type-described value) to
    /// type the header it names; a Header Object with neither declares no type, so a
    /// Redoc/Swagger/codegen client has nothing to bind or render. The response-side
    /// analogue of `parameters_missing_schema_or_content`: every CamaraSim response
    /// echoes `x-correlator` via a `#/components/headers/XCorrelator` Header Object,
    /// and a `schema:` line lost or dedented in the paste that vendors a new spec
    /// leaves an untyped header no other contract test inspects (the parameter tests
    /// scope to `in:` parameters, the media-type tests to `content:` mappings — a
    /// Header Object under `components.headers` carries neither an `in:` nor a
    /// media-type child, so both skip it).
    ///
    /// A header entry given as a `$ref` (a Reference Object, `$ref:` at the Header
    /// Object's own child indent) is exempt — it inherits its type from the
    /// referenced component. Scopes exactly like `component_pointers` (top-level
    /// `components:` → the 2-space `headers:` section → an exact-4-space Header
    /// Object key), then scans that object's own 6-space direct children for a
    /// `schema:`/`content:`/`$ref:`; a `schema:` nested deeper (inside a `content:`
    /// media type) sits below the object's own indent and never satisfies the check.
    fn component_headers_missing_schema_or_content(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        let mut out = Vec::new();
        let mut in_components = false;
        let mut in_headers = false;
        for (i, line) in lines.iter().enumerate() {
            let is_top_level_key =
                !line.is_empty() && !line.starts_with(char::is_whitespace);
            if is_top_level_key {
                in_components = line.trim_end() == "components:";
                in_headers = false;
                continue;
            }
            if !in_components {
                continue;
            }
            // A 2-space direct child of `components:` opens (or closes) the `headers`
            // section; any other 2-space section key leaves it.
            if let Some(rest) = line.strip_prefix("  ") {
                if !rest.starts_with(char::is_whitespace) {
                    in_headers = rest.trim_end() == "headers:";
                    continue;
                }
            }
            if !in_headers {
                continue;
            }
            // An exact-4-space bare `Name:` key (non-space at column 5, nothing after
            // the colon) is a Header Object definition; a key bearing an inline value
            // opens no block object and is skipped (mirrors `component_pointers`).
            let Some(rest) = line.strip_prefix("    ") else { continue };
            if rest.starts_with(char::is_whitespace) {
                continue;
            }
            let Some(name) = rest.trim_end().strip_suffix(':') else { continue };
            if name.is_empty() || name.contains(char::is_whitespace) {
                continue;
            }
            // Scan the Header Object's own 6-space direct children until it dedents
            // (indent <= 4) or a blank line closes it. `$ref:` exempts (inherits);
            // `schema:`/`content:` types it. Deeper lines (a nested media-type
            // `schema:`) are ignored — they are not the header's own type field.
            let mut typed_or_ref = false;
            for l in &lines[i + 1..] {
                if l.trim().is_empty() {
                    break;
                }
                let li = indent(l);
                if li <= 4 {
                    break;
                }
                if li != 6 {
                    continue;
                }
                let t = l.trim_start();
                if t.starts_with("schema:")
                    || t.starts_with("content:")
                    || t.starts_with("$ref:")
                {
                    typed_or_ref = true;
                    break;
                }
            }
            if !typed_or_ref {
                out.push(format!("#/components/headers/{name}@line {}", i + 1));
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

    /// The `METHOD /path` label of every body-less-method operation
    /// (`GET`/`DELETE`/`HEAD`) a spec declares that nonetheless carries a
    /// `requestBody` — without a YAML dep.
    ///
    /// A request body on these methods has no defined semantics: RFC 9110
    /// (§9.3.1 GET, §9.3.2 HEAD, §9.3.5 DELETE) leaves the payload's meaning
    /// undefined, and the OpenAPI 3.0.x spec says a `requestBody` outside the
    /// methods with explicitly-defined body semantics "SHALL be ignored" by
    /// consumers — so a client/codegen tool drops it. The CAMARA API Design
    /// Guidelines match that, reserving request bodies for POST/PUT/PATCH (a read
    /// or delete carries its inputs in the path or query — exactly why CamaraSim's
    /// read APIs use `POST /retrieve` when they need a body). A `requestBody:`
    /// under a `get`/`delete`/`head` is therefore a body silently discarded at the
    /// point a caller believed it was sent.
    ///
    /// The complement of [`request_bodies_missing_content`], which inspects a
    /// *declared* body's shape but deliberately exempts a GET/DELETE that declares
    /// none — this flags the GET/DELETE/HEAD that declares one at all. Mirrors that
    /// sibling's path-item/method scoping exactly (a 4-space HTTP-verb key under a
    /// 2-space `/…` path item beneath the top-level `paths:` block); within such an
    /// operation it looks for the 6-space `requestBody:` key (a Request Body Object
    /// is a direct child of the Operation Object). A `requestBody` elsewhere — a
    /// component under `components.requestBodies`, or a schema property literally
    /// *named* `requestBody` — is not under an operation, so it is never seen.
    fn bodyless_method_operations_with_request_body(body: &str) -> Vec<String> {
        const BODYLESS: [&str; 3] = ["get", "delete", "head"];
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
                    let key =
                        rest.trim_end().strip_suffix(':').unwrap_or(rest.trim_end());
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
            if name.contains(char::is_whitespace) || !BODYLESS.contains(&name) {
                continue;
            }
            // Within this operation's block, look for a 6-space `requestBody:` key
            // (a Request Body Object is a direct child of the Operation Object).
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
                    if let Some((k, _)) = l.trim_start().split_once(':') {
                        if k == "requestBody" {
                            out.push(format!("{} {}", name.to_uppercase(), current_path));
                            break;
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

    #[test]
    fn every_operation_id_is_a_well_formed_token() {
        // Contract-harness invariant (OpenAPI structural + DESIGN §9): every
        // `operationId` a mounted spec declares MUST be a codegen-safe identifier —
        // it begins with an ASCII letter and thereafter holds only ASCII
        // alphanumerics, `_`, or `-`. The operationId is the operation's canonical
        // machine name: a client generator (OpenAPI Generator, Redocly, …) renders
        // it into a method name, so a token carrying whitespace, a leading digit, or
        // punctuation a code identifier can't hold (`.`/`/`/`:`/`(`) is mangled or
        // dropped exactly where a caller expects to call it.
        //
        // This is the *form* complement of the two existing operationId contract
        // tests: `operations_without_operation_id` asserts an operation *declares*
        // one and `operation_ids_are_unique_within_each_spec` asserts none repeats
        // within a document — but both take the token verbatim and never inspect its
        // characters, so a present, unique-but-malformed id (`get status`, a pasted
        // `2gConnect`, a stray `retrieve.status`) sails through both. A live hazard
        // in these copy-paste-drafted specs, where an operationId is hand-typed per
        // endpoint. CAMARA's own `send-sms` / `KYC_Fill-in` (a `-`/`_` normalised to
        // a word boundary by every generator) are deliberately admitted; only
        // genuinely unrenderable tokens are rejected. Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let malformed: Vec<String> = operation_ids(api.body)
                .into_iter()
                .filter(|id| !operation_id_is_well_formed(id))
                .collect();
            assert!(
                malformed.is_empty(),
                "{} spec declares a malformed operationId (must start with a letter \
                 then hold only alphanumerics/`_`/`-`): {:?}",
                api.name,
                malformed
            );
        }
    }

    #[test]
    fn operation_id_wellformedness_rules() {
        // Unit-cover the `operation_id_is_well_formed` predicate so the contract
        // test above can't pass vacuously and its accept/reject boundary is pinned.
        // Accepted: a plain camelCase id, an underscore/hyphen id (CAMARA's own
        // `KYC_Fill-in` / `send-sms`), a trailing digit, an all-caps segment.
        for ok in [
            "getSession",
            "retrieveSessionsByDevice",
            "KYC_Fill-in",
            "send-sms",
            "verifyAge2",
            "POSTThing",
        ] {
            assert!(operation_id_is_well_formed(ok), "should accept `{ok}`");
        }
        // Rejected: empty; a leading digit; embedded whitespace; and punctuation a
        // code identifier can't hold (`.`/`/`/`:`/`(`/non-ASCII).
        for bad in [
            "",
            "2gConnect",
            "get status",
            "get\tstatus",
            "retrieve.status",
            "path/op",
            "op:read",
            "call()",
            "vérifier",
        ] {
            assert!(!operation_id_is_well_formed(bad), "should reject `{bad:?}`");
        }

        // Non-vacuous floor: the corpus declares many operationIds and every one is
        // well-formed (the invariant the contract test asserts), so the accept path
        // runs on real data and a broken (always-true) predicate can't hide behind
        // an empty loop.
        let mut total = 0usize;
        for api in APIS {
            for id in operation_ids(api.body) {
                assert!(
                    operation_id_is_well_formed(&id),
                    "{}: operationId `{}` must be well-formed",
                    api.name,
                    id
                );
                total += 1;
            }
        }
        assert!(
            total >= 100,
            "expected many operationIds across specs, got {total}"
        );
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
    fn every_scenario_block_is_well_formed() {
        // Contract-harness invariant (DESIGN §7, §9): the sibling
        // `every_spec_documents_functional_cases` proves each spec declares ≥1
        // `x-camarasim-scenarios` block, but never looks *inside* one — a block
        // whose `cases:` sequence was lost or dedented in the copy-paste that
        // drafts a new operation, an empty `cases:` with no `- input:` case, or a
        // case missing its `result:` documents no functional case yet still counts
        // toward that existence check. Those are exactly the drifts the
        // existence/identity/wiring tests can't see (they never read a block's body).
        // This asserts every block declares a `cases:` sequence holding ≥1
        // `{ input, result }` case, so the vendored spec's machine-readable record
        // of the server's parameter-driven behaviour is never an empty shell.
        // Verified true across all mounted specs (142 blocks, 864 cases) before
        // asserting.
        for api in APIS {
            let malformed = malformed_scenario_blocks(api.body);
            assert!(
                malformed.is_empty(),
                "{} spec has malformed x-camarasim-scenarios block(s): {:?} \
                 (each block documents its functional cases as a non-empty `cases:` \
                 sequence of `{{ input, result }}` cases — DESIGN §7, §9)",
                api.name,
                malformed
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
    fn cross_file_ref_target_extraction_rules() {
        // Unit-cover the `cross_file_refs_to_unserved_files` classifier so the
        // contract test below can't pass vacuously and its file-half rules are
        // pinned: a local `#/…` ref (empty file half) and a cross-file ref into
        // either served fragment are allowed; a cross-file ref to any other file
        // half is flagged; a fragmentless target is left to another contract.
        let body = "\
components:
  schemas:
    A:
      properties:
        local:
          $ref: '#/components/schemas/B'
        served_err:
          $ref: '../../shared/errors.yaml#/components/responses/Generic400'
        served_auth:
          $ref: '../../auth/openapi.yaml#/components/securitySchemes/camaraOAuth'
        template_leftover:
          $ref: '../CAMARA_common.yaml#/components/responses/Generic404'
        sibling_spec:
          $ref: './OtherApi.yaml#/components/schemas/Foo'
        bare_errors:
          $ref: 'errors.yaml#/components/responses/NotFound'
        fragmentless:
          $ref: 'errors.yaml'
";
        assert_eq!(
            cross_file_refs_to_unserved_files(body),
            vec![
                "../CAMARA_common.yaml#/components/responses/Generic404",
                "./OtherApi.yaml#/components/schemas/Foo",
                "errors.yaml#/components/responses/NotFound",
            ]
        );
        // The two served fragments and a local ref are never flagged.
        let clean = "\
    $ref: '#/components/schemas/Local'
    $ref: '../../shared/errors.yaml#/components/responses/Generic429'
    $ref: '../../auth/openapi.yaml#/y'
";
        assert!(cross_file_refs_to_unserved_files(clean).is_empty());
        // A `- $ref:` sequence item to an unserved file is still classified (the
        // underlying extractor handles the sequence form).
        let seq = "  parameters:\n    - $ref: '../common/params.yaml#/components/parameters/XCorr'\n";
        assert_eq!(
            cross_file_refs_to_unserved_files(seq),
            vec!["../common/params.yaml#/components/parameters/XCorr"]
        );
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
    fn malformed_scenario_block_extraction_rules() {
        // Unit-cover `malformed_scenario_blocks` so the contract test above can't
        // pass vacuously (an extractor that always returned `[]` would make
        // `malformed.is_empty()` trivially true) and so each failure mode is pinned.

        // A well-formed block (a `cases:` with two `{ input, result }` cases) is
        // never flagged.
        let ok = "      x-camarasim-scenarios:\n\
                  \x20       description: text\n\
                  \x20       cases:\n\
                  \x20         - input: a\n\
                  \x20           result: \"200 x\"\n\
                  \x20         - input: b\n\
                  \x20           result: \"404 y\"\n";
        assert!(malformed_scenario_blocks(ok).is_empty());

        // No `cases:` at all → flagged.
        let no_cases = "      x-camarasim-scenarios:\n\
                        \x20       description: text only\n";
        assert_eq!(
            malformed_scenario_blocks(no_cases),
            vec!["scenarios block #1: no `cases:` sequence".to_string()]
        );

        // A `cases:` with no `- input:` case → flagged.
        let empty_cases = "      x-camarasim-scenarios:\n\
                           \x20       cases:\n\
                           \x20 next: dedented\n";
        assert_eq!(
            malformed_scenario_blocks(empty_cases),
            vec!["scenarios block #1: `cases:` holds no `- input:` case".to_string()]
        );

        // A case missing its `result:` is caught even when a *later* case has one
        // (a two-result case never covers for a result-less earlier case).
        let missing_result = "      x-camarasim-scenarios:\n\
                              \x20       cases:\n\
                              \x20         - input: a\n\
                              \x20         - input: b\n\
                              \x20           result: ok\n";
        assert_eq!(
            malformed_scenario_blocks(missing_result),
            vec!["scenarios block #1: a `- input:` case has no `result:`".to_string()]
        );

        // Ordinals count blocks in document order; a well-formed first block and a
        // broken second block report only the second.
        let two = format!("{ok}{no_cases}");
        assert_eq!(
            malformed_scenario_blocks(&two),
            vec!["scenarios block #2: no `cases:` sequence".to_string()]
        );

        // Non-vacuous floor: every mounted spec's blocks parse clean, and there are
        // real blocks to parse (so the clean result is earned, not empty input).
        let total_blocks: usize = APIS.iter().map(|a| scenario_blocks(a.body)).sum();
        assert!(
            total_blocks >= 100,
            "expected many scenario blocks, got {total_blocks}"
        );
        for api in APIS {
            assert!(
                malformed_scenario_blocks(api.body).is_empty(),
                "{} has a malformed scenarios block",
                api.name
            );
        }
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
    fn every_paths_object_lists_distinct_path_keys() {
        // Contract-harness invariant (OpenAPI / YAML structural rule): a document's
        // `paths` object is a mapping keyed by path template, so a mounted spec MUST
        // NOT list the same path template twice under `paths:`. A repeated key is an
        // invalid mapping every YAML/JSON parser resolves by keeping only the *last*
        // Path Item — so the earlier item's entire operation set (its `get`/`post`/…,
        // their parameters and responses) is dropped without a trace, and the
        // endpoints a client/codegen tool binds for that path are whichever block
        // came last.
        //
        // The Paths-Object member of the "no-duplicates" family: the sibling
        // distinct tests check a `required` list's entries, a parameter array's
        // `(name, location)` pairs, an enum's values, a `properties:` mapping's
        // property names, and an operation's id — none looks at the *path* keys. In
        // these specs a new endpoint's path item is drafted by copy-pasting a
        // sibling path block, so a template pasted and left unrenamed (two
        // `/sessions:` keys) is a live hazard that silently erases one path's
        // operations while every operation-scoped test still passes on the surviving
        // copy. Verified true across all mounted specs before asserting.
        for api in APIS {
            let keys = path_item_keys(api.body);
            let mut seen = HashSet::new();
            let dups: Vec<&String> =
                keys.iter().filter(|k| !seen.insert((*k).clone())).collect();
            assert!(
                dups.is_empty(),
                "{} spec repeats a `paths:` key (path templates must be distinct; a \
                 duplicate silently drops the earlier path item's operations): {:?}",
                api.name,
                dups
            );
        }
    }

    #[test]
    fn path_item_key_duplicate_detection_rules() {
        // Unit-cover the duplicate detection the contract test above relies on so it
        // can't pass vacuously: pin that `path_item_keys` preserves *every*
        // occurrence of a repeated path template (it does not de-duplicate), and
        // that a plain seen-set repeat detector — the contract test's logic — flags
        // exactly the second copy. A set-collapsing extractor would make the
        // contract test blind to the very drift it guards.
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
  /sessions:
    delete:
      operationId: drop
      responses:
        '204':
          description: gone
";
        // `/sessions` appears twice (the pasted-and-unrenamed hazard) and
        // `/sessions/{id}` once — the extractor returns all three in document order,
        // so the repeat is visible to the contract test.
        assert_eq!(
            path_item_keys(body),
            vec![
                "/sessions".to_string(),
                "/sessions/{id}".to_string(),
                "/sessions".to_string(),
            ]
        );
        let keys = path_item_keys(body);
        let mut seen = HashSet::new();
        let dups: Vec<&String> =
            keys.iter().filter(|k| !seen.insert((*k).clone())).collect();
        assert_eq!(dups, vec![&"/sessions".to_string()]);

        // Non-vacuous floor: across every registered spec no path template repeats
        // (the invariant the contract test asserts), and the corpus declares many
        // distinct path keys, so the repeat-detection path runs on real data and a
        // broken (always-distinct) extractor can't hide behind a corpus with no
        // paths.
        let mut total = 0usize;
        for api in APIS {
            let keys = path_item_keys(api.body);
            let mut seen = HashSet::new();
            for k in &keys {
                assert!(
                    seen.insert(k.clone()),
                    "{}: duplicate path key `{}`",
                    api.name,
                    k
                );
            }
            total += keys.len();
        }
        assert!(total >= 100, "expected many path keys across specs, got {total}");
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
    fn no_bodyless_method_operation_declares_a_request_body() {
        // Contract-harness invariant (OpenAPI 3.0.x semantics + CAMARA API Design
        // Guidelines): no `GET`/`DELETE`/`HEAD` operation a mounted spec declares
        // may carry a `requestBody`. A request body on these methods has no defined
        // semantics — RFC 9110 leaves a GET (§9.3.1), HEAD (§9.3.2), or DELETE
        // (§9.3.5) payload's meaning undefined, and the OpenAPI 3.0.x spec says a
        // `requestBody` outside the methods with explicitly-defined body semantics
        // "SHALL be ignored" by consumers — so a client/codegen tool drops it. The
        // CAMARA guidelines reserve request bodies for POST/PUT/PATCH (a read or
        // delete carries its inputs in the path or query, which is exactly why
        // CamaraSim's read APIs use `POST /retrieve` when they need a body). A
        // `requestBody` under a `get`/`delete`/`head` is therefore a body silently
        // discarded at the point a caller believed it was sending one.
        //
        // The complement of `every_request_body_declares_content`, which inspects a
        // *declared* body's shape but deliberately exempts a GET/DELETE that
        // declares none: this catches the GET/DELETE/HEAD that declares one at all.
        // No other contract test sees it — the method/path scoping tests
        // (`operations_without_responses`, the path-templating and parameter tests)
        // inspect an operation's responses, path variables, or parameters, never
        // whether a body-less method mistakenly consumes a body. A live copy-paste
        // hazard in these specs, where a read endpoint is drafted from a POST
        // sibling and keeps its pasted `requestBody:` block. Verified true across
        // all mounted specs before asserting.
        for api in APIS {
            let offenders = bodyless_method_operations_with_request_body(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec declares a GET/DELETE/HEAD operation carrying a \
                 `requestBody` (a body OpenAPI 3.0.x / RFC 9110 leave with no \
                 defined semantics — reserve request bodies for POST/PUT/PATCH): \
                 {:?}",
                api.name,
                offenders
            );
        }
    }

    #[test]
    fn bodyless_method_request_body_extraction_rules() {
        // Unit-cover `bodyless_method_operations_with_request_body` so the contract
        // test above can't pass vacuously and its scoping is pinned: a `requestBody`
        // under a `post` is never flagged (a body is well-defined there); one under
        // a `get`/`delete`/`head` is flagged with its `METHOD /path` label; a
        // body-less method that declares *no* `requestBody` is not flagged; and a
        // `requestBody:` that is not an operation's — a schema property literally
        // named `requestBody` under `components` — is never seen (it is not under
        // `paths:`).
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /good:
    post:
      operationId: postGood
      requestBody:
        content:
          application/json:
            schema:
              type: object
      responses:
        '200':
          description: ok
  /read/{id}:
    get:
      operationId: getRead
      requestBody:
        content:
          application/json:
            schema:
              type: object
      responses:
        '200':
          description: ok
    delete:
      operationId: deleteRead
      requestBody:
        content:
          application/json:
            schema:
              type: object
      responses:
        '204':
          description: gone
  /probe:
    head:
      operationId: probeHead
      requestBody:
        content:
          application/json:
            schema:
              type: object
      responses:
        '200':
          description: ok
  /clean/{id}:
    get:
      operationId: getClean
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      responses:
        '200':
          description: ok
components:
  schemas:
    Thing:
      type: object
      properties:
        requestBody:
          type: string
";
        // Flagged, in document order: `GET /read/{id}` and `DELETE /read/{id}`
        // (both declare a `requestBody` under a body-less method) and `HEAD /probe`.
        // Not flagged: `POST /good` (a body is well-defined on POST); `GET
        // /clean/{id}` (a body-less method with no `requestBody` — only a
        // `parameters:` block); and the schema property literally *named*
        // `requestBody` under `components.schemas.Thing.properties`, which is not
        // under `paths:` and so is never an operation's request body.
        assert_eq!(
            bodyless_method_operations_with_request_body(body),
            vec![
                "GET /read/{id}".to_string(),
                "DELETE /read/{id}".to_string(),
                "HEAD /probe".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec no body-less-method
        // operation declares a `requestBody` (the invariant the contract test
        // asserts), and the corpus actually declares many GET/DELETE/HEAD
        // operations — so the scan runs on a real, non-empty population rather than
        // an empty loop. Count body-less method keys with a detector independent of
        // the extractor.
        let mut bodyless_ops = 0usize;
        for api in APIS {
            assert!(
                bodyless_method_operations_with_request_body(api.body).is_empty(),
                "{}: no GET/DELETE/HEAD operation may declare a `requestBody`",
                api.name
            );
            for line in api.body.lines() {
                if matches!(line.trim(), "get:" | "delete:" | "head:") {
                    bodyless_ops += 1;
                }
            }
        }
        assert!(
            bodyless_ops >= 30,
            "expected many GET/DELETE/HEAD operations across specs, got {bodyless_ops}"
        );
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
    fn every_component_header_declares_a_schema_or_content() {
        // Contract-harness invariant (OpenAPI structural rule): every Header Object a
        // mounted spec defines under `components.headers:` MUST carry one of `schema`
        // or `content` — the field that types the header's value. A Header Object
        // "follows the structure of the Parameter Object" (OpenAPI 3.0.x), so — like a
        // parameter — an untyped one declares nothing a Redoc/Swagger/codegen client
        // can bind or render.
        //
        // The response-side analogue of `every_parameter_declares_a_schema_or_content`
        // (which pins the same field on request/path/query parameters): every
        // CamaraSim response echoes `x-correlator` via a
        // `#/components/headers/XCorrelator` Header Object, and a `schema:` line lost
        // or dedented in the paste that vendors a new spec leaves an untyped header no
        // other contract test inspects — the parameter tests scope to `in:` parameters
        // and the media-type tests to `content:` mappings, and a Header Object under
        // `components.headers` carries neither, so both skip it. A `$ref` header entry
        // is exempt (it inherits its type). Verified true across all mounted specs
        // before asserting.
        for api in APIS {
            let untyped = component_headers_missing_schema_or_content(api.body);
            assert!(
                untyped.is_empty(),
                "{} spec defines a `components.headers` Header Object with neither a \
                 `schema` nor a `content` (an OpenAPI Header Object MUST declare one): \
                 {:?}",
                api.name,
                untyped
            );
        }
    }

    #[test]
    fn component_header_schema_or_content_extraction_rules() {
        // Unit-cover the `component_headers_missing_schema_or_content` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a
        // Header Object under `components.headers` is flagged only when its object
        // carries none of `schema:`/`content:`/`$ref:` at its own child indent — a
        // `schema`-typed header and a `content`-typed header pass, a `$ref` header is
        // exempt (inherits), and a `schema:` nested inside a `content:` media type does
        // NOT count as the header's own type. Header maps outside `components.headers`
        // (a response's inline `headers:`, a schema property named `headers`) are not
        // scanned here — the corpus declares its Header Objects only under
        // `components.headers`, all via a shared `XCorrelator` definition.
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
          headers:
            x-correlator:
              $ref: '#/components/headers/XCorrelator'
components:
  headers:
    XCorrelator:
      description: echoed
      required: false
      schema:
        type: string
    ContentTyped:
      description: media-typed value
      content:
        application/json:
          schema:
            type: object
    RefHeader:
      $ref: '#/components/headers/XCorrelator'
    Broken:
      description: no type at all
      required: false
  schemas:
    Thing:
      type: object
";
        // Flagged: only `Broken` — its object carries `description`/`required` but no
        // `schema:`/`content:`/`$ref:`. Not flagged: `XCorrelator` (`schema:`),
        // `ContentTyped` (`content:`, whose nested media-type `schema:` sits deeper and
        // is not the header's own), and `RefHeader` (`$ref:`, exempt). The response's
        // inline `x-correlator` `$ref` under `/a` is outside `components.headers` and
        // is never scanned.
        assert_eq!(
            component_headers_missing_schema_or_content(body),
            vec!["#/components/headers/Broken@line 30".to_string()]
        );

        // Non-vacuous floor: across every registered spec, every `components.headers`
        // Header Object declares a `schema` or `content` (the invariant the contract
        // test asserts), and the corpus actually defines many such headers (each spec
        // carries at least its `XCorrelator`), so a broken extractor can't hide behind
        // an empty scan.
        let mut total_headers = 0usize;
        for api in APIS {
            assert!(
                component_headers_missing_schema_or_content(api.body).is_empty(),
                "{}: every `components.headers` Header Object must declare a `schema` \
                 or `content`",
                api.name
            );
            // Count exact-4-space Header Object keys under a `components.headers`
            // section, mirroring the extractor's scoping.
            let mut in_components = false;
            let mut in_headers = false;
            for line in api.body.lines() {
                if !line.is_empty() && !line.starts_with(char::is_whitespace) {
                    in_components = line.trim_end() == "components:";
                    in_headers = false;
                    continue;
                }
                if !in_components {
                    continue;
                }
                if let Some(rest) = line.strip_prefix("  ") {
                    if !rest.starts_with(char::is_whitespace) {
                        in_headers = rest.trim_end() == "headers:";
                        continue;
                    }
                }
                if !in_headers {
                    continue;
                }
                if let Some(rest) = line.strip_prefix("    ") {
                    if !rest.starts_with(char::is_whitespace)
                        && rest.trim_end().ends_with(':')
                    {
                        total_headers += 1;
                    }
                }
            }
        }
        assert!(
            total_headers >= 50,
            "expected many components.headers Header Objects across specs, got {total_headers}"
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
    fn every_cross_file_ref_targets_a_served_fragment() {
        // Contract-harness invariant (DESIGN §8/§9 + `apis::openapi` serving): every
        // *cross-file* `$ref` a mounted spec makes — a `<relative-path>#/…` with a
        // non-empty path before the `#` — MUST target one of the only two shared
        // fragments the server serves alongside the spec: the error model
        // (`../../shared/errors.yaml`) or the auth scheme (`../../auth/openapi.yaml`).
        // A spec served at `/{name}/{version}/openapi.yaml` resolves a cross-file ref
        // relative to that URL, and the server serves nothing else across files — so a
        // ref to any other file half resolves to a URL it never serves, and the served
        // spec is unresolvable for any client (Redoc/Swagger/codegen) that follows it.
        //
        // This closes the gap every sibling ref test leaves for a *fragment-bearing*
        // cross-file ref to an unserved file (a CAMARA-template leftover
        // `../CAMARA_common.yaml#/…`, a sibling API's spec, a mistyped shared path):
        // - `every_ref_target_is_a_fragment_pointer` only checks a ref *has* a `#/`
        //   fragment — this ref has one, so it passes there;
        // - `shared_fragment_refs_use_the_canonical_relative_path` only inspects refs
        //   whose target already names one of the two shared files — an unrelated file
        //   half is never examined;
        // - `shared_error_refs`/`shared_auth_refs_resolve_to_defined_components` only
        //   dereference pointers whose file half is one of those two fragments;
        // - `local_component_refs_resolve_within_their_own_spec` only inspects refs
        //   with an *empty* file half (local `#/…`).
        // So a cross-file ref to a third file falls through all of them. (It overlaps
        // the canonical-path test only on the bare `errors.yaml#/…` form, which both
        // reject — deliberate defence in depth.)
        //
        // Non-vacuous: the classifier must actually be seeing cross-file refs, so we
        // also tally the *allowed* cross-file refs (into the two served fragments) and
        // assert a floor — an extractor that found none would make the per-spec
        // assertions unreachable.
        let mut served_cross_file = 0usize;
        for api in APIS {
            let offenders = cross_file_refs_to_unserved_files(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec makes cross-file $ref(s) to a file the server does not serve \
                 (only `../../shared/errors.yaml` and `../../auth/openapi.yaml` resolve \
                 when the spec is served) — they 404 for any client that follows them: \
                 {:?}",
                api.name,
                offenders
            );
            for t in ref_targets(api.body) {
                if let Some((file, _)) = t.split_once('#') {
                    if SERVED_SHARED_FRAGMENTS.contains(&file) {
                        served_cross_file += 1;
                    }
                }
            }
        }
        assert!(
            served_cross_file >= 50,
            "expected the mounted specs to make ≥50 cross-file $refs into the two \
             served shared fragments (each business op $refs the shared error \
             responses), got {served_cross_file} — the classifier may not be seeing \
             cross-file refs, so the contract could pass vacuously"
        );
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

    /// Enumerate every object-schema `required:` entry a spec declares that names a
    /// property the *same object* does not define under its sibling `properties:`
    /// block — reported as `"required '<entry>' not in properties [<keys>]"` in
    /// document order, without a YAML dep.
    ///
    /// A JSON-Schema / OpenAPI object schema's `required` array names properties an
    /// instance MUST carry, and those names are only meaningful against the object's
    /// `properties`: a `required` entry that matches no declared property is an
    /// **unsatisfiable** constraint — the schema demands a field it never defines, so
    /// no payload validates and a codegen client emits a presence check on a member it
    /// can't generate. The live drift it catches: a `required:` block pasted from a
    /// sibling schema and only half-edited, or a property since renamed while its
    /// `required` entry was left stale (`phoneNumber` required, `phone_number`
    /// defined). This is invisible to every sibling test — the distinct-entries test
    /// checks a `required` array's names are *unique*, the distinct-property-names test
    /// checks a `properties` block's keys are unique, but neither ever *cross-checks*
    /// the two, and the parameter/response/enum/`$ref` tests look elsewhere entirely.
    ///
    /// Scope, to stay false-positive-free on composed schemas: a `required` array is
    /// judged only when the object declares a sibling `properties:` block (else the
    /// list can't be resolved — skipped) and is **not** part of a schema composition,
    /// where a required name may legitimately be defined in a *different* branch. Both
    /// forms are excluded: an ancestor `allOf`/`oneOf`/`anyOf` up the indent ladder
    /// (mirroring the `example:` ancestor walk in `facet_keyword_type_mismatches`) —
    /// so an `allOf` member's `required` referencing an inherited property is not
    /// flagged — and a sibling `allOf`/`oneOf`/`anyOf` at the object's own level. The
    /// scalar `required: true`/`false` flag (a parameter/requestBody boolean, not a
    /// schema's property list) is never read as an array. Names and property keys are
    /// unquoted and a trailing ` #` comment trimmed. Whole-document scan (required
    /// arrays live under `components.schemas` and inline request/response schemas
    /// alike), mirroring the enum / required-uniqueness tests.
    fn required_entries_without_a_declared_property(body: &str) -> Vec<String> {
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
        // The trimmed mapping-key of a line (text before its first `:`), or `None` for
        // a sequence item (`- …`) or a non-key line.
        let key_of = |l: &str| -> Option<String> {
            let t = l.trim_start();
            if t.starts_with('-') {
                return None;
            }
            let (k, _) = t.split_once(':')?;
            let k = k.trim();
            if k.is_empty() {
                None
            } else {
                Some(k.to_string())
            }
        };
        // True when line `i` (indent `c`) sits inside a schema-composition subtree —
        // some enclosing container key up the indent ladder is `allOf`/`oneOf`/`anyOf`
        // — so a `required` entry there may name a property inherited from a *sibling*
        // composition branch, not the local `properties` block (mirrors the ancestor
        // walk in `facet_keyword_type_mismatches`).
        let inside_composition = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some(key) = key_of(l) {
                        if key == "allOf" || key == "oneOf" || key == "anyOf" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The direct property keys of the sibling `properties:` block in the same object
        // as line `i` (indent `c`): scan down then up at exactly indent `c`, dedent-
        // bounded (mirroring the sibling-`type` scan in `facet_keyword_type_mismatches`),
        // for a block-form `properties:` opener; then collect its first-child-indent
        // mapping keys. `None` when the object declares no sibling `properties` block
        // (the required list can't be resolved), an inline (flow) `properties:` value
        // (not enumerable line-by-line), or a sibling `allOf`/`oneOf`/`anyOf` (a
        // property may be composed in) — all "can't judge" outcomes.
        let sibling_property_keys = |i: usize, c: usize| -> Option<Vec<String>> {
            let props_line_from = |l: &str| -> Option<Option<()>> {
                // Some(Some(())) => a block-form `properties:` opener; Some(None) => an
                // inline `properties: {…}` (abort); None => not a `properties` key.
                let key = key_of(l)?;
                if key != "properties" {
                    return None;
                }
                let after = l
                    .trim_start()
                    .split_once(':')
                    .map(|(_, v)| v)
                    .unwrap_or("");
                let after = after.split('#').next().unwrap_or("").trim();
                Some(if after.is_empty() { Some(()) } else { None })
            };
            let mut props_line: Option<usize> = None;
            // Down-scan the rest of the object.
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                let li = indent(l);
                if li < c {
                    break;
                }
                if li == c {
                    if let Some(key) = key_of(l) {
                        if key == "allOf" || key == "oneOf" || key == "anyOf" {
                            return None;
                        }
                    }
                    match props_line_from(l) {
                        Some(Some(())) if props_line.is_none() => props_line = Some(j),
                        Some(None) => return None, // inline properties — can't judge
                        _ => {}
                    }
                }
                j += 1;
            }
            // Up-scan the earlier keys of the same object.
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < c {
                    break;
                }
                if li == c {
                    if let Some(key) = key_of(l) {
                        if key == "allOf" || key == "oneOf" || key == "anyOf" {
                            return None;
                        }
                    }
                    match props_line_from(l) {
                        Some(Some(())) if props_line.is_none() => props_line = Some(k),
                        Some(None) => return None,
                        _ => {}
                    }
                }
            }
            let p = props_line?;
            let cp = indent(lines[p]);
            // Collect the property block's direct children (its first, shallowest child
            // indent); deeper lines are a property's own schema, not a property key.
            let mut child_indent: Option<usize> = None;
            let mut keys: Vec<String> = Vec::new();
            let mut j = p + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() || l.trim_start().starts_with('#') {
                    j += 1;
                    continue;
                }
                let li = indent(l);
                if li <= cp {
                    break;
                }
                let ci = *child_indent.get_or_insert(li);
                if li < ci {
                    break;
                }
                if li == ci {
                    if let Some(key) = key_of(l) {
                        keys.push(norm(&key));
                    }
                }
                j += 1;
            }
            Some(keys)
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
            let c = indent(line);
            let rest = t["required:".len()..].trim_start();
            let ri = i;
            let mut entries: Vec<String> = Vec::new();
            if rest.starts_with('[') {
                // Flow list — gather across lines to the closing `]`.
                let mut buf = rest.to_string();
                let mut kk = i;
                while !buf.contains(']') && kk + 1 < lines.len() {
                    kk += 1;
                    buf.push(' ');
                    buf.push_str(lines[kk].trim());
                }
                let open = buf.find('[').map(|x| x + 1).unwrap_or(0);
                let close = buf.rfind(']').unwrap_or(buf.len());
                let inner = if close >= open { &buf[open..close] } else { "" };
                if !inner.trim().is_empty() {
                    entries = inner.split(',').map(|s| norm(s)).filter(|v| !v.is_empty()).collect();
                }
                i = kk + 1;
            } else if rest.is_empty() || rest.starts_with('#') {
                // Block list — `- name` children at a deeper indent, only when the first
                // non-blank child is a `-` item (else a scalar/mapping, not an array).
                let base = c;
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
                        break;
                    }
                    let item = l.trim_start();
                    if !first_child_seen {
                        first_child_seen = true;
                        is_list = item.starts_with('-');
                        if !is_list {
                            break;
                        }
                    }
                    if !item.starts_with('-') {
                        break;
                    }
                    let val = norm(item[1..].trim_start());
                    if !val.is_empty() {
                        entries.push(val);
                    }
                    j += 1;
                }
                if !is_list {
                    entries.clear();
                }
                i = j;
            } else {
                // Scalar `required: true`/`false` — a boolean flag, not an array.
                i += 1;
                continue;
            }
            if entries.is_empty() {
                continue;
            }
            if inside_composition(ri, c) {
                continue;
            }
            let Some(keys) = sibling_property_keys(ri, c) else {
                continue;
            };
            let kset: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
            for e in &entries {
                if !kset.contains(e.as_str()) {
                    out.push(format!("required '{}' not in properties [{}]", e, keys.join(", ")));
                }
            }
        }
        out
    }

    #[test]
    fn every_required_entry_names_a_declared_property() {
        // Contract-harness invariant (OpenAPI / JSON-Schema structural rule): every
        // entry of an object-schema `required:` array a mounted spec declares MUST name
        // a property that same object defines under `properties:`. A `required` name
        // with no matching property is an *unsatisfiable* schema — the object demands a
        // field it never declares, so no payload validates and a codegen client emits a
        // presence check on a member it can't generate.
        //
        // Cross-checks the two halves no sibling test connects: the distinct-entries
        // test pins a `required` array's names are unique, the distinct-property-names
        // test pins a `properties` block's keys are unique, but neither ever matches one
        // against the other — so a `required:` block pasted from a sibling and
        // half-edited, or a property renamed while its `required` entry was left stale,
        // is invisible to both (and to the parameter/response/enum/`$ref` tests).
        // Composed schemas are excluded (a required name may live in another `allOf`/
        // `oneOf`/`anyOf` branch), as are objects with no `properties` sibling.
        // Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = required_entries_without_a_declared_property(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares `required` entr(y/ies) naming no declared property \
                 (a schema's required names must be defined in its properties): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn required_entry_property_membership_extraction_rules() {
        // Unit-cover the `required_entries_without_a_declared_property` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a block
        // required array and a flow required array each naming a property the object
        // does not declare are flagged (with the offending name + the available keys),
        // in document order; a required name defined in a *sibling* `allOf` branch (the
        // object is a composition member) is NOT flagged; an object that both composes
        // and lists its own `required`/`properties` is NOT flagged (a property may be
        // composed in); a scalar `required: true` is never read as an array; and an
        // object with a `required` array but no `properties` sibling is skipped (the
        // list can't be resolved).
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /x:
    get:
      operationId: getX
      parameters:
        - name: q
          in: query
          required: true
      responses:
        '200':
          description: ok
components:
  schemas:
    Good:
      type: object
      required:
        - device
      properties:
        device:
          type: string
    Bad:
      type: object
      required:
        - device
        - missing
      properties:
        device:
          type: string
    FlowBad:
      type: object
      required: [a, b]
      properties:
        a:
          type: string
    Composed:
      allOf:
        - $ref: '#/components/schemas/Good'
        - type: object
          required:
            - networkId
            - id
          properties:
            id:
              type: string
    SelfComposer:
      allOf:
        - type: object
      required:
        - ghost
      properties:
        other:
          type: string
    NoProps:
      type: object
      required:
        - lonely
";
        // Flagged, in document order: `Bad`'s `missing` (block array; `device` is
        // declared, `missing` is not) and `FlowBad`'s `b` (flow array; `a` is declared,
        // `b` is not). Not flagged: `Good` (device declared), `Composed` (`networkId`
        // is inherited from the `$ref` branch — the required sits inside an `allOf`
        // subtree), `SelfComposer` (a sibling `allOf` — a property may be composed in),
        // the parameter's scalar `required: true`, and `NoProps` (no `properties`
        // sibling to resolve against).
        assert_eq!(
            required_entries_without_a_declared_property(body),
            vec![
                "required 'missing' not in properties [device]".to_string(),
                "required 'b' not in properties [a]".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec no `required` entry names an
        // undeclared property (the invariant the contract test asserts), and the corpus
        // actually declares many judged objects — a `required` array with a sibling
        // `properties` block, outside any composition — so a broken extractor can't hide
        // behind an empty scan. Count them with an independent minimal sibling-scan.
        let mut judged = 0usize;
        for api in APIS {
            assert!(
                required_entries_without_a_declared_property(api.body).is_empty(),
                "{}: every `required` entry must name a declared property",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (idx, line) in lines.iter().enumerate() {
                let t = line.trim_start();
                let Some(rest) = t.strip_prefix("required:") else { continue };
                let rest = rest.trim_start();
                let is_array = rest.starts_with('[')
                    || ((rest.is_empty() || rest.starts_with('#'))
                        && lines[idx + 1..]
                            .iter()
                            .map(|l| l.trim())
                            .find(|l| !l.is_empty() && !l.starts_with('#'))
                            .is_some_and(|l| l.starts_with('-')));
                if !is_array {
                    continue;
                }
                let c = indent(line);
                // A sibling `properties:` at exactly indent `c`, scanning down then up,
                // dedent-bounded (independent of the extractor's own scan).
                let mut has_props = false;
                let mut composed = false;
                for dir in [true, false] {
                    let mut j = idx;
                    loop {
                        if dir {
                            j += 1;
                            if j >= lines.len() {
                                break;
                            }
                        } else if j == 0 {
                            break;
                        } else {
                            j -= 1;
                        }
                        let l = lines[j];
                        if l.trim().is_empty() {
                            continue;
                        }
                        let li = indent(l);
                        if li < c {
                            break;
                        }
                        if li == c {
                            let k = l.trim_start();
                            if k.starts_with("properties:") {
                                has_props = true;
                            }
                            if k.starts_with("allOf:") || k.starts_with("oneOf:") || k.starts_with("anyOf:") {
                                composed = true;
                            }
                        }
                    }
                }
                if has_props && !composed {
                    judged += 1;
                }
            }
        }
        assert!(
            judged >= 100,
            "expected many judged `required`+`properties` objects across specs, got {judged}"
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

    /// Returns the 1-based line numbers of length/size/count bound keywords whose
    /// inline scalar value is **not a non-negative integer** — without a YAML dep.
    ///
    /// In OpenAPI 3.0.x (JSON Schema) the string-length, array-size and object-
    /// property-count bounds — `minLength`/`maxLength`, `minItems`/`maxItems`,
    /// `minProperties`/`maxProperties` — MUST each be a non-negative integer: they
    /// count characters / elements / properties, so a negative or fractional value
    /// is an invalid schema a validator rejects and a Redoc/Swagger/codegen client
    /// cannot honour. (`minimum`/`maximum` are deliberately excluded — those bound a
    /// numeric *value*, which may legitimately be negative or fractional; their
    /// ordering, not their domain, is `schema_bounds_inverted`'s concern.)
    ///
    /// For each line whose key — leading whitespace stripped, an optional `- `
    /// sequence marker tolerated — is one of the six keywords and that carries an
    /// inline scalar (a value on the same line; a bound opening a block has none and
    /// is a property literally named e.g. `minItems`, so it is skipped), the value
    /// is parsed as a number after stripping an inline `#` comment and surrounding
    /// quotes. A value `< 0`, a fractional value, or a non-numeric one is flagged
    /// (an integer spelled as a float, e.g. `3.0`, has a zero fractional part and
    /// passes, matching how validators treat the JSON-Schema integer type).
    fn size_bounds_out_of_domain(body: &str) -> Vec<usize> {
        const KEYS: [&str; 6] = [
            "minLength",
            "maxLength",
            "minItems",
            "maxItems",
            "minProperties",
            "maxProperties",
        ];
        let mut out = Vec::new();
        for (i, line) in body.lines().enumerate() {
            let trimmed = line.trim_start();
            let after_dash = trimmed.strip_prefix("- ").unwrap_or(trimmed);
            let Some((k, v)) = after_dash.split_once(':') else {
                continue;
            };
            if !KEYS.contains(&k.trim()) {
                continue;
            }
            let v = v
                .split('#')
                .next()
                .unwrap_or(v)
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            if v.is_empty() {
                continue; // opens a block / no inline value — not a scalar bound
            }
            let ok = matches!(v.parse::<f64>(), Ok(n) if n >= 0.0 && n.fract() == 0.0);
            if !ok {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_size_bound_is_a_non_negative_integer() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // the length/size/count bounds a Schema Object declares — `minLength`/
        // `maxLength`, `minItems`/`maxItems`, `minProperties`/`maxProperties` — MUST
        // each be a non-negative integer. They count characters, array elements, or
        // object properties, so a negative bound (`minItems: -1`) or a fractional one
        // (`maxLength: 2.5`) is an invalid, unsatisfiable schema: a validator rejects
        // it outright and a Redoc/Swagger/codegen client is handed a constraint it
        // cannot apply at exactly the point a caller reads or builds the payload.
        //
        // This is the domain complement of
        // `every_numeric_bound_is_ordered_low_to_high`: that test compares a lower
        // bound against its upper sibling (ordering) but never checks either against
        // its own domain, so a lone `minLength: -1` (no `maxLength` sibling to pair
        // with) or a `maxItems: 1.5` passes it untouched. A live hazard in these
        // scenario-table-heavy specs, where these caps are hand-tuned per API (an
        // array-size ceiling, an identifier length): a sign typo, or a value pasted
        // and half-edited into a fraction. It is invisible to every other test too
        // (the enum/required/array/`$ref`/type tests check a value list, required
        // entries, an element type, a ref target, or a type name, never a size
        // bound's value). `minimum`/`maximum` are excluded — those bound a numeric
        // value, which may be negative or fractional. Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let bad = size_bounds_out_of_domain(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a length/size/count bound that is not a \
                 non-negative integer (negative, fractional, or non-numeric) at \
                 line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn size_bound_domain_extraction_rules() {
        // Unit-cover the `size_bounds_out_of_domain` extractor so the contract test
        // above can't pass vacuously and its detection is pinned: a non-negative
        // integer bound (incl. `0` and a float-spelled integer `3.0`) passes; a
        // negative, fractional, or non-numeric value is flagged in document order; a
        // bound opening a block (a property literally named `minItems`, no inline
        // value) is skipped; and `minimum`/`maximum` are never inspected.
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
      minProperties: 0
      properties:
        s:
          type: string
          minLength: 1
          maxLength: 30
        list:
          type: array
          minItems: 3
          maxItems: 3.0
    BadNeg:
      type: string
      minLength: -1
    BadFrac:
      type: array
      maxItems: 2.5
    BadWord:
      type: object
      maxProperties: many
    Range:
      type: integer
      minimum: -5
      maximum: 10
    Named:
      type: object
      properties:
        minItems:
          type: integer
";
        // Flagged, in document order: `BadNeg.minLength: -1` (line 28), `BadFrac.
        // maxItems: 2.5` (line 31) and `BadWord.maxProperties: many` (line 34). Not
        // flagged: every `Good` bound incl. `minProperties: 0` and the float-spelled
        // integer `maxItems: 3.0`; `Range.minimum: -5` (a value bound, not a size
        // bound); and the property literally named `minItems:` under `Named.properties`
        // (it opens a block, carrying no inline value).
        assert_eq!(size_bounds_out_of_domain(body), vec![28, 31, 34]);

        // Non-vacuous floor: across every registered spec every size bound is a
        // non-negative integer (the invariant the contract test asserts), and the
        // corpus actually declares many such bounds — so the value-parsing path runs
        // on real data and a broken (always-empty) extractor can't hide behind a
        // corpus that never declares one. Count with a presence-only detector
        // independent of the extractor's value parsing.
        let mut bounds = 0usize;
        for api in APIS {
            assert!(
                size_bounds_out_of_domain(api.body).is_empty(),
                "{}: every size/length/count bound must be a non-negative integer",
                api.name
            );
            for line in api.body.lines() {
                if let Some((k, v)) = line.trim_start().split_once(':') {
                    if matches!(
                        k.trim(),
                        "minLength"
                            | "maxLength"
                            | "minItems"
                            | "maxItems"
                            | "minProperties"
                            | "maxProperties"
                    ) && !v.split('#').next().unwrap_or(v).trim().is_empty()
                    {
                        bounds += 1;
                    }
                }
            }
        }
        assert!(
            bounds >= 200,
            "expected many size/length/count bounds across specs, got {bounds}"
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

    /// The 1-based line numbers, in document order, of every named **Example
    /// Object** — an entry under an OpenAPI `examples:` map — that declares
    /// neither `value` nor `externalValue` (nor a `$ref`), without a YAML dep.
    ///
    /// In OpenAPI 3.0.x an `examples:` field (on a Media Type, a Parameter, a
    /// Header, or `components.examples`) is a *map* of named Example Objects, and
    /// an Example Object carries the sample payload in exactly one of `value` (an
    /// embedded literal) or `externalValue` (a URI to it) — the two are mutually
    /// exclusive and one is what makes the example an example. A named entry that
    /// declares neither (only a `summary`/`description`, or an empty block left by
    /// a half-finished paste) documents *no* sample at all: a Redoc/Swagger "try
    /// it" panel renders an empty example and a codegen client's sample generator
    /// has nothing to emit, exactly where a caller reads how to build the payload.
    ///
    /// The Example-Object analogue of `media_types_missing_schema` /
    /// `every_parameter_declares_a_schema_or_content` (each pins the one field that
    /// gives its object meaning): CamaraSim documents its scenario matrix as named
    /// examples on every response's media type (`swapped`/`notSwapped`, one per
    /// functional case), and a `value:` line lost or dedented in the paste that
    /// vendors a new spec leaves a contentless example no other contract test
    /// inspects — `no_object_declares_both_example_and_examples` checks the
    /// `example`/`examples` pair never *co-occurs*, the numeric/length example
    /// tests bound a schema-level singular `example:`, and neither ever looks
    /// inside a named example for its `value`.
    ///
    /// Pure structural scan. An `examples:` *map* is the key `examples` opening a
    /// block (no inline scalar); an `examples:` nested inside an outer
    /// `example:`/`examples:` payload is sample data (an ancestor walk skips it),
    /// mirroring the sibling example tests' `inside_example` guard. Its named
    /// entries sit at the first deeper indent under it; for each such entry that
    /// opens a block (an inline-valued entry — a flow `$ref`/object — carries its
    /// own value and is skipped), the entry's own child indent is scanned for a
    /// `value:`/`externalValue:`/`$ref:` key, bounded by the dedent that closes the
    /// entry so a `value` nested inside a *different* example's payload never
    /// satisfies it. A block-form `$ref:` entry is exempt (a Reference Object
    /// inherits the referenced Example Object's `value`).
    fn example_objects_missing_value(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The `(key, inline-value)` of a line, inline comment stripped; the value is
        // empty when the key opens a block. `None` when the line has no `key:`.
        let key_of = |l: &str| -> Option<(String, String)> {
            let (k, v) = l.trim_start().split_once(':')?;
            let v = v.split('#').next().unwrap_or(v).trim();
            Some((k.trim().to_string(), v.to_string()))
        };
        // True when line `i` (indent `c`) sits inside an outer `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` — so an `examples:` there is sample data, not the
        // OpenAPI examples field. Mirrors the sibling example tests' guard.
        let inside_example_payload = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = key_of(l) {
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            // An `examples:` map: the key opens a block (no inline scalar) and is not
            // itself sample data inside an example payload.
            let Some((k, v)) = key_of(line) else {
                continue;
            };
            if k != "examples" || !v.is_empty() {
                continue;
            }
            let c = indent(line);
            if inside_example_payload(i, c) {
                continue;
            }
            // The named entries sit at the first deeper indent under the map.
            let mut child_indent = None;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break; // an empty examples map — no entries to inspect
                }
                child_indent = Some(indent(l));
                break;
            }
            let Some(ci) = child_indent else { continue };
            // Walk each named-example key at exactly `ci` within the map's block.
            let mut n = i + 1;
            while n < lines.len() {
                let l = lines[n];
                if l.trim().is_empty() {
                    n += 1;
                    continue;
                }
                let li = indent(l);
                if li <= c {
                    break; // dedented out of the examples map
                }
                if li == ci {
                    // An inline-valued entry (a flow `$ref`/object) carries its own
                    // value; only a block-opening entry is scanned for a `value` child.
                    let inline_valued =
                        key_of(l).is_some_and(|(_, ev)| !ev.is_empty());
                    if !inline_valued {
                        let mut obj_child = None;
                        let mut has_value = false;
                        let mut m = n + 1;
                        while m < lines.len() {
                            let ll = lines[m];
                            if ll.trim().is_empty() {
                                m += 1;
                                continue;
                            }
                            let mi = indent(ll);
                            if mi <= ci {
                                break; // out of this Example Object
                            }
                            if obj_child.is_none() {
                                obj_child = Some(mi);
                            }
                            if Some(mi) == obj_child {
                                if let Some((kk, _)) = key_of(ll) {
                                    if kk == "value"
                                        || kk == "externalValue"
                                        || kk == "$ref"
                                    {
                                        has_value = true;
                                        break;
                                    }
                                }
                            }
                            m += 1;
                        }
                        if !has_value {
                            out.push(n + 1);
                        }
                    }
                }
                n += 1;
            }
        }
        out
    }

    #[test]
    fn every_example_object_declares_a_value() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every named
        // Example Object a mounted spec declares under an `examples:` map MUST carry
        // one of `value`/`externalValue` — the field that supplies the sample the
        // example exists to show. An Example Object with neither (only a
        // `summary`/`description`, or an entry emptied by a half-finished edit)
        // documents no payload at all, so a Redoc/Swagger "try it" prefill renders an
        // empty example and a codegen client's sample generator has nothing to emit —
        // right where a caller reads how to build the request/response.
        //
        // CamaraSim expresses its per-scenario functional cases as named examples on
        // every response media type (one `value:` per case), so a `value:` dropped or
        // dedented in the paste that vendors a new spec is a routine hazard. It is
        // invisible to every existing test: `no_object_declares_both_example_and_examples`
        // checks the `example`/`examples` pair never co-occurs, the numeric/length
        // `example` tests bound a schema-level singular `example:`, and the
        // media-type/parameter tests pin a payload's `schema`/`content` — none looks
        // inside a named Example Object for its `value`. Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let contentless = example_objects_missing_value(api.body);
            assert!(
                contentless.is_empty(),
                "{} spec declares a named Example Object under an `examples:` map with \
                 neither a `value` nor an `externalValue` (a documented sample that \
                 shows nothing) at entry line(s): {:?}",
                api.name,
                contentless
            );
        }
    }

    #[test]
    fn example_object_value_extraction_rules() {
        // Unit-cover `example_objects_missing_value` so the contract test above can't
        // pass vacuously and its detection is pinned: a named example carrying a
        // `value` passes; one with only `summary`/`description` is flagged; an
        // `externalValue` entry and a block-`$ref` entry pass (both supply/inherit a
        // value); a `value` nested inside one example's payload never satisfies a
        // *different* value-less sibling (dedent-bounded child scan); a `components.
        // examples` reusable Example Object is scanned the same way; a second media
        // type's examples map is inspected independently; and an `examples:` nested
        // inside an outer `example:` payload (sample data) is skipped entirely.
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
          content:
            application/json:
              examples:
                good:
                  summary: has a value
                  value:
                    ok: true
                bad:
                  summary: no value here
                  description: still no value
                ext:
                  externalValue: https://example.com/e.json
                reffed:
                  $ref: \"#/components/examples/Shared\"
            application/xml:
              examples:
                alsoBad:
                  summary: xml example without a value
components:
  examples:
    Shared:
      value:
        ok: false
  schemas:
    S:
      type: object
      example:
        examples:
          fakeExample:
            summary: sample data, not a real Example Object
";
        // Flagged, in document order: line 19 (`bad`, only `summary`/`description`)
        // and line 28 (`alsoBad`, only `summary`). Not flagged: `good` (line 15, its
        // `value:` child at line 17); `ext` (line 22, an `externalValue` child); `reffed`
        // (line 24, a block-`$ref` — a Reference Object inherits its value); `Shared`
        // (line 32 under `components.examples`, its `value:` child at line 33); and
        // `fakeExample` (line 40) whose `examples:` opener (line 39) sits inside the
        // schema's `example:` payload (line 38) and is skipped as sample data.
        assert_eq!(example_objects_missing_value(body), vec![19, 28]);

        // Non-vacuous floor: across every registered spec every named Example Object
        // declares its `value`/`externalValue` (the invariant the contract test
        // asserts), and the corpus actually declares many named examples (a `value:`
        // per functional case on every response) — so the value-lookup path runs on
        // real data and a broken (always-empty) extractor can't hide behind a corpus
        // with no named examples. Count entries with a detector independent of the
        // extractor's value lookup.
        let mut named_examples = 0usize;
        for api in APIS {
            assert!(
                example_objects_missing_value(api.body).is_empty(),
                "{}: every named Example Object must declare a value or externalValue",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let is_examples_map = |l: &str| {
                l.trim_start()
                    .split_once(':')
                    .is_some_and(|(k, v)| {
                        k.trim() == "examples"
                            && v.split('#').next().unwrap_or(v).trim().is_empty()
                    })
            };
            for (i, line) in lines.iter().enumerate() {
                if !is_examples_map(line) {
                    continue;
                }
                let c = indent(line);
                // Count the direct child keys at the map's first deeper indent.
                let mut ci = None;
                let mut j = i + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() {
                        j += 1;
                        continue;
                    }
                    let li = indent(l);
                    if li <= c {
                        break;
                    }
                    let entry_indent = *ci.get_or_insert(li);
                    if li == entry_indent {
                        named_examples += 1;
                    }
                    j += 1;
                }
            }
        }
        assert!(
            named_examples >= 100,
            "expected many named Example Objects across specs, got {named_examples}"
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

    /// Extract the 1-based line number of every Discriminator Object whose
    /// `propertyName` is **not listed in the enclosing schema's `required` array**
    /// — without a YAML dep.
    ///
    /// OpenAPI 3.0.3 §4.8.25.1: for a discriminator to be usable the property it
    /// names in `propertyName` MUST be a required member of the schema — a payload
    /// can only be routed to a concrete variant when the selecting property is
    /// guaranteed present. A discriminator whose `propertyName` is *optional*
    /// (absent from `required`, or the schema declaring no `required` array at all)
    /// is a broken polymorphic schema: a Redoc/Swagger/codegen client can be handed
    /// a payload with no discriminating value and cannot pick a variant to
    /// deserialize or generate, so the polymorphism fails exactly where a caller
    /// reads or builds the body.
    ///
    /// This is the *required-membership* complement of
    /// `discriminators_missing_property_name` (which only checks the `propertyName`
    /// field is present); no existing test links the named property to the schema's
    /// `required` list — the required-array tests
    /// (`every_required_array_lists_distinct_entries`,
    /// `every_required_entry_names_a_declared_property`,
    /// `every_required_array_sits_on_an_object_type`) inspect a `required` array's
    /// entries, membership and sibling type, never a discriminator, and the
    /// discriminator test checks only field presence.
    ///
    /// For each block-form `discriminator:` at indent `c`: its `propertyName`
    /// scalar is read from the discriminator's own block (a child indented past
    /// `c`, bounded by the dedent to `c` that closes the object). A discriminator
    /// with no `propertyName` is skipped (that omission is
    /// `discriminators_missing_property_name`'s concern). The enclosing schema
    /// object's `required:` array is then located among the `discriminator:`
    /// siblings — a same-indent (`c`) key found by scanning the object's block down
    /// then up, dedent-bounded exactly like `schema_bounds_inverted`, so a nested or
    /// neighbouring object's `required` never pairs — and its entries collected (an
    /// inline flow `[a, b]` or the block `- ` items indented past the key). The
    /// discriminator is flagged when the `propertyName` value is absent from those
    /// entries, including when the schema declares no sibling `required` at all.
    fn discriminators_with_optional_property_name(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a `name:` key (inline comment + surrounding quotes
        // stripped); `None` for a different key or a block opener (empty value).
        let scalar = |l: &str, name: &str| -> Option<String> {
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
                None
            } else {
                Some(v.to_string())
            }
        };
        // The `required` array entries of the schema object owning the
        // `discriminator:` at line `i` (indent `c`): find a same-indent `required:`
        // sibling (down through the object's block, then up, dedent-bounded), then
        // read its entries — an inline flow `[a, b, …]` or the block `- ` items
        // indented past it. `None` when the object declares no `required:` sibling.
        let required_entries = |i: usize, c: usize| -> Option<HashSet<String>> {
            let key_is = |l: &str, name: &str| -> bool {
                l.trim_start()
                    .split_once(':')
                    .is_some_and(|(k, _)| k.trim() == name)
            };
            let read_entries = |ri: usize, rline: &str| -> HashSet<String> {
                let mut set = HashSet::new();
                let after = rline
                    .trim_start()
                    .split_once(':')
                    .map(|(_, v)| v)
                    .unwrap_or("");
                let after = after.split('#').next().unwrap_or(after).trim();
                if let Some(inner) = after.strip_prefix('[') {
                    // inline flow sequence: `required: [a, b, …]`
                    for tok in inner.trim_end_matches(']').split(',') {
                        let t = tok.trim().trim_matches('"').trim_matches('\'');
                        if !t.is_empty() {
                            set.insert(t.to_string());
                        }
                    }
                    return set;
                }
                // block sequence: `- name` items indented past the `required:` key
                let rc = indent(rline);
                let mut j = ri + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() {
                        j += 1;
                        continue;
                    }
                    if indent(l) <= rc {
                        break;
                    }
                    if let Some(item) = l.trim_start().strip_prefix('-') {
                        let t = item
                            .split('#')
                            .next()
                            .unwrap_or(item)
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'');
                        if !t.is_empty() {
                            set.insert(t.to_string());
                        }
                    }
                    j += 1;
                }
                set
            };
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c && key_is(l, "required") {
                    return Some(read_entries(j, l));
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c && key_is(l, "required") {
                    return Some(read_entries(k, l));
                }
            }
            None
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            // A Discriminator Object: block-form `discriminator:` (no inline value).
            let Some((k, v)) = line.trim_start().split_once(':') else {
                continue;
            };
            if k.trim() != "discriminator"
                || !v.split('#').next().unwrap_or(v).trim().is_empty()
            {
                continue;
            }
            let c = indent(line);
            // Read the discriminator's `propertyName` child (within its block).
            let mut prop: Option<String> = None;
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
                if let Some(p) = scalar(l, "propertyName") {
                    prop = Some(p);
                    break;
                }
                j += 1;
            }
            let Some(prop) = prop else {
                continue; // missing propertyName — the sibling test's concern
            };
            let listed = required_entries(i, c).is_some_and(|set| set.contains(&prop));
            if !listed {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_discriminator_property_name_is_required() {
        // Contract-harness invariant (OpenAPI 3.0.3 §4.8.25.1): the property a
        // Discriminator Object names in `propertyName` MUST be a *required* member
        // of the enclosing schema — the payload can only be routed to a concrete
        // variant when the selecting property is guaranteed present. A discriminator
        // whose `propertyName` is optional (absent from the schema's `required`
        // array, or the schema declaring no `required` at all) is a broken
        // polymorphic schema: a Redoc/Swagger/codegen client can be handed a payload
        // with no discriminating value and cannot pick a variant to deserialize or
        // generate — the polymorphism fails at exactly the point a caller reads or
        // builds the body.
        //
        // The *required-membership* complement of
        // `every_discriminator_declares_a_property_name` (which checks only that the
        // `propertyName` field is present): CamaraSim's polymorphic families
        // (`Area`/`Device`/`AccessDetail`) all key off a required discriminator
        // property, and a discriminator block pasted from a sibling can keep
        // `propertyName` while its `required:` sibling drifts (a renamed property, a
        // dropped `required` line). No existing test links the named property to the
        // schema's `required` list — the required-array tests inspect a `required`
        // array's entries, membership and sibling type, never a discriminator.
        // Verified true across all mounted specs before asserting.
        for api in APIS {
            let offenders = discriminators_with_optional_property_name(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec declares a `discriminator` whose `propertyName` is not \
                 listed in the enclosing schema's `required` array (an optional \
                 discriminator property a client can't switch on) at \
                 `discriminator:` line(s): {:?}",
                api.name,
                offenders
            );
        }
    }

    #[test]
    fn discriminator_property_name_required_extraction_rules() {
        // Unit-cover `discriminators_with_optional_property_name` so the contract
        // test above can't pass vacuously and its detection is pinned: a
        // discriminator whose `propertyName` is listed in a same-object `required`
        // block passes; one listed in an inline-flow `required: [ … ]` passes; a
        // `required` declared *below* the discriminator (down-scan) still pairs; a
        // `propertyName` absent from the `required` entries is flagged; a schema
        // with no `required` sibling at all is flagged; and a discriminator missing
        // `propertyName` entirely is skipped (that omission is the sibling
        // `discriminators_missing_property_name`'s concern, not this test's).
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
      type: object
      required:
        - kind
      discriminator:
        propertyName: kind
      properties:
        kind:
          type: string
    GoodFlow:
      type: object
      required: [type]
      discriminator:
        propertyName: type
      properties:
        type:
          type: string
    GoodRequiredBelow:
      type: object
      discriminator:
        propertyName: areaType
        mapping:
          CIRCLE: '#/components/schemas/Circle'
      required:
        - areaType
      properties:
        areaType:
          type: string
    BadNotListed:
      type: object
      required:
        - other
      discriminator:
        propertyName: kind
      properties:
        kind:
          type: string
        other:
          type: string
    BadNoRequired:
      type: object
      discriminator:
        propertyName: kind
      properties:
        kind:
          type: string
    MissingPropName:
      type: object
      discriminator:
        mapping:
          CIRCLE: '#/components/schemas/Circle'
      required:
        - areaType
      properties:
        areaType:
          type: string
";
        // Flagged, in document order: `BadNotListed`'s discriminator at line 46 (its
        // `required` lists only `other`, not the named `kind`) and `BadNoRequired`'s
        // at line 55 (the schema declares no `required` sibling). Not flagged:
        // `GoodBlock` (line 18, `kind` in the block `required`), `GoodFlow` (line 26,
        // `type` in the inline-flow `required: [type]`), `GoodRequiredBelow` (line 33,
        // `areaType` in a `required` declared *below* the discriminator — down-scan),
        // and `MissingPropName` (line 62, no `propertyName` — the sibling presence
        // test's concern).
        assert_eq!(
            discriminators_with_optional_property_name(body),
            vec![46, 55]
        );

        // Non-vacuous floor: across every registered spec every discriminator's
        // `propertyName` is listed in its schema's `required` array (the invariant
        // the contract test asserts), and the corpus actually declares several such
        // discriminators (the `Area`/`Device`/`AccessDetail` families) — so the
        // membership-comparison path runs on real data and a broken (always-empty)
        // extractor can't hide behind a corpus that never pairs a discriminator with
        // a required property. Count required-property discriminators with a detector
        // independent of the extractor's sibling-scan: a block-form `discriminator:`
        // whose `propertyName` value appears verbatim as some `- <value>` list item
        // anywhere in the same spec.
        let mut required_discriminators = 0usize;
        for api in APIS {
            assert!(
                discriminators_with_optional_property_name(api.body).is_empty(),
                "{}: every discriminator propertyName must be a required member",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                if l.trim() != "discriminator:" {
                    continue;
                }
                let c = indent(l);
                let mut prop: Option<String> = None;
                let mut j = i + 1;
                while j < lines.len() {
                    let x = lines[j];
                    if x.trim().is_empty() {
                        j += 1;
                        continue;
                    }
                    if indent(x) <= c {
                        break;
                    }
                    if let Some((k, v)) = x.trim_start().split_once(':') {
                        if k.trim() == "propertyName" {
                            let v = v
                                .split('#')
                                .next()
                                .unwrap_or(v)
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\'');
                            if !v.is_empty() {
                                prop = Some(v.to_string());
                            }
                            break;
                        }
                    }
                    j += 1;
                }
                let Some(prop) = prop else {
                    continue;
                };
                let listed = lines.iter().any(|x| {
                    x.trim_start().strip_prefix('-').is_some_and(|r| {
                        r.split('#')
                            .next()
                            .unwrap_or(r)
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            == prop
                    })
                });
                if listed {
                    required_discriminators += 1;
                }
            }
        }
        assert!(
            required_discriminators >= 5,
            "expected several required-property discriminators across specs, got {required_discriminators}"
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

    /// Extract the 1-based line number of every `oneOf`/`anyOf`/`allOf` a spec
    /// declares whose value is an **empty** flow sequence (`[]`) — without a YAML
    /// dep.
    ///
    /// In OpenAPI 3.0.x the schema-composition keywords `oneOf`, `anyOf` and
    /// `allOf` are arrays of Schema Objects, and the JSON-Schema dialect they use
    /// (Wright Draft 00 / draft-04) requires each such array to have **at least one
    /// element**. An empty composer is not just pointless but broken: an empty
    /// `oneOf`/`anyOf` is *unsatisfiable* — no instance can match "exactly/at least
    /// one of nothing", so the schema validates nothing — and an empty `allOf`
    /// composes no constraint at all, so a Redoc/Swagger/codegen client renders an
    /// empty or contradictory model exactly where a caller reads or builds the
    /// payload. CamaraSim leans on `allOf` to extend the shared `CamaraError` with
    /// each API's `code` enum and for the `Area`/`Device` polymorphic families, so
    /// an accidentally-emptied composer (a `- ` block collapsed to `[]` in an edit)
    /// silently drops that composition.
    ///
    /// The non-emptiness complement of `composers_not_a_sequence`
    /// (`every_composer_keyword_declares_a_sequence`), which checks a composer *is*
    /// a sequence but accepts any `[`-opening value — so a well-formed but empty
    /// `allOf: []` sails through it — and no other test looks at a composer's
    /// element *count*. It mirrors `every_enum_lists_unique_non_empty_values`'s
    /// non-emptiness guard for the value-list keyword. An empty composer is only
    /// expressible as an inline flow sequence (a block-form composer with no `- `
    /// items has no value at all, which the sequence-ness test already flags), so
    /// only an inline `oneOf`/`anyOf`/`allOf` whose value opens with `[` and whose
    /// bracket closes on the same line with nothing but whitespace between is
    /// flagged; a non-empty flow (`[ {…} ]`), a block form, a non-`[` scalar (the
    /// sequence-ness test's concern), a property literally *named* for the keyword
    /// (it opens a block, empty inline value), and a keyword inside an
    /// `example:`/`examples:` payload (ancestor-chain walk) are all skipped. A
    /// multi-line flow whose `[` does not close on the keyword's line is treated as
    /// non-empty (its items sit on following lines).
    fn empty_composer_sequences(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` (mirroring `composers`/`items` siblings).
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some((k, v)) = line.trim_start().split_once(':') else {
                continue;
            };
            let k = k.trim();
            if k != "oneOf" && k != "anyOf" && k != "allOf" {
                continue;
            }
            // Strip a trailing `# comment`; what remains, trimmed, is the inline
            // value (empty ⇒ a block form / property-named-for-the-keyword — the
            // sequence-ness test's concern, never an inline empty array).
            let inline = v.split('#').next().unwrap_or(v).trim();
            let Some(rest) = inline.strip_prefix('[') else {
                continue; // block form or a non-`[` scalar — not an inline sequence
            };
            // Empty iff the bracket closes on this line with only whitespace inside.
            let Some(close) = rest.find(']') else {
                continue; // multi-line flow: items are on following lines
            };
            if rest[..close].trim().is_empty() && !inside_example(i, indent(line)) {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_composer_keyword_lists_at_least_one_subschema() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema draft-04
        // structural rule): every `oneOf`/`anyOf`/`allOf` a mounted spec declares
        // MUST list at least one subschema. The dialect OpenAPI 3.0.x uses requires
        // a composition array to be non-empty, and an empty one is broken beyond
        // pointlessness: an empty `oneOf`/`anyOf` is *unsatisfiable* (nothing can
        // match one-of/any-of an empty set), and an empty `allOf` composes no
        // constraint — so a validator and a Redoc/Swagger/codegen client render a
        // contradictory or empty model exactly where a caller reads or builds the
        // payload.
        //
        // The non-emptiness complement of `every_composer_keyword_declares_a_sequence`,
        // which checks a composer *is* a sequence but — matching how it accepts an
        // inline `[ … ]` by its opening bracket alone — lets a well-formed but empty
        // `allOf: []` through; no other test inspects a composer's element count. A
        // live hazard in these `allOf`-heavy specs (the shared `CamaraError`
        // `code`-enum extension, the `Area`/`Device` families), where a `- ` block
        // can collapse to `[]` in an edit and silently drop the composition. Mirrors
        // `every_enum_lists_unique_non_empty_values`'s non-emptiness guard for the
        // sibling value-list keyword. Verified true across all mounted specs before
        // asserting.
        for api in APIS {
            let empties = empty_composer_sequences(api.body);
            assert!(
                empties.is_empty(),
                "{} spec declares an empty `oneOf`/`anyOf`/`allOf` (`[]`) — a \
                 composition with no subschemas, unsatisfiable for oneOf/anyOf — at \
                 line(s): {:?}",
                api.name,
                empties
            );
        }
    }

    #[test]
    fn composer_non_empty_extraction_rules() {
        // Unit-cover `empty_composer_sequences` so the contract test above can't
        // pass vacuously and its detection is pinned: a non-empty inline flow
        // (`oneOf: [ {…}, {…} ]`) and a block-form composer both pass; an empty
        // inline flow — `oneOf: []`, `anyOf: [ ]` (whitespace inside), and
        // `allOf: []  # trailing comment` — is flagged in document order; a property
        // literally *named* `oneOf` (it opens a block — empty inline value, not a
        // `[`) is skipped; and an empty `allOf: []` sitting inside an `example:`
        // payload is skipped (example data, not a schema keyword).
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
    GoodInline:
      oneOf: [ { type: string }, { type: integer } ]
    GoodBlock:
      allOf:
        - $ref: '#/components/schemas/Base'
        - type: object
    BadEmptyOneOf:
      oneOf: []
    BadEmptyAnyOf:
      anyOf: [ ]
    BadEmptyAllOf:
      allOf: []  # nothing here
    NamedOneOf:
      type: object
      properties:
        oneOf:
          type: string
    InExample:
      type: object
      example:
        allOf: []
";
        // Flagged, in document order: `BadEmptyOneOf.oneOf` (line 21, `[]`),
        // `BadEmptyAnyOf.anyOf` (line 23, `[ ]` — whitespace inside), and
        // `BadEmptyAllOf.allOf` (line 25, `[]` before a trailing comment). Not
        // flagged: `GoodInline` (a non-empty flow), `GoodBlock` (a block form with
        // `- ` items), `NamedOneOf.properties.oneOf` (a property named `oneOf`
        // opening a block — empty inline value, not a `[`), and `InExample`'s
        // `allOf: []` (inside the outer `example:` payload).
        assert_eq!(empty_composer_sequences(body), vec![21, 23, 25]);

        // Non-vacuous floor: across every registered spec every composer lists at
        // least one subschema (the invariant), and the corpus actually declares many
        // composers (`allOf` over `CamaraError` + the polymorphic families) — so the
        // scan runs on real data and a broken (always-empty) extractor can't hide
        // behind a corpus that never declares a composer. Count composer keywords
        // with a detection independent of the extractor.
        let mut composers = 0usize;
        for api in APIS {
            assert!(
                empty_composer_sequences(api.body).is_empty(),
                "{}: every `oneOf`/`anyOf`/`allOf` must list at least one subschema",
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

    /// Extract the 1-based line number of every `items:` a spec declares whose
    /// value is a **sequence** — the invalid OpenAPI 3.0.x tuple form — without a
    /// YAML dep.
    ///
    /// In OpenAPI 3.0.x a Schema Object's `items` MUST be a **single** Schema
    /// Object describing every element of the array; unlike JSON Schema / OAS 3.1
    /// it does NOT admit the positional-tuple `items: [ … ]` form. So an `items:`
    /// whose value is a sequence — an inline flow `items: [ … ]`, or a block whose
    /// first non-blank child is a `- ` item — is an invalid document: a
    /// Redoc/Swagger/codegen client expecting one element schema is handed a list
    /// it can't apply, so the array's element type silently breaks where a caller
    /// reads or builds the payload.
    ///
    /// The exact structural mirror of `composers_not_a_sequence` (`oneOf`/`anyOf`/
    /// `allOf` MUST be sequences; `items` MUST NOT be one) — the same inline-`[` /
    /// first-child-`-` detection, inverted. Only an `items:` at the start of its
    /// line is inspected, so a property literally *named* `items` (its value is a
    /// Schema Object mapping, never a `- ` sequence) is never flagged; and an
    /// `items:` appearing as data inside an `example:`/`examples:` payload (a JSON
    /// field named `items` holding an array) is excluded by walking the ancestor
    /// chain, mirroring `type_values_not_a_valid_type`.
    fn items_declared_as_a_sequence(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some((k, v)) = line.trim_start().split_once(':') else {
                continue;
            };
            if k.trim() != "items" {
                continue;
            }
            // Strip a trailing `# comment` from the value; what remains, trimmed,
            // is the inline value (empty ⇒ the key opens a block).
            let inline = v.split('#').next().unwrap_or(v).trim();
            let is_sequence = if !inline.is_empty() {
                // Inline value: only a flow sequence `[ … ]` is a tuple; a mapping
                // `{ … }` or a `$ref` scalar is a single schema.
                inline.starts_with('[')
            } else {
                // Block form: the first non-blank following line decides it — a
                // `- ` sequence item at the key's own indent or deeper is a tuple;
                // a mapping child (a single schema), or a dedent (an empty value,
                // a different test's concern), is not.
                let c = indent(line);
                let mut seq = false;
                let mut j = i + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() {
                        j += 1;
                        continue;
                    }
                    if indent(l) < c {
                        break;
                    }
                    seq = l.trim_start().starts_with('-');
                    break;
                }
                seq
            };
            if is_sequence && !inside_example(i, indent(line)) {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_items_declares_a_single_schema() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every Schema
        // Object `items:` a mounted spec declares MUST be a *single* Schema Object
        // describing every element of the array — not a sequence. (OAS 3.0.x, unlike
        // JSON Schema / OAS 3.1, does not admit the positional-tuple `items: [ … ]`
        // form.) An `items:` whose value is a sequence — an inline flow
        // `items: [ … ]` or a block whose first child is a `- ` item — is an invalid
        // document: a Redoc/Swagger/codegen client expecting one element schema is
        // handed a list it can't apply, so the array's element type silently breaks
        // where a caller reads or builds the payload.
        //
        // The exact structural mirror of `every_composer_keyword_declares_a_sequence`
        // (`oneOf`/`anyOf`/`allOf` MUST be sequences; `items` MUST NOT be one) and a
        // live hazard in these vendored specs: a spec drafted or migrated with a 3.1
        // idiom, or an `items` block pasted from a `oneOf`/`anyOf` sibling with its
        // `- ` markers left in place, yields a tuple `items` no other test sees —
        // `every_array_schema_declares_items` proves an array *has* an `items`, never
        // that the `items` it has is a single schema, and the composer test inspects
        // only the three composer keywords. Verified true across all mounted specs
        // before asserting.
        for api in APIS {
            let bad = items_declared_as_a_sequence(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares an `items:` whose value is a sequence (the invalid \
                 OpenAPI 3.0.x tuple form; `items` must be a single schema) at \
                 line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn items_single_schema_extraction_rules() {
        // Unit-cover the `items_declared_as_a_sequence` extractor so the contract
        // test above can't pass vacuously and its detection is pinned: an `items:`
        // opening a mapping (a `type:` child) and an inline `{ … }`/`$ref` scalar
        // are single schemas and pass; an `items:` opening a `- ` sequence (block
        // form) and an inline flow `[ … ]` are the invalid tuple form and are
        // flagged, in document order; a property literally *named* `items` (its
        // value a Schema Object) and an `items:` inside an `example:` payload (a JSON
        // array field) are never flagged.
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
      type: array
      items:
        type: string
    GoodInlineRef:
      type: array
      items: { $ref: '#/components/schemas/GoodBlock' }
    GoodNamedProperty:
      type: object
      properties:
        items:
          type: array
          items:
            type: integer
    WithExample:
      type: object
      example:
        items:
          - a
          - b
    BadBlockTuple:
      type: array
      items:
        - type: string
        - type: integer
    BadInlineTuple:
      type: array
      items: [ { type: string }, { type: integer } ]
";
        // Flagged, in document order: `BadBlockTuple`'s `items` at line 36 (its first
        // child is a `- ` sequence item — the 3.1 tuple form) and `BadInlineTuple`'s
        // `items` at line 41 (an inline flow `[ … ]` sequence). Not flagged:
        // `GoodBlock` (a mapping child), `GoodInlineRef` (an inline `{ … }` mapping),
        // both `items:` under `GoodNamedProperty` (the property name and its real
        // array item both open single-schema mappings), and the `items:` inside
        // `WithExample`'s `example:` payload (example data, a `- ` list).
        assert_eq!(items_declared_as_a_sequence(body), vec![36, 41]);

        // Non-vacuous floor: across every registered spec every `items` is a single
        // schema (the invariant the contract asserts), and the corpus declares many
        // block-form `items` keys (every array-typed schema), so a broken extractor
        // can't hide behind an empty scan. Count block-form `items:` keys with a
        // detection independent of the extractor.
        let mut items = 0usize;
        for api in APIS {
            assert!(
                items_declared_as_a_sequence(api.body).is_empty(),
                "{}: every `items` must be a single schema",
                api.name
            );
            for line in api.body.lines() {
                if line.trim() == "items:" {
                    items += 1;
                }
            }
        }
        assert!(
            items >= 50,
            "expected many `items` keywords across specs, got {items}"
        );
    }

    /// Extract the 1-based line number of every Schema Object `type:` a spec
    /// declares whose scalar value names **no valid OpenAPI 3.0.x type** — without
    /// a YAML dep.
    ///
    /// In OpenAPI 3.0.x a Schema Object's `type` MUST be one of the six JSON Schema
    /// primitive types — `string`, `number`, `integer`, `boolean`, `array`,
    /// `object`. (3.0.x, unlike 3.1, does not admit `null` as a type; nullability
    /// is `nullable: true`.) A value outside that set — a typo (`sting`,
    /// `interger`, `bool`) or a stray token — is an invalid schema a
    /// Redoc/Swagger/codegen client can neither validate against nor generate for,
    /// so it breaks silently at the point a caller reads or builds the payload.
    ///
    /// `type:` is not unique to Schema Objects: a Security Scheme Object keys it
    /// too (`oauth2`/`http`/`apiKey`/`openIdConnect`/`mutualTLS` — CamaraSim's auth
    /// spec defines an inline `openIdConnect` scheme), so those five tokens are
    /// accepted as well; a typo still lands in neither set and is caught. Two
    /// further contexts are excluded: a `type:` with an *empty* value is a property
    /// literally named `type` (its value is a schema, e.g. a CloudEvent's `type`
    /// field), not a type declaration; and a `type:` appearing as data inside an
    /// `example:`/`examples:` payload (CamaraSim's CloudEvent examples carry a
    /// `type: "org.camaraproject…"` URN) is example data, detected by walking the
    /// ancestor chain for an enclosing `example:`/`examples:` key. Only a `type:`
    /// at the start of its line (after indentation) is inspected, so a `type` inside
    /// a flow mapping (`{ type: string }`) is left to the composer/array tests.
    fn type_values_not_a_valid_type(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The scalar value of a `type:` key, inline comment and quotes stripped;
        // `None` when the line is not a `type:` key.
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
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The six JSON Schema types plus the five Security Scheme `type` tokens —
        // both are legitimate `type:` values; a typo falls in neither.
        const VALID: [&str; 11] = [
            "string",
            "number",
            "integer",
            "boolean",
            "array",
            "object",
            "oauth2",
            "http",
            "apiKey",
            "openIdConnect",
            "mutualTLS",
        ];
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(v) = type_value(line) else {
                continue;
            };
            if v.is_empty() {
                continue; // a property named `type`, or a block opener
            }
            if VALID.contains(&v) {
                continue;
            }
            if inside_example(i, indent(line)) {
                continue; // example/CloudEvent data, not a type keyword
            }
            out.push(i + 1);
        }
        out
    }

    #[test]
    fn every_type_names_a_valid_schema_type() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every Schema
        // Object `type:` a mounted spec declares MUST name one of the six JSON
        // Schema primitive types — `string`/`number`/`integer`/`boolean`/`array`/
        // `object`. (3.0.x, unlike 3.1, does not admit `null` as a type; nullability
        // is `nullable: true`.) A value outside that set — a typo (`sting`,
        // `interger`, `bool`) or a stray token — is an invalid schema a
        // Redoc/Swagger/codegen client can neither validate against nor generate
        // for, breaking silently at the point a caller reads or builds the payload.
        //
        // `type:` is not unique to Schema Objects: a Security Scheme Object keys it
        // too (`oauth2`/`http`/`apiKey`/`openIdConnect`/`mutualTLS` — CamaraSim's
        // auth spec defines an inline `openIdConnect` scheme), so those five tokens
        // are accepted as well; a typo still lands in neither set and is caught. A
        // property literally *named* `type` (its value a schema) and a `type:`
        // appearing as data inside an `example:`/`examples:` payload (a CloudEvent
        // `type` URN) are both excluded — neither is a type keyword. It is invisible
        // to every existing test, which check an `items` schema, a composer's
        // sequence-ness, a value list, a discriminator's completeness, or a ref
        // target, never that a `type` names a real type. Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let bad = type_values_not_a_valid_type(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares `type:` value(s) naming no valid OpenAPI type \
                 (one of string/number/integer/boolean/array/object) at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn schema_type_value_extraction_rules() {
        // Unit-cover the `type_values_not_a_valid_type` extractor so the contract
        // test above can't pass vacuously and its detection is pinned: valid schema
        // types (`object`/`string`/`integer`) and a Security Scheme `type`
        // (`openIdConnect`) pass; a property literally named `type` (empty value)
        // and a `type:` inside an `example:` payload are skipped; a typo in a schema
        // — at the top level *and* nested under `properties:` — is flagged in
        // document order.
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
  securitySchemes:
    oidc:
      type: openIdConnect
      openIdConnectUrl: https://example/x
  schemas:
    Good:
      type: object
      properties:
        name:
          type: string
        count:
          type: integer
        type:
          type: string
    Event:
      type: object
      example:
        type: \"org.camaraproject.x.v1.thing\"
        id: abc
    BadTypo:
      type: sting
    BadInSchema:
      type: object
      properties:
        x:
          type: nonsense
";
        // Flagged, in document order: `BadTypo`'s `type: sting` at line 33 and
        // `BadInSchema.x`'s `type: nonsense` at line 38. Not flagged: the inline
        // security `openIdConnect` (line 15), every valid schema type, the property
        // literally named `type` (line 25, empty value), and the CloudEvent
        // `type: "org.camaraproject…"` inside the `example:` payload (line 30).
        assert_eq!(type_values_not_a_valid_type(body), vec![33, 38]);

        // Non-vacuous floor: across every registered spec every `type:` names a
        // valid token (the invariant the contract test asserts), and the corpus
        // declares many schema `type:` keys, so a broken extractor can't hide behind
        // an empty scan. Count `type:` lines naming a valid schema type with a
        // detection independent of the extractor.
        let mut typed = 0usize;
        for api in APIS {
            assert!(
                type_values_not_a_valid_type(api.body).is_empty(),
                "{}: every `type:` must name a valid OpenAPI type",
                api.name
            );
            for line in api.body.lines() {
                if let Some(v) = line.trim_start().strip_prefix("type:") {
                    let v = v
                        .split('#')
                        .next()
                        .unwrap_or(v)
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if matches!(
                        v,
                        "string" | "number" | "integer" | "boolean" | "array" | "object"
                    ) {
                        typed += 1;
                    }
                }
            }
        }
        assert!(
            typed >= 500,
            "expected many schema `type:` keys across specs, got {typed}"
        );
    }

    /// The scalar value of every `format:` key a spec declares that names no
    /// recognized OpenAPI 3.0.x / JSON-Schema-Validation format, returned as
    /// 1-based line numbers in document order.
    ///
    /// In OpenAPI 3.0.x a Schema Object's `format` is a free-text *modifier* on
    /// its `type` drawn, in practice, from a well-known vocabulary — the OAS Data
    /// Type formats (`int32`/`int64`/`float`/`double`/`byte`/`binary`/`date`/
    /// `date-time`/`password`) plus the JSON-Schema-Validation string formats
    /// (`email`/`hostname`/`ipv4`/`ipv6`/`uri`/`uri-reference`/`uuid`/`regex`/…).
    /// Tooling keys real behaviour off these strings (Redoc renders a format hint,
    /// codegen picks a concrete type, a validator applies the matching check), so a
    /// typo — `datetime` for `date-time`, `int_32` for `int32`, `uid` for `uuid` —
    /// silently degrades to an unconstrained field wherever a caller reads or
    /// builds the payload. Every one of CamaraSim's `format:` values is a standard
    /// format, so any value outside the recognized universe is a slip, not a
    /// deliberate custom format.
    ///
    /// Two contexts are excluded, mirroring `type_values_not_a_valid_type`: a
    /// `format:` with an *empty* value is a property literally named `format` (its
    /// value is a schema, not a format keyword), and a `format:` appearing as data
    /// inside an `example:`/`examples:` payload is example data, detected by walking
    /// the ancestor chain for an enclosing `example:`/`examples:` key. Only a
    /// `format:` at the start of its line (after indentation) is inspected.
    fn format_values_not_recognized(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The scalar value of a `format:` key, inline comment and quotes stripped;
        // `None` when the line is not a `format:` key.
        fn format_value(l: &str) -> Option<&str> {
            l.trim_start().strip_prefix("format:").map(|v| {
                v.split('#')
                    .next()
                    .unwrap_or(v)
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
            })
        }
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The recognized OpenAPI 3.0.x / JSON-Schema-Validation format vocabulary:
        // the OAS Data Type formats plus the JSON Schema draft string formats. A
        // typo of any of these lands in neither and is caught.
        const RECOGNIZED: [&str; 27] = [
            // OAS 3.0.x Data Type formats.
            "int32",
            "int64",
            "float",
            "double",
            "byte",
            "binary",
            "date",
            "date-time",
            "password",
            // JSON-Schema-Validation date/time formats.
            "time",
            "duration",
            // …e-mail / host / network formats.
            "email",
            "idn-email",
            "hostname",
            "idn-hostname",
            "ipv4",
            "ipv6",
            // …resource-identifier formats.
            "uri",
            "uri-reference",
            "iri",
            "iri-reference",
            "uri-template",
            "uuid",
            // …JSON-pointer / regex formats.
            "json-pointer",
            "relative-json-pointer",
            "regex",
            "regexp",
        ];
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(v) = format_value(line) else {
                continue;
            };
            if v.is_empty() {
                continue; // a property named `format`, or a block opener
            }
            if RECOGNIZED.contains(&v) {
                continue;
            }
            if inside_example(i, indent(line)) {
                continue; // example data, not a format keyword
            }
            out.push(i + 1);
        }
        out
    }

    #[test]
    fn every_format_names_a_recognized_format() {
        // Contract-harness invariant (OpenAPI 3.0.x convention): every Schema
        // Object `format:` a mounted spec declares MUST name a recognized format —
        // an OAS Data Type format (`int32`/`int64`/`float`/`double`/`byte`/`binary`/
        // `date`/`date-time`/`password`) or a JSON-Schema-Validation string format
        // (`email`/`hostname`/`ipv4`/`ipv6`/`uri`/`uri-reference`/`uuid`/`regex`/…).
        // Tooling keys behaviour off the exact string (a format hint, a codegen type,
        // a validation check), so a typo — `datetime`, `int_32`, `uid` — silently
        // drops the constraint wherever a caller reads or builds the payload. A
        // property literally *named* `format` (empty value) and a `format:` inside an
        // `example:`/`examples:` payload are excluded — neither is a format keyword.
        // It is invisible to every existing test: the `type:` test checks the sibling
        // `type` token, never the `format` modifier, and the size/numeric-bound tests
        // inspect bound values, never a format string. Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let bad = format_values_not_recognized(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares `format:` value(s) naming no recognized OpenAPI/JSON-Schema \
                 format (likely a typo of a standard format) at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn format_value_extraction_rules() {
        // Unit-cover the `format_values_not_recognized` extractor so the contract
        // test above can't pass vacuously and its detection is pinned: recognized
        // formats (`uuid`/`date-time`/`int32`) pass; a property literally named
        // `format` (empty value) and a `format:` inside an `example:` payload are
        // skipped; a typo in a schema — at the top level *and* nested under
        // `properties:` — is flagged in document order.
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
      properties:
        id:
          type: string
          format: uuid
        at:
          type: string
          format: date-time
        n:
          type: integer
          format: int32
        format:
          type: string
    Event:
      type: object
      example:
        format: not-a-real-format
        id: abc
    BadTypo:
      type: string
      format: datetime
    BadInSchema:
      type: object
      properties:
        x:
          type: string
          format: uid
";
        // Flagged, in document order: `BadTypo`'s `format: datetime` at line 35 and
        // `BadInSchema.x`'s `format: uid` at line 41. Not flagged: the recognized
        // `uuid`/`date-time`/`int32`, the property literally named `format` (line 26,
        // empty value), and the `format: not-a-real-format` inside the `example:`
        // payload (line 31).
        assert_eq!(format_values_not_recognized(body), vec![35, 41]);

        // Non-vacuous floor: across every registered spec every `format:` names a
        // recognized format (the invariant the contract test asserts), and the corpus
        // declares many `format:` keys, so a broken extractor can't hide behind an
        // empty scan. Count `format:` lines naming a recognized format with a
        // detection independent of the extractor.
        let mut formatted = 0usize;
        for api in APIS {
            assert!(
                format_values_not_recognized(api.body).is_empty(),
                "{}: every `format:` must name a recognized format",
                api.name
            );
            for line in api.body.lines() {
                if let Some(v) = line.trim_start().strip_prefix("format:") {
                    let v = v
                        .split('#')
                        .next()
                        .unwrap_or(v)
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if matches!(
                        v,
                        "int32"
                            | "int64"
                            | "float"
                            | "double"
                            | "byte"
                            | "date"
                            | "date-time"
                            | "email"
                            | "ipv4"
                            | "ipv6"
                            | "uri"
                            | "uri-reference"
                            | "uuid"
                    ) {
                        formatted += 1;
                    }
                }
            }
        }
        assert!(
            formatted >= 200,
            "expected many schema `format:` keys across specs, got {formatted}"
        );
    }

    /// The 1-based line numbers, in document order, of every schema `format:` whose
    /// value names a recognized format but whose sibling `type:` names the wrong
    /// JSON type for that format's family — without a YAML dep.
    ///
    /// In OpenAPI 3.0.x a `format` is a modifier on one specific `type`: the
    /// integer formats `int32`/`int64` require `type: integer`, the numeric formats
    /// `float`/`double` require `type: number`, and every string format
    /// (`date`/`date-time`/`time`/`duration`/`byte`/`binary`/`password`/`email`/
    /// `hostname`/`ipv4`/`ipv6`/`uri`/`uri-reference`/`uuid`/`regex`/…) requires
    /// `type: string`. A recognized format sitting on the wrong type (`format: uuid`
    /// under `type: integer`, or a `format: int32` under `type: string`) is a
    /// self-contradictory schema: the format can never constrain a value of that
    /// type, so a Redoc/Swagger/codegen client keeps the type and silently drops the
    /// format hint wherever a caller reads or builds the payload.
    ///
    /// Only a `format:` that (a) names a recognized format — an unrecognized value
    /// is a typo owned by `format_values_not_recognized`, not a type mismatch — and
    /// (b) has a `type:` *scalar sibling* in the same Schema Object (same indent,
    /// scanning down through the object's block then up, dedent-bounded exactly like
    /// `schema_bounds_inverted`) is inspected. A `format` whose `type` is absent
    /// (inherited via `allOf`/`$ref`, or declared in an outer object) or opens a
    /// block is skipped — nothing to compare — and a `format:` inside an
    /// `example:`/`examples:` payload is example data, excluded by walking the
    /// ancestor chain (mirroring `format_values_not_recognized`). Only a `format:`
    /// at the start of its line (after indentation) is inspected.
    fn format_type_mismatches(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a `name:` key (inline comment + surrounding quotes
        // stripped); `None` when the line is a different key or opens a block (no
        // inline value).
        let scalar = |l: &str, name: &str| -> Option<String> {
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
                None
            } else {
                Some(v.to_string())
            }
        };
        // The JSON type a recognized format modifies, or `None` when the format is
        // not recognized (a typo — owned by the recognition test, not this one).
        fn required_type(fmt: &str) -> Option<&'static str> {
            Some(match fmt {
                "int32" | "int64" => "integer",
                "float" | "double" => "number",
                "byte" | "binary" | "date" | "date-time" | "password" | "time"
                | "duration" | "email" | "idn-email" | "hostname" | "idn-hostname"
                | "ipv4" | "ipv6" | "uri" | "uri-reference" | "iri" | "iri-reference"
                | "uri-template" | "uuid" | "json-pointer" | "relative-json-pointer"
                | "regex" | "regexp" => "string",
                _ => return None,
            })
        }
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The sibling `type:` scalar in the same object as line `i` (indent `c`):
        // scan down through the object's block for a same-indent `type`, then up,
        // dedent-bounded so a nested/following object's `type` never pairs.
        let sibling_type = |i: usize, c: usize| -> Option<String> {
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar(l, "type") {
                        return Some(v);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar(l, "type") {
                        return Some(v);
                    }
                }
            }
            None
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(fmt) = scalar(line, "format") else {
                continue;
            };
            let Some(want) = required_type(&fmt) else {
                continue; // an unrecognized format is the recognition test's concern
            };
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            if let Some(ty) = sibling_type(i, c) {
                if ty != want {
                    out.push(i + 1);
                }
            }
        }
        out
    }

    #[test]
    fn every_format_matches_its_type() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares a recognized `format` as a sibling of a
        // `type` scalar, the type MUST be the one the format modifies — the integer
        // formats `int32`/`int64` on `type: integer`, the numeric formats
        // `float`/`double` on `type: number`, every string format (`date-time`,
        // `uuid`, `uri`, `ipv4`, `byte`, …) on `type: string`. A recognized format
        // on the wrong type (`format: uuid` under `type: integer`) is a
        // self-contradictory schema: the format can never constrain a value of that
        // type, so a Redoc/Swagger/codegen client keeps the type and silently drops
        // the format hint wherever a caller reads or builds the payload.
        //
        // This is the type-agreement complement of
        // `every_format_names_a_recognized_format`: that test proves each `format`
        // string is spelled from the known vocabulary but never looks at the sibling
        // `type`, so a correctly-spelled `format: int32` left on a `type: string` (a
        // field retyped without its format updated, or a format pasted from an
        // integer sibling onto a string one) sails through it. It is invisible to
        // `every_type_names_a_valid_schema_type` too — that checks the `type` token
        // is a valid type, never against a sibling format. Only a recognized format
        // with a `type:` scalar sibling in the same object is inspected (an
        // inherited/absent type, or an unrecognized format, is skipped). Verified
        // true across all mounted specs before asserting.
        for api in APIS {
            let bad = format_type_mismatches(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a recognized `format:` on a `type:` that does not \
                 match the format's family (e.g. `uuid` off `string`, `int32` off \
                 `integer`) at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn format_type_consistency_extraction_rules() {
        // Unit-cover `format_type_mismatches` so the contract test above can't pass
        // vacuously and its detection is pinned: a recognized format on its correct
        // type passes (`uuid` on string, `int32` on integer, `double` on number,
        // whether `type` is declared before or after `format`); a recognized format
        // on the wrong type is flagged in document order (`uuid` on integer, `int32`
        // on string); an *unrecognized* format is never flagged here (the
        // recognition test owns it); a `format` with no sibling `type` scalar (type
        // inherited/absent) is skipped; a property literally named `format` (empty
        // value) is skipped; and a `format:` inside an `example:` payload is skipped.
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
    GoodStr:
      type: string
      format: uuid
    GoodInt:
      type: integer
      format: int32
    GoodNum:
      format: double
      type: number
    BadUuidOnInt:
      type: integer
      format: uuid
    BadInt32OnStr:
      format: int32
      type: string
    UnknownFormat:
      type: integer
      format: datetime
    NoType:
      format: uuid
    NamedFormat:
      type: object
      properties:
        format:
          type: string
    InExample:
      type: object
      example:
        type: integer
        format: uuid
";
        // Flagged, in document order: BadUuidOnInt's `format: uuid` (line 25, its
        // sibling `type: integer` wants string) and BadInt32OnStr's `format: int32`
        // (line 27, its sibling `type: string` wants integer). Not flagged: the
        // three Good schemas; UnknownFormat (`datetime` is unrecognized — the
        // recognition test owns it); NoType (no sibling `type` scalar); the property
        // literally named `format` (line 37, empty value); and the `format: uuid`
        // inside the `example:` payload (line 43).
        assert_eq!(format_type_mismatches(body), vec![25, 27]);

        // Non-vacuous floor: across every registered spec every recognized format
        // sits on its matching type (the invariant the contract test asserts), and
        // the corpus declares many format+type pairs that actually agree — so the
        // value-comparison path runs on real data and a broken (always-empty)
        // extractor can't hide behind a corpus that never pairs a format with a
        // type. Count agreeing pairs with a presence detector that pairs the same
        // way but compares for a match rather than a mismatch.
        let mut agree = 0usize;
        for api in APIS {
            assert!(
                format_type_mismatches(api.body).is_empty(),
                "{}: every recognized format must sit on its matching type",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let scalar = |l: &str, name: &str| -> Option<String> {
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
                    None
                } else {
                    Some(v.to_string())
                }
            };
            let want = |fmt: &str| -> Option<&'static str> {
                Some(match fmt {
                    "int32" | "int64" => "integer",
                    "float" | "double" => "number",
                    "byte" | "binary" | "date" | "date-time" | "password" | "time"
                    | "duration" | "email" | "idn-email" | "hostname" | "idn-hostname"
                    | "ipv4" | "ipv6" | "uri" | "uri-reference" | "iri" | "iri-reference"
                    | "uri-template" | "uuid" | "json-pointer" | "relative-json-pointer"
                    | "regex" | "regexp" => "string",
                    _ => return None,
                })
            };
            for (i, l) in lines.iter().enumerate() {
                let Some(fmt) = scalar(l, "format") else {
                    continue;
                };
                let Some(w) = want(&fmt) else { continue };
                let c = indent(l);
                let mut ty = None;
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
                    if indent(x) == c {
                        if let Some(v) = scalar(x, "type") {
                            ty = Some(v);
                            break;
                        }
                    }
                    j += 1;
                }
                if ty.is_none() {
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
                        if indent(x) == c {
                            if let Some(v) = scalar(x, "type") {
                                ty = Some(v);
                                break;
                            }
                        }
                    }
                }
                if ty.as_deref() == Some(w) {
                    agree += 1;
                }
            }
        }
        assert!(
            agree >= 100,
            "expected many agreeing format+type pairs across specs, got {agree}"
        );
    }

    /// The 1-based line numbers, in document order, of every `enum:` at least one
    /// of whose values contradicts its sibling scalar `type:` — without a YAML dep.
    ///
    /// An `enum` fixes the closed set of values a schema may take, and a sibling
    /// `type` fixes their JSON type; the two apply to the *same* Schema Object, so
    /// every enum value must conform to the type. A value that does not — an
    /// unquoted `true`/`5` under `type: string` (YAML parses it as a boolean/number,
    /// not a string), a quoted `'1'` or a fractional `1.5` under `type: integer`, a
    /// non-boolean under `type: boolean` — is a self-contradictory schema: the
    /// value the enum offers is one the type's own validator rejects, so a
    /// Redoc/Swagger form pre-fills or a codegen client emits a member the field can
    /// never legally hold.
    ///
    /// Invisible to `every_enum_lists_unique_non_empty_values` (which checks a
    /// value list's own members are non-empty and unique, never against a type) and
    /// to `every_type_names_a_valid_schema_type` / `every_format_matches_its_type`
    /// (which check the `type` token itself, or a `format` modifier, never the enum
    /// values a type constrains).
    ///
    /// Only an enum with a `type:` *scalar sibling* naming one of the four scalar
    /// JSON types (`string`/`integer`/`number`/`boolean`) in the same object is
    /// inspected — the sibling is found by the same same-indent, dedent-bounded
    /// scan (down through the object's block then up) as `format_type_mismatches`.
    /// An enum with no sibling type (inherited via `allOf`/`$ref`, or typeless), or
    /// a non-scalar sibling type (`object`/`array`, whose members are structured),
    /// is skipped — nothing to compare against value-by-value. A `null`/`~` member
    /// (JSON null, legal in a nullable enum of any type), a property literally named
    /// `enum` (a block whose first child is not a `- ` item), and an `enum:` inside
    /// an `example:`/`examples:` payload are all skipped. Both YAML enum forms are
    /// handled: a flow list (`enum: [A, B]`, gathered to its `]`) and a block list
    /// (`enum:` then `- ` children), mirroring `enums_with_no_values_or_duplicates`;
    /// a quoted value is a string regardless of what its unquoted text would parse
    /// as, so quoting is preserved (not stripped) before classification.
    fn enum_values_inconsistent_with_type(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a `name:` key (inline comment + surrounding quotes
        // stripped); `None` when the line is a different key or opens a block.
        let scalar = |l: &str, name: &str| -> Option<String> {
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
                None
            } else {
                Some(v.to_string())
            }
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — mirrors `format_type_mismatches`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The sibling `type:` scalar in the same object as line `i` (indent `c`):
        // scan down through the object's block for a same-indent `type`, then up,
        // dedent-bounded so a nested/following object's `type` never pairs.
        let sibling_type = |i: usize, c: usize| -> Option<String> {
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar(l, "type") {
                        return Some(v);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar(l, "type") {
                        return Some(v);
                    }
                }
            }
            None
        };
        // Whether a raw (as-written) enum value token contradicts scalar type `ty`.
        // Quoting is significant: a quoted token is always a YAML string, whatever
        // its inner text would otherwise parse as.
        fn inconsistent(raw: &str, ty: &str) -> bool {
            let mut v = raw.trim();
            if let Some(p) = v.find(" #") {
                v = v[..p].trim_end();
            }
            let v = v.trim();
            if v.is_empty() || v == "null" || v == "~" {
                return false; // JSON null is legal in a nullable enum of any type
            }
            let quoted = v.len() >= 2
                && ((v.starts_with('"') && v.ends_with('"'))
                    || (v.starts_with('\'') && v.ends_with('\'')));
            let is_bool = !quoted
                && matches!(v, "true" | "false" | "True" | "False" | "TRUE" | "FALSE");
            let is_int = !quoted && v.parse::<i64>().is_ok();
            let is_num = !quoted && v.parse::<f64>().is_ok();
            match ty {
                "string" => is_bool || is_num, // an unquoted bool/number is not a string
                "boolean" => !is_bool,
                "integer" => !is_int,
                "number" => !is_num,
                _ => false,
            }
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let t = line.trim_start();
            if !t.starts_with("enum:") {
                i += 1;
                continue;
            }
            let c = indent(line);
            let enum_line = i;
            let rest = t["enum:".len()..].trim_start();
            let mut raws: Vec<String> = Vec::new();
            let mut is_enum_list = true;
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
                if !inner.trim().is_empty() {
                    raws = inner
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                i = k + 1;
            } else if rest.is_empty() || rest.starts_with('#') {
                // Block list — `- ` items at a deeper indent, but only when this is
                // genuinely an enum list (first child is a `-`, not a property named
                // `enum` whose value is a mapping).
                let base = c;
                let mut first_child_seen = false;
                let mut j = i + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() || l.trim_start().starts_with('#') {
                        j += 1;
                        continue;
                    }
                    if indent(l) <= base {
                        break;
                    }
                    let item = l.trim_start();
                    if !first_child_seen {
                        first_child_seen = true;
                        if !item.starts_with('-') {
                            is_enum_list = false;
                            break;
                        }
                    }
                    if !item.starts_with('-') {
                        break;
                    }
                    raws.push(item[1..].trim_start().to_string());
                    j += 1;
                }
                i = j;
            } else {
                // `enum: <scalar>` — not a list; a property named `enum`. Skip.
                i += 1;
                continue;
            }
            if !is_enum_list || raws.is_empty() {
                continue;
            }
            if inside_example(enum_line, c) {
                continue;
            }
            let Some(ty) = sibling_type(enum_line, c) else {
                continue;
            };
            if !matches!(ty.as_str(), "string" | "integer" | "number" | "boolean") {
                continue;
            }
            if raws.iter().any(|r| inconsistent(r, &ty)) {
                out.push(enum_line + 1);
            }
        }
        out
    }

    #[test]
    fn every_enum_value_matches_its_schema_type() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares an `enum` beside a scalar `type`, every
        // enum value MUST conform to that type. An `enum` fixes the closed set the
        // field may take and the sibling `type` fixes their JSON type, so a value
        // outside the type — an unquoted `true`/`5` under `type: string` (YAML reads
        // it as a boolean/number), a quoted `'1'` or fractional `1.5` under
        // `type: integer`, a non-boolean under `type: boolean` — is a
        // self-contradictory schema: the enum offers a member the type's own
        // validator would reject, so a Redoc/Swagger form pre-fills, or a codegen
        // client emits, a value the field can never legally hold.
        //
        // This is the value-conformance complement of the enum and type tests:
        // `every_enum_lists_unique_non_empty_values` checks the value list's own
        // members are non-empty and unique but never against a type, and
        // `every_type_names_a_valid_schema_type` / `every_format_matches_its_type`
        // check the `type` token itself (or a `format` modifier) but never the enum
        // values the type constrains. Only an enum with a scalar `type:` sibling
        // (string/integer/number/boolean) in the same object is inspected; a
        // typeless enum, a non-scalar sibling type, a `null` member, a property
        // named `enum`, and an `enum:` inside an `example:` payload are skipped.
        // Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = enum_values_inconsistent_with_type(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares an `enum` with a value that contradicts its sibling \
                 scalar `type:` (e.g. an unquoted bool/number under `type: string`, a \
                 quoted or fractional value under `type: integer`) at `enum:` line(s): \
                 {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn enum_value_type_consistency_extraction_rules() {
        // Unit-cover `enum_values_inconsistent_with_type` so the contract test above
        // can't pass vacuously and its detection is pinned: a string enum of bare
        // words / a flow string enum / an integer enum / a boolean enum all pass; an
        // unquoted `true` under `type: string` and a quoted `'1'` under
        // `type: integer` are flagged in document order; a typeless enum, a property
        // literally named `enum`, an `enum:` inside an `example:` payload, and a
        // `null` member of a nullable string enum are all skipped.
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
    GoodStr:
      type: string
      enum:
        - active
        - inactive
    GoodStrFlow:
      type: string
      enum: [asc, desc]
    GoodInt:
      type: integer
      enum:
        - 1
        - 2
    GoodBool:
      type: boolean
      enum: [true, false]
    BadStrHasBool:
      type: string
      enum:
        - active
        - true
    BadIntHasQuoted:
      type: integer
      enum: ['1', 2]
    NoType:
      enum:
        - x
        - y
    NamedEnum:
      type: object
      properties:
        enum:
          type: string
    InExample:
      type: object
      example:
        type: string
        enum:
          - 1
          - 2
    NullableStr:
      type: string
      nullable: true
      enum:
        - active
        - null
";
        // Flagged, in document order: BadStrHasBool's `enum:` (line 32 — its value
        // `true` is a YAML boolean, not a string) and BadIntHasQuoted's `enum:`
        // (line 37 — its quoted `'1'` is a string, not an integer). Not flagged: the
        // four Good schemas; NoType (no sibling `type`); the property literally named
        // `enum` (block whose first child opens a mapping); the `enum:` inside the
        // `example:` payload; and NullableStr (its only off-type member is `null`).
        assert_eq!(enum_values_inconsistent_with_type(body), vec![32, 37]);

        // Non-vacuous floor: across every registered spec every enum value conforms
        // to its sibling scalar type (the invariant the contract test asserts), and
        // the corpus declares many scalar-typed enums (the status/order/network-type/
        // credential-type/event-type enums) — so the value-comparison path runs on
        // real data and a broken (always-empty) extractor can't hide behind a corpus
        // that never pairs an enum with a scalar type. Count conforming scalar-typed
        // enums with a presence detector: an `enum:` whose nearest preceding
        // non-blank line at the same indent declares a scalar `type:`.
        let mut typed_enums = 0usize;
        for api in APIS {
            assert!(
                enum_values_inconsistent_with_type(api.body).is_empty(),
                "{}: every enum value must conform to its sibling scalar type",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                if l.trim_start() != "enum:" && !l.trim_start().starts_with("enum: [") {
                    continue;
                }
                let c = indent(l);
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
                    if indent(x) == c {
                        let xt = x.trim_start();
                        if matches!(
                            xt,
                            "type: string"
                                | "type: integer"
                                | "type: number"
                                | "type: boolean"
                        ) {
                            typed_enums += 1;
                        }
                        break;
                    }
                }
            }
        }
        assert!(
            typed_enums >= 10,
            "expected many scalar-typed enums across specs, got {typed_enums}"
        );
    }

    /// Line numbers (1-based) of OpenAPI 3.0.x boolean-valued keywords whose
    /// declared value is not a JSON boolean (`true`/`false`).
    ///
    /// In OpenAPI 3.0.x these modifier keywords are all boolean-valued —
    /// `nullable`, `readOnly`, `writeOnly`, `deprecated`, `uniqueItems`, and
    /// `exclusiveMinimum`/`exclusiveMaximum` (the last two only became *numbers*
    /// in OpenAPI 3.1 / JSON Schema 2020-12) — so any other scalar (a 3.1-style
    /// number `exclusiveMinimum: 5`, a stringified `"true"`, a YAML-truthy typo
    /// `yes`/`on`) is an invalid 3.0.x document a validator/codegen tool rejects
    /// or silently mis-reads.
    ///
    /// Two contexts are excluded, mirroring `format_values_not_recognized`: a
    /// keyword with an *empty* inline value is a property literally named for the
    /// keyword (its value opens a schema, not a boolean), and a keyword appearing
    /// as data inside an `example:`/`examples:` payload is example data, detected
    /// by walking the ancestor chain for an enclosing `example:`/`examples:` key.
    /// Only a keyword at the start of its line (after indentation) is inspected.
    fn boolean_keyword_non_boolean_values(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a boolean-keyword line (inline comment and quotes
        // stripped); `None` when the line is not one of the boolean keywords. The
        // `:` must immediately follow the keyword, so a longer key sharing the
        // prefix (`readOnlyFlag:`) does not match.
        fn keyword_value(l: &str) -> Option<&str> {
            const BOOL_KEYWORDS: [&str; 7] = [
                "nullable",
                "readOnly",
                "writeOnly",
                "deprecated",
                "uniqueItems",
                "exclusiveMinimum",
                "exclusiveMaximum",
            ];
            let t = l.trim_start();
            for kw in BOOL_KEYWORDS {
                if let Some(rest) = t.strip_prefix(kw) {
                    if let Some(v) = rest.strip_prefix(':') {
                        return Some(
                            v.split('#')
                                .next()
                                .unwrap_or(v)
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\''),
                        );
                    }
                }
            }
            None
        }
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(v) = keyword_value(line) else {
                continue;
            };
            if v.is_empty() {
                continue; // a property literally named for the keyword (block opener)
            }
            if v == "true" || v == "false" {
                continue;
            }
            if inside_example(i, indent(line)) {
                continue; // example data, not a boolean keyword
            }
            out.push(i + 1);
        }
        out
    }

    #[test]
    fn every_boolean_schema_keyword_carries_a_boolean() {
        // Contract-harness invariant (OpenAPI 3.0.x): every boolean-valued keyword
        // a mounted spec declares — `nullable`/`readOnly`/`writeOnly`/`deprecated`/
        // `uniqueItems`/`exclusiveMinimum`/`exclusiveMaximum` — MUST carry a JSON
        // boolean (`true`/`false`). The two `exclusive*` keywords are the live
        // hazard: they are booleans in 3.0.x but *numbers* in 3.1, so a spec drafted
        // or migrated with a 3.1 idiom (`exclusiveMinimum: 5`) — or any keyword given
        // a stringified/`yes`-style value — is an invalid 3.0.x document a
        // validator/codegen tool rejects or mis-reads. Invisible to every existing
        // test: the `type:`/`format:` vocabulary tests inspect those sibling tokens,
        // and the numeric/size-bound tests inspect a bound's *value*, never a boolean
        // modifier's value. A property literally *named* for a keyword (empty value)
        // and a keyword inside an `example:`/`examples:` payload are excluded —
        // neither is a boolean keyword. Verified true across all mounted specs before
        // asserting.
        for api in APIS {
            let bad = boolean_keyword_non_boolean_values(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares an OpenAPI 3.0.x boolean keyword \
                 (nullable/readOnly/writeOnly/deprecated/uniqueItems/exclusiveMinimum/\
                 exclusiveMaximum) with a non-boolean value at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn boolean_keyword_value_extraction_rules() {
        // Unit-cover the `boolean_keyword_non_boolean_values` extractor so the
        // contract test above can't pass vacuously and its detection is pinned:
        // boolean values (`nullable: true`, `readOnly: false`, `uniqueItems: true`)
        // pass; a property literally named `nullable` (empty value, block opener) and
        // a `readOnly:` inside an `example:` payload are skipped; a YAML-truthy typo
        // (`nullable: yes`) and a 3.1-style numeric `exclusiveMinimum: 5` are flagged
        // in document order.
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
      properties:
        id:
          type: string
          nullable: true
        tag:
          type: string
          readOnly: false
        nullable:
          type: boolean
      uniqueItems: true
    Event:
      type: object
      example:
        readOnly: notabool
        id: abc
    BadWord:
      type: string
      nullable: yes
    BadNumber:
      type: object
      properties:
        n:
          type: integer
          exclusiveMinimum: 5
";
        // Flagged, in document order: `BadWord`'s `nullable: yes` at line 33 and
        // `BadNumber.n`'s `exclusiveMinimum: 5` at line 39. Not flagged: the boolean
        // `true`/`false` values (lines 19, 22, 25), the property literally named
        // `nullable` (line 23, empty value), and the `readOnly: notabool` inside the
        // `example:` payload (line 29).
        assert_eq!(boolean_keyword_non_boolean_values(body), vec![33, 39]);

        // Non-vacuous floor: across every registered spec every boolean keyword
        // carries a boolean (the invariant the contract test asserts), and the corpus
        // declares many such keywords, so a broken extractor can't hide behind an
        // empty scan. Count boolean-keyword lines carrying a literal `true`/`false`
        // with a detection independent of the extractor.
        let mut booleans = 0usize;
        for api in APIS {
            assert!(
                boolean_keyword_non_boolean_values(api.body).is_empty(),
                "{}: every OpenAPI 3.0.x boolean keyword must carry a boolean",
                api.name
            );
            for line in api.body.lines() {
                let t = line.trim_start();
                for kw in [
                    "nullable:",
                    "readOnly:",
                    "writeOnly:",
                    "deprecated:",
                    "uniqueItems:",
                    "exclusiveMinimum:",
                    "exclusiveMaximum:",
                ] {
                    if let Some(v) = t.strip_prefix(kw) {
                        let v = v.split('#').next().unwrap_or(v).trim();
                        if v == "true" || v == "false" {
                            booleans += 1;
                        }
                    }
                }
            }
        }
        assert!(
            booleans >= 20,
            "expected many OpenAPI 3.0.x boolean keywords across specs, got {booleans}"
        );
    }

    /// Returns the 1-based line numbers of a schema's `readOnly: true` keyword whose
    /// same-object sibling also declares `writeOnly: true` — a contradictory
    /// property declaration.
    ///
    /// OpenAPI 3.0.x (JSON Schema): a Schema Object MUST NOT mark a property as both
    /// `readOnly` and `writeOnly` true. `readOnly: true` bars the value from a
    /// request body (it is only ever sent *to* the client); `writeOnly: true` bars
    /// it from a response body (it is only ever sent *by* the client); both true bars
    /// it from either direction, so the property can never legally appear at all — an
    /// unsatisfiable declaration a Redoc/Swagger/codegen client cannot honour (it
    /// would omit the field from every generated request *and* response model). Both
    /// default to `false` when unset, so only the `true`/`true` pair is the fault; a
    /// `readOnly: true` beside a `writeOnly: false` (or vice versa) is a normal,
    /// legal read-only field.
    ///
    /// This is invisible to `every_boolean_schema_keyword_carries_a_boolean`, which
    /// checks that each of `readOnly`/`writeOnly` *individually* carries a boolean
    /// value but never compares the two to each other; and to every other test, none
    /// of which pairs two access modifiers. A live copy-paste hazard: a property
    /// block pasted from a read-only sibling keeps its `readOnly: true` while a
    /// `writeOnly: true` is added for the new (write) use, or the wrong one of the
    /// pair is left in place.
    ///
    /// Pure and YAML-dep-free, mirroring `schema_bounds_inverted`: for each
    /// `readOnly: true` line at indent `c`, scan its object's block both directions
    /// (down then up), each bounded by the first line indented *below* `c` (the
    /// dedent that closes the object), for a `writeOnly: true` sibling at *exactly*
    /// `c`. The exact-indent, dedent-bounded match keeps a modifier nested in a
    /// sub-schema, or belonging to a following sibling property, from being mistaken
    /// for the pair. A `readOnly`/`writeOnly` appearing as data inside an
    /// `example:`/`examples:` payload is skipped (it is a literal example value, not
    /// a schema keyword), detected by walking the ancestor chain — mirroring
    /// `numeric_keyword_non_numeric_values`. Only the literal boolean `true` counts.
    fn properties_both_read_only_and_write_only(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // Whether `name:` on this line carries the inline boolean `true` (inline
        // comment and surrounding quotes stripped). A longer key sharing the prefix
        // (`readOnlyFlag`) does not match — the trimmed key must equal `name`.
        let is_true = |l: &str, name: &str| -> bool {
            let Some((k, v)) = l.trim_start().split_once(':') else {
                return false;
            };
            if k.trim() != name {
                return false;
            }
            let v = v
                .split('#')
                .next()
                .unwrap_or(v)
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            v == "true"
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if !is_true(line, "readOnly") {
                continue;
            }
            let c = indent(line);
            if inside_example(i, c) {
                continue; // example data, not a schema keyword
            }
            let mut paired = false;
            // Scan down through this object's block for the `writeOnly` sibling.
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
                if indent(l) == c && is_true(l, "writeOnly") {
                    paired = true;
                    break;
                }
                j += 1;
            }
            // The `writeOnly` sibling may be declared before the `readOnly`; scan up.
            if !paired {
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
                    if indent(l) == c && is_true(l, "writeOnly") {
                        paired = true;
                        break;
                    }
                }
            }
            if paired {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn no_property_declares_both_read_only_and_write_only() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule): no
        // Schema Object may declare BOTH `readOnly: true` and `writeOnly: true`.
        // `readOnly` bars the field from a request, `writeOnly` bars it from a
        // response; both true bars it from either, so the property can never legally
        // appear — an unsatisfiable declaration a Redoc/Swagger/codegen client cannot
        // honour. Both default to `false`, so a `readOnly: true` beside a
        // `writeOnly: false` is a legal read-only field; only the `true`/`true` pair
        // is the fault.
        //
        // A live copy-paste hazard in these specs (a property block pasted from a
        // read-only sibling that keeps its `readOnly` while a `writeOnly` is bolted
        // on), and invisible to `every_boolean_schema_keyword_carries_a_boolean`,
        // which validates each modifier's value type but never compares the two.
        // Verified true across all mounted specs before asserting.
        for api in APIS {
            let both = properties_both_read_only_and_write_only(api.body);
            assert!(
                both.is_empty(),
                "{} spec declares a property that is BOTH readOnly: true and \
                 writeOnly: true (it can appear in neither a request nor a response — \
                 an unsatisfiable declaration) at readOnly line(s): {:?}",
                api.name,
                both
            );
        }
    }

    #[test]
    fn read_only_write_only_exclusion_extraction_rules() {
        // Unit-cover the `properties_both_read_only_and_write_only` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a
        // `readOnly: true` whose `writeOnly: true` sibling sits below *or* above it in
        // the same object is flagged; a `readOnly`/`writeOnly` split across two
        // distinct properties is not; a `readOnly: true` beside a `writeOnly: false`
        // (the normal read-only field) is not; and a both-true pair inside an
        // `example:` payload is example data, not a schema keyword, so it is skipped.
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
    Distinct:
      type: object
      properties:
        a:
          type: string
          readOnly: true
        b:
          type: string
          writeOnly: true
    BadDown:
      type: string
      readOnly: true
      writeOnly: true
    BadUp:
      type: string
      writeOnly: true
      readOnly: true
    FalseGuard:
      type: string
      readOnly: true
      writeOnly: false
    Event:
      type: object
      example:
        readOnly: true
        writeOnly: true
        id: abc
";
        // Flagged, in document order: `BadDown.readOnly` (line 25, its `writeOnly:
        // true` sibling is the next line) and `BadUp.readOnly` (line 30, its
        // `writeOnly: true` sibling is the line above). Not flagged: `Distinct`
        // (readOnly on property `a`, writeOnly on the *following* property `b` past a
        // dedent — different objects); `FalseGuard` (readOnly: true beside writeOnly:
        // **false** — a legal read-only field); and `Event` (both `true` but inside an
        // `example:` payload — literal data, not schema keywords).
        assert_eq!(
            properties_both_read_only_and_write_only(body),
            vec![25, 30]
        );

        // Non-vacuous floor: across every registered spec no property is both
        // readOnly and writeOnly true (the invariant the contract test asserts), and
        // the corpus actually declares many `readOnly: true` access modifiers — so the
        // pairing scan runs on real data and a broken (always-empty) extractor can't
        // hide behind a corpus that never declares one. Count `readOnly: true` lines
        // (the loop's entry points) with a detector independent of the extractor.
        let mut read_only_trues = 0usize;
        for api in APIS {
            assert!(
                properties_both_read_only_and_write_only(api.body).is_empty(),
                "{}: no property may be both readOnly and writeOnly true",
                api.name
            );
            for line in api.body.lines() {
                if let Some(v) = line.trim_start().strip_prefix("readOnly:") {
                    if v.split('#').next().unwrap_or(v).trim() == "true" {
                        read_only_trues += 1;
                    }
                }
            }
        }
        assert!(
            read_only_trues >= 15,
            "expected many `readOnly: true` modifiers across specs, got {read_only_trues}"
        );
    }

    /// Line numbers (1-based) where a `204`/`304` response object declares a
    /// `content:` field — a message body on a status whose HTTP semantics forbid
    /// one.
    ///
    /// HTTP `204 No Content` and `304 Not Modified` MUST NOT carry a message body
    /// (RFC 9110 §15.3.5 / §15.4.5, and the CAMARA API design guidelines), so an
    /// OpenAPI Response Object for either status must not define `content`: the
    /// payload it advertises can never be sent, and a Redoc/Swagger "try it" panel
    /// or codegen client is handed a response model no response will ever fill. It
    /// is the response-side complement of the body-bearing structural tests
    /// (`request_bodies_missing_content` / `media_types_missing_schema`), which
    /// assert a body-bearing object *has* content/schema; this asserts a no-body
    /// response *lacks* one.
    ///
    /// A live copy-paste hazard: a `204` block pasted from a body-bearing sibling
    /// (a `200`/`201`) that kept its `content:` when the status was changed.
    /// Invisible to every existing test — `responses_missing_description` checks a
    /// response *has* a description, `operations_without_success_response` checks a
    /// `2xx` *exists*, and `media_types_missing_schema` inspects a media type's
    /// *schema*; none ever asserts a status forbids content.
    ///
    /// A `204`/`304` supplied as a Reference Object (its first child is `$ref`) is
    /// exempt — its body-ness is defined by the referenced component, not here. A
    /// `"204"` appearing as data inside an `example:`/`examples:` payload (a
    /// response map shown as a sample) is excluded via the ancestor walk. The
    /// status key is matched quoted or bare (`"204":`/`204:`) and only as a block
    /// opener (empty inline value); `content:` is credited only at the response
    /// object's own child indent, so a `content` nested deeper (under a header's
    /// schema, say) never counts. The offending `content:` line is reported.
    fn bodyless_responses_declaring_content(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // `Some("204"|"304")` when a line opens a no-body response block: a `204`/
        // `304` key (quoted or bare) with an empty inline value (a block opener, not
        // a scalar). `None` for any other key, an inline value, or a non-matching
        // status.
        let no_body_opener = |l: &str| -> Option<&'static str> {
            let (k, v) = l.trim_start().split_once(':')?;
            if !v.split('#').next().unwrap_or(v).trim().is_empty() {
                return None; // inline value → not a block opener
            }
            match k.trim().trim_matches('"').trim_matches('\'') {
                "204" => Some("204"),
                "304" => Some("304"),
                _ => None,
            }
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        fn field_key(l: &str) -> Option<&str> {
            l.trim_start().split_once(':').map(|(k, _)| k.trim())
        }
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if no_body_opener(line).is_none() {
                continue;
            }
            let c = indent(line);
            if inside_example(i, c) {
                continue; // a response map shown as example data, not a real response
            }
            // The response object's own child indent = the indent of its first
            // non-empty child. If that first child is `$ref`, the response is a
            // Reference Object — its content is defined elsewhere, so exempt.
            let mut child_indent = None;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break; // empty block / next sibling — no children to inspect
                }
                if field_key(l) != Some("$ref") {
                    child_indent = Some(indent(l));
                }
                break;
            }
            let Some(ci) = child_indent else { continue };
            // Scan the response object's block for a `content:` field at its own
            // child indent; a `content` nested deeper is not the response's own.
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break; // dedented out of this response object
                }
                if indent(l) == ci && field_key(l) == Some("content") {
                    out.push(j + 1);
                    break;
                }
                j += 1;
            }
        }
        out
    }

    #[test]
    fn no_bodyless_status_response_declares_content() {
        // Contract-harness invariant (HTTP / OpenAPI structural rule): a `204 No
        // Content` or `304 Not Modified` Response Object MUST NOT declare `content`.
        // Both statuses forbid a message body (RFC 9110), so a `content` block on
        // one advertises a payload that can never be sent — a Redoc/Swagger "try it"
        // panel / codegen client is handed a response model no response will fill.
        //
        // A live copy-paste hazard: a `204` pasted from a body-bearing `200`/`201`
        // sibling that kept its `content:` after the status changed. Invisible to
        // every existing test — `responses_missing_description` checks a response
        // *has* a description, `operations_without_success_response` checks a `2xx`
        // *exists*, `media_types_missing_schema` inspects a media type's *schema*;
        // none asserts a status forbids content. A `204`/`304` given as a `$ref`
        // Reference Object is exempt (its body-ness lives in the referenced
        // component). Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = bodyless_responses_declaring_content(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares `content` on a 204/304 response (a no-body HTTP \
                 status must not carry a message body) at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn bodyless_response_content_extraction_rules() {
        // Unit-cover the `bodyless_responses_declaring_content` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a
        // `204`/`304` response whose own object declares `content` is flagged (at the
        // `content:` line); a `204` carrying only `description`/`headers` is not; a
        // `200` with `content` is not (only no-body statuses are inspected); a `204`
        // given as a `$ref` Reference Object is not; and a `"204"` inside an
        // `example:` payload is example data, not a response, so it is skipped.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    delete:
      operationId: delA
      responses:
        \"204\":
          description: deleted
          headers:
            x-correlator:
              $ref: \"#/components/headers/XCorrelator\"
        \"200\":
          description: ok
          content:
            application/json:
              schema:
                type: object
    post:
      operationId: postA
      responses:
        \"204\":
          description: bad no-body with a body
          content:
            application/json:
              schema:
                type: object
        \"304\":
          description: also bad
          content:
            application/json:
              schema:
                type: string
        \"205\":
          $ref: \"#/components/responses/Reset\"
  /b:
    get:
      operationId: getB
      responses:
        \"204\":
          $ref: \"#/components/responses/NoBody\"
components:
  schemas:
    Sample:
      type: object
      example:
        responses:
          \"204\":
            content:
              application/json: {}
";
        // Flagged, in document order: the `content:` of the POST /a `204` (line 26)
        // and of its `304` (line 32). Not flagged: the DELETE /a `204` (only
        // description + headers, no content); the `200` (a body-bearing status —
        // never inspected); the `205` (not a no-body status); the GET /b `204` (a
        // `$ref` Reference Object — exempt); and the `Sample.example` `204` (a
        // response map inside an `example:` payload — literal data, not a response).
        assert_eq!(bodyless_responses_declaring_content(body), vec![26, 32]);

        // Non-vacuous floor: across every registered spec no 204/304 response
        // declares content (the contract), and the corpus actually declares many
        // no-body responses — so the scan runs on real data and a broken
        // (always-empty) extractor can't hide behind a corpus that never declares
        // one. Count 204/304 block-opener response keys with a detector independent
        // of the extractor.
        let mut no_body_responses = 0usize;
        for api in APIS {
            assert!(
                bodyless_responses_declaring_content(api.body).is_empty(),
                "{}: no 204/304 response may declare content",
                api.name
            );
            for line in api.body.lines() {
                let t = line.trim_start();
                if let Some((k, v)) = t.split_once(':') {
                    let k = k.trim().trim_matches('"').trim_matches('\'');
                    if (k == "204" || k == "304")
                        && v.split('#').next().unwrap_or(v).trim().is_empty()
                    {
                        no_body_responses += 1;
                    }
                }
            }
        }
        assert!(
            no_body_responses >= 20,
            "expected many 204/304 responses across specs, got {no_body_responses}"
        );
    }

    /// Line numbers (1-based) of OpenAPI 3.0.x number-valued schema keywords whose
    /// declared value is not a JSON number — `minimum`, `maximum`, `multipleOf` —
    /// plus any `multipleOf` that is not strictly greater than 0.
    ///
    /// In OpenAPI 3.0.x these three keywords are the number-valued Schema Object
    /// facets: `minimum`/`maximum` bound a numeric value (either may itself be
    /// negative or fractional), and `multipleOf` constrains the value to a multiple
    /// of a number that MUST be strictly greater than 0. So a non-numeric scalar (a
    /// word, a YAML anchor, a stray range like `1..10`) is an invalid document a
    /// validator/codegen tool rejects or mis-reads, and a `multipleOf: 0`/negative
    /// (a division by a non-positive step) is unsatisfiable — both at exactly the
    /// point a caller reads or builds the payload.
    ///
    /// This is the number-family analogue of `boolean_keyword_non_boolean_values`
    /// (boolean modifiers) and `size_bounds_out_of_domain` (the integer
    /// length/size/count caps), and the *value-type* complement of
    /// `every_numeric_bound_is_ordered_low_to_high`: that ordering test compares a
    /// `minimum`/`maximum` pair only when *both* are present and *already parse as
    /// numbers*, so a lone `minimum` with no `maximum` sibling — or either given a
    /// non-numeric value — is never checked for numeric-ness, and `multipleOf` (which
    /// has no lower/upper sibling) is inspected by no test at all.
    ///
    /// Two contexts are excluded, mirroring the boolean/format extractors: a keyword
    /// with an *empty* inline value is a property literally named for the keyword (it
    /// opens a schema, not a scalar), and a keyword appearing as data inside an
    /// `example:`/`examples:` payload is example data, detected by walking the
    /// ancestor chain for an enclosing `example:`/`examples:` key. The inline scalar
    /// is read with inline-comment and surrounding quotes stripped (as its siblings
    /// do). Only a keyword at the start of its line (after indentation), whose `:`
    /// immediately follows it, is inspected — a longer key sharing the prefix
    /// (`minimumAge:`) does not match. Offending lines are returned in document order.
    fn numeric_keyword_non_numeric_values(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The (keyword, inline-scalar) of a number-keyword line; `None` when the
        // line is not one of the number keywords.
        fn keyword_value(l: &str) -> Option<(&'static str, &str)> {
            const NUM_KEYWORDS: [&str; 3] = ["minimum", "maximum", "multipleOf"];
            let t = l.trim_start();
            for kw in NUM_KEYWORDS {
                if let Some(rest) = t.strip_prefix(kw) {
                    if let Some(v) = rest.strip_prefix(':') {
                        return Some((
                            kw,
                            v.split('#')
                                .next()
                                .unwrap_or(v)
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\''),
                        ));
                    }
                }
            }
            None
        }
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some((kw, v)) = keyword_value(line) else {
                continue;
            };
            if v.is_empty() {
                continue; // a property literally named for the keyword (block opener)
            }
            if inside_example(i, indent(line)) {
                continue; // example data, not a schema keyword
            }
            match v.parse::<f64>() {
                // `multipleOf` MUST be strictly greater than 0 (OpenAPI 3.0.x);
                // `minimum`/`maximum` may be any finite number.
                Ok(n) if n.is_finite() => {
                    if kw == "multipleOf" && n <= 0.0 {
                        out.push(i + 1);
                    }
                }
                _ => out.push(i + 1),
            }
        }
        out
    }

    #[test]
    fn every_numeric_schema_keyword_carries_a_number() {
        // Contract-harness invariant (OpenAPI 3.0.x): every number-valued schema
        // keyword a mounted spec declares — `minimum`, `maximum`, `multipleOf` — MUST
        // carry a JSON number, and a `multipleOf` MUST be strictly greater than 0. A
        // non-numeric value (a word, a stray range) is an invalid document a
        // validator/codegen tool rejects or mis-reads, and a non-positive
        // `multipleOf` is an unsatisfiable constraint — both where a caller reads or
        // builds the payload.
        //
        // This is the value-type complement of
        // `every_numeric_bound_is_ordered_low_to_high`, which compares a
        // `minimum`/`maximum` pair only when both are present and already numeric —
        // so a lone `minimum`, either given a non-numeric value, or any `multipleOf`
        // (no sibling to pair with) escapes it — and the number-family analogue of
        // the boolean-keyword and size-bound value tests. A live hazard in these
        // scenario-table-heavy specs, whose numeric ranges (a `maxAge`, a `radius`, a
        // currency `multipleOf`) are hand-tuned per API. A property literally *named*
        // for a keyword (empty value) and a keyword inside an `example:`/`examples:`
        // payload are excluded. Verified true across all mounted specs before
        // asserting.
        for api in APIS {
            let bad = numeric_keyword_non_numeric_values(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares an OpenAPI 3.0.x number keyword \
                 (minimum/maximum/multipleOf) with a non-numeric value, or a \
                 multipleOf that is not > 0, at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn numeric_keyword_value_extraction_rules() {
        // Unit-cover the `numeric_keyword_non_numeric_values` extractor so the
        // contract test above can't pass vacuously and its detection is pinned:
        // numeric values (incl. negative `-90` and fractional `0.001`) pass; a
        // property literally named `minimum` (empty value, block opener) and a
        // `maximum:` inside an `example:` payload are skipped; a non-numeric
        // `maximum: many` and a non-positive `multipleOf: 0` / `multipleOf: -2` are
        // flagged in document order.
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
      properties:
        lat:
          type: number
          minimum: -90
          maximum: 90
        amt:
          type: number
          multipleOf: 0.001
        minimum:
          type: integer
    Event:
      type: object
      example:
        maximum: lots
        id: abc
    BadWord:
      type: integer
      maximum: many
    BadZero:
      type: number
      multipleOf: 0
    BadNeg:
      type: number
      multipleOf: -2
";
        // Flagged, in document order: `BadWord.maximum: many` (line 33),
        // `BadZero.multipleOf: 0` (line 36) and `BadNeg.multipleOf: -2` (line 39).
        // Not flagged: the numeric `minimum: -90`/`maximum: 90`/`multipleOf: 0.001`
        // (lines 19, 20, 23), the property literally named `minimum:` (line 24, empty
        // value), and the `maximum: lots` inside the `example:` payload (line 29).
        assert_eq!(numeric_keyword_non_numeric_values(body), vec![33, 36, 39]);

        // Non-vacuous floor: across every registered spec every number keyword
        // carries a number and every `multipleOf` is positive (the invariant the
        // contract test asserts), and the corpus declares many such keywords — so the
        // value-parsing path runs on real data and a broken (always-empty) extractor
        // can't hide behind a corpus that never declares one. Count keyword lines
        // carrying a non-empty inline value with a detector independent of the
        // extractor's parsing.
        let mut numerics = 0usize;
        for api in APIS {
            assert!(
                numeric_keyword_non_numeric_values(api.body).is_empty(),
                "{}: every minimum/maximum must be numeric and every multipleOf > 0",
                api.name
            );
            for line in api.body.lines() {
                if let Some((k, v)) = line.trim_start().split_once(':') {
                    if matches!(k.trim(), "minimum" | "maximum" | "multipleOf")
                        && !v.split('#').next().unwrap_or(v).trim().is_empty()
                    {
                        numerics += 1;
                    }
                }
            }
        }
        assert!(
            numerics >= 150,
            "expected many OpenAPI 3.0.x number keywords across specs, got {numerics}"
        );
    }

    /// The path-template key of every Path Item a spec declares under `paths:`
    /// that is not a well-formed OpenAPI *path template*.
    ///
    /// OpenAPI "path templating" lets a `paths:` key embed one or more
    /// `{parameter}` expressions — `/sessions/{sessionId}` — each of which MUST be
    /// a balanced `{`…`}` pair enclosing a non-empty parameter name; the key is a
    /// URL path, so it also carries no whitespace and no query (`?`) or fragment
    /// (`#`) delimiter (those begin the *other* URL components, not the path). A
    /// key that breaks templating — an unclosed `/{sessionId` (a paste that lost
    /// the `}`), a nested `/{a{b}}`, an empty `/{}`, a stray `}`, or a `?`/space
    /// smuggled in — is an invalid document a Redoc/Swagger/codegen client can't
    /// bind a route to.
    ///
    /// This is invisible to the sibling path tests: `every_paths_object_declares_
    /// slash_prefixed_path_items` checks only the *leading* `/`,
    /// `every_paths_object_lists_distinct_path_keys` checks only *uniqueness*, and
    /// `path_template_params_match_declared_path_parameters` extracts `{…}` *spans*
    /// and matches their names against declared path parameters — a malformed brace
    /// simply yields no span, so a `/{id` whose operation also dropped its `id`
    /// path-parameter declaration matches nothing on either side and sails through.
    /// None of them inspect the brace structure of the key itself.
    ///
    /// Reuses the (unit-covered) `path_item_keys` extractor — which already scopes
    /// to the top-level `paths:` block, unquotes the key, and excludes `x-`
    /// Paths-Object extensions — then validates each returned template. Returns the
    /// offending keys in document order.
    fn malformed_path_template_keys(body: &str) -> Vec<String> {
        let mut out = Vec::new();
        for key in path_item_keys(body) {
            // `depth`: 0 = outside a `{…}`, 1 = inside one (nesting is illegal, so
            // it never exceeds 1). `segment_has_name`: whether the current `{…}`
            // has enclosed at least one character before its `}`.
            let mut depth = 0usize;
            let mut segment_has_name = false;
            let mut ok = true;
            for c in key.chars() {
                if c.is_whitespace() || c == '?' || c == '#' {
                    ok = false;
                    break;
                }
                match c {
                    '{' => {
                        if depth > 0 {
                            ok = false; // a nested `{`
                            break;
                        }
                        depth = 1;
                        segment_has_name = false;
                    }
                    '}' => {
                        if depth == 0 || !segment_has_name {
                            ok = false; // a stray `}` or an empty `{}`
                            break;
                        }
                        depth = 0;
                    }
                    _ => {
                        if depth == 1 {
                            segment_has_name = true;
                        }
                    }
                }
            }
            if depth != 0 {
                ok = false; // an unclosed `{`
            }
            if !ok {
                out.push(key);
            }
        }
        out
    }

    #[test]
    fn every_path_template_key_is_well_formed() {
        // Contract-harness invariant (OpenAPI path-templating structural rule):
        // every `paths:` key a mounted spec declares MUST be a well-formed path
        // template — each `{parameter}` a balanced `{`…`}` pair around a non-empty
        // name, and no whitespace or query (`?`)/fragment (`#`) delimiter in the
        // path. A key that breaks this — an unclosed `/{sessionId`, a nested
        // `/{a{b}}`, an empty `/{}`, a stray `}`, a `?`/space — is a document a
        // client can't bind a route to. It is invisible to every sibling path test:
        // the slash-prefix test checks only the leading `/`, the distinct-keys test
        // only uniqueness, and the path-templating test matches `{…}` *spans* by
        // name (a malformed brace yields no span, so it slips through). Verified true
        // across all mounted specs before asserting.
        for api in APIS {
            let malformed = malformed_path_template_keys(api.body);
            assert!(
                malformed.is_empty(),
                "{} spec declares a `paths:` key that is not a well-formed path template \
                 (unbalanced/empty `{{}}`, or whitespace/`?`/`#`): {:?}",
                api.name,
                malformed
            );
        }
    }

    #[test]
    fn path_template_key_wellformedness_rules() {
        // Unit-cover the `malformed_path_template_keys` extractor so the contract
        // test above can't pass vacuously and its detection is pinned: valid
        // single- and multi-parameter templates pass; an unclosed brace, an empty
        // `{}`, a nested `{a{b}}`, a stray `}`, a whitespace, and a query `?` are
        // each flagged, in document order; an `x-` Paths-Object extension is not a
        // path item (excluded upstream by `path_item_keys`) so it is never flagged.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /sessions/{sessionId}:
    get: noop
  /a/{id}/b/{sub}:
    get: noop
  /bad/{id:
    get: noop
  /empty/{}:
    get: noop
  /nest/{a{b}}:
    get: noop
  /stray/}x:
    get: noop
  /has space/x:
    get: noop
  /query/x?y=1:
    get: noop
  x-tension:
    note: ok
";
        // Flagged, in document order: the unclosed `/bad/{id`, the empty `/empty/{}`,
        // the nested `/nest/{a{b}}`, the stray `/stray/}x`, the whitespace
        // `/has space/x`, and the query `/query/x?y=1`. Not flagged: the two valid
        // templates (`/sessions/{sessionId}`, `/a/{id}/b/{sub}`) and the `x-tension`
        // extension (not a Path Item).
        assert_eq!(
            malformed_path_template_keys(body),
            vec![
                "/bad/{id".to_string(),
                "/empty/{}".to_string(),
                "/nest/{a{b}}".to_string(),
                "/stray/}x".to_string(),
                "/has space/x".to_string(),
                "/query/x?y=1".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec every `paths:` key is a
        // well-formed template (the invariant the contract test asserts), and the
        // corpus declares many path items, so a broken extractor can't hide behind
        // an empty scan. Count path keys with a detection independent of the
        // extractor.
        let mut path_keys = 0usize;
        for api in APIS {
            assert!(
                malformed_path_template_keys(api.body).is_empty(),
                "{}: every `paths:` key must be a well-formed path template",
                api.name
            );
            path_keys += path_item_keys(api.body).len();
        }
        assert!(
            path_keys >= 100,
            "expected many path items across specs, got {path_keys}"
        );
    }

    /// Extract a descriptor for every `properties:` object a spec declares that
    /// lists the **same property name twice** — without a YAML dep.
    ///
    /// A Schema Object's `properties:` is a YAML mapping keyed by property name,
    /// so the names must be unique: a mapping that repeats a key is invalid, and
    /// every YAML/JSON parser silently keeps only the **last** occurrence — so the
    /// earlier property's schema (its `type`, `format`, bounds, `description`) is
    /// discarded without a trace. The routine hazard in these specs is a property
    /// block grown by pasting a sibling property and forgetting to rename it, so
    /// the field a caller reads is governed by whichever copy came last.
    ///
    /// For each block-opening `properties:` at indent `C` (an empty value or only a
    /// trailing `# comment` — a `properties:` with an inline value opens no mapping)
    /// this finds the first-child indent `D` (the first deeper non-blank line) and
    /// collects the mapping keys at *exactly* `D`, bounded by the first non-blank
    /// line that dedents to `C` or shallower (the sibling schema keyword — `required:`,
    /// `additionalProperties:` — or enclosing dedent that closes the block). Keys
    /// deeper than `D` are a property's own schema (its `type:`/`description:`, or a
    /// nested `properties:` handled as its own block on its own opener), never direct
    /// property names, so they are skipped. A repeated key at `D` is flagged with the
    /// offending name and the block's line, in document order.
    fn properties_objects_with_duplicate_names(body: &str) -> Vec<String> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // Unquote a property-name key.
        let unquote = |name: &str| -> String {
            name.strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| name.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
                .unwrap_or(name)
                .to_string()
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            // A `properties:` mapping opener: the key `properties`, its value empty
            // or a trailing comment (an inline value would be a scalar/flow, not a
            // block mapping of property definitions).
            let Some(rest) = t.strip_prefix("properties:") else { continue };
            let rest = rest.trim_start();
            if !(rest.is_empty() || rest.starts_with('#')) {
                continue;
            }
            let c = indent(line);
            // First-child indent D (first non-blank, non-comment line deeper than C).
            let mut d: Option<usize> = None;
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                let tl = l.trim();
                if tl.is_empty() || tl.starts_with('#') {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break; // empty properties block
                }
                d = Some(indent(l));
                break;
            }
            let Some(d) = d else { continue };
            // Collect direct property keys (exactly indent D) until the block closes.
            let mut seen = HashSet::new();
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                let tl = l.trim();
                if tl.is_empty() || tl.starts_with('#') {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break; // dedented out of the properties block
                }
                if indent(l) == d && !tl.starts_with('-') {
                    if let Some((k, _)) = tl.split_once(':') {
                        let name = unquote(k.trim());
                        if !name.is_empty() && !seen.insert(name.clone()) {
                            out.push(format!("`{name}` (properties block at line {})", i + 1));
                        }
                    }
                }
                j += 1;
            }
        }
        out
    }

    #[test]
    fn every_properties_object_lists_distinct_property_names() {
        // Contract-harness invariant (OpenAPI / YAML structural rule): a Schema
        // Object's `properties:` is a mapping keyed by property name, so a mounted
        // spec MUST NOT list the same property name twice in one `properties:`
        // block. A repeated key is an invalid mapping every parser resolves by
        // keeping only the last copy — so the earlier property's schema is dropped
        // silently, and the field a caller reads/generates is whichever definition
        // came last.
        //
        // No sibling "distinct" test looks at property *names*: the required-array,
        // parameter-array, enum, and operationId duplicate tests check a `required`
        // list, a `(name, location)` pair, an enum's values, or an operation's id —
        // never the keys of a `properties:` mapping. In these scenario-table-heavy
        // specs a property pasted from a sibling schema and left unrenamed is a live
        // copy-paste hazard. Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = properties_objects_with_duplicate_names(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a `properties:` object that repeats a property \
                 name (a schema's property keys must be distinct): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn properties_object_duplicate_name_extraction_rules() {
        // Unit-cover the `properties_objects_with_duplicate_names` extractor so the
        // contract test above can't pass vacuously and its detection is pinned: a
        // `properties:` block that repeats a name is flagged (with the name and the
        // block's line); a property's own schema keywords (deeper than the property
        // indent) are never counted as names; a nested `properties:` is scanned as
        // its own block, so a name reused across the outer and an inner block is not
        // a duplicate; and a clean block passes. All in document order.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths: {}
components:
  schemas:
    Dup:
      type: object
      properties:
        phoneNumber:
          type: string
        amount:
          type: number
        phoneNumber:
          type: string
    Clean:
      type: object
      properties:
        a:
          type: string
        b:
          type: object
          properties:
            a:
              type: string
            c:
              type: string
";
        // Flagged, in document order: only `Dup`'s block (line 10) repeats
        // `phoneNumber`. Not flagged: `Clean`'s outer block (`a`, `b` — the deeper
        // `type:` keywords are the properties' own schema, not names), and `b`'s
        // nested block (`a`, `c` — its `a` is a different mapping, so reusing the
        // outer name `a` is legitimate).
        assert_eq!(
            properties_objects_with_duplicate_names(body),
            vec!["`phoneNumber` (properties block at line 10)".to_string()]
        );

        // Non-vacuous floor: across every registered spec no `properties:` block
        // repeats a name (the invariant the contract test asserts), and the corpus
        // actually declares many direct property keys, so a broken extractor can't
        // hide behind an empty scan. Count direct property keys (a mapping key at the
        // first-child indent of a block-opening `properties:`) independently of the
        // extractor.
        let mut property_keys = 0usize;
        for api in APIS {
            assert!(
                properties_objects_with_duplicate_names(api.body).is_empty(),
                "{}: every `properties:` object must list distinct property names",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, line) in lines.iter().enumerate() {
                let t = line.trim_start();
                let Some(rest) = t.strip_prefix("properties:") else { continue };
                let rest = rest.trim_start();
                if !(rest.is_empty() || rest.starts_with('#')) {
                    continue;
                }
                let c = indent(line);
                // First-child indent D.
                let mut d: Option<usize> = None;
                for l in &lines[i + 1..] {
                    let tl = l.trim();
                    if tl.is_empty() || tl.starts_with('#') {
                        continue;
                    }
                    if indent(l) <= c {
                        break;
                    }
                    d = Some(indent(l));
                    break;
                }
                let Some(d) = d else { continue };
                for l in &lines[i + 1..] {
                    let tl = l.trim();
                    if tl.is_empty() || tl.starts_with('#') {
                        continue;
                    }
                    if indent(l) <= c {
                        break;
                    }
                    if indent(l) == d && !tl.starts_with('-') && tl.contains(':') {
                        property_keys += 1;
                    }
                }
            }
        }
        assert!(
            property_keys >= 500,
            "expected many direct property keys across specs, got {property_keys}"
        );
    }

    /// Returns the 1-based line numbers of every `default:` scalar whose schema
    /// object also declares a sibling `enum:` list that does **not** contain the
    /// default's value — a self-contradictory constraint: the schema offers a
    /// default value that a validator built from the very same `enum` would
    /// reject, and a Redoc/Swagger form pre-fills a field with a value outside
    /// its own closed set.
    ///
    /// Pure and YAML-dep-free, mirroring `schema_bounds_inverted`'s
    /// sibling-pairing: for each `default:` key with an inline scalar value at
    /// indent `c`, scan its object's block (down then up, each bounded by the
    /// first line indented *below* `c`, the dedent that closes the object) for an
    /// `enum:` key at *exactly* `c`. When one is found, collect that enum's values
    /// (the flow `enum: [A, B]` and block `enum:`/`- A` forms, unquoted and
    /// comment-trimmed, mirroring `enums_with_no_values_or_duplicates`) and flag
    /// the `default` when its normalized value is absent from them. The
    /// exact-indent, dedent-bounded match keeps a `default` in one property from
    /// pairing with a following sibling property's `enum`. Skipped (nothing to
    /// compare): a `default:` that opens a block rather than holding an inline
    /// scalar (an object/array default, or a schema property literally named
    /// `default`), a `default` with no sibling `enum` (unconstrained, always
    /// valid), and an `enum:` sibling that is a mapping named `enum` rather than a
    /// value list (its collected value set is empty).
    fn defaults_outside_their_enum(body: &str) -> Vec<usize> {
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
        // The inline scalar value of a `name:` key. `None` when the line is a
        // different key or opens a block (no inline value after the colon).
        let inline_val = |l: &str, name: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != name {
                return None;
            }
            let n = norm(v);
            if n.is_empty() {
                None
            } else {
                Some(n)
            }
        };
        // The values of the enum whose `enum:` key sits at line index `e`.
        let enum_values_at = |e: usize| -> Vec<String> {
            let line = lines[e];
            let rest = line.trim_start()["enum:".len()..].trim_start();
            if rest.starts_with('[') {
                // Flow list — gather across lines until the closing `]`.
                let mut buf = rest.to_string();
                let mut k = e;
                while !buf.contains(']') && k + 1 < lines.len() {
                    k += 1;
                    buf.push(' ');
                    buf.push_str(lines[k].trim());
                }
                let open = buf.find('[').map(|x| x + 1).unwrap_or(0);
                let close = buf.rfind(']').unwrap_or(buf.len());
                let inner = if close >= open { &buf[open..close] } else { "" };
                if inner.trim().is_empty() {
                    Vec::new()
                } else {
                    inner.split(',').map(|s| norm(s)).filter(|v| !v.is_empty()).collect()
                }
            } else if rest.is_empty() || rest.starts_with('#') {
                // Block list — `- ` children at a deeper indent, but only when the
                // first child is a `-` item (else it is a property named `enum`).
                let base = indent(line);
                let mut values: Vec<String> = Vec::new();
                let mut first_child_seen = false;
                let mut j = e + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() || l.trim_start().starts_with('#') {
                        j += 1;
                        continue;
                    }
                    if indent(l) <= base {
                        break;
                    }
                    let item = l.trim_start();
                    if !first_child_seen {
                        first_child_seen = true;
                        if !item.starts_with('-') {
                            break;
                        }
                    }
                    if !item.starts_with('-') {
                        break;
                    }
                    let val = norm(item[1..].trim_start());
                    if !val.is_empty() {
                        values.push(val);
                    }
                    j += 1;
                }
                values
            } else {
                Vec::new()
            }
        };
        let is_enum_key =
            |l: &str| l.trim_start().starts_with("enum:") && !l.trim_start().starts_with("enums");
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(dv) = inline_val(line, "default") else { continue };
            let c = indent(line);
            let mut e = None;
            // Scan down through this object's block for a sibling `enum:`.
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c && is_enum_key(l) {
                    e = Some(j);
                    break;
                }
                j += 1;
            }
            // The enum may be declared before the default; scan up too.
            if e.is_none() {
                let mut k = i;
                while k > 0 {
                    k -= 1;
                    let l = lines[k];
                    if l.trim().is_empty() {
                        continue;
                    }
                    if indent(l) < c {
                        break;
                    }
                    if indent(l) == c && is_enum_key(l) {
                        e = Some(k);
                        break;
                    }
                }
            }
            let Some(e) = e else { continue };
            let values = enum_values_at(e);
            if !values.is_empty() && !values.iter().any(|v| v == &dv) {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_default_is_a_member_of_its_enum() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares BOTH a `default` and an `enum`, the
        // default MUST be one of the enum's values. An `enum` fixes the closed set
        // a field may take; a `default` outside that set is self-contradictory —
        // the schema pre-supplies a value its own validator rejects, so a
        // Redoc/Swagger form pre-fills a control with an option the field can never
        // legally hold and a codegen client's default fails the enum's own check.
        //
        // A routine hazard in these scenario-table-heavy specs, where enum/default
        // pairs are hand-authored per API (an `order` param `[asc, desc]`, a
        // `deviceStatus` list, an edge-zone `edgeCloudZoneStatus`): a default typed
        // from memory, or an enum member renamed after the default was set, leaves
        // the two disagreeing. It is invisible to every existing test — the enum
        // test checks a value list's own members (unique/non-empty), the
        // numeric-bound test compares two numeric keywords, and the identity/
        // wiring/`$ref` tests never compare a default against its enum. Verified
        // true across all mounted specs before asserting.
        for api in APIS {
            let offenders = defaults_outside_their_enum(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec declares a `default` outside its sibling `enum` (a value \
                 the enum's own validator would reject) at line(s): {:?}",
                api.name,
                offenders
            );
        }
    }

    #[test]
    fn default_enum_membership_extraction_rules() {
        // Unit-cover the `defaults_outside_their_enum` extractor so the contract
        // test above can't pass vacuously and its detection is pinned: a `default`
        // is flagged only when a same-indent sibling `enum` (declared before OR
        // after it) omits the default's value; quotes/comments are normalized on
        // both sides before comparison; a `default` with no sibling enum is never
        // flagged; a `default` in one property never pairs with a following
        // property's enum across the dedent; and a `default:` that opens a block
        // (an object/array default or a property literally named `default`) is
        // skipped.
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
    GoodFlow:
      type: string
      enum: [asc, desc]
      default: desc
    BadAfter:
      type: string
      enum: [red, green]
      default: blue
    GoodBefore:
      type: string
      default: left
      enum:
        - left
        - right
    BadBefore:
      type: string
      default: up
      enum:
        - left
        - right
    NoEnum:
      type: string
      default: anything
    Quoted:
      type: string
      enum: [asc, desc]
      default: \"asc\"
    Split:
      type: object
      properties:
        a:
          type: string
          default: solo
        b:
          type: string
          enum: [x, y]
    NamedDefault:
      type: object
      properties:
        default:
          type: string
";
        // Flagged, in document order: line 21 (`BadAfter.default: blue`, whose
        // `enum: [red, green]` sibling above omits it) and line 30
        // (`BadBefore.default: up`, whose block `enum` below omits it). Not flagged:
        // `GoodFlow`/`GoodBefore` (default in enum, after / before it), `Quoted`
        // (`\"asc\"` normalizes into `[asc, desc]`), `NoEnum` (no sibling enum),
        // `Split.a.default: solo` (property `b`'s enum sits past the dedent, never
        // pairs), and `NamedDefault` (a `default:` opening a block has no inline
        // scalar to compare).
        assert_eq!(defaults_outside_their_enum(body), vec![21, 30]);

        // Non-vacuous floor: across every registered spec every default with a
        // sibling enum is a member of it (the invariant the contract test asserts),
        // and the corpus actually declares several enum-bearing defaults (e.g. an
        // `order` param, status enums), so the membership path runs on real data
        // and a broken (always-empty) extractor can't hide behind a corpus that
        // never pairs a default with an enum. Count pairs with a window detector
        // independent of the extractor's membership comparison.
        let mut pairs = 0usize;
        for api in APIS {
            assert!(
                defaults_outside_their_enum(api.body).is_empty(),
                "{}: every default with a sibling enum must be one of its values",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                let t = l.trim_start();
                if !t.starts_with("default:") {
                    continue;
                }
                let v = t["default:".len()..].trim();
                if v.is_empty() || v.starts_with('#') {
                    continue;
                }
                let c = indent(l);
                let lo = i.saturating_sub(8);
                let hi = (i + 8).min(lines.len());
                let has_enum = (lo..hi).any(|j| {
                    j != i && indent(lines[j]) == c && lines[j].trim_start().starts_with("enum:")
                });
                if has_enum {
                    pairs += 1;
                }
            }
        }
        assert!(
            pairs >= 4,
            "expected several default+enum sibling pairs across specs, got {pairs}"
        );
    }

    /// Returns the 1-based line numbers of every `example:` scalar whose schema
    /// object also declares a sibling `enum:` list that does **not** contain the
    /// example's value — the `example` analogue of `defaults_outside_their_enum`.
    ///
    /// An `example` is a sample *instance* of the schema, so beside an `enum` —
    /// the closed set the field may take — it MUST be one of the enum's values. A
    /// non-member is a sample the field's own validator rejects: a Redoc/Swagger
    /// "try it" prefill and a codegen client's generated sample carry a value the
    /// enum can never legally hold.
    ///
    /// Pure and YAML-dep-free; the body mirrors `defaults_outside_their_enum`
    /// exactly (only the pivot key differs — `example:` for `default:`): for each
    /// `example:` key with an inline scalar value at indent `c`, scan its object's
    /// block (down then up, each bounded by the first line indented *below* `c`,
    /// the dedent that closes the object) for an `enum:` key at *exactly* `c`. When
    /// one is found, collect that enum's values (the flow `enum: [A, B]` and block
    /// `enum:`/`- A` forms, unquoted and comment-trimmed) and flag the `example`
    /// when its normalized value is absent from them. The exact-indent,
    /// dedent-bounded match keeps an `example` in one property from pairing with a
    /// following sibling property's `enum`. Skipped (nothing to compare, or not a
    /// Schema Object example): an `example:` that opens a block rather than holding
    /// an inline scalar (an object/array example, or a property literally named
    /// `example`), an `example` with no sibling `enum` (a Media Type / Parameter
    /// Object example, or an unconstrained schema example — always valid), and an
    /// `enum:` sibling that is a mapping named `enum` rather than a value list (its
    /// collected value set is empty).
    fn examples_outside_their_enum(body: &str) -> Vec<usize> {
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
        // The inline scalar value of a `name:` key. `None` when the line is a
        // different key or opens a block (no inline value after the colon).
        let inline_val = |l: &str, name: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != name {
                return None;
            }
            let n = norm(v);
            if n.is_empty() {
                None
            } else {
                Some(n)
            }
        };
        // The values of the enum whose `enum:` key sits at line index `e`.
        let enum_values_at = |e: usize| -> Vec<String> {
            let line = lines[e];
            let rest = line.trim_start()["enum:".len()..].trim_start();
            if rest.starts_with('[') {
                // Flow list — gather across lines until the closing `]`.
                let mut buf = rest.to_string();
                let mut k = e;
                while !buf.contains(']') && k + 1 < lines.len() {
                    k += 1;
                    buf.push(' ');
                    buf.push_str(lines[k].trim());
                }
                let open = buf.find('[').map(|x| x + 1).unwrap_or(0);
                let close = buf.rfind(']').unwrap_or(buf.len());
                let inner = if close >= open { &buf[open..close] } else { "" };
                if inner.trim().is_empty() {
                    Vec::new()
                } else {
                    inner.split(',').map(|s| norm(s)).filter(|v| !v.is_empty()).collect()
                }
            } else if rest.is_empty() || rest.starts_with('#') {
                // Block list — `- ` children at a deeper indent, but only when the
                // first child is a `-` item (else it is a property named `enum`).
                let base = indent(line);
                let mut values: Vec<String> = Vec::new();
                let mut first_child_seen = false;
                let mut j = e + 1;
                while j < lines.len() {
                    let l = lines[j];
                    if l.trim().is_empty() || l.trim_start().starts_with('#') {
                        j += 1;
                        continue;
                    }
                    if indent(l) <= base {
                        break;
                    }
                    let item = l.trim_start();
                    if !first_child_seen {
                        first_child_seen = true;
                        if !item.starts_with('-') {
                            break;
                        }
                    }
                    if !item.starts_with('-') {
                        break;
                    }
                    let val = norm(item[1..].trim_start());
                    if !val.is_empty() {
                        values.push(val);
                    }
                    j += 1;
                }
                values
            } else {
                Vec::new()
            }
        };
        let is_enum_key =
            |l: &str| l.trim_start().starts_with("enum:") && !l.trim_start().starts_with("enums");
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(ev) = inline_val(line, "example") else { continue };
            let c = indent(line);
            let mut e = None;
            // Scan down through this object's block for a sibling `enum:`.
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c && is_enum_key(l) {
                    e = Some(j);
                    break;
                }
                j += 1;
            }
            // The enum may be declared before the example; scan up too.
            if e.is_none() {
                let mut k = i;
                while k > 0 {
                    k -= 1;
                    let l = lines[k];
                    if l.trim().is_empty() {
                        continue;
                    }
                    if indent(l) < c {
                        break;
                    }
                    if indent(l) == c && is_enum_key(l) {
                        e = Some(k);
                        break;
                    }
                }
            }
            let Some(e) = e else { continue };
            let values = enum_values_at(e);
            if !values.is_empty() && !values.iter().any(|v| v == &ev) {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_example_is_a_member_of_its_enum() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares BOTH an `example` and an `enum`, the
        // example MUST be one of the enum's values. An `enum` fixes the closed set
        // a field may take; an `example` outside that set advertises a sample the
        // field's own validator rejects, so a Redoc/Swagger "try it" prefill and a
        // codegen client's generated sample carry a value the field can never
        // legally hold.
        //
        // A routine hazard in these scenario-table-heavy specs, where enum/example
        // pairs are hand-authored per API (a `connectedNetworkType` `5G`, a
        // `qosStatus` `AVAILABLE`, a security-mode `WPA3-Enterprise`): an example
        // typed from memory, or an enum member renamed after the example was set,
        // leaves the two disagreeing. It is the `example` analogue of
        // `every_default_is_a_member_of_its_enum` (which pins the *default* against
        // its enum) and invisible to every other existing test — the enum test
        // checks a value list's own members (unique/non-empty),
        // `every_example_matches_its_schema_type` checks the example's JSON *type*
        // not its enum membership, and the identity/wiring/`$ref` tests never
        // compare an example against its enum. Verified true across all mounted
        // specs before asserting.
        for api in APIS {
            let offenders = examples_outside_their_enum(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec declares an `example` outside its sibling `enum` (a value \
                 the enum's own validator would reject) at line(s): {:?}",
                api.name,
                offenders
            );
        }
    }

    #[test]
    fn example_enum_membership_extraction_rules() {
        // Unit-cover the `examples_outside_their_enum` extractor so the contract
        // test above can't pass vacuously and its detection is pinned: an `example`
        // is flagged only when a same-indent sibling `enum` (declared before OR
        // after it) omits the example's value; quotes/comments are normalized on
        // both sides before comparison; an `example` with no sibling enum (a Media
        // Type / Parameter Object example, or an unconstrained schema one) is never
        // flagged; an `example` in one property never pairs with a following
        // property's enum across the dedent; and an `example:` that opens a block
        // (an object/array example or a property literally named `example`) is
        // skipped.
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
    GoodFlow:
      type: string
      enum: [asc, desc]
      example: desc
    BadAfter:
      type: string
      enum: [red, green]
      example: blue
    GoodBefore:
      type: string
      example: left
      enum:
        - left
        - right
    BadBefore:
      type: string
      example: up
      enum:
        - left
        - right
    NoEnum:
      type: string
      example: anything
    Quoted:
      type: string
      enum: [asc, desc]
      example: \"asc\"
    Split:
      type: object
      properties:
        a:
          type: string
          example: solo
        b:
          type: string
          enum: [x, y]
    NamedExample:
      type: object
      properties:
        example:
          type: string
";
        // Flagged, in document order: line 21 (`BadAfter.example: blue`, whose
        // `enum: [red, green]` sibling above omits it) and line 30
        // (`BadBefore.example: up`, whose block `enum` below omits it). Not flagged:
        // `GoodFlow`/`GoodBefore` (example in enum, after / before it), `Quoted`
        // (`\"asc\"` normalizes into `[asc, desc]`), `NoEnum` (no sibling enum),
        // `Split.a.example: solo` (property `b`'s enum sits past the dedent, never
        // pairs), and `NamedExample` (an `example:` opening a block has no inline
        // scalar to compare).
        assert_eq!(examples_outside_their_enum(body), vec![21, 30]);

        // Non-vacuous floor: across every registered spec every example with a
        // sibling enum is a member of it (the invariant the contract test asserts),
        // and the corpus actually declares many enum-bearing examples (status
        // enums, network-type enums, security-mode enums), so the membership path
        // runs on real data and a broken (always-empty) extractor can't hide behind
        // a corpus that never pairs an example with an enum. Count pairs with a
        // window detector independent of the extractor's membership comparison.
        let mut pairs = 0usize;
        for api in APIS {
            assert!(
                examples_outside_their_enum(api.body).is_empty(),
                "{}: every example with a sibling enum must be one of its values",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                let t = l.trim_start();
                if !t.starts_with("example:") {
                    continue;
                }
                let v = t["example:".len()..].trim();
                if v.is_empty() || v.starts_with('#') {
                    continue;
                }
                let c = indent(l);
                let lo = i.saturating_sub(8);
                let hi = (i + 8).min(lines.len());
                let has_enum = (lo..hi).any(|j| {
                    j != i && indent(lines[j]) == c && lines[j].trim_start().starts_with("enum:")
                });
                if has_enum {
                    pairs += 1;
                }
            }
        }
        assert!(
            pairs >= 10,
            "expected several example+enum sibling pairs across specs, got {pairs}"
        );
    }

    /// Line numbers (1-based) of `default:` keywords whose inline scalar value
    /// contradicts the sibling scalar `type:` in the same Schema Object — the
    /// `default` analogue of `enum_values_inconsistent_with_type`, without a YAML dep.
    ///
    /// In OpenAPI 3.0.x a `default` supplies a fall-back value for the schema, so it
    /// MUST itself be a valid instance of that schema. Where the object declares a
    /// scalar `type` (string/integer/number/boolean), a default of the wrong JSON
    /// type — an unquoted `true`/`5` under `type: string` (YAML reads it as a
    /// boolean/number), a quoted or fractional value under `type: integer`, a
    /// non-numeric value under `type: number` — is a self-contradictory schema: the
    /// schema pre-supplies a value its own validator rejects.
    ///
    /// Only a `default` carrying an inline scalar *and* a sibling scalar `type:` is
    /// inspected. Skipped: a `default:` that opens a block (an object/array default,
    /// or a property literally named `default`, neither of which has an inline
    /// scalar); a `default` with no scalar `type:` sibling (e.g. a server-variable
    /// default, which is untyped); and a `default:` inside an `example:`/`examples:`
    /// payload. Quoting is significant — a quoted token is always a YAML string,
    /// whatever its inner text would parse as. A `null`/`~` default is legal for a
    /// nullable schema of any type and is not flagged.
    fn defaults_inconsistent_with_type(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The raw inline value token of a `name:` key (inline comment stripped, but
        // quoting *preserved* so a quoted scalar stays classifiable as a string);
        // `None` when the line is a different key or opens a block (no inline value).
        let raw_inline = |l: &str, name: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != name {
                return None;
            }
            let v = v.split('#').next().unwrap_or(v).trim();
            if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            }
        };
        // The sibling `type:` scalar in the same object as line `i` (indent `c`):
        // scan down through the object's block for a same-indent `type`, then up,
        // dedent-bounded so a nested/following object's `type` never pairs. Mirrors
        // `enum_values_inconsistent_with_type`'s `sibling_type`.
        let sibling_type = |i: usize, c: usize| -> Option<String> {
            let scalar_type = |l: &str| -> Option<String> {
                let (k, v) = l.trim_start().split_once(':')?;
                if k.trim() != "type" {
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
                    None
                } else {
                    Some(v.to_string())
                }
            };
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar_type(l) {
                        return Some(v);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar_type(l) {
                        return Some(v);
                    }
                }
            }
            None
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`. Mirrors `enum_values_inconsistent_with_type`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // Whether the raw (as-written) default token `raw` contradicts scalar type
        // `ty`. Quoting is significant: a quoted token is always a YAML string,
        // whatever its inner text would otherwise parse as. Shares the classification
        // rules of `enum_values_inconsistent_with_type::inconsistent`.
        fn inconsistent(raw: &str, ty: &str) -> bool {
            let v = raw.trim();
            if v.is_empty() || v == "null" || v == "~" {
                return false; // JSON null is legal for a nullable schema of any type
            }
            let quoted = v.len() >= 2
                && ((v.starts_with('"') && v.ends_with('"'))
                    || (v.starts_with('\'') && v.ends_with('\'')));
            let is_bool = !quoted
                && matches!(v, "true" | "false" | "True" | "False" | "TRUE" | "FALSE");
            let is_int = !quoted && v.parse::<i64>().is_ok();
            let is_num = !quoted && v.parse::<f64>().is_ok();
            match ty {
                "string" => is_bool || is_num, // an unquoted bool/number is not a string
                "boolean" => !is_bool,
                "integer" => !is_int,
                "number" => !is_num,
                _ => false,
            }
        }
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(raw) = raw_inline(line, "default") else {
                continue;
            };
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            let Some(ty) = sibling_type(i, c) else {
                continue;
            };
            if !matches!(ty.as_str(), "string" | "integer" | "number" | "boolean") {
                continue;
            }
            if inconsistent(&raw, &ty) {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_default_matches_its_schema_type() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares a `default` beside a scalar `type`, the
        // default MUST conform to that type. A `default` is a fall-back *instance* of
        // the schema, so a value of the wrong JSON type — an unquoted `true`/`5`
        // under `type: string` (YAML reads it as a boolean/number), a quoted or
        // fractional value under `type: integer`, a non-numeric value under
        // `type: number`, a non-boolean under `type: boolean` — is a
        // self-contradictory schema: the schema pre-supplies a value its own
        // validator would reject, so a Redoc/Swagger form pre-fills a control with a
        // value the field can never legally hold and a codegen client's default fails
        // the type's own check at the point a caller reads or builds it.
        //
        // This is the `default` analogue of `every_enum_value_matches_its_schema_type`
        // (which checks enum *members* against a scalar type) and the type-conformance
        // complement of `every_default_is_a_member_of_its_enum` (which checks a
        // default against a sibling *enum*, but only when one is present — a default
        // on a plain typed schema with no enum escapes it entirely). No existing test
        // compares a `default`'s value against its own `type`. Only a default with a
        // scalar `type:` sibling (string/integer/number/boolean) in the same object is
        // inspected; an untyped default (e.g. a server variable's), a non-scalar
        // sibling type, a `null`/`~` default, a property named `default`, and a
        // `default:` inside an `example:` payload are skipped. Verified true across
        // all mounted specs before asserting.
        for api in APIS {
            let bad = defaults_inconsistent_with_type(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a `default` that contradicts its sibling scalar \
                 `type:` (e.g. an unquoted bool/number under `type: string`, a quoted \
                 or fractional value under `type: integer`) at `default:` line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn default_type_consistency_extraction_rules() {
        // Unit-cover `defaults_inconsistent_with_type` so the contract test above
        // can't pass vacuously and its detection is pinned: string/integer/boolean/
        // number defaults that match their type all pass; a quoted numeric default
        // under `type: string` passes (quoting makes it a string); an unquoted `true`
        // under `type: string`, a quoted `'1'` and a fractional `2.5` under
        // `type: integer`, and a non-numeric default under `type: number` are flagged
        // in document order; a typeless default, a property literally named `default`,
        // a `default:` inside an `example:` payload, a `null` default of a nullable
        // schema, and a default whose only same-indent `type` sits in a following
        // property across a dedent are all skipped.
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
    GoodStr:
      type: string
      default: desc
    GoodInt:
      type: integer
      default: 10
    GoodBool:
      type: boolean
      default: false
    GoodNum:
      type: number
      default: 1.5
    GoodStrQuotedNum:
      type: string
      default: '5'
    BadStrBool:
      type: string
      default: true
    BadIntQuoted:
      type: integer
      default: '1'
    BadIntFrac:
      type: integer
      default: 2.5
    BadNumText:
      type: number
      default: notanumber
    NoType:
      default: anything
    NamedDefault:
      type: object
      properties:
        default:
          type: string
    InExample:
      type: object
      example:
        type: integer
        default: hello
    Nullable:
      type: integer
      nullable: true
      default: null
    Split:
      type: object
      properties:
        a:
          default: solo
        b:
          type: integer
";
        // Flagged, in document order: line 31 (`BadStrBool.default: true` — a YAML
        // boolean, not a string), line 34 (`BadIntQuoted.default: '1'` — a quoted
        // string, not an integer), line 37 (`BadIntFrac.default: 2.5` — fractional,
        // not an integer) and line 40 (`BadNumText.default: notanumber` — not a
        // number). Not flagged: the four Good schemas; `GoodStrQuotedNum` (`'5'` is a
        // quoted string under `type: string`); `NoType` (no sibling type); the
        // property literally named `default` (opens a block, no inline scalar); the
        // `default:` inside the `example:` payload; `Nullable` (its `null` default is
        // legal for any nullable type); and `Split.a.default: solo`, whose only
        // candidate `type: integer` sits in the following property `Split.b` past a
        // dedent, so the two never pair.
        assert_eq!(defaults_inconsistent_with_type(body), vec![31, 34, 37, 40]);

        // Non-vacuous floor: across every registered spec every scalar-typed default
        // conforms to its type (the invariant the contract test asserts), and the
        // corpus actually declares many typed defaults (a `maxAge`, a page size, a
        // boolean opt-in flag, a status enum's default) — so the value-comparison
        // path runs on real data and a broken (always-empty) extractor can't hide
        // behind a corpus that never pairs a default with a scalar type. Count typed
        // defaults with a presence detector independent of the value comparison: a
        // `default:` inline scalar whose same-indent object declares a scalar `type:`.
        let mut typed_defaults = 0usize;
        for api in APIS {
            assert!(
                defaults_inconsistent_with_type(api.body).is_empty(),
                "{}: every scalar-typed default must conform to its type",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let scalar_type_line = |l: &str| {
                matches!(
                    l.trim_start(),
                    "type: string" | "type: integer" | "type: number" | "type: boolean"
                )
            };
            for (i, l) in lines.iter().enumerate() {
                let t = l.trim_start();
                let Some((k, v)) = t.split_once(':') else {
                    continue;
                };
                if k.trim() != "default" || v.split('#').next().unwrap_or(v).trim().is_empty() {
                    continue;
                }
                let c = indent(l);
                // Same-indent scalar `type:` sibling, scanning down then up
                // (dedent-bounded), independent of the extractor's classification.
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
                    if indent(x) == c && scalar_type_line(x) {
                        has = true;
                        break;
                    }
                    j += 1;
                }
                if !has {
                    let mut m = i;
                    while m > 0 {
                        m -= 1;
                        let x = lines[m];
                        if x.trim().is_empty() {
                            continue;
                        }
                        if indent(x) < c {
                            break;
                        }
                        if indent(x) == c && scalar_type_line(x) {
                            has = true;
                            break;
                        }
                    }
                }
                if has {
                    typed_defaults += 1;
                }
            }
        }
        assert!(
            typed_defaults >= 10,
            "expected many scalar-typed defaults across specs, got {typed_defaults}"
        );
    }

    /// The 1-based line numbers, in document order, of every schema-level `example:`
    /// whose inline scalar value contradicts its sibling scalar `type:` — without a
    /// YAML dep.
    ///
    /// An OpenAPI Schema Object's `example` is a sample *instance* of that schema, so
    /// like a `default` it must conform to the schema's own `type`. A value of the
    /// wrong JSON type — an unquoted `true`/`5` under `type: string` (YAML reads it as
    /// a boolean/number, not a string), a quoted or fractional value under
    /// `type: integer`, a non-numeric value under `type: number`, a non-boolean under
    /// `type: boolean` — is a self-contradictory schema: the sample the field advertises
    /// is one the type's own validator would reject, so a Redoc/Swagger "try it" prefill
    /// and a codegen client's generated sample carry a value the field can never legally
    /// hold.
    ///
    /// This is the `example` analogue of `defaults_inconsistent_with_type` — same
    /// classification (`inconsistent`) and same same-indent, dedent-bounded sibling
    /// `type:` scan (down through the object's block then up) — so only a schema-level
    /// `example` with a scalar `type:` sibling (`string`/`integer`/`number`/`boolean`)
    /// in the *same object* is inspected. That scan is what confines the check to
    /// Schema Object examples: a **Media Type Object** or **Parameter Object** `example`
    /// (whose siblings are `schema`/`examples`, never a same-indent `type`) finds no
    /// sibling type and is skipped, as is an example inherited via `allOf`/`$ref`. Also
    /// skipped: an `example:` that opens a block (an object/array example, so no inline
    /// scalar to classify), a `null`/`~` example (JSON null is legal for a nullable
    /// schema of any type), a property literally named `example`, and an `example:`
    /// nested inside another `example:`/`examples:` payload (the `inside_example` walk,
    /// checked before the type scan — so an example object that itself contains a
    /// `type`/`example` pair never mispairs). A quoted value is a string regardless of
    /// what its unquoted text would parse as, so quoting is preserved before
    /// classification.
    fn examples_inconsistent_with_type(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The raw inline value token of a `name:` key (inline comment stripped, quoting
        // *preserved* so a quoted scalar stays classifiable as a string); `None` when
        // the line is a different key or opens a block (no inline value).
        let raw_inline = |l: &str, name: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != name {
                return None;
            }
            let v = v.split('#').next().unwrap_or(v).trim();
            if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            }
        };
        // The sibling `type:` scalar in the same object as line `i` (indent `c`): scan
        // down through the object's block for a same-indent `type`, then up,
        // dedent-bounded so a nested/following object's `type` never pairs. Mirrors
        // `defaults_inconsistent_with_type`.
        let sibling_type = |i: usize, c: usize| -> Option<String> {
            let scalar_type = |l: &str| -> Option<String> {
                let (k, v) = l.trim_start().split_once(':')?;
                if k.trim() != "type" {
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
                    None
                } else {
                    Some(v.to_string())
                }
            };
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar_type(l) {
                        return Some(v);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar_type(l) {
                        return Some(v);
                    }
                }
            }
            None
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:` payload
        // — some enclosing container key up the indent ladder is `example`/`examples`.
        // Mirrors `defaults_inconsistent_with_type`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // Whether the raw (as-written) example token `raw` contradicts scalar type `ty`.
        // Quoting is significant: a quoted token is always a YAML string, whatever its
        // inner text would otherwise parse as. Shares the classification rules of
        // `defaults_inconsistent_with_type::inconsistent`.
        fn inconsistent(raw: &str, ty: &str) -> bool {
            let v = raw.trim();
            if v.is_empty() || v == "null" || v == "~" {
                return false; // JSON null is legal for a nullable schema of any type
            }
            let quoted = v.len() >= 2
                && ((v.starts_with('"') && v.ends_with('"'))
                    || (v.starts_with('\'') && v.ends_with('\'')));
            let is_bool = !quoted
                && matches!(v, "true" | "false" | "True" | "False" | "TRUE" | "FALSE");
            let is_int = !quoted && v.parse::<i64>().is_ok();
            let is_num = !quoted && v.parse::<f64>().is_ok();
            match ty {
                "string" => is_bool || is_num, // an unquoted bool/number is not a string
                "boolean" => !is_bool,
                "integer" => !is_int,
                "number" => !is_num,
                _ => false,
            }
        }
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(raw) = raw_inline(line, "example") else {
                continue;
            };
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            let Some(ty) = sibling_type(i, c) else {
                continue;
            };
            if !matches!(ty.as_str(), "string" | "integer" | "number" | "boolean") {
                continue;
            }
            if inconsistent(&raw, &ty) {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_example_matches_its_schema_type() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares an `example` beside a scalar `type`, the
        // example MUST conform to that type. An `example` is a sample *instance* of the
        // schema, so a value of the wrong JSON type — an unquoted `true`/`5` under
        // `type: string` (YAML reads it as a boolean/number), a quoted or fractional
        // value under `type: integer`, a non-numeric value under `type: number`, a
        // non-boolean under `type: boolean` — is a self-contradictory schema: the
        // sample the field advertises is one its own type would reject, so a
        // Redoc/Swagger "try it" prefill and a codegen client's generated sample carry
        // a value the field can never legally hold.
        //
        // This is the `example` analogue of `every_default_matches_its_schema_type`
        // (the `default` value against its scalar type) and of
        // `every_enum_value_matches_its_schema_type` (an enum's *members* against a
        // scalar type). No existing test compares an `example`'s value against its own
        // `type`: `no_object_declares_both_example_and_examples` checks how a sample is
        // *expressed* (never its value), and the type/format tests check the `type`
        // token or a `format` modifier, never the example a type constrains. Only a
        // schema-level example with a scalar `type:` sibling is inspected — a Media
        // Type / Parameter Object example (no same-indent `type`), an example inherited
        // via `allOf`/`$ref`, a block (object/array) example, a `null`/`~` example, a
        // property named `example`, and an example nested inside another example
        // payload are skipped. Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = examples_inconsistent_with_type(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a schema `example` that contradicts its sibling scalar \
                 `type:` (e.g. an unquoted bool/number under `type: string`, a quoted or \
                 fractional value under `type: integer`) at `example:` line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn example_type_consistency_extraction_rules() {
        // Unit-cover `examples_inconsistent_with_type` so the contract test above can't
        // pass vacuously and its detection is pinned: string/integer/boolean/number
        // examples that match their type all pass; a quoted numeric example under
        // `type: string` passes (quoting makes it a string); an unquoted `true` under
        // `type: string`, a quoted `'1'` and a fractional `2.5` under `type: integer`,
        // and a non-numeric example under `type: number` are flagged in document order;
        // a Media Type Object example (no same-indent `type`), a typeless example, a
        // property literally named `example`, an `example:` inside another example
        // payload, a `null` example of a nullable schema, and an example whose only
        // same-indent `type` sits in a following property across a dedent are all
        // skipped.
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
          content:
            application/json:
              schema:
                type: string
              example: 42
components:
  schemas:
    GoodStr:
      type: string
      example: desc
    GoodInt:
      type: integer
      example: 10
    GoodBool:
      type: boolean
      example: false
    GoodNum:
      type: number
      example: 1.5
    GoodStrQuotedNum:
      type: string
      example: '5'
    BadStrBool:
      type: string
      example: true
    BadIntQuoted:
      type: integer
      example: '1'
    BadIntFrac:
      type: integer
      example: 2.5
    BadNumText:
      type: number
      example: notanumber
    NoType:
      example: anything
    NamedExample:
      type: object
      properties:
        example:
          type: string
    InExample:
      type: object
      example:
        type: integer
        example: hi
    Nullable:
      type: string
      example: null
    Split:
      type: object
      properties:
        a:
          example: solo
        b:
          type: integer
";
        // Flagged, in document order: line 36 (`BadStrBool.example: true` — a YAML
        // boolean, not a string), line 39 (`BadIntQuoted.example: '1'` — a quoted
        // string, not an integer), line 42 (`BadIntFrac.example: 2.5` — fractional, not
        // an integer) and line 45 (`BadNumText.example: notanumber` — not a number).
        // Not flagged: the Media Type Object `example: 42` at line 16 (its siblings are
        // `schema`, no same-indent `type`); the four Good schemas; `GoodStrQuotedNum`
        // (`'5'` is a quoted string under `type: string`); `NoType` (no sibling type);
        // the property literally named `example` (opens a block, no inline scalar); the
        // `example: hi` inside the `example:` payload (an enclosing `example` key);
        // `Nullable` (its `null` example is legal for any nullable type); and
        // `Split.a.example: solo`, whose only candidate `type: integer` sits in the
        // following property `Split.b` past a dedent, so the two never pair.
        assert_eq!(examples_inconsistent_with_type(body), vec![36, 39, 42, 45]);

        // Non-vacuous floor: across every registered spec every scalar-typed example
        // conforms to its type (the invariant the contract test asserts), and the
        // corpus actually declares many typed examples (an E.164 `phoneNumber`, a port
        // number, a latitude, a boolean flag) — so the value-comparison path runs on
        // real data and a broken (always-empty) extractor can't hide behind a corpus
        // that never pairs an example with a scalar type. Count typed examples with a
        // presence detector independent of the value comparison: an `example:` inline
        // scalar whose same-indent object declares a scalar `type:` and which is not
        // itself inside an `example:`/`examples:` payload.
        let mut typed_examples = 0usize;
        for api in APIS {
            assert!(
                examples_inconsistent_with_type(api.body).is_empty(),
                "{}: every scalar-typed example must conform to its type",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let scalar_type_line = |l: &str| {
                matches!(
                    l.trim_start(),
                    "type: string" | "type: integer" | "type: number" | "type: boolean"
                )
            };
            for (i, l) in lines.iter().enumerate() {
                let t = l.trim_start();
                let Some((k, v)) = t.split_once(':') else {
                    continue;
                };
                if k.trim() != "example" || v.split('#').next().unwrap_or(v).trim().is_empty() {
                    continue;
                }
                let c = indent(l);
                // Skip an example nested inside another example payload (an enclosing
                // `example`/`examples` key up the indent ladder).
                let mut in_ex = false;
                {
                    let mut level = c;
                    let mut m = i;
                    while m > 0 {
                        m -= 1;
                        let x = lines[m];
                        if x.trim().is_empty() {
                            continue;
                        }
                        let li = indent(x);
                        if li < level {
                            if let Some((key, _)) = x.trim_start().split_once(':') {
                                let key = key.trim();
                                if key == "example" || key == "examples" {
                                    in_ex = true;
                                    break;
                                }
                            }
                            level = li;
                            if li == 0 {
                                break;
                            }
                        }
                    }
                }
                if in_ex {
                    continue;
                }
                // Same-indent scalar `type:` sibling, scanning down then up
                // (dedent-bounded), independent of the extractor's classification.
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
                    if indent(x) == c && scalar_type_line(x) {
                        has = true;
                        break;
                    }
                    j += 1;
                }
                if !has {
                    let mut m = i;
                    while m > 0 {
                        m -= 1;
                        let x = lines[m];
                        if x.trim().is_empty() {
                            continue;
                        }
                        if indent(x) < c {
                            break;
                        }
                        if indent(x) == c && scalar_type_line(x) {
                            has = true;
                            break;
                        }
                    }
                }
                if has {
                    typed_examples += 1;
                }
            }
        }
        assert!(
            typed_examples >= 20,
            "expected many scalar-typed examples across specs, got {typed_examples}"
        );
    }

    /// The 1-based line numbers, in document order, of every `default:` keyword whose
    /// inline numeric value falls outside a sibling numeric bound — `minimum` or
    /// `maximum` — declared in the same Schema Object, without a YAML dep.
    ///
    /// In OpenAPI 3.0.x (JSON Schema) a `default` is a fall-back *instance* of the
    /// schema, so it MUST satisfy the schema's own constraints. Where the object bounds
    /// a numeric value with `minimum`/`maximum`, a numeric `default` below the `minimum`
    /// or above the `maximum` is a self-contradictory schema: the schema pre-supplies a
    /// value its own validator rejects, so a Redoc/Swagger form pre-fills a control with
    /// an out-of-range value and a codegen client's default fails the bound's own check
    /// exactly where a caller reads or builds the payload.
    ///
    /// Only a `default` carrying an inline *unquoted numeric* scalar and at least one
    /// same-object numeric bound sibling is inspected; each bound is scanned at the
    /// default's own indent, down through the object's block then up, dedent-bounded
    /// exactly like `schema_bounds_inverted` / `defaults_inconsistent_with_type`, so a
    /// nested or following sibling object's bound never pairs. Skipped: a `default:`
    /// that opens a block (an object/array default, or a property literally named
    /// `default`); a quoted or non-numeric default (a numeric bound constrains only
    /// numbers — a mistyped default is `defaults_inconsistent_with_type`'s concern); a
    /// default with no numeric bound sibling; and a `default:` inside an
    /// `example:`/`examples:` payload. The comparison is inclusive — only a value
    /// strictly below `minimum` or strictly above `maximum` is flagged — so an
    /// exclusive-bound (`exclusiveMinimum`/`exclusiveMaximum`) equality edge is never a
    /// false positive (that strictness refinement is out of scope).
    fn defaults_outside_their_numeric_bounds(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a `name:` key (inline comment stripped; surrounding
        // quotes preserved so a quoted token stays distinguishable from a bare number);
        // `None` when the line is a different key or opens a block (no inline value).
        let raw_inline = |l: &str, name: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != name {
                return None;
            }
            let v = v.split('#').next().unwrap_or(v).trim();
            if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            }
        };
        // A same-indent numeric bound sibling `key` in the same object as line `i`
        // (indent `c`): scan down through the object's block then up, dedent-bounded so
        // a nested or following object's bound never pairs. Returns the parsed number
        // only for an unquoted numeric scalar (a quoted or non-numeric bound has no
        // magnitude to compare against and is treated as absent here).
        let sibling_num = |i: usize, c: usize, key: &str| -> Option<f64> {
            let parse_num = |l: &str| -> Option<f64> {
                let raw = raw_inline(l, key)?;
                if raw.starts_with('"') || raw.starts_with('\'') {
                    return None; // quoted → not a number
                }
                raw.parse::<f64>().ok()
            };
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(n) = parse_num(l) {
                        return Some(n);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(n) = parse_num(l) {
                        return Some(n);
                    }
                }
            }
            None
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:` payload
        // — some enclosing container key up the indent ladder is `example`/`examples`
        // (mirroring `defaults_inconsistent_with_type`).
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(raw) = raw_inline(line, "default") else {
                continue;
            };
            // Only an unquoted numeric default can violate a numeric bound; a quoted or
            // non-numeric default is `defaults_inconsistent_with_type`'s concern.
            if raw.starts_with('"') || raw.starts_with('\'') {
                continue;
            }
            let Ok(val) = raw.parse::<f64>() else {
                continue;
            };
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            let min = sibling_num(i, c, "minimum");
            let max = sibling_num(i, c, "maximum");
            if min.is_none() && max.is_none() {
                continue;
            }
            let below = min.is_some_and(|m| val < m);
            let above = max.is_some_and(|m| val > m);
            if below || above {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_default_is_within_its_numeric_bounds() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares a numeric `default` beside a `minimum` and/or
        // `maximum`, the default MUST lie within those bounds. A `default` is a
        // fall-back *instance* of the schema, so a value below the `minimum` or above
        // the `maximum` — a `default: 0` under `minimum: 1`, a `default: 300` under
        // `maximum: 240` — is a self-contradictory schema: the schema pre-supplies a
        // value its own validator rejects, so a Redoc/Swagger form pre-fills a control
        // with an out-of-range value and a codegen client's default fails the bound's
        // own check at the point a caller reads or builds the payload.
        //
        // A routine hazard in these scenario-table-heavy specs, where a bounded,
        // defaulted control param is hand-tuned per API (a `maxAge` 1..2400 default
        // 240, a page size 1..100 default 20): a bound narrowed after the default was
        // set, or a default pasted from a sibling with a different range, leaves the two
        // disagreeing. It is the numeric-range complement of
        // `every_default_is_a_member_of_its_enum` (which checks a default against a
        // sibling *enum*) and `every_default_matches_its_schema_type` (which checks a
        // default's *type*, never its magnitude); the numeric-bound test
        // (`every_numeric_bound_is_ordered_low_to_high`) compares the two bounds to each
        // other but never against a default. No existing test compares a `default`'s
        // value against its own bounds. Verified true across all mounted specs before
        // asserting.
        for api in APIS {
            let offenders = defaults_outside_their_numeric_bounds(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec declares a numeric `default` outside its sibling `minimum`/\
                 `maximum` bound (a value the bound's own validator would reject) at \
                 `default:` line(s): {:?}",
                api.name,
                offenders
            );
        }
    }

    #[test]
    fn default_numeric_bound_extraction_rules() {
        // Unit-cover `defaults_outside_their_numeric_bounds` so the contract test above
        // can't pass vacuously and its detection is pinned: a default within its bounds
        // passes; a default below a `minimum` (declared above it) and one above a
        // `maximum` (declared above it) are flagged in document order; a default equal
        // to a bound passes (inclusive); a `minimum` declared *below* the default is
        // still paired (down-scan); a quoted or non-numeric default is skipped (nothing
        // numeric to compare); a default with no bound sibling is skipped; a `default:`
        // inside an `example:` payload is skipped; a default in one property never pairs
        // with a following property's bound across the dedent; and a `default:` opening
        // a block (a property literally named `default`) is skipped.
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
    GoodRange:
      type: integer
      minimum: 1
      maximum: 10
      default: 5
    BadBelow:
      type: integer
      minimum: 100
      default: 1
    BadAbove:
      type: integer
      maximum: 10
      default: 99
    MinOnlyGood:
      type: integer
      default: 7
      minimum: 1
    Equal:
      type: integer
      minimum: 5
      maximum: 5
      default: 5
    Quoted:
      type: string
      minimum: 1
      default: '0'
    NonNumeric:
      type: string
      minimum: 1
      default: hello
    NoBound:
      type: integer
      default: 42
    InExample:
      type: object
      example:
        minimum: 100
        default: 1
    Split:
      type: object
      properties:
        a:
          default: 0
        b:
          type: integer
          minimum: 100
    NamedDefault:
      type: object
      properties:
        default:
          type: integer
          minimum: 100
";
        // Flagged, in document order: line 22 (`BadBelow.default: 1` < its
        // `minimum: 100` sibling above) and line 26 (`BadAbove.default: 99` > its
        // `maximum: 10` sibling above). Not flagged: `GoodRange` (5 in [1,10]);
        // `MinOnlyGood` (7 >= a `minimum: 1` declared *below* it — down-scan);
        // `Equal` (5 == both bounds, inclusive); `Quoted` (`'0'` is a quoted string,
        // not a number, though 0 < 1); `NonNumeric` (`hello` isn't numeric);
        // `NoBound` (no bound sibling); `InExample` (its `default: 1` sits inside the
        // `example:` payload); `Split.a.default: 0`, whose only candidate `minimum: 100`
        // sits in the following property `Split.b` past a dedent, so the two never pair;
        // and `NamedDefault` (a `default:` opening a block has no inline scalar).
        assert_eq!(defaults_outside_their_numeric_bounds(body), vec![22, 26]);

        // Non-vacuous floor: across every registered spec every numeric default with a
        // sibling bound lies within it (the invariant the contract test asserts), and
        // the corpus actually declares several bounded defaults (a `maxAge`, a page
        // size, an array-window cap) — so the magnitude-comparison path runs on real
        // data and a broken (always-empty) extractor can't hide behind a corpus that
        // never pairs a default with a bound. Count pairs with a window detector
        // independent of the extractor's magnitude comparison.
        let mut bounded_defaults = 0usize;
        for api in APIS {
            assert!(
                defaults_outside_their_numeric_bounds(api.body).is_empty(),
                "{}: every numeric default must lie within its sibling min/max bound",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let is_num_key = |l: &str, name: &str| {
                l.trim_start().split_once(':').is_some_and(|(k, v)| {
                    k.trim() == name
                        && v.split('#')
                            .next()
                            .unwrap_or(v)
                            .trim()
                            .parse::<f64>()
                            .is_ok()
                })
            };
            for (i, l) in lines.iter().enumerate() {
                if !is_num_key(l, "default") {
                    continue;
                }
                let c = indent(l);
                let lo = i.saturating_sub(8);
                let hi = (i + 8).min(lines.len());
                let has_bound = (lo..hi).any(|j| {
                    j != i
                        && indent(lines[j]) == c
                        && (is_num_key(lines[j], "minimum") || is_num_key(lines[j], "maximum"))
                });
                if has_bound {
                    bounded_defaults += 1;
                }
            }
        }
        assert!(
            bounded_defaults >= 3,
            "expected several numeric default+bound sibling pairs across specs, got {bounded_defaults}"
        );
    }

    /// The 1-based line numbers, in document order, of every `example:` keyword whose
    /// inline numeric value falls outside a sibling numeric bound — `minimum` or
    /// `maximum` — declared in the same Schema Object, without a YAML dep. The
    /// `example` analogue of `defaults_outside_their_numeric_bounds` (only the pivot
    /// key differs — `example:` for `default:`).
    ///
    /// In OpenAPI 3.0.x (JSON Schema) an `example` is a sample *instance* of the
    /// schema, so it MUST satisfy the schema's own constraints. Where the object bounds
    /// a numeric value with `minimum`/`maximum`, a numeric `example` below the `minimum`
    /// or above the `maximum` is a self-contradictory schema: the schema advertises a
    /// sample its own validator rejects, so a Redoc/Swagger "try it" prefill and a
    /// codegen client's generated sample carry a value the bound can never legally hold.
    ///
    /// Only an `example` carrying an inline *unquoted numeric* scalar and at least one
    /// same-object numeric bound sibling is inspected; each bound is scanned at the
    /// example's own indent, down through the object's block then up, dedent-bounded
    /// exactly like `defaults_outside_their_numeric_bounds`, so a nested or following
    /// sibling object's bound never pairs. Skipped: an `example:` that opens a block (an
    /// object/array example, or a property literally named `example`); a quoted or
    /// non-numeric example (a numeric bound constrains only numbers — a mistyped example
    /// is `examples_inconsistent_with_type`'s concern); an example with no numeric bound
    /// sibling; and an `example:` nested inside an outer `example:`/`examples:` payload
    /// (example data whose inner `example` key is not a schema keyword). The comparison
    /// is inclusive — only a value strictly below `minimum` or strictly above `maximum`
    /// is flagged — so an exclusive-bound equality edge is never a false positive.
    fn examples_outside_their_numeric_bounds(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a `name:` key (inline comment stripped; surrounding
        // quotes preserved so a quoted token stays distinguishable from a bare number);
        // `None` when the line is a different key or opens a block (no inline value).
        let raw_inline = |l: &str, name: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != name {
                return None;
            }
            let v = v.split('#').next().unwrap_or(v).trim();
            if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            }
        };
        // A same-indent numeric bound sibling `key` in the same object as line `i`
        // (indent `c`): scan down through the object's block then up, dedent-bounded so
        // a nested or following object's bound never pairs. Returns the parsed number
        // only for an unquoted numeric scalar (a quoted or non-numeric bound has no
        // magnitude to compare against and is treated as absent here).
        let sibling_num = |i: usize, c: usize, key: &str| -> Option<f64> {
            let parse_num = |l: &str| -> Option<f64> {
                let raw = raw_inline(l, key)?;
                if raw.starts_with('"') || raw.starts_with('\'') {
                    return None; // quoted → not a number
                }
                raw.parse::<f64>().ok()
            };
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(n) = parse_num(l) {
                        return Some(n);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(n) = parse_num(l) {
                        return Some(n);
                    }
                }
            }
            None
        };
        // True when line `i` (indent `c`) sits inside an outer `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` (mirroring `defaults_outside_their_numeric_bounds`), so
        // an inner `example` key there is sample data, not a schema keyword.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(raw) = raw_inline(line, "example") else {
                continue;
            };
            // Only an unquoted numeric example can violate a numeric bound; a quoted or
            // non-numeric example is `examples_inconsistent_with_type`'s concern.
            if raw.starts_with('"') || raw.starts_with('\'') {
                continue;
            }
            let Ok(val) = raw.parse::<f64>() else {
                continue;
            };
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            let min = sibling_num(i, c, "minimum");
            let max = sibling_num(i, c, "maximum");
            if min.is_none() && max.is_none() {
                continue;
            }
            let below = min.is_some_and(|m| val < m);
            let above = max.is_some_and(|m| val > m);
            if below || above {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_example_is_within_its_numeric_bounds() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares a numeric `example` beside a `minimum` and/or
        // `maximum`, the example MUST lie within those bounds. An `example` is a sample
        // *instance* of the schema, so a value below the `minimum` or above the
        // `maximum` — an `example: 0` under `minimum: 1`, an `example: 300` under
        // `maximum: 240` — is a self-contradictory schema: the schema advertises a
        // sample its own validator rejects, so a Redoc/Swagger "try it" form pre-fills a
        // control with an out-of-range value and a codegen client's generated sample
        // fails the bound's own check.
        //
        // The `example` analogue of `every_default_is_within_its_numeric_bounds` — the
        // same routine hazard in these scenario-table-heavy specs, where a bounded
        // numeric param carries a hand-tuned example (a `maxAge` 1..2400 example 240, a
        // `radius`/`duration` sample) that a later bound-narrowing or a paste from a
        // sibling with a different range leaves out of range. It is invisible to its
        // example siblings: `every_example_is_a_member_of_its_enum` checks an example
        // against a sibling *enum*, `every_example_matches_its_schema_type` checks an
        // example's *type* never its magnitude, and the numeric-bound test
        // (`every_numeric_bound_is_ordered_low_to_high`) compares the two bounds to each
        // other but never against an example. No existing test compares an `example`'s
        // value against its own bounds. Verified true across all mounted specs before
        // asserting.
        for api in APIS {
            let offenders = examples_outside_their_numeric_bounds(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec declares a numeric `example` outside its sibling `minimum`/\
                 `maximum` bound (a value the bound's own validator would reject) at \
                 `example:` line(s): {:?}",
                api.name,
                offenders
            );
        }
    }

    #[test]
    fn example_numeric_bound_extraction_rules() {
        // Unit-cover `examples_outside_their_numeric_bounds` so the contract test above
        // can't pass vacuously and its detection is pinned: an example within its bounds
        // passes; an example below a `minimum` (declared above it) and one above a
        // `maximum` (declared above it) are flagged in document order; an example equal
        // to a bound passes (inclusive); a `minimum` declared *below* the example is
        // still paired (down-scan); a quoted or non-numeric example is skipped (nothing
        // numeric to compare); an example with no bound sibling is skipped; an `example:`
        // nested inside an outer `example:` payload is skipped; an example in one
        // property never pairs with a following property's bound across the dedent; and
        // an `example:` opening a block (a property literally named `example`) is skipped.
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
    GoodRange:
      type: integer
      minimum: 1
      maximum: 10
      example: 5
    BadBelow:
      type: integer
      minimum: 100
      example: 1
    BadAbove:
      type: integer
      maximum: 10
      example: 99
    MinOnlyGood:
      type: integer
      example: 7
      minimum: 1
    Equal:
      type: integer
      minimum: 5
      maximum: 5
      example: 5
    Quoted:
      type: string
      minimum: 1
      example: '0'
    NonNumeric:
      type: string
      minimum: 1
      example: hello
    NoBound:
      type: integer
      example: 42
    NestedExample:
      type: object
      example:
        minimum: 100
        example: 1
    Split:
      type: object
      properties:
        a:
          example: 0
        b:
          type: integer
          minimum: 100
    NamedExample:
      type: object
      properties:
        example:
          type: integer
          minimum: 100
";
        // Flagged, in document order: line 22 (`BadBelow.example: 1` < its
        // `minimum: 100` sibling above) and line 26 (`BadAbove.example: 99` > its
        // `maximum: 10` sibling above). Not flagged: `GoodRange` (5 in [1,10]);
        // `MinOnlyGood` (7 >= a `minimum: 1` declared *below* it — down-scan);
        // `Equal` (5 == both bounds, inclusive); `Quoted` (`'0'` is a quoted string,
        // not a number, though 0 < 1); `NonNumeric` (`hello` isn't numeric);
        // `NoBound` (no bound sibling); `NestedExample` (its inner `example: 1` sits
        // inside the outer `example:` payload); `Split.a.example: 0`, whose only
        // candidate `minimum: 100` sits in the following property `Split.b` past a
        // dedent, so the two never pair; and `NamedExample` (an `example:` opening a
        // block has no inline scalar).
        assert_eq!(examples_outside_their_numeric_bounds(body), vec![22, 26]);

        // Non-vacuous floor: across every registered spec every numeric example with a
        // sibling bound lies within it (the invariant the contract test asserts), and
        // the corpus actually declares many bounded examples (bounded `radius`,
        // `maxAge`, `duration`, page-size samples) — so the magnitude-comparison path
        // runs on real data and a broken (always-empty) extractor can't hide behind a
        // corpus that never pairs an example with a bound. Count pairs with a window
        // detector independent of the extractor's magnitude comparison.
        let mut bounded_examples = 0usize;
        for api in APIS {
            assert!(
                examples_outside_their_numeric_bounds(api.body).is_empty(),
                "{}: every numeric example must lie within its sibling min/max bound",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let is_num_key = |l: &str, name: &str| {
                l.trim_start().split_once(':').is_some_and(|(k, v)| {
                    k.trim() == name
                        && v.split('#')
                            .next()
                            .unwrap_or(v)
                            .trim()
                            .parse::<f64>()
                            .is_ok()
                })
            };
            for (i, l) in lines.iter().enumerate() {
                if !is_num_key(l, "example") {
                    continue;
                }
                let c = indent(l);
                let lo = i.saturating_sub(8);
                let hi = (i + 8).min(lines.len());
                let has_bound = (lo..hi).any(|j| {
                    j != i
                        && indent(lines[j]) == c
                        && (is_num_key(lines[j], "minimum") || is_num_key(lines[j], "maximum"))
                });
                if has_bound {
                    bounded_examples += 1;
                }
            }
        }
        assert!(
            bounded_examples >= 3,
            "expected several numeric example+bound sibling pairs across specs, got {bounded_examples}"
        );
    }

    /// The 1-based line numbers, in document order, of every `example:` keyword whose
    /// inline **quoted-string** value has a character length outside a sibling string
    /// bound — `minLength` or `maxLength` — declared in the same Schema Object, without
    /// a YAML dep. The string-length analogue of `examples_outside_their_numeric_bounds`
    /// (which guards a numeric example against `minimum`/`maximum`); together the two
    /// cover both value families an `example` can carry.
    ///
    /// In OpenAPI 3.0.x (JSON Schema) an `example` is a sample *instance* of the schema,
    /// so it MUST satisfy the schema's own constraints. Where the object bounds a string
    /// with `minLength`/`maxLength`, a quoted example shorter than `minLength` or longer
    /// than `maxLength` is a self-contradictory schema: the schema advertises a sample
    /// its own validator rejects, so a Redoc/Swagger "try it" prefill and a codegen
    /// client's generated sample carry a value the length bound can never legally hold.
    /// `minLength`/`maxLength` count characters, so length is measured in Unicode scalar
    /// values (`chars().count()`), matching a validator.
    ///
    /// Only an `example` carrying an inline *quoted* scalar (a single- or double-quoted
    /// string) and at least one same-object string-length bound sibling is inspected;
    /// each bound is scanned at the example's own indent, down through the object's block
    /// then up, dedent-bounded exactly like `examples_outside_their_numeric_bounds`, so a
    /// nested or following sibling object's bound never pairs, and read only when it is a
    /// non-negative-integer scalar. Skipped: an `example:` that opens a block (an
    /// object/array or block-scalar example, or a property literally named `example`); an
    /// *unquoted* example (a bare number/boolean is the numeric-bound / type test's
    /// concern, and a bare string is a rare ambiguous case left to the type test); an
    /// example with no length-bound sibling; and an `example:` nested inside an outer
    /// `example:`/`examples:` payload (sample data, not a schema keyword). The comparison
    /// is inclusive — only a length strictly below `minLength` or strictly above
    /// `maxLength` is flagged.
    fn examples_outside_their_length_bounds(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a `name:` key (inline comment stripped; surrounding
        // quotes preserved so a quoted token stays distinguishable from a bare number);
        // `None` when the line is a different key or opens a block (no inline value).
        let raw_inline = |l: &str, name: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != name {
                return None;
            }
            let v = v.split('#').next().unwrap_or(v).trim();
            if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            }
        };
        // A same-indent non-negative-integer length bound sibling `key` in the same
        // object as line `i` (indent `c`): scan down through the object's block then up,
        // dedent-bounded so a nested or following object's bound never pairs. A quoted or
        // non-integer bound has no length to compare against and is treated as absent
        // (its own domain is `every_size_bound_is_a_non_negative_integer`'s concern).
        let sibling_len = |i: usize, c: usize, key: &str| -> Option<usize> {
            let parse_len = |l: &str| -> Option<usize> {
                let raw = raw_inline(l, key)?;
                if raw.starts_with('"') || raw.starts_with('\'') {
                    return None; // quoted → not a plain integer
                }
                raw.parse::<usize>().ok()
            };
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(n) = parse_len(l) {
                        return Some(n);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(n) = parse_len(l) {
                        return Some(n);
                    }
                }
            }
            None
        };
        // True when line `i` (indent `c`) sits inside an outer `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` (mirroring `examples_outside_their_numeric_bounds`), so an
        // inner `example` key there is sample data, not a schema keyword.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The character count of a quoted scalar's inner text (one matching leading and
        // trailing quote stripped); `None` when the value is not a quoted string.
        let quoted_len = |raw: &str| -> Option<usize> {
            let inner = raw
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| raw.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))?;
            Some(inner.chars().count())
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(raw) = raw_inline(line, "example") else {
                continue;
            };
            // Only a quoted-string example has a character length to bound; an unquoted
            // scalar (number/bool/bare string) is left to the numeric-bound / type tests.
            let Some(len) = quoted_len(&raw) else {
                continue;
            };
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            let min = sibling_len(i, c, "minLength");
            let max = sibling_len(i, c, "maxLength");
            if min.is_none() && max.is_none() {
                continue;
            }
            let below = min.is_some_and(|m| len < m);
            let above = max.is_some_and(|m| len > m);
            if below || above {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_example_respects_its_string_length_bounds() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares a quoted-string `example` beside a `minLength`
        // and/or `maxLength`, the example's character length MUST lie within those
        // bounds. An `example` is a sample *instance* of the schema, so a string shorter
        // than `minLength` or longer than `maxLength` — an over-long token pasted beside
        // a tightened cap, a placeholder below a raised floor — is a self-contradictory
        // schema whose own validator rejects the sample it advertises, so a
        // Redoc/Swagger "try it" prefill and a codegen client's generated sample carry a
        // value the length bound can never legally hold.
        //
        // The string-length complement of `every_example_is_within_its_numeric_bounds`
        // (which guards a *numeric* example against `minimum`/`maximum`): together the
        // two cover both value families an `example` can carry, and no existing test
        // compares a string example's *length* against its bounds —
        // `every_example_matches_its_schema_type` checks the example's type, and the
        // size-bound tests (`every_size_bound_is_a_non_negative_integer`,
        // `every_numeric_bound_is_ordered_low_to_high`) check the bounds' own domain and
        // ordering, never against an example. Verified true across all mounted specs
        // before asserting.
        for api in APIS {
            let offenders = examples_outside_their_length_bounds(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec declares a quoted-string `example` whose length falls outside \
                 its sibling `minLength`/`maxLength` bound (a sample the bound's own \
                 validator would reject) at `example:` line(s): {:?}",
                api.name,
                offenders
            );
        }
    }

    #[test]
    fn example_length_bound_extraction_rules() {
        // Unit-cover `examples_outside_their_length_bounds` so the contract test above
        // can't pass vacuously and its detection is pinned: a quoted example whose length
        // is within its bounds passes; one below a `minLength` (declared above it) and
        // one above a `maxLength` (declared above it) are flagged in document order; an
        // example whose length equals a bound passes (inclusive); a `minLength` declared
        // *below* the example is still paired (down-scan); an unquoted example is skipped
        // (no quoted-string length to compare); an example with no length-bound sibling
        // is skipped; an `example:` nested inside an outer `example:` payload is skipped;
        // an example in one property never pairs with a following property's bound across
        // the dedent; a property literally named `example` (opening a block) is skipped;
        // and a block-scalar example (`example: |`) is skipped (its inline value is the
        // `|` indicator, not a quoted string).
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
    GoodLen:
      type: string
      minLength: 2
      maxLength: 8
      example: \"hello\"
    TooShort:
      type: string
      minLength: 5
      example: \"ab\"
    TooLong:
      type: string
      maxLength: 3
      example: \"abcdef\"
    MinBelow:
      type: string
      example: \"abcdefgh\"
      minLength: 3
    EqualBound:
      type: string
      minLength: 3
      maxLength: 3
      example: \"abc\"
    Unquoted:
      type: string
      minLength: 5
      example: hi
    InExample:
      type: object
      example:
        minLength: 5
        example: \"ab\"
    Split:
      type: object
      properties:
        a:
          example: \"x\"
        b:
          type: string
          minLength: 5
    NamedExample:
      type: object
      properties:
        example:
          type: string
          minLength: 5
    BlockExample:
      type: string
      maxLength: 2
      example: |
        a long block scalar
";
        // Flagged, in document order: line 22 (`TooShort.example: \"ab\"` length 2 <
        // its `minLength: 5` sibling above) and line 26 (`TooLong.example: \"abcdef\"`
        // length 6 > its `maxLength: 3` sibling above). Not flagged: `GoodLen` (5 in
        // [2,8]); `MinBelow` (length 8 >= a `minLength: 3` declared *below* it —
        // down-scan, no max); `EqualBound` (length 3 == both bounds, inclusive);
        // `Unquoted` (`hi` is unquoted — no quoted-string length, though 2 < 5);
        // `InExample` (its inner `example: \"ab\"` sits inside the outer `example:`
        // payload); `Split.a.example: \"x\"`, whose only candidate `minLength: 5` sits in
        // the following property `Split.b` past a dedent, so the two never pair;
        // `NamedExample` (an `example:` opening a block has no inline scalar); and
        // `BlockExample` (its inline value is the block-scalar `|` indicator, not a
        // quoted string).
        assert_eq!(examples_outside_their_length_bounds(body), vec![22, 26]);

        // Non-vacuous floor: across every registered spec every quoted-string example
        // with a sibling length bound lies within it (the invariant the contract test
        // asserts), and the corpus actually declares many bounded string examples (a
        // phoneNumber, an id, a hex token) — so the length-comparison path runs on real
        // data and a broken (always-empty) extractor can't hide behind a corpus that
        // never pairs an example with a length bound. Count pairs with a window detector
        // independent of the extractor's length comparison.
        let mut bounded_examples = 0usize;
        for api in APIS {
            assert!(
                examples_outside_their_length_bounds(api.body).is_empty(),
                "{}: every quoted-string example must lie within its sibling \
                 minLength/maxLength bound",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let is_quoted_example = |l: &str| {
                l.trim_start().split_once(':').is_some_and(|(k, v)| {
                    if k.trim() != "example" {
                        return false;
                    }
                    let v = v.split('#').next().unwrap_or(v).trim();
                    (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
                        || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2)
                })
            };
            let is_int_key = |l: &str, name: &str| {
                l.trim_start().split_once(':').is_some_and(|(k, v)| {
                    k.trim() == name
                        && v.split('#')
                            .next()
                            .unwrap_or(v)
                            .trim()
                            .parse::<usize>()
                            .is_ok()
                })
            };
            for (i, l) in lines.iter().enumerate() {
                if !is_quoted_example(l) {
                    continue;
                }
                let c = indent(l);
                let lo = i.saturating_sub(8);
                let hi = (i + 8).min(lines.len());
                let has_bound = (lo..hi).any(|j| {
                    j != i
                        && indent(lines[j]) == c
                        && (is_int_key(lines[j], "minLength") || is_int_key(lines[j], "maxLength"))
                });
                if has_bound {
                    bounded_examples += 1;
                }
            }
        }
        assert!(
            bounded_examples >= 20,
            "expected many quoted-string example+length-bound sibling pairs across specs, got {bounded_examples}"
        );
    }

    /// The 1-based line numbers, in document order, of every `properties:` mapping
    /// opener whose sibling `type:` scalar names a JSON type other than `object` —
    /// without a YAML dep.
    ///
    /// In OpenAPI 3.0.x (JSON Schema) `properties` describes the members of an
    /// **object**, so a Schema Object that declares a `properties:` mapping must be
    /// object-typed: either `type: object` or no `type` at all (an implicit object).
    /// A `properties:` sitting beside a scalar `type:` that is *not* `object` —
    /// `type: array` (a left-over from a retype where the author swapped `items:` for
    /// `properties:`), or a scalar `type: string`/`integer`/`number`/`boolean` pasted
    /// from a sibling — is a self-contradictory schema: JSON-Schema `properties`
    /// applies only to objects, so a Redoc/Swagger/codegen client renders the wrong
    /// shape (a scalar/array field, or an object whose members are silently ignored)
    /// exactly where a caller reads or builds the payload.
    ///
    /// Only a `properties:` that (a) opens a mapping — an empty inline value or only a
    /// trailing `# comment`, mirroring `properties_objects_with_duplicate_names`
    /// (a `properties:` with an inline scalar/flow value opens no member mapping) — and
    /// (b) has a `type:` *scalar sibling* in the same Schema Object (same indent,
    /// scanning down through the object's block then up, dedent-bounded exactly like
    /// `schema_bounds_inverted` / `format_type_mismatches`) is inspected. A
    /// `properties:` whose sibling `type:` is absent (an implicit object) or opens a
    /// block (a property literally named `type`, whose value is its own schema) is
    /// skipped — nothing conflicting to compare — as is a `properties:` inside an
    /// `example:`/`examples:` payload (example data, not a schema keyword), detected by
    /// walking the ancestor chain. A property literally named `properties` (a key in a
    /// parent `properties:` mapping) opens its own schema block, so its siblings are
    /// the parent's other property *names*, never a same-indent schema `type:` scalar —
    /// so it is never mistaken for a mapping opener with a conflicting type.
    fn properties_openers_with_non_object_type(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a `name:` key (inline comment + surrounding quotes
        // stripped); `None` when the line is a different key or opens a block.
        let scalar = |l: &str, name: &str| -> Option<String> {
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
                None
            } else {
                Some(v.to_string())
            }
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` (mirroring `format_type_mismatches`).
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The sibling `type:` scalar in the same object as line `i` (indent `c`):
        // scan down through the object's block for a same-indent `type`, then up,
        // dedent-bounded so a nested/following object's `type` never pairs.
        let sibling_type = |i: usize, c: usize| -> Option<String> {
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar(l, "type") {
                        return Some(v);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = scalar(l, "type") {
                        return Some(v);
                    }
                }
            }
            None
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(rest) = line.trim_start().strip_prefix("properties:") else {
                continue;
            };
            let rest = rest.trim_start();
            if !(rest.is_empty() || rest.starts_with('#')) {
                continue; // an inline value opens no member mapping
            }
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            if let Some(ty) = sibling_type(i, c) {
                if ty != "object" {
                    out.push(i + 1);
                }
            }
        }
        out
    }

    #[test]
    fn every_properties_object_is_object_typed() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // `properties` describes the members of an **object**, so wherever a mounted
        // spec declares a `properties:` mapping beside a scalar `type:`, that type MUST
        // be `object` (or absent — an implicit object). A `properties:` sitting beside
        // a non-object scalar type — `type: array`, or a `type: string`/`integer`/
        // `number`/`boolean` — is a self-contradictory schema: JSON-Schema `properties`
        // applies only to objects, so a Redoc/Swagger/codegen client renders the wrong
        // shape (a scalar/array field, or an object whose declared members are silently
        // dropped) exactly where a caller reads or builds the payload.
        //
        // The type-agreement complement of the existing structural-keyword tests, and
        // invisible to all of them: `every_array_schema_declares_items` proves the
        // converse for arrays (`type: array` ⟹ has `items`) but never looks at
        // `properties`; `every_properties_object_lists_distinct_property_names` and
        // `every_type_names_a_valid_schema_type` check a mapping's *keys* or the `type`
        // token's spelling, never whether a `properties:` and its sibling `type:`
        // agree. The routine hazard in these hand-tuned specs: a schema retyped
        // `object`→`array` (or a scalar) with its `properties:` block left in place, or
        // a `type: array` pasted from a sibling above a members mapping. Verified true
        // across all mounted specs before asserting.
        for api in APIS {
            let bad = properties_openers_with_non_object_type(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a `properties:` mapping beside a non-`object` scalar \
                 `type:` (a schema with `properties` must be object-typed) at \
                 `properties:` line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn properties_object_type_consistency_extraction_rules() {
        // Unit-cover `properties_openers_with_non_object_type` so the contract test
        // above can't pass vacuously and its detection is pinned: a `properties:`
        // beside `type: object` and one with no sibling type (implicit object) both
        // pass; a `properties:` beside `type: array` or a scalar `type: string` is
        // flagged (the sibling type declared before *or* after the mapping); a property
        // literally named `properties` (whose siblings are the parent's property names,
        // not a schema `type:`) is skipped; a `properties:` inside an `example:` payload
        // is skipped; and a nested object's `properties:` is judged against its own
        // sibling type. All in document order.
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
    GoodObject:
      type: object
      properties:
        a:
          type: string
    ImplicitObject:
      properties:
        b:
          type: string
    BadArray:
      type: array
      properties:
        c:
          type: string
    BadString:
      type: string
      properties:
        d:
          type: string
    TypeAfter:
      properties:
        e:
          type: string
      type: integer
    NamedProperties:
      type: object
      properties:
        properties:
          type: string
    InExample:
      type: object
      example:
        type: array
        properties:
          x: 1
    Nested:
      type: object
      properties:
        inner:
          type: object
          properties:
            f:
              type: string
";
        // Flagged, in document order: line 25 (`BadArray.properties` beside
        // `type: array`, its sibling declared above), line 30 (`BadString.properties`
        // beside `type: string`), and line 34 (`TypeAfter.properties` beside a
        // `type: integer` declared *below* it, found by the down-scan). Not flagged:
        // `GoodObject` (`type: object`); `ImplicitObject` (no sibling type — an
        // implicit object); `NamedProperties`' inner `properties:` at line 41, a
        // property literally *named* `properties` whose only same-indent siblings are
        // parent property names (no schema `type:` scalar); the `properties:` inside
        // `InExample`'s `example:` payload (example data); and both `Nested`
        // `properties:` blocks (each beside its own `type: object`).
        assert_eq!(
            properties_openers_with_non_object_type(body),
            vec![25, 30, 34]
        );

        // Non-vacuous floor: across every registered spec every `properties:` mapping
        // is object-typed (the invariant the contract test asserts), and the corpus
        // actually declares many `properties:` blocks beside a `type: object` — so the
        // type-comparison path runs on real data and a broken (always-empty) extractor
        // can't hide behind a corpus that never pairs a mapping with a type. Count
        // object-typed `properties:` openers with a presence detector independent of
        // the extractor: a `properties:` block opener whose same-indent object declares
        // `type: object` (scanning down then up, dedent-bounded).
        let mut object_typed = 0usize;
        for api in APIS {
            assert!(
                properties_openers_with_non_object_type(api.body).is_empty(),
                "{}: every `properties:` mapping must be object-typed",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                let Some(rest) = l.trim_start().strip_prefix("properties:") else {
                    continue;
                };
                let rest = rest.trim_start();
                if !(rest.is_empty() || rest.starts_with('#')) {
                    continue;
                }
                let c = indent(l);
                let is_object_type = |x: &str| x.trim_start() == "type: object";
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
                    if indent(x) == c && is_object_type(x) {
                        has = true;
                        break;
                    }
                    j += 1;
                }
                if !has {
                    let mut m = i;
                    while m > 0 {
                        m -= 1;
                        let x = lines[m];
                        if x.trim().is_empty() {
                            continue;
                        }
                        if indent(x) < c {
                            break;
                        }
                        if indent(x) == c && is_object_type(x) {
                            has = true;
                            break;
                        }
                    }
                }
                if has {
                    object_typed += 1;
                }
            }
        }
        assert!(
            object_typed >= 30,
            "expected many object-typed properties blocks across specs, got {object_typed}"
        );
    }

    /// The 1-based line numbers, in document order, of every **array-form** `required:`
    /// keyword whose sibling `type:` scalar names a JSON type other than `object` —
    /// without a YAML dep.
    ///
    /// In OpenAPI 3.0.x (JSON Schema) `required` lists the mandatory *members of an
    /// object*, so a Schema Object that declares a `required:` array must be
    /// object-typed: either `type: object` or no `type` at all (an implicit object). A
    /// `required:` array sitting beside a scalar `type:` that is *not* `object` —
    /// `type: array` (a left-over from a retype where the author swapped `items:` for a
    /// members list), or a scalar `type: string`/`integer`/`number`/`boolean` pasted
    /// from a sibling — is a self-contradictory schema: JSON-Schema `required` applies
    /// only to objects, so a validator ignores the constraint and a Redoc/Swagger/codegen
    /// client renders the wrong shape (a scalar/array field that silently drops the
    /// "mandatory member" contract) exactly where a caller reads or builds the payload.
    ///
    /// Only a `required:` in **array form** — a flow sequence (`required: [a, b]`) or a
    /// block sequence (an empty inline value whose first non-blank child, deeper indented,
    /// is a `- ` item) — is inspected; the scalar `required: true`/`false` **flag** of a
    /// Parameter / Request Body / Schema-property Object opens no members list and is
    /// skipped (mirroring `required_arrays_with_duplicate_entries`). A property literally
    /// *named* `required` in a parent `properties:` mapping opens its own schema block
    /// (its first child is a schema keyword like `type:`, not a `- ` item), so it is never
    /// mistaken for an array. Of the array-form occurrences, only one with a `type:`
    /// *scalar sibling* in the same Schema Object (same indent, scanning down through the
    /// object's block then up, dedent-bounded exactly like
    /// `properties_openers_with_non_object_type` / `format_type_mismatches`) is judged; a
    /// `required:` whose sibling `type:` is absent (an implicit object, or an inherited
    /// type via `allOf`/`$ref`) or opens a block (a property literally named `type`) is
    /// skipped — nothing conflicting to compare — as is a `required:` inside an
    /// `example:`/`examples:` payload (sample data, not a schema keyword), detected by
    /// walking the ancestor chain.
    fn required_arrays_on_a_non_object_type(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline scalar of a `type:` key (inline comment + surrounding quotes
        // stripped); `None` when the line is a different key or opens a block.
        let type_scalar = |l: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != "type" {
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
                None
            } else {
                Some(v.to_string())
            }
        };
        // True when a `required:` line at index `i` opens an **array** — a flow `[ … ]`
        // inline value, or a block whose first non-blank following line is deeper-indented
        // and a `- ` sequence item. A scalar value (`true`/`false`) or a block whose first
        // child is a mapping key (a property literally named `required`) is not an array.
        let is_array_required = |i: usize, rest: &str| -> bool {
            let rest = rest.split('#').next().unwrap_or(rest).trim();
            if rest.starts_with('[') {
                return true; // flow sequence
            }
            if !rest.is_empty() {
                return false; // a scalar (`true`/`false`) or other inline value
            }
            let c = indent(lines[i]);
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                return indent(l) > c && l.trim_start().starts_with("- ");
            }
            false
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:` payload —
        // some enclosing container key up the indent ladder is `example`/`examples`
        // (mirroring `properties_openers_with_non_object_type`).
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The sibling `type:` scalar in the same object as line `i` (indent `c`): scan
        // down through the object's block for a same-indent `type`, then up, dedent-bounded
        // so a nested/following object's `type` never pairs.
        let sibling_type = |i: usize, c: usize| -> Option<String> {
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = type_scalar(l) {
                        return Some(v);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = type_scalar(l) {
                        return Some(v);
                    }
                }
            }
            None
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(rest) = line.trim_start().strip_prefix("required:") else {
                continue;
            };
            if !is_array_required(i, rest) {
                continue; // a scalar `required: true`/`false` flag opens no members list
            }
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            if let Some(ty) = sibling_type(i, c) {
                if ty != "object" {
                    out.push(i + 1);
                }
            }
        }
        out
    }

    #[test]
    fn every_required_array_sits_on_an_object_type() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // `required` lists the mandatory *members of an object*, so wherever a mounted
        // spec declares a `required:` array beside a scalar `type:`, that type MUST be
        // `object` (or absent — an implicit object). A `required:` array sitting beside a
        // non-object scalar type — `type: array`, or a `type: string`/`integer`/`number`/
        // `boolean` — is a self-contradictory schema: JSON-Schema `required` applies only
        // to objects, so a validator ignores the constraint and a Redoc/Swagger/codegen
        // client renders the wrong shape (a scalar/array field that silently drops the
        // "mandatory member" contract) exactly where a caller reads or builds the payload.
        //
        // The `required`-keyword sibling of `every_properties_object_is_object_typed`:
        // that pins the *other* object-only keyword (`properties`) against its sibling
        // type, this pins `required`. Neither `facet_required_type` (which covers the
        // string/array/object *facet* keywords — `minLength`/`minItems`/`minProperties`/…
        // — but not `required`) nor the required-array *content* tests
        // (`every_required_array_lists_distinct_entries`,
        // `every_required_entry_names_a_declared_property`, which check a `required`
        // array's entries are unique / name declared properties) ever compares a
        // `required:` array against its sibling `type:`. The routine hazard in these
        // hand-tuned specs: a schema retyped `object`→`array` (or a scalar) with its
        // `required:` block left in place, or a `type: array` pasted from a sibling above
        // a members list. Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = required_arrays_on_a_non_object_type(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a `required:` array beside a non-`object` scalar \
                 `type:` (a schema with a `required` members list must be object-typed) \
                 at `required:` line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn required_array_object_type_extraction_rules() {
        // Unit-cover `required_arrays_on_a_non_object_type` so the contract test above
        // can't pass vacuously and its detection is pinned: an array-form `required:`
        // (flow or block) beside `type: object`, and one with no sibling type (implicit
        // object), both pass; an array-form `required:` beside `type: array` or a scalar
        // `type: string` is flagged (the sibling type declared before *or* after the
        // array); the scalar `required: true` flag of a parameter is skipped (opens no
        // members list); a property literally *named* `required` (whose value is its own
        // schema block, not a `- ` list) is skipped; a `required:` inside an `example:`
        // payload is skipped; and a nested object's `required:` is judged against its own
        // sibling type. All in document order.
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
        - name: q
          in: query
          required: true
          schema:
            type: string
      responses:
        '200':
          description: ok
components:
  schemas:
    GoodObjectFlow:
      type: object
      required: [a]
      properties:
        a:
          type: string
    GoodObjectBlock:
      type: object
      required:
        - b
      properties:
        b:
          type: string
    ImplicitObject:
      required: [c]
      properties:
        c:
          type: string
    BadArray:
      type: array
      required: [d]
    BadStringBlock:
      type: string
      required:
        - e
    TypeAfter:
      required: [f]
      type: integer
    NamedRequired:
      type: object
      properties:
        required:
          type: string
    InExample:
      type: object
      example:
        type: array
        required:
          - x
    Nested:
      type: object
      required: [inner]
      properties:
        inner:
          type: object
          required: [g]
          properties:
            g:
              type: string
";
        // Flagged, in document order: line 40 (`BadArray.required: [d]` beside
        // `type: array`, its sibling declared above), line 43 (`BadStringBlock.required`
        // block beside `type: string`), and line 46 (`TypeAfter.required: [f]` beside a
        // `type: integer` declared *below* it, found by the down-scan). Not flagged:
        // `GoodObjectFlow`/`GoodObjectBlock` (`type: object`); `ImplicitObject` (no
        // sibling type — an implicit object); the query parameter's scalar
        // `required: true` (opens no members list); `NamedRequired`'s inner `required:` at
        // line 51, a property literally *named* `required` whose value is a schema block
        // (first child `type: string`, not a `- ` item); the `required:` inside
        // `InExample`'s `example:` payload (sample data); and both `Nested` `required:`
        // arrays (each beside its own `type: object`).
        assert_eq!(
            required_arrays_on_a_non_object_type(body),
            vec![40, 43, 46]
        );

        // Non-vacuous floor: across every registered spec every array-form `required:` is
        // object-typed (the invariant the contract test asserts), and the corpus actually
        // declares many `required:` arrays beside a `type: object` — so the
        // type-comparison path runs on real data and a broken (always-empty) extractor
        // can't hide behind a corpus that never pairs an array with a type. Count
        // object-typed array-form `required:` blocks with a presence detector independent
        // of the extractor: a `required:` array opener (flow `[` or a block whose first
        // child is a `- ` item) whose same-indent object declares `type: object` (scanning
        // down then up, dedent-bounded).
        let mut object_typed = 0usize;
        for api in APIS {
            assert!(
                required_arrays_on_a_non_object_type(api.body).is_empty(),
                "{}: every array-form `required:` must sit on an object type",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                let Some(rest) = l.trim_start().strip_prefix("required:") else {
                    continue;
                };
                let rest = rest.split('#').next().unwrap_or(rest).trim();
                let is_array = if rest.starts_with('[') {
                    true
                } else if rest.is_empty() {
                    let c = indent(l);
                    let mut j = i + 1;
                    let mut arr = false;
                    while j < lines.len() {
                        if lines[j].trim().is_empty() {
                            j += 1;
                            continue;
                        }
                        arr = indent(lines[j]) > c && lines[j].trim_start().starts_with("- ");
                        break;
                    }
                    arr
                } else {
                    false
                };
                if !is_array {
                    continue;
                }
                let c = indent(l);
                let is_object_type = |x: &str| x.trim_start() == "type: object";
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
                    if indent(x) == c && is_object_type(x) {
                        has = true;
                        break;
                    }
                    j += 1;
                }
                if !has {
                    let mut m = i;
                    while m > 0 {
                        m -= 1;
                        let x = lines[m];
                        if x.trim().is_empty() {
                            continue;
                        }
                        if indent(x) < c {
                            break;
                        }
                        if indent(x) == c && is_object_type(x) {
                            has = true;
                            break;
                        }
                    }
                }
                if has {
                    object_typed += 1;
                }
            }
        }
        assert!(
            object_typed >= 100,
            "expected many object-typed required arrays across specs, got {object_typed}"
        );
    }

    // The `type` a string/array/object validation *facet* keyword modifies, or `None`
    // when the key is not a facet keyword. String facets constrain the characters of a
    // string, array facets the elements of an array, object facets the members of an
    // object — each is meaningless on any other type.
    fn facet_required_type(key: &str) -> Option<&'static str> {
        Some(match key {
            "minLength" | "maxLength" | "pattern" => "string",
            "minItems" | "maxItems" | "uniqueItems" => "array",
            "minProperties" | "maxProperties" => "object",
            _ => return None,
        })
    }

    fn facet_keyword_type_mismatches(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // True when a line's key holds an inline scalar value (so it is a keyword
        // occurrence, not a block-opening property literally *named* the keyword);
        // returns the key when so. Surrounding quotes/inline comments are irrelevant —
        // only presence of a non-empty value matters here.
        let inline_key = |l: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            let v = v.split('#').next().unwrap_or(v).trim();
            if v.is_empty() {
                None
            } else {
                Some(k.trim().to_string())
            }
        };
        // The inline scalar of a `type:` key (inline comment + surrounding quotes
        // stripped), or `None` for any other key / a block opener.
        let type_scalar = |l: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != "type" {
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
                None
            } else {
                Some(v.to_string())
            }
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` (mirroring `format_type_mismatches`).
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The sibling `type:` scalar in the same object as line `i` (indent `c`):
        // scan down through the object's block for a same-indent `type`, then up,
        // dedent-bounded so a nested/following object's `type` never pairs.
        let sibling_type = |i: usize, c: usize| -> Option<String> {
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = type_scalar(l) {
                        return Some(v);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = type_scalar(l) {
                        return Some(v);
                    }
                }
            }
            None
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(key) = inline_key(line) else {
                continue;
            };
            let Some(want) = facet_required_type(&key) else {
                continue; // not a facet keyword
            };
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            if let Some(ty) = sibling_type(i, c) {
                if ty != want {
                    out.push(i + 1);
                }
            }
        }
        out
    }

    /// True for a *numeric* validation facet keyword — `minimum`/`maximum`/
    /// `exclusiveMinimum`/`exclusiveMaximum`/`multipleOf`. Unlike the single-typed
    /// string/array/object facets (`facet_required_type`), a numeric facet's required
    /// type is the numeric *pair* {`integer`, `number`}, so it needs its own predicate.
    fn is_numeric_facet(key: &str) -> bool {
        matches!(
            key,
            "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf"
        )
    }

    /// Line numbers (1-based) where a numeric validation facet keyword sits beside a
    /// scalar `type:` that is neither `integer` nor `number`. Mirrors
    /// `facet_keyword_type_mismatches` exactly (inline-value keyword detection,
    /// `example:` skip, dedent-bounded down-then-up sibling-`type` scan); only the
    /// accepted-type test differs — the numeric pair rather than one fixed family.
    fn numeric_facet_type_mismatches(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The keyword name when a line holds an inline scalar value (so it is a keyword
        // occurrence, not a block-opening property literally *named* the keyword).
        let inline_key = |l: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            let v = v.split('#').next().unwrap_or(v).trim();
            if v.is_empty() {
                None
            } else {
                Some(k.trim().to_string())
            }
        };
        // The inline scalar of a `type:` key (inline comment + surrounding quotes
        // stripped), or `None` for any other key / a block opener.
        let type_scalar = |l: &str| -> Option<String> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != "type" {
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
                None
            } else {
                Some(v.to_string())
            }
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:` payload.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // The sibling `type:` scalar in the same object as line `i` (indent `c`):
        // scan down through the object's block for a same-indent `type`, then up,
        // dedent-bounded so a nested/following object's `type` never pairs.
        let sibling_type = |i: usize, c: usize| -> Option<String> {
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = type_scalar(l) {
                        return Some(v);
                    }
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c {
                    if let Some(v) = type_scalar(l) {
                        return Some(v);
                    }
                }
            }
            None
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(key) = inline_key(line) else {
                continue;
            };
            if !is_numeric_facet(&key) {
                continue;
            }
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            if let Some(ty) = sibling_type(i, c) {
                if ty != "integer" && ty != "number" {
                    out.push(i + 1);
                }
            }
        }
        out
    }

    #[test]
    fn every_facet_keyword_sits_on_its_required_type() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares a string/array/object validation *facet*
        // keyword as a sibling of a `type:` scalar, that type MUST be the one the facet
        // constrains — the string facets `minLength`/`maxLength`/`pattern` on
        // `type: string`, the array facets `minItems`/`maxItems`/`uniqueItems` on
        // `type: array`, the object facets `minProperties`/`maxProperties` on
        // `type: object`. A facet on the wrong type (`pattern` under `type: integer`,
        // `minItems` under `type: string`) is a self-contradictory schema: the keyword
        // can never constrain a value of that type, so a validator ignores it and a
        // Redoc/Swagger/codegen client silently drops the constraint exactly where a
        // caller reads or builds the payload.
        //
        // This is the type-agreement complement of the two facet-*value* tests:
        // `every_size_bound_is_a_non_negative_integer` checks a size facet's value is a
        // non-negative integer and `every_numeric_bound_is_ordered_low_to_high` checks
        // a lower/upper pair's ordering — neither ever looks at the sibling `type`, so a
        // domain-valid, well-ordered `maxLength: 10` left on a `type: integer` (a field
        // retyped without its facets updated, or a facet pasted from a string sibling
        // onto a numeric one) sails through both. It mirrors `every_format_matches_its_type`
        // (format↔type) for the validation facets. Only a facet keyword with a `type:`
        // scalar sibling in the same object is inspected (an inherited/absent type — e.g.
        // a facet on an `allOf`/`$ref` composition — or a property literally *named* the
        // keyword, is skipped). Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = facet_keyword_type_mismatches(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a string/array/object validation facet keyword on a \
                 `type:` that does not match the facet's family (e.g. `pattern` off \
                 `string`, `minItems` off `array`, `minProperties` off `object`) at \
                 line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn facet_keyword_type_consistency_extraction_rules() {
        // Unit-cover `facet_keyword_type_mismatches` so the contract test above can't
        // pass vacuously and its detection is pinned: a facet on its correct type passes
        // (string facets on string, array facets on array incl. `uniqueItems`, object
        // facets on object, whether `type` is declared before or after the facet); a
        // facet on the wrong type is flagged in document order (`pattern` on integer,
        // `minItems` on string, `minProperties` on array); a facet with no sibling `type`
        // scalar (type inherited/absent) is skipped; a property literally *named* a facet
        // keyword (a block opener, no inline value) is skipped; and a facet inside an
        // `example:` payload is skipped.
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
    GoodStr:
      type: string
      minLength: 1
      maxLength: 5
      pattern: '^x+$'
    GoodArr:
      type: array
      minItems: 1
      maxItems: 3
      uniqueItems: true
      items:
        type: string
    GoodObj:
      type: object
      minProperties: 1
      maxProperties: 4
    BadPatternOnInt:
      type: integer
      pattern: '^x+$'
    BadMinItemsOnStr:
      minItems: 2
      type: string
    BadMinPropsOnArr:
      type: array
      minProperties: 1
    NoType:
      maxLength: 9
    NamedFacet:
      type: object
      properties:
        pattern:
          type: string
    InExample:
      type: object
      example:
        type: integer
        maxLength: 3
";
        // Flagged, in document order: line 32 (`BadPatternOnInt.pattern` beside
        // `type: integer`, a string facet), line 34 (`BadMinItemsOnStr.minItems` beside
        // `type: string` declared below — an array facet, found by the down-scan), and
        // line 38 (`BadMinPropsOnArr.minProperties` beside `type: array`, an object
        // facet). Not flagged: the three Good schemas (string/array/object facets each on
        // their matching type — incl. `uniqueItems: true` on the array); `NoType`'s
        // `maxLength` (no sibling `type` scalar); the property literally *named* `pattern`
        // (line 44, a block opener with no inline value); and the `maxLength` inside the
        // `example:` payload (line 50).
        assert_eq!(
            facet_keyword_type_mismatches(body),
            vec![32, 34, 38]
        );

        // Non-vacuous floor: across every registered spec every facet keyword sits on
        // its matching type (the invariant the contract test asserts), and the corpus
        // declares many facet+type pairs that actually agree — so the type-comparison
        // path runs on real data and a broken (always-empty) extractor can't hide behind
        // a corpus that never pairs a facet with a type. Count agreeing pairs with a
        // presence detector that pairs the same way but compares for a match.
        let mut agree = 0usize;
        for api in APIS {
            assert!(
                facet_keyword_type_mismatches(api.body).is_empty(),
                "{}: every validation facet keyword must sit on its matching type",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let type_of = |l: &str| -> Option<String> {
                let (k, v) = l.trim_start().split_once(':')?;
                if k.trim() != "type" {
                    return None;
                }
                let v = v
                    .split('#')
                    .next()
                    .unwrap_or(v)
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                (!v.is_empty()).then(|| v.to_string())
            };
            for (i, l) in lines.iter().enumerate() {
                let Some((k, v)) = l.trim_start().split_once(':') else {
                    continue;
                };
                if v.split('#').next().unwrap_or(v).trim().is_empty() {
                    continue; // block opener, not a keyword occurrence
                }
                let Some(want) = facet_required_type(k.trim()) else {
                    continue;
                };
                let c = indent(l);
                // sibling type: down then up, dedent-bounded (mirrors the extractor)
                let mut ty = None;
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
                    if indent(x) == c {
                        if let Some(t) = type_of(x) {
                            ty = Some(t);
                            break;
                        }
                    }
                    j += 1;
                }
                if ty.is_none() {
                    let mut m = i;
                    while m > 0 {
                        m -= 1;
                        let x = lines[m];
                        if x.trim().is_empty() {
                            continue;
                        }
                        if indent(x) < c {
                            break;
                        }
                        if indent(x) == c {
                            if let Some(t) = type_of(x) {
                                ty = Some(t);
                                break;
                            }
                        }
                    }
                }
                if ty.as_deref() == Some(want) {
                    agree += 1;
                }
            }
        }
        assert!(
            agree >= 30,
            "expected many facet+type pairs that agree across specs, got {agree}"
        );
    }

    #[test]
    fn every_numeric_facet_sits_on_a_numeric_type() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // where a Schema Object declares a *numeric* validation facet keyword —
        // `minimum`/`maximum`/`exclusiveMinimum`/`exclusiveMaximum`/`multipleOf` — as a
        // sibling of a `type:` scalar, that type MUST be `integer` or `number`. A numeric
        // facet on a non-numeric type (`minimum` under `type: string`, `multipleOf` under
        // `type: array`) is a self-contradictory schema: the keyword can never constrain a
        // value of that type, so a validator ignores it and a Redoc/Swagger/codegen client
        // silently drops the bound exactly where a caller reads or builds the payload.
        //
        // The numeric-family sibling of `every_facet_keyword_sits_on_its_required_type`,
        // which covers only the single-typed string/array/object facets (`minLength`/…,
        // `minItems`/…, `minProperties`/…) — a numeric facet's required type is the pair
        // {integer, number}, so it needs its own check. Also the type-agreement complement
        // of the two numeric-facet-*value* tests: `every_numeric_bound_is_ordered_low_to_high`
        // (a lower/upper pair's ordering) and `every_size_bound_is_a_non_negative_integer`
        // (a size facet's value domain) — neither ever looks at the sibling `type`, so a
        // domain-valid, well-ordered `minimum: 0`/`maximum: 10` left on a `type: string`
        // (a field retyped without its facets updated, or a bound pasted from a numeric
        // sibling onto a string one) sails through both. Only a numeric facet with a `type:`
        // scalar sibling in the same object is inspected (an inherited/absent type — e.g. a
        // facet on an `allOf`/`$ref` composition — or a property literally *named* the
        // keyword, is skipped). Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = numeric_facet_type_mismatches(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a numeric validation facet keyword (minimum/maximum/\
                 exclusiveMinimum/exclusiveMaximum/multipleOf) on a `type:` that is neither \
                 `integer` nor `number` at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn numeric_facet_type_consistency_extraction_rules() {
        // Unit-cover `numeric_facet_type_mismatches` so the contract test above can't pass
        // vacuously and its detection is pinned: a numeric facet on `integer`/`number`
        // passes (bounds and `multipleOf`, whether `type` is declared before or after the
        // facet, incl. a boolean `exclusiveMinimum` beside a numeric type); a numeric facet
        // on a non-numeric type is flagged in document order (`minimum` on string, `maximum`
        // on boolean, `multipleOf` on array); a facet with no sibling `type` scalar (type
        // inherited/absent) is skipped; a property literally *named* a numeric facet (a block
        // opener, no inline value) is skipped; and a facet inside an `example:` payload is
        // skipped.
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
    GoodInt:
      type: integer
      minimum: 0
      maximum: 10
    GoodNum:
      type: number
      minimum: 0.5
      multipleOf: 0.1
    GoodExclusive:
      exclusiveMinimum: true
      type: integer
      minimum: 1
    BadMinOnStr:
      type: string
      minimum: 3
    BadMaxOnBool:
      maximum: 5
      type: boolean
    BadMultipleOfOnArr:
      type: array
      multipleOf: 2
      items:
        type: string
    NoType:
      minimum: 9
    NamedFacet:
      type: object
      properties:
        minimum:
          type: number
    InExample:
      type: object
      example:
        minimum: 3
";
        // Flagged, in document order: line 28 (`BadMinOnStr.minimum` beside `type: string`
        // declared above), line 30 (`BadMaxOnBool.maximum` beside `type: boolean` declared
        // below — found by the down-scan), and line 34 (`BadMultipleOfOnArr.multipleOf`
        // beside `type: array`; its nested `items.type: string` is deeper-indented and never
        // pairs). Not flagged: the three Good schemas (numeric facets each on integer/number,
        // incl. `exclusiveMinimum: true` beside `type: integer`); `NoType`'s `minimum` (no
        // sibling `type` scalar); the property literally *named* `minimum` (line 42, a block
        // opener with no inline value); and the `minimum` inside the `example:` payload
        // (line 47).
        assert_eq!(numeric_facet_type_mismatches(body), vec![28, 30, 34]);

        // Non-vacuous floor: across every registered spec every numeric facet sits on a
        // numeric type (the invariant the contract test asserts), and the corpus declares
        // many numeric-facet+type pairs that actually agree (port ranges, coordinate
        // bounds, page sizes) — so the type-comparison path runs on real data and a broken
        // (always-empty) extractor can't hide behind a corpus that never pairs a numeric
        // facet with a type. Count agreeing pairs with a presence detector that pairs the
        // same way but confirms the sibling type is numeric.
        let mut agree = 0usize;
        for api in APIS {
            assert!(
                numeric_facet_type_mismatches(api.body).is_empty(),
                "{}: every numeric facet keyword must sit on a numeric type",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let type_of = |l: &str| -> Option<String> {
                let (k, v) = l.trim_start().split_once(':')?;
                if k.trim() != "type" {
                    return None;
                }
                let v = v
                    .split('#')
                    .next()
                    .unwrap_or(v)
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                (!v.is_empty()).then(|| v.to_string())
            };
            for (i, l) in lines.iter().enumerate() {
                let Some((k, v)) = l.trim_start().split_once(':') else {
                    continue;
                };
                if v.split('#').next().unwrap_or(v).trim().is_empty() {
                    continue; // block opener, not a keyword occurrence
                }
                if !is_numeric_facet(k.trim()) {
                    continue;
                }
                let c = indent(l);
                // sibling type: down then up, dedent-bounded (mirrors the extractor)
                let mut ty = None;
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
                    if indent(x) == c {
                        if let Some(t) = type_of(x) {
                            ty = Some(t);
                            break;
                        }
                    }
                    j += 1;
                }
                if ty.is_none() {
                    let mut m = i;
                    while m > 0 {
                        m -= 1;
                        let x = lines[m];
                        if x.trim().is_empty() {
                            continue;
                        }
                        if indent(x) < c {
                            break;
                        }
                        if indent(x) == c {
                            if let Some(t) = type_of(x) {
                                ty = Some(t);
                                break;
                            }
                        }
                    }
                }
                if matches!(ty.as_deref(), Some("integer") | Some("number")) {
                    agree += 1;
                }
            }
        }
        assert!(
            agree >= 30,
            "expected many numeric-facet+type pairs that agree across specs, got {agree}"
        );
    }

    /// Line numbers (1-based), in document order, of every `nullable:` modifier that
    /// sits on a Schema Object carrying **no type context** — neither a sibling
    /// `type:` scalar nor a sibling composition keyword (`allOf`/`anyOf`/`oneOf`/
    /// `$ref`) — without a YAML dep.
    ///
    /// In OpenAPI 3.0.x `nullable` is a *modifier* on a typed schema: it extends a
    /// declared type's value space to also admit `null` (`type: string` +
    /// `nullable: true` ⇒ "a string or null"). On its own it is meaningless — a
    /// `nullable: true` with no type to extend admits nothing new, so the `null` the
    /// author meant to allow is silently disallowed: a validator ignores the keyword
    /// and a Redoc/Swagger/codegen client drops it exactly where a caller reads or
    /// builds the payload. Because a bare `$ref` ignores its siblings in 3.0.x, the
    /// canonical *nullable reference* idiom wraps the ref in a composition —
    /// `nullable: true` beside `allOf: [ $ref ]` (or `anyOf`/`oneOf`) — so a sibling
    /// composition keyword is an equally valid type context and is NOT flagged.
    ///
    /// This is the modifier-placement analogue of `every_facet_keyword_sits_on_its_
    /// required_type` / `every_numeric_facet_sits_on_a_numeric_type` (which pin a
    /// *validation facet* to its constrained type but never look at `nullable`), and the
    /// placement complement of `every_boolean_schema_keyword_carries_a_boolean` (which
    /// checks `nullable`'s value *type* is a boolean but never whether it has a type to
    /// modify). Only a `nullable:` carrying an inline boolean (`true`/`false`) is judged
    /// — a `nullable:` opening a block (a property literally *named* `nullable`) has no
    /// inline modifier value and is skipped, as is a `nullable:` inside an
    /// `example:`/`examples:` payload (sample data, walked up the ancestor chain). The
    /// type context is found by the same dedent-bounded down-then-up same-indent sibling
    /// scan the facet-placement extractors use.
    fn nullable_modifiers_without_a_type_context(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline boolean value of a `nullable:` line (inline comment + surrounding
        // quotes stripped); `None` for any other key, a block opener (no inline value —
        // a property literally *named* `nullable`), or a non-boolean value (its value
        // type is `every_boolean_schema_keyword_carries_a_boolean`'s concern — here we
        // only need to confirm this is a real `nullable` modifier occurrence).
        let nullable_bool = |l: &str| -> Option<bool> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != "nullable" {
                return None;
            }
            let v = v
                .split('#')
                .next()
                .unwrap_or(v)
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            match v {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            }
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:` payload.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // True when a line is a sibling that supplies a type context: a `type:` scalar
        // with a non-empty value, or a composition keyword (`allOf`/`anyOf`/`oneOf`/
        // `$ref`).
        let is_context = |l: &str| -> bool {
            let Some((k, v)) = l.trim_start().split_once(':') else {
                return false;
            };
            let k = k.trim();
            if matches!(k, "allOf" | "anyOf" | "oneOf" | "$ref") {
                return true;
            }
            if k == "type" {
                // a property literally *named* `type` opens a block (empty inline value)
                // and is not itself a type declaration; a real `type:` carries a scalar.
                return !v.split('#').next().unwrap_or(v).trim().is_empty();
            }
            false
        };
        // True when the object holding line `i` (indent `c`) has a same-indent sibling
        // supplying a type context: scan down through the object's block then up,
        // dedent-bounded so a nested or following object's keyword never pairs.
        let has_type_context = |i: usize, c: usize| -> bool {
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c && is_context(l) {
                    return true;
                }
                j += 1;
            }
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                if indent(l) < c {
                    break;
                }
                if indent(l) == c && is_context(l) {
                    return true;
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if nullable_bool(line).is_none() {
                continue;
            }
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            if !has_type_context(i, c) {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_nullable_modifier_sits_on_a_typed_schema() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every `nullable`
        // modifier a mounted spec declares MUST sit on a Schema Object that has a type
        // to modify — a sibling `type:` scalar, or a sibling composition keyword
        // (`allOf`/`anyOf`/`oneOf`/`$ref`, the canonical *nullable reference* idiom).
        // `nullable` extends a declared type's value space to also admit `null`; with no
        // type to extend it is a no-op, so the `null` the author meant to allow is
        // silently disallowed — a validator ignores the keyword and a Redoc/Swagger/
        // codegen client drops it exactly where a caller reads or builds the payload.
        //
        // The modifier-placement analogue of the facet-placement tests
        // (`every_facet_keyword_sits_on_its_required_type` /
        // `every_numeric_facet_sits_on_a_numeric_type`), which pin a *validation facet*
        // to its constrained type but never look at `nullable`; and the placement
        // complement of `every_boolean_schema_keyword_carries_a_boolean`, which checks
        // `nullable`'s value is a boolean but never whether it has a type to modify.
        // Verified true across all mounted specs before asserting (the 3.0.x nullable-
        // reference idiom — `nullable: true` beside `allOf: [ $ref ]` — is a valid type
        // context and is correctly not flagged).
        for api in APIS {
            let orphans = nullable_modifiers_without_a_type_context(api.body);
            assert!(
                orphans.is_empty(),
                "{} spec declares a `nullable` modifier with no type to modify (no \
                 sibling `type:` scalar and no `allOf`/`anyOf`/`oneOf`/`$ref` \
                 composition — a no-op that silently disallows the intended null) at \
                 `nullable:` line(s): {:?}",
                api.name,
                orphans
            );
        }
    }

    #[test]
    fn nullable_modifier_placement_extraction_rules() {
        // Unit-cover `nullable_modifiers_without_a_type_context` so the contract test
        // above can't pass vacuously and its detection is pinned: a `nullable` beside a
        // `type:` scalar (declared before *or* after it) passes; a `nullable` beside an
        // `allOf`/`anyOf`/`oneOf` composition or a sibling `$ref` (the nullable-reference
        // idiom) passes; an orphan `nullable` whose only sibling is a `description`
        // (whether `true` or `false` — placement is judged regardless of value) is
        // flagged in document order; a property literally *named* `nullable` (a block
        // opener, no inline value) is skipped; and a `nullable:` inside an `example:`
        // payload is skipped.
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
    GoodTyped:
      type: string
      nullable: true
    GoodTypedAfter:
      nullable: true
      type: integer
    GoodAllOf:
      nullable: true
      allOf:
        - $ref: \"#/components/schemas/GoodTyped\"
    GoodRef:
      nullable: true
      $ref: \"#/components/schemas/GoodTyped\"
    BadOrphan:
      nullable: true
      description: no type here
    BadOrphanFalse:
      description: still no type
      nullable: false
    NamedNullable:
      type: object
      properties:
        nullable:
          type: boolean
    InExample:
      type: object
      example:
        nullable: true
        id: abc
";
        // Flagged, in document order: line 28 (`BadOrphan.nullable: true`, whose only
        // sibling is a `description` — no `type`, no composition) and line 32
        // (`BadOrphanFalse.nullable: false`, likewise orphaned — placement is judged
        // regardless of the boolean value). Not flagged: `GoodTyped` (a `type: string`
        // sibling above), `GoodTypedAfter` (a `type: integer` sibling below, found by
        // the down-scan), `GoodAllOf` (an `allOf` composition sibling), `GoodRef` (a
        // sibling `$ref`); `NamedNullable`'s property literally *named* `nullable` (a
        // block opener, no inline value); and the `nullable: true` inside the
        // `example:` payload (sample data).
        assert_eq!(nullable_modifiers_without_a_type_context(body), vec![28, 32]);

        // Non-vacuous floor: across every registered spec every `nullable` modifier
        // sits on a schema with a type context (the invariant the contract test
        // asserts), and the corpus actually declares many `nullable` modifiers (each on
        // a `type:` scalar or an `allOf`/`$ref` nullable-reference composition) — so the
        // context-detection path runs on real data and a broken (always-empty) extractor
        // can't hide behind a corpus that never declares a bounded `nullable`. Count
        // context-bearing nullables with a detector independent of the extractor's
        // negation.
        let mut with_context = 0usize;
        for api in APIS {
            assert!(
                nullable_modifiers_without_a_type_context(api.body).is_empty(),
                "{}: every `nullable` modifier must sit on a schema with a type context",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let is_nullable = |l: &str| {
                l.trim_start().split_once(':').is_some_and(|(k, v)| {
                    let v = v
                        .split('#')
                        .next()
                        .unwrap_or(v)
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    k.trim() == "nullable" && (v == "true" || v == "false")
                })
            };
            let is_ctx = |l: &str| {
                l.trim_start().split_once(':').is_some_and(|(k, v)| {
                    let k = k.trim();
                    matches!(k, "allOf" | "anyOf" | "oneOf" | "$ref")
                        || (k == "type"
                            && !v.split('#').next().unwrap_or(v).trim().is_empty())
                })
            };
            for (i, l) in lines.iter().enumerate() {
                if !is_nullable(l) {
                    continue;
                }
                let c = indent(l);
                let lo = i.saturating_sub(8);
                let hi = (i + 8).min(lines.len());
                let ctx = (lo..hi).any(|j| j != i && indent(lines[j]) == c && is_ctx(lines[j]));
                if ctx {
                    with_context += 1;
                }
            }
        }
        assert!(
            with_context >= 10,
            "expected many context-bearing `nullable` modifiers across specs, got {with_context}"
        );
    }

    /// Line numbers (1-based), in document order, of every `additionalProperties:`
    /// keyword whose inline *scalar* value is neither the JSON boolean `true`/`false`
    /// nor an inline flow-mapping schema (`{ … }`) — a value OpenAPI 3.0.x forbids.
    ///
    /// In OpenAPI 3.0.x `additionalProperties` is polymorphic: it is EITHER a JSON
    /// boolean (does the object permit members beyond those in `properties`?) OR a
    /// Schema Object constraining those extra members. So an *inline* value that is
    /// neither — a stringified `"false"`, a number, a YAML-truthy typo (`no`/`yes`),
    /// a bare word — is an invalid document: a Redoc/Swagger/codegen client can no
    /// longer tell whether extra members are allowed (or under what schema), so the
    /// open/closed contract silently breaks where a caller reads or builds the
    /// payload. This is exactly why `every_boolean_schema_keyword_carries_a_boolean`
    /// deliberately EXCLUDES `additionalProperties` (its value is not always a
    /// boolean); no other test inspects its value at all.
    ///
    /// Only an `additionalProperties` carrying an inline scalar is judged. Skipped:
    /// an `additionalProperties:` opening a block (an empty inline value — the
    /// Schema-Object form, or a property literally *named* `additionalProperties`);
    /// an inline flow-mapping value (`{ … }` — an inline Schema Object, valid); and
    /// an `additionalProperties:` appearing as data inside an `example:`/`examples:`
    /// payload (walked up the ancestor chain, mirroring
    /// `boolean_keyword_non_boolean_values`). The `:` must immediately follow the
    /// keyword, so a longer key sharing the prefix (`additionalPropertiesFoo:`) does
    /// not match. Quotes are PRESERVED before the boolean comparison, so a quoted
    /// `"true"`/`"false"` (a string, not a boolean, and not a Schema Object) is
    /// flagged rather than coerced.
    fn additional_properties_non_boolean_scalars(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline value of an `additionalProperties:` line (inline comment
        // stripped, surrounding whitespace trimmed; quotes PRESERVED so a quoted
        // string stays distinguishable from a bare boolean). `None` when the line is
        // a different key. An empty string marks a block opener (no inline value).
        let inline = |l: &str| -> Option<String> {
            let rest = l.trim_start().strip_prefix("additionalProperties")?;
            let v = rest.strip_prefix(':')?;
            Some(v.split('#').next().unwrap_or(v).trim().to_string())
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples`.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(v) = inline(line) else { continue };
            if v.is_empty() {
                continue; // block opener: the Schema-Object form / a named property
            }
            if v.starts_with('{') {
                continue; // an inline flow-mapping Schema Object (valid)
            }
            if v == "true" || v == "false" {
                continue; // the JSON boolean form (valid)
            }
            if inside_example(i, indent(line)) {
                continue; // example data, not a schema keyword
            }
            out.push(i + 1);
        }
        out
    }

    #[test]
    fn every_additional_properties_scalar_is_a_boolean() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every
        // `additionalProperties` a mounted spec declares with an inline *scalar*
        // value MUST be the JSON boolean `true` or `false`. `additionalProperties`
        // is polymorphic — a boolean (are members beyond `properties` allowed?) or a
        // Schema Object (the schema those extra members must satisfy) — so an inline
        // scalar that is neither a boolean nor a flow-mapping schema (`{ … }`) — a
        // quoted `"false"`, a number, a YAML-truthy `no`/`yes`, a bare word — is an
        // invalid document a Redoc/Swagger/codegen client can't read: it can no
        // longer tell whether the object is open or closed, so the deny-unknown-
        // fields contract these specs lean on (`additionalProperties: false` on
        // every request/response body) silently breaks.
        //
        // The value-side complement of `every_boolean_schema_keyword_carries_a_boolean`,
        // which deliberately EXCLUDES `additionalProperties` because — unlike
        // nullable/readOnly/deprecated/… — its value is not always a boolean; no
        // other test inspects its value at all (the `type:`/`format:` vocabulary
        // tests and the numeric/size-bound tests never look at it). Verified true
        // across all mounted specs before asserting.
        for api in APIS {
            let bad = additional_properties_non_boolean_scalars(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares an `additionalProperties` with an inline scalar that \
                 is neither a boolean nor a flow-mapping schema (an open/closed \
                 contract a validator can't read) at line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn additional_properties_scalar_extraction_rules() {
        // Unit-cover `additional_properties_non_boolean_scalars` so the contract test
        // above can't pass vacuously and its detection is pinned: the two boolean
        // forms (`additionalProperties: false`/`true`), an inline flow-mapping schema
        // (`{ type: string }`), and a block-opening Schema Object (empty inline
        // value) all pass; a quoted `"false"` (a string, not a boolean), a number
        // (`0`), and a YAML-truthy word (`no`) are flagged in document order; and an
        // `additionalProperties:` sitting inside an `example:` payload is skipped.
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
    GoodFalse:
      type: object
      additionalProperties: false
    GoodTrue:
      type: object
      additionalProperties: true
    GoodInlineSchema:
      type: object
      additionalProperties: { type: string }
    GoodBlockSchema:
      type: object
      additionalProperties:
        type: string
    BadQuoted:
      type: object
      additionalProperties: \"false\"
    BadNumber:
      type: object
      additionalProperties: 0
    BadWord:
      type: object
      additionalProperties: no
    InExample:
      type: object
      example:
        additionalProperties: 0
";
        // Flagged, in document order: line 29 (`BadQuoted` — a quoted `\"false\"` is a
        // string, neither a boolean nor a Schema Object), line 32 (`BadNumber` — a
        // number), and line 35 (`BadWord` — a YAML-truthy bare word). Not flagged:
        // `GoodFalse`/`GoodTrue` (bare booleans), `GoodInlineSchema` (a flow-mapping
        // Schema Object, `{ … }`), `GoodBlockSchema` (a block-opening Schema Object,
        // empty inline value), and the `InExample` occurrence (its
        // `additionalProperties: 0` sits inside the `example:` payload).
        assert_eq!(
            additional_properties_non_boolean_scalars(body),
            vec![29, 32, 35]
        );

        // Non-vacuous floor: across every registered spec every `additionalProperties`
        // scalar is a boolean (the invariant the contract test asserts), and the
        // corpus actually declares many `additionalProperties: false`/`true` (the
        // deny-unknown-fields flag on request/response bodies) — so the
        // boolean-comparison path runs on real data and a broken (always-empty)
        // extractor can't hide behind a corpus that never declares one. Count the
        // bare-boolean occurrences with a detector independent of the extractor.
        let mut boolean_ap = 0usize;
        for api in APIS {
            assert!(
                additional_properties_non_boolean_scalars(api.body).is_empty(),
                "{}: every `additionalProperties` scalar must be a boolean",
                api.name
            );
            for l in api.body.lines() {
                if let Some(rest) = l.trim_start().strip_prefix("additionalProperties:") {
                    let v = rest.split('#').next().unwrap_or(rest).trim();
                    if v == "true" || v == "false" {
                        boolean_ap += 1;
                    }
                }
            }
        }
        assert!(
            boolean_ap >= 30,
            "expected many boolean `additionalProperties` across specs, got {boolean_ap}"
        );
    }

    /// The 1-based line numbers, in document order, of every OpenAPI `security`
    /// field a spec declares whose value is **not a sequence** (array), without a
    /// YAML dep.
    ///
    /// The `security` field of an OpenAPI document — at the root or on an Operation
    /// Object — is a *Security Requirement Object array* (`[{scheme: [scopes]}, …]`,
    /// or the empty array `[]` to opt an operation out of a global requirement). So a
    /// `security:` whose value is a mapping (`security:` opening `openId: []` with no
    /// `-` dash), a bare scalar (`security: null`), or an empty block (a `security:`
    /// with nothing indented under it → YAML `null`) is an invalid document: a
    /// Redoc/Swagger/codegen client and the resource server read the operation's auth
    /// from a shape that isn't the requirement *list* they expect, so the endpoint's
    /// declared protection silently doesn't parse.
    ///
    /// This closes a genuine vacuous-pass gap in the sibling security tests: both
    /// [`security_requirement_schemes`] (feeding
    /// `every_security_requirement_references_a_defined_scheme`) and
    /// `operations_with_scopeless_security` only ever collect requirement items *from
    /// within* a `security:` block by matching `- <scheme>:` sequence items — so a
    /// `security:` a paste turned into a mapping (or emptied) yields **zero**
    /// requirement items and passes every one of them silently, its broken auth shape
    /// unseen. This test inspects the field's *shape* itself, which none of them does.
    ///
    /// A `security:` field is recognised exactly as its siblings recognise it — the
    /// key is `security` and the `:` immediately follows (so `securitySchemes:` never
    /// matches) — and is judged as a sequence when: its inline value opens a flow
    /// sequence (`[`, covering `[]` and `[ … ]`); or, opening a block (empty inline
    /// value), the first non-blank line indented deeper than the key is a `- `
    /// sequence item. It is flagged when the inline value is any other non-empty
    /// scalar, or the block's first deeper line is a mapping key (no dash), or the
    /// block has no deeper line at all (an empty/`null` field). A `security:` inside an
    /// `example:`/`examples:` payload is skipped via the ancestor walk (sample data,
    /// not the field). As with the sibling security extractors, a schema property
    /// literally *named* `security` opening a mapping would be flagged — the specs
    /// declare none (a documented scoping trade shared with
    /// [`security_requirement_schemes`], which treats every `security:` block as the
    /// field).
    fn security_fields_not_a_sequence(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline value of a `security:` line (inline comment stripped, surrounding
        // whitespace trimmed). `None` when the line is a different key; an empty string
        // marks a block opener (no inline value).
        let inline = |l: &str| -> Option<String> {
            let rest = l.trim_start().strip_prefix("security")?;
            let v = rest.strip_prefix(':')?;
            Some(v.split('#').next().unwrap_or(v).trim().to_string())
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` (mirroring the other keyword extractors).
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(v) = inline(line) else { continue };
            let c = indent(line);
            if inside_example(i, c) {
                continue; // example data, not the field
            }
            if v.starts_with('[') {
                continue; // an inline flow sequence (`[]` / `[ … ]`) — a sequence
            }
            if !v.is_empty() {
                out.push(i + 1); // a non-empty non-sequence scalar (`null`, `{}`, …)
                continue;
            }
            // Block opener: the field is a sequence iff its first non-blank line
            // indented deeper than the key is a `- ` sequence item.
            let mut j = i + 1;
            let is_seq = loop {
                if j >= lines.len() {
                    break false; // no deeper line — an empty/`null` field
                }
                let l = lines[j];
                if l.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break false; // dedented out with nothing under `security:`
                }
                break l.trim_start().starts_with("- ");
            };
            if !is_seq {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_security_field_is_a_sequence() {
        // Contract-harness invariant (OpenAPI 3.0.x structural rule): every `security`
        // field a mounted spec declares — the document-root or Operation-Object list
        // of Security Requirement Objects — MUST be a sequence (array). The value is a
        // requirement *list* (`[{openId: [scopes]}, …]`, or `[]` to opt out of a
        // global requirement), so a `security:` that is a mapping, a bare scalar, or
        // an empty/`null` block is an invalid document: a Redoc/Swagger/codegen client
        // and the resource server read the endpoint's auth from a shape that isn't the
        // list they expect, so the declared protection silently doesn't parse.
        //
        // Closes a real vacuous-pass gap the sibling security tests leave open: both
        // `every_security_requirement_references_a_defined_scheme` (via
        // `security_requirement_schemes`) and `every_security_requirement_declares_a_scope`
        // (via `operations_with_scopeless_security`) collect requirement items only by
        // matching `- <scheme>:` sequence items *inside* a `security:` block — so a
        // `security:` a paste turned into a mapping or emptied yields zero items and
        // passes both silently, its broken shape unseen. This test inspects the
        // field's shape itself. Verified true across all mounted specs before asserting.
        for api in APIS {
            let bad = security_fields_not_a_sequence(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a `security` field that is not a sequence (array) — \
                 an Operation/root Security Requirement list a client and the resource \
                 server can't parse — at `security:` line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn security_field_sequence_extraction_rules() {
        // Unit-cover `security_fields_not_a_sequence` so the contract test above can't
        // pass vacuously and its detection is pinned: a block sequence (`security:` +
        // `- openId: []`) and an inline empty flow sequence (`security: []`) pass; a
        // mapping-form block (`security:` + `openId: []`, no dash), a bare scalar
        // (`security: null`), and an empty block (`security:` with nothing under it)
        // are flagged in document order; and a `security:` inside an `example:` payload
        // is skipped.
        let body = "\
openapi: 3.0.3
info:
  title: t
  version: 1.0.0
paths:
  /a:
    get:
      operationId: getA
      security:
        - openId: []
      responses:
        '200':
          description: ok
  /b:
    get:
      operationId: getB
      security: []
      responses:
        '200':
          description: ok
  /c:
    get:
      operationId: getC
      security:
        openId: []
      responses:
        '200':
          description: ok
  /d:
    get:
      operationId: getD
      security: null
      responses:
        '200':
          description: ok
  /e:
    get:
      operationId: getE
      security:
      responses:
        '200':
          description: ok
components:
  schemas:
    InExample:
      type: object
      example:
        security: null
";
        // Flagged, in document order: line 24 (`/c` — a mapping child `openId: []`
        // with no dash, not a sequence), line 32 (`/d` — a bare scalar `null`), and
        // line 39 (`/e` — a `security:` block with only `responses:` at or above its
        // own indent under it, i.e. an empty/`null` field). Not flagged: `/a` (a block
        // sequence), `/b` (`security: []`, an inline empty flow sequence), and the
        // `InExample` occurrence (its `security: null` sits inside the `example:`
        // payload).
        assert_eq!(security_fields_not_a_sequence(body), vec![24, 32, 39]);

        // Non-vacuous floor: across every registered spec every `security` field is a
        // sequence (the invariant the contract test asserts), and the corpus actually
        // declares many block-sequence `security:` fields (one per OAuth-protected
        // operation) — so the sequence-recognition path runs on real data and a broken
        // (always-empty) extractor can't hide behind a corpus that never declares one.
        // Count the block-sequence fields with a detector independent of the extractor.
        let mut seq_fields = 0usize;
        for api in APIS {
            assert!(
                security_fields_not_a_sequence(api.body).is_empty(),
                "{}: every `security` field must be a sequence",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                if l.trim() != "security:" {
                    continue;
                }
                let c = indent(l);
                let mut j = i + 1;
                while j < lines.len() && lines[j].trim().is_empty() {
                    j += 1;
                }
                if j < lines.len()
                    && indent(lines[j]) > c
                    && lines[j].trim_start().starts_with("- ")
                {
                    seq_fields += 1;
                }
            }
        }
        assert!(
            seq_fields >= 30,
            "expected many block-sequence `security` fields across specs, got {seq_fields}"
        );
    }

    /// The 1-based line numbers, in document order, of every schema `pattern:` keyword
    /// whose inline scalar value is an **empty quoted string** (`''` or `""`), without a
    /// YAML dep.
    ///
    /// In OpenAPI 3.0.x (JSON Schema) `pattern` constrains a string to an ECMA-262 regex
    /// *source*, and an empty regex matches at position 0 of every string — so
    /// `pattern: ''` imposes no constraint at all: the schema advertises a format
    /// restriction its own validator never enforces, and a Redoc/Swagger/codegen client
    /// drops the intended check silently at exactly the point a caller reads or builds the
    /// payload. This is the value-side complement of `every_facet_keyword_sits_on_its_required_type`,
    /// which pins `pattern`'s sibling `type:` but never inspects `pattern`'s own value —
    /// no existing test reads a `pattern` value at all — and it mirrors the suite's other
    /// non-emptiness guards (`every_enum_lists_unique_non_empty_values`,
    /// `every_composer_keyword_lists_at_least_one_subschema`).
    ///
    /// Only a `pattern:` carrying an inline scalar is judged, and only the exactly-empty
    /// quoted forms `''`/`""` are flagged: a non-empty regex (quoted or bare — a real
    /// pattern begins `'^…`/`"^…`, never `''`), an escaped-quote scalar (`''''` = a string
    /// holding one `'`), a whitespace-only pattern (a valid regex matching spaces), a
    /// `pattern:` that opens a block or carries an inline flow `{ … }`/`[ … ]` (a property
    /// literally *named* `pattern`, i.e. a Schema Object — `patternProperties` never
    /// matches, its key is a different token), and a `pattern:` inside an
    /// `example:`/`examples:` payload (sample data, not a schema keyword) are all skipped.
    /// Matching an empty quoted form by its two leading quote characters — not by
    /// comment-stripping the value — keeps a `#` *inside* a regex from ever being mistaken
    /// for an inline comment, so a real pattern is never mis-read as empty.
    fn patterns_with_empty_value(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // True when line `i` (indent `c`) sits inside an outer `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` (mirroring the value-consistency extractors), so an inner
        // `pattern` key there is sample data, not a schema keyword.
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        // Whether `vt` (the leading-trimmed inline value) is an exactly-empty quoted
        // string: it opens with a quote immediately followed by the matching quote, and
        // nothing but whitespace / a `# comment` follows the closing quote. `''''` (an
        // escaped single quote — a one-char string) and `'^x$'` (a real pattern) both
        // fail, since after the second quote non-blank, non-comment text remains.
        let is_empty_quoted = |vt: &str| -> bool {
            for q in ['\'', '"'] {
                if let Some(rest) = vt.strip_prefix(q) {
                    if let Some(after) = rest.strip_prefix(q) {
                        let after = after.trim_start();
                        if after.is_empty() || after.starts_with('#') {
                            return true;
                        }
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some((k, v)) = line.trim_start().split_once(':') else {
                continue;
            };
            if k.trim() != "pattern" {
                continue; // `patternProperties` and every other key are a different token
            }
            let vt = v.trim();
            if vt.is_empty() || vt.starts_with('{') || vt.starts_with('[') {
                continue; // block opener / inline flow → a property named `pattern`, not the keyword
            }
            if !is_empty_quoted(vt) {
                continue; // a non-empty regex (quoted or bare)
            }
            let c = indent(line);
            if inside_example(i, c) {
                continue;
            }
            out.push(i + 1);
        }
        out
    }

    #[test]
    fn every_pattern_declares_a_non_empty_string() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule): every
        // Schema Object `pattern` keyword a mounted spec declares MUST carry a non-empty
        // regex string. `pattern` constrains a string to an ECMA-262 regular expression,
        // and an empty regex matches at position 0 of *every* string — so `pattern: ''`
        // (or `""`) is a vacuous constraint: the schema advertises a format restriction
        // its own validator never enforces, so a Redoc/Swagger/codegen client silently
        // drops the intended check exactly where a caller reads or builds the payload
        // (an author who meant to bound an id/token/phone-number format ends up bounding
        // nothing).
        //
        // The value-side complement of `every_facet_keyword_sits_on_its_required_type`,
        // which pins `pattern`'s sibling `type:` (string) but never inspects `pattern`'s
        // own value — no existing test reads a `pattern` value at all. It mirrors the
        // suite's non-emptiness guards for the other value-bearing keywords
        // (`every_enum_lists_unique_non_empty_values`,
        // `every_composer_keyword_lists_at_least_one_subschema`). Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let offenders = patterns_with_empty_value(api.body);
            assert!(
                offenders.is_empty(),
                "{} spec declares an empty `pattern` string (`''`/`\"\"` — a regex that \
                 matches every string, silently dropping the intended constraint) at \
                 `pattern:` line(s): {:?}",
                api.name,
                offenders
            );
        }
    }

    #[test]
    fn pattern_value_non_empty_extraction_rules() {
        // Unit-cover `patterns_with_empty_value` so the contract test above can't pass
        // vacuously and its detection is pinned: a non-empty regex — single-quoted,
        // double-quoted, or bare — passes; an empty `pattern: ''` and `pattern: ""` are
        // flagged in document order; an escaped-quote scalar (`''''`, a one-char string)
        // passes; a property literally *named* `pattern` (a block opener) and one carrying
        // an inline flow schema (`{ … }`) are skipped (not the keyword); a `pattern:`
        // inside an `example:` payload is skipped; and an empty pattern trailed by a
        // `# comment` is still flagged (the comment does not make it non-empty).
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
    GoodQuoted:
      type: string
      pattern: '^x+$'
    GoodDouble:
      type: string
      pattern: \"^y$\"
    GoodBare:
      type: string
      pattern: ^z$
    EmptySingle:
      type: string
      pattern: ''
    EmptyDouble:
      type: string
      pattern: \"\"
    EscapedQuote:
      type: string
      pattern: ''''
    NamedPattern:
      type: object
      properties:
        pattern:
          type: string
    FlowPattern:
      type: object
      properties:
        pattern: {type: string}
    InExample:
      type: object
      example:
        pattern: ''
    EmptyWithComment:
      type: string
      pattern: '' # legacy
";
        // Flagged, in document order: line 25 (`EmptySingle.pattern: ''`), line 28
        // (`EmptyDouble.pattern: \"\"`) and line 47 (`EmptyWithComment.pattern: '' # legacy`
        // — the trailing comment does not make the empty regex non-empty). Not flagged:
        // `GoodQuoted`/`GoodDouble`/`GoodBare` (non-empty regexes); `EscapedQuote`
        // (`''''` holds one `'`, a one-char string); `NamedPattern` (a `pattern:` opening
        // a block is a property, not the keyword); `FlowPattern` (an inline flow schema
        // `{ … }` is a property named `pattern`); and `InExample` (its `pattern: ''` sits
        // inside the `example:` payload).
        assert_eq!(patterns_with_empty_value(body), vec![25, 28, 47]);

        // Non-vacuous floor: across every registered spec every schema `pattern` carries a
        // non-empty regex (the invariant the contract test asserts), and the corpus
        // actually declares many patterns (phoneNumber `^\\+[1-9]…`, UUID, hex-token and
        // `dpv:` scopes) — so the empty-value path runs on real data and a broken
        // (always-empty) extractor can't hide behind a corpus that never declares a
        // `pattern`. Count the non-empty `pattern` keywords with a detector independent of
        // the extractor's empty-string comparison.
        let mut patterns = 0usize;
        for api in APIS {
            assert!(
                patterns_with_empty_value(api.body).is_empty(),
                "{}: every schema `pattern` must be a non-empty regex string",
                api.name
            );
            for l in api.body.lines() {
                let Some((k, v)) = l.trim_start().split_once(':') else {
                    continue;
                };
                if k.trim() != "pattern" {
                    continue;
                }
                let vt = v.trim();
                // A non-empty inline scalar that is neither an empty quoted form nor a
                // flow/block opener — a real regex.
                if !vt.is_empty()
                    && !vt.starts_with('{')
                    && !vt.starts_with('[')
                    && vt != "''"
                    && vt != "\"\""
                {
                    patterns += 1;
                }
            }
        }
        assert!(
            patterns >= 50,
            "expected many non-empty `pattern` keywords across specs, got {patterns}"
        );
    }

    /// The 1-based line numbers, in document order, of every schema `enum` field a
    /// spec declares whose value is **not a sequence** (array), without a YAML dep.
    ///
    /// In OpenAPI 3.0.x (JSON Schema) an `enum` fixes the closed set of values a field
    /// may take, and the keyword's value MUST be an array. So an `enum:` whose value is
    /// a bare scalar (`enum: ACTIVE`, `enum: null`), an inline flow mapping
    /// (`enum: {a: b}`), a block mapping (`enum:` then `key: value` children, no dash),
    /// or an empty/`null` block (`enum:` with nothing indented under it) is an invalid
    /// document: a Redoc/Swagger/codegen client and a validator read the field's value
    /// set from a shape that is not the value *list* they expect, so the intended closed
    /// set silently doesn't parse at exactly the point a caller reads or builds the
    /// payload.
    ///
    /// This closes a real vacuous-pass gap the sibling enum tests leave open: both
    /// `every_enum_lists_unique_non_empty_values` (via `enums_with_no_values_or_duplicates`)
    /// and `every_enum_value_matches_its_schema_type` collect an enum's members only by
    /// gathering a flow list's `[ … ]` elements or a block list's deeper `- ` items — so
    /// an `enum:` a paste turned into a mapping or a lone scalar yields **zero** members
    /// and passes both silently, its broken shape unseen. This test inspects the field's
    /// *shape* itself, which neither does. It is the `enum` analogue of
    /// `every_security_field_is_a_sequence` and `every_composer_keyword_declares_a_sequence`.
    ///
    /// An `enum:` field is recognised exactly — the key is `enum` and the `:` immediately
    /// follows (so `enumeration:` / an `x-enum-varnames:` extension never match) — and is
    /// judged a sequence when: its inline value opens a flow sequence (`[`, covering `[]`
    /// and `[ … ]`); or, opening a block (empty inline value), its first non-blank,
    /// non-comment line indented deeper than the key is a `-` sequence item. It is flagged
    /// when the inline value is any other non-empty scalar/flow, or the block's first
    /// deeper line is a mapping key (no dash), or the block has no deeper line at all. An
    /// `enum:` inside an `example:`/`examples:` payload is skipped via the ancestor walk
    /// (sample data, not the keyword). As with the sibling sequence-shape extractors, a
    /// schema property literally *named* `enum` opening a mapping would be flagged — the
    /// specs declare none (a documented scoping trade shared with
    /// `enums_with_no_values_or_duplicates`, which likewise treats a `- `-item block as
    /// the enum and a mapping-first-child block as a property named `enum`).
    fn enum_fields_not_a_sequence(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline value of an `enum:` line (inline comment stripped, surrounding
        // whitespace trimmed). `None` when the line is a different key; an empty string
        // marks a block opener (no inline value).
        let inline = |l: &str| -> Option<String> {
            let rest = l.trim_start().strip_prefix("enum")?;
            let v = rest.strip_prefix(':')?;
            Some(v.split('#').next().unwrap_or(v).trim().to_string())
        };
        // True when line `i` (indent `c`) sits inside an `example:`/`examples:`
        // payload — some enclosing container key up the indent ladder is
        // `example`/`examples` (mirroring the other keyword extractors).
        let inside_example = |i: usize, c: usize| -> bool {
            let mut level = c;
            let mut k = i;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    if let Some((key, _)) = l.trim_start().split_once(':') {
                        let key = key.trim();
                        if key == "example" || key == "examples" {
                            return true;
                        }
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            false
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(v) = inline(line) else { continue };
            let c = indent(line);
            if inside_example(i, c) {
                continue; // example data, not the keyword
            }
            if v.starts_with('[') {
                continue; // an inline flow sequence (`[]` / `[ … ]`) — a sequence
            }
            if !v.is_empty() {
                out.push(i + 1); // a non-empty non-sequence scalar/flow (`ACTIVE`, `null`, `{…}`)
                continue;
            }
            // Block opener: the field is a sequence iff its first non-blank, non-comment
            // line indented deeper than the key is a `-` sequence item.
            let mut j = i + 1;
            let is_seq = loop {
                if j >= lines.len() {
                    break false; // no deeper line — an empty/`null` field
                }
                let l = lines[j];
                if l.trim().is_empty() || l.trim_start().starts_with('#') {
                    j += 1;
                    continue;
                }
                if indent(l) <= c {
                    break false; // dedented out with nothing under `enum:`
                }
                break l.trim_start().starts_with('-');
            };
            if !is_seq {
                out.push(i + 1);
            }
        }
        out
    }

    #[test]
    fn every_enum_field_is_a_sequence() {
        // Contract-harness invariant (OpenAPI 3.0.x / JSON-Schema structural rule):
        // every schema `enum` field a mounted spec declares MUST be a sequence (array).
        // The keyword fixes the closed set of values a field may take, so an `enum:`
        // that is a bare scalar, an inline/block mapping, or an empty/`null` block is an
        // invalid document: a Redoc/Swagger/codegen client and a validator read the
        // field's value set from a shape that isn't the value list they expect, so the
        // intended closed set silently doesn't parse where a caller reads or builds the
        // payload.
        //
        // Closes a real vacuous-pass gap the sibling enum tests leave open: both
        // `every_enum_lists_unique_non_empty_values` (via
        // `enums_with_no_values_or_duplicates`) and `every_enum_value_matches_its_schema_type`
        // collect an enum's members only by gathering a flow list's `[ … ]` elements or a
        // block list's `- ` items — so an `enum:` a paste turned into a mapping or a lone
        // scalar yields zero members and passes both silently, its broken shape unseen.
        // This test inspects the field's shape itself, the `enum` analogue of
        // `every_security_field_is_a_sequence`. Verified true across all mounted specs
        // before asserting.
        for api in APIS {
            let bad = enum_fields_not_a_sequence(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares an `enum` field that is not a sequence (array) — a \
                 closed value set a client and a validator can't parse — at `enum:` \
                 line(s): {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn enum_field_sequence_extraction_rules() {
        // Unit-cover `enum_fields_not_a_sequence` so the contract test above can't pass
        // vacuously and its detection is pinned: a flow sequence (`enum: [A, B]`), a
        // block sequence (`enum:` + `- A`), and an inline empty flow (`enum: []`) pass; a
        // bare scalar (`enum: ACTIVE`), a scalar `null` (`enum: null`), a mapping-form
        // block (`enum:` + `foo: bar`, no dash), and an empty block (`enum:` with nothing
        // deeper under it) are flagged in document order; and an `enum:` inside an
        // `example:` payload is skipped.
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
    FlowSeq:
      type: string
      enum: [ACTIVE, INACTIVE]
    BlockSeq:
      type: string
      enum:
        - ACTIVE
        - INACTIVE
    EmptyFlow:
      type: string
      enum: []
    Scalar:
      type: string
      enum: ACTIVE
    ScalarNull:
      type: string
      enum: null
    MappingBlock:
      type: string
      enum:
        foo: bar
    EmptyBlock:
      type: string
      enum:
    InExample:
      type: object
      example:
        enum: nope
";
        // Flagged, in document order: line 27 (`Scalar.enum: ACTIVE` — a bare scalar),
        // line 30 (`ScalarNull.enum: null` — a scalar `null`), line 33 (`MappingBlock` —
        // a block whose first deeper line `foo: bar` is a mapping key, no dash), and line
        // 37 (`EmptyBlock` — a block `enum:` with only the dedented `InExample:` below
        // it, i.e. an empty/`null` field). Not flagged: `FlowSeq` (inline flow), `BlockSeq`
        // (block sequence), `EmptyFlow` (`enum: []`, an inline empty flow sequence —
        // emptiness is `every_enum_lists_unique_non_empty_values`' concern), and the
        // `InExample` occurrence (its `enum: nope` sits inside the `example:` payload).
        assert_eq!(enum_fields_not_a_sequence(body), vec![27, 30, 33, 37]);

        // Non-vacuous floor: across every registered spec every `enum` field is a
        // sequence (the invariant the contract test asserts), and the corpus actually
        // declares many enums (status/type/unit value sets, in both flow and block form)
        // — so the sequence-recognition path runs on real data and a broken
        // (always-empty) extractor can't hide behind a corpus that never declares one.
        // Count the sequence-shaped enum fields with a detector independent of the
        // extractor.
        let mut seq_enums = 0usize;
        for api in APIS {
            assert!(
                enum_fields_not_a_sequence(api.body).is_empty(),
                "{}: every `enum` field must be a sequence",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                let Some(rest) = l.trim_start().strip_prefix("enum") else {
                    continue;
                };
                let Some(val) = rest.strip_prefix(':') else { continue };
                let val = val.split('#').next().unwrap_or(val).trim();
                if val.starts_with('[') {
                    seq_enums += 1; // an inline flow sequence
                    continue;
                }
                if !val.is_empty() {
                    continue; // a scalar — the extractor flags it, never a sequence
                }
                let c = indent(l);
                let mut j = i + 1;
                while j < lines.len()
                    && (lines[j].trim().is_empty() || lines[j].trim_start().starts_with('#'))
                {
                    j += 1;
                }
                if j < lines.len()
                    && indent(lines[j]) > c
                    && lines[j].trim_start().starts_with('-')
                {
                    seq_enums += 1; // a deeper `- `-item block sequence
                }
            }
        }
        assert!(
            seq_enums >= 100,
            "expected many sequence `enum` fields across specs, got {seq_enums}"
        );
    }

    /// The `METHOD /path <status>` label of every **response entry** a spec
    /// declares whose Response Object carries an inline `description` field with an
    /// **empty** value — a present-but-blank description — without a YAML dep.
    ///
    /// `description` is the single REQUIRED field of an OpenAPI Response Object and
    /// exists to give the outcome a human-readable summary; a present-but-empty
    /// value — `description: ""`/`''`, a bare `description:` (a YAML null), or an
    /// empty block scalar — documents nothing, so a Redoc/Swagger "try it" panel
    /// and a codegen client render a described-yet-blank status right where a caller
    /// reads what the response means. It is the value-side complement of
    /// [`responses_missing_description`], which accepts a `description:` key (or a
    /// `$ref`) by *presence* alone and never inspects its value.
    ///
    /// Reuses that helper's exact response-object scoping (a 4-space HTTP-verb key
    /// under a 2-space `/…` path item beneath the top-level `paths:` block → its
    /// 6-space `responses:` block → each 8-space status/`default`/`NXX` entry), then
    /// locates the response's own 10-space `description:` field and judges its value
    /// empty when it is:
    /// * a bare `description:` (nothing after the colon → a YAML null); or
    /// * an exactly-empty quoted string (`""`/`''`, optionally trailed by a
    ///   `# comment`) — recognised by two leading matching quote chars, never by
    ///   comment-stripping, so a `#` inside a real description is never read as a
    ///   comment and an escaped-quote scalar (`''''` = one `'`) is not empty; or
    /// * a block scalar (`|`/`>`, any chomping/indent indicator) with no non-blank
    ///   line following it deeper than the field indent.
    ///
    /// Matching the field at exactly the response object's own child indent (10)
    /// means a `description` nested deeper — a property literally *named*
    /// `description` inside a `content` schema, or a `headers` entry — is never the
    /// response's own and so is never inspected; a response with no indent-10
    /// `description` at all is left to [`responses_missing_description`], and a
    /// `$ref` response has no description of its own and is never reached here.
    fn responses_with_empty_description(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
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
        // Whether the `description:` at line `m` (child indent `ci`), with inline
        // value `rest` (everything after the first colon), carries no text.
        let value_is_empty = |rest: &str, m: usize, ci: usize| -> bool {
            let v = rest.trim();
            if v.is_empty() {
                return true; // bare `description:` → a YAML null
            }
            if v.starts_with('|') || v.starts_with('>') {
                // Block scalar: content lives on the following deeper lines.
                let mut k = m + 1;
                while k < lines.len() {
                    let l = lines[k];
                    if l.trim().is_empty() {
                        k += 1;
                        continue;
                    }
                    // First non-blank line: content only if indented past the field.
                    return indent(l) <= ci;
                }
                return true; // EOF with no content line — an empty block
            }
            // Exactly-empty quoted string: two leading matching quote chars with
            // nothing but optional whitespace/comment after them.
            let b = v.as_bytes();
            if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[1] == b[0] {
                let after = v[2..].trim_start();
                if after.is_empty() || after.starts_with('#') {
                    return true;
                }
            }
            false
        };
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
            // inspect each 8-space response entry under it.
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
                        if is_status_key(status) {
                            // Find this response object's own 10-space `description:`.
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
                                    if let Some((field, rest)) = e.trim_start().split_once(':') {
                                        if field == "description" {
                                            if value_is_empty(rest, m, 10) {
                                                out.push(format!(
                                                    "{} {} {}",
                                                    name.to_uppercase(),
                                                    current_path,
                                                    status
                                                ));
                                            }
                                            break; // one description per response object
                                        }
                                    }
                                }
                                m += 1;
                            }
                        }
                    }
                }
                j += 1;
            }
        }
        out
    }

    #[test]
    fn every_response_description_is_non_empty() {
        // Contract-harness invariant (OpenAPI structural rule): where a mounted spec
        // gives a Response Object an inline `description`, that description MUST carry
        // text. `description` is the single REQUIRED field of a Response Object and
        // exists to summarise the outcome, so a present-but-empty value —
        // `description: ""`/`''`, a bare `description:` (a YAML null), or an empty
        // block scalar — documents nothing: a Redoc/Swagger "try it" panel and a
        // codegen client render a described-yet-blank status exactly where a caller
        // reads what the response means.
        //
        // The value-side complement of `every_declared_response_has_a_description`,
        // which accepts a `description:` key by presence alone (or a `$ref`) and
        // never inspects its value — so a description truncated to empty by a
        // half-finished paste satisfies it, its blankness unseen. Mirrors the suite's
        // non-emptiness guards for the other value-bearing keywords
        // (`every_spec_declares_a_non_empty_info_description`,
        // `every_pattern_declares_a_non_empty_string`). Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let empty = responses_with_empty_description(api.body);
            assert!(
                empty.is_empty(),
                "{} spec declares a response whose `description` (the single REQUIRED \
                 field of an OpenAPI Response Object) is present but empty: {:?}",
                api.name,
                empty
            );
        }
    }

    #[test]
    fn response_description_non_empty_extraction_rules() {
        // Unit-cover `responses_with_empty_description` so the contract test above
        // can't pass vacuously and its accept/reject boundary is pinned: an inline
        // non-empty description, a quoted-text one, an escaped-quote `''''` (one `'`),
        // and a block scalar *with* content pass; a bare `description:` (null), an
        // empty quoted `""`/`''`, and an empty block scalar are flagged in document
        // order; a `description` nested inside a `content` schema (not the response's
        // own indent) is ignored, and a `$ref` response is never reached.
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
        '201':
          description: \"\"
        '202':
          description:
        '400':
          $ref: \"../../shared/errors.yaml#/components/responses/BadRequest\"
    post:
      operationId: postA
      responses:
        '200':
          description: ''
        '201':
          description: |
        '202':
          description: |
            a multi-line summary
        '203':
          content:
            application/json:
              schema:
                properties:
                  description:
                    type: string
          description: real
        '204':
          description: ''''
components:
  schemas:
    W:
      type: object
";
        // Flagged, in document order: GET /a 201 (empty `\"\"`), GET /a 202 (bare
        // null), POST /a 200 (empty `''`), POST /a 201 (empty block scalar). Not
        // flagged: GET /a 200 (`ok`); GET /a 400 (a `$ref`, no description of its
        // own); POST /a 202 (a block scalar *with* a content line); POST /a 203 (its
        // own `description: real` at indent 10 — the property literally named
        // `description` inside the response's `content` schema sits far deeper and is
        // ignored); POST /a 204 (`''''` holds one `'`, text).
        assert_eq!(
            responses_with_empty_description(body),
            vec![
                "GET /a 201".to_string(),
                "GET /a 202".to_string(),
                "POST /a 200".to_string(),
                "POST /a 201".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec no response description is
        // empty (the invariant the contract test asserts), and the corpus actually
        // declares many inline (non-`$ref`) descriptions at the response object's own
        // child indent — so the value-inspection path runs on real data and a broken
        // (always-empty) extractor can't hide behind a corpus with no descriptions to
        // check. Count indent-10 `description:` fields under `paths:` with a detector
        // independent of the extractor's emptiness comparison. This is a superset of
        // response descriptions (a parameter's or a request body's own `description`
        // also lands at indent 10 under a path item), which only strengthens the
        // floor — every one is a real, non-empty inline description the corpus carries.
        let mut inline_descriptions = 0usize;
        for api in APIS {
            assert!(
                responses_with_empty_description(api.body).is_empty(),
                "{}: every inline response `description` must be non-empty",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let mut in_paths = false;
            for l in &lines {
                if !l.is_empty() && !l.starts_with(char::is_whitespace) {
                    in_paths = l.trim_end() == "paths:";
                    continue;
                }
                if in_paths
                    && indent(l) == 10
                    && l.trim_start().split_once(':').map(|(k, _)| k) == Some("description")
                {
                    inline_descriptions += 1;
                }
            }
        }
        assert!(
            inline_descriptions >= 100,
            "expected many inline response descriptions across specs, got {inline_descriptions}"
        );
    }

    /// The `METHOD /path` label of every operation a spec declares whose own
    /// 6-space `summary:` key is present but carries no text — a bare `summary:`
    /// (a YAML null), an exactly-empty quoted `""`/`''`, or an empty block scalar
    /// — without a YAML dep.
    ///
    /// An operation's `summary` is the short label a Redoc/Swagger client renders
    /// as the operation's name in its navigation sidebar (and the hint many code
    /// generators prefer over the operationId); a present-but-empty one documents
    /// nothing, so the nav renders an anonymous entry exactly where a caller reads
    /// what the operation does. The value-side complement of
    /// [`operations_without_summary`], which credits a `summary:` by key presence
    /// alone (it matches the key name before the colon, whatever the value) and so
    /// counts a blank one as present — mirroring how `responses_with_empty_description`
    /// complements `every_declared_response_has_a_description`.
    ///
    /// Mirrors [`operations_without_summary`]'s path-item/method scoping (a 4-space
    /// HTTP-verb key under a 2-space `/…` path item beneath the top-level `paths:`
    /// block) and inspects only the operation's own 6-space `summary:` — so a
    /// `summary` nested deeper (an `examples` entry's `summary`) or a Path Item
    /// Object's own 4-space `summary` is never read, and an operation with *no*
    /// summary at all is left to `operations_without_summary` (this extractor flags
    /// only a present-but-blank one). Emptiness is judged exactly as
    /// `responses_with_empty_description` judges a blank `description`.
    fn operations_with_empty_summary(body: &str) -> Vec<String> {
        const METHODS: [&str; 8] =
            ["get", "put", "post", "delete", "patch", "options", "head", "trace"];
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // Whether the `summary:` at line `m` (child indent `ci`), with inline value
        // `rest` (everything after the first colon), carries no text — mirroring
        // `responses_with_empty_description`'s `value_is_empty`.
        let value_is_empty = |rest: &str, m: usize, ci: usize| -> bool {
            let v = rest.trim();
            if v.is_empty() {
                return true; // bare `summary:` → a YAML null
            }
            if v.starts_with('|') || v.starts_with('>') {
                // Block scalar: content lives on the following deeper lines.
                let mut k = m + 1;
                while k < lines.len() {
                    let l = lines[k];
                    if l.trim().is_empty() {
                        k += 1;
                        continue;
                    }
                    // First non-blank line: content only if indented past the field.
                    return indent(l) <= ci;
                }
                return true; // EOF with no content line — an empty block
            }
            // Exactly-empty quoted string: two leading matching quote chars with
            // nothing but optional whitespace/comment after them.
            let b = v.as_bytes();
            if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[1] == b[0] {
                let after = v[2..].trim_start();
                if after.is_empty() || after.starts_with('#') {
                    return true;
                }
            }
            false
        };
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
            // Within this operation's block, inspect its own 6-space `summary:`.
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
                if indent(l) == 6 {
                    if let Some((field, rest)) = l.trim_start().split_once(':') {
                        if field == "summary" {
                            if value_is_empty(rest, j, 6) {
                                out.push(format!(
                                    "{} {}",
                                    name.to_uppercase(),
                                    current_path
                                ));
                            }
                            break; // one summary per operation
                        }
                    }
                }
                j += 1;
            }
        }
        out
    }

    #[test]
    fn every_operation_summary_is_non_empty() {
        // Contract-harness invariant (CAMARA API Design Guidelines + DESIGN §9):
        // where an operation declares a `summary`, that summary MUST carry text.
        // `summary` is the short label a Redoc/Swagger client renders as the
        // operation's name in its navigation sidebar (and the hint many code
        // generators prefer over the operationId), so a present-but-empty value —
        // `summary: ""`/`''`, a bare `summary:` (a YAML null), or an empty block
        // scalar — renders an anonymous nav entry exactly where a caller reads what
        // the operation does.
        //
        // The value-side complement of `every_operation_declares_a_summary`, which
        // credits a `summary:` by key presence alone (whatever its value) — so a
        // summary truncated to empty by a half-finished paste satisfies it, its
        // blankness unseen. Mirrors the suite's non-emptiness guards for the other
        // value-bearing fields (`every_response_description_is_non_empty`,
        // `every_spec_declares_a_non_empty_info_title`,
        // `every_pattern_declares_a_non_empty_string`). Verified true across all
        // mounted specs before asserting.
        for api in APIS {
            let empty = operations_with_empty_summary(api.body);
            assert!(
                empty.is_empty(),
                "{} spec declares operation(s) whose `summary` is present but empty \
                 (Redoc renders an anonymous nav entry): {:?}",
                api.name,
                empty
            );
        }
    }

    #[test]
    fn operation_summary_non_empty_extraction_rules() {
        // Unit-cover `operations_with_empty_summary` so the contract test above
        // can't pass vacuously and its accept/reject boundary is pinned: a non-empty
        // inline summary and a block scalar *with* content pass; an empty quoted
        // `''`/`""`, a bare `summary:` (null), and an empty block scalar are flagged
        // in document order; a `summary` nested inside an `examples` entry (deeper
        // than the operation's own indent) is ignored; a Path Item Object's own
        // 4-space `summary` is not an operation's; an operation with *no* summary is
        // left to the presence test (not flagged here); and an HTTP verb used as a
        // schema property name is not an operation.
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
                  summary: ''
                  value: {}
    post:
      operationId: postA
      summary: ''
      responses:
        '201':
          description: created
  /b:
    summary: a path-item summary, not the operation's
    get:
      operationId: getB
      summary:
      responses:
        '200':
          description: ok
    post:
      operationId: postB
      summary: \"\"
      responses:
        '201':
          description: created
  /c:
    get:
      operationId: getC
      summary: |
      responses:
        '200':
          description: ok
    post:
      operationId: postC
      summary: |
        A block summary
      responses:
        '201':
          description: created
    put:
      operationId: putC
      responses:
        '200':
          description: ok
components:
  schemas:
    Widget:
      type: object
      properties:
        summary:
          type: string
";
        // Flagged, in document order: POST /a (empty `''`), GET /b (bare null),
        // POST /b (empty `\"\"`), GET /c (empty block scalar). Not flagged: GET /a
        // (`Get A`, and its nested `examples.ok.summary: ''` sits far deeper than the
        // operation's own 6-space `summary`, so it is never read); the Path Item
        // Object's own 4-space `summary` under `/b` (not an operation's); POST /c (a
        // block scalar *with* a content line); PUT /c (no `summary` — a *missing*
        // summary is `every_operation_declares_a_summary`'s concern, not this test's);
        // and the `summary` schema *property* under `components` (not under `paths:`).
        assert_eq!(
            operations_with_empty_summary(body),
            vec![
                "POST /a".to_string(),
                "GET /b".to_string(),
                "POST /b".to_string(),
                "GET /c".to_string(),
            ]
        );

        // Non-vacuous floor: across every registered spec no present operation
        // summary is empty (the invariant the contract test asserts), and the corpus
        // actually declares many operation-level summaries — so the value-inspection
        // path runs on real data and a broken (always-empty) extractor can't hide
        // behind a corpus with no summaries to check. Count operation-level (indent-6)
        // `summary:` fields under `paths:` with a detector independent of the
        // extractor's emptiness comparison (a Path Item Object's own summary sits at
        // indent 4, so it is never counted here).
        let mut op_summaries = 0usize;
        for api in APIS {
            assert!(
                operations_with_empty_summary(api.body).is_empty(),
                "{}: every present operation `summary` must be non-empty",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            let mut in_paths = false;
            for l in &lines {
                if !l.is_empty() && !l.starts_with(char::is_whitespace) {
                    in_paths = l.trim_end() == "paths:";
                    continue;
                }
                if in_paths
                    && indent(l) == 6
                    && l.trim_start().split_once(':').map(|(k, _)| k) == Some("summary")
                {
                    op_summaries += 1;
                }
            }
        }
        assert!(
            op_summaries >= 100,
            "expected many operation summaries across specs, got {op_summaries}"
        );
    }

    /// The 1-based line numbers, in document order, of every response **example**
    /// `status:` field whose integer value disagrees with the numeric HTTP status-code
    /// key of the Response Object that encloses it, without a YAML dep.
    ///
    /// The CAMARA error model (`specs/shared/errors.yaml` `CamaraError`) carries the
    /// HTTP status *into the body* as a `status` field, so an example illustrating a
    /// `"404"` response MUST show `status: 404`. A sample under one status key that
    /// carries a different status number — a `status: 400` pasted into a `"409"` block
    /// and left un-retargeted — is a self-contradictory spec: the documented sample
    /// disagrees with the very status it illustrates, so a Redoc/Swagger "try it"
    /// prefill and a codegen client's generated sample show a caller a body whose
    /// `status` field can never match the response it lives under. A routine hazard in
    /// these scenario-table-heavy specs, where each error case is one hand-written
    /// example copied from a sibling and re-tuned.
    ///
    /// Invisible to every existing test: `every_responses_object_key_is_a_valid_status`
    /// checks the response *key* is a well-formed status but never reads an example's
    /// body, and the example tests (`every_example_object_declares_a_value`, the
    /// enum/type/bound example tests) inspect an example against its *own* schema, never
    /// against the status code of the response that contains it. This is the only test
    /// that links an example's `status` body field to its enclosing response key.
    ///
    /// Only a `status:` carrying an inline **integer** scalar, sitting inside an
    /// `example:`/`examples:`/`value:` payload (some ancestor key up the indent ladder
    /// is `example`/`examples`/`value`, mirroring the suite's `inside_example` walk)
    /// whose nearest enclosing Response-Object key is a numeric HTTP status
    /// (`"NNN"`/`NNN`) is compared. Skipped: a non-integer `status` (a `SessionInfo`/
    /// `SubscriptionInfo` lifecycle enum such as `AVAILABLE`/`ACTIVE`, which has no
    /// status-code magnitude); a `status` under a `default:` response or a named
    /// `components.responses.<Name>` entry (no numeric key to compare — the up-walk
    /// halts at the `responses:` container or exhausts before any numeric key); and a
    /// `status:` that opens a block (a schema property literally named `status`, whose
    /// inline value is empty).
    fn error_example_status_mismatches(body: &str) -> Vec<usize> {
        let lines: Vec<&str> = body.lines().collect();
        let indent = |l: &str| l.len() - l.trim_start().len();
        // The inline integer value of a `status:` line (inline comment + surrounding
        // quotes stripped); `None` when the line is a different key, opens a block, or
        // carries a non-integer scalar (a lifecycle enum, a quoted non-number).
        let status_int = |l: &str| -> Option<i64> {
            let (k, v) = l.trim_start().split_once(':')?;
            if k.trim() != "status" {
                return None;
            }
            let v = v
                .split('#')
                .next()
                .unwrap_or(v)
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            v.parse::<i64>().ok()
        };
        // The numeric HTTP status-code key of the Response Object enclosing line `i`
        // (indent `c`), paired with whether the path from `i` up to it passed through
        // an `example`/`examples`/`value` container. Walk up the indent ladder (strictly
        // decreasing levels, dedent-tracked like the suite's ancestor walks): the first
        // shallower key that is a bare 3-digit status → its value; a `responses:`
        // container reached first → `None` (the enclosing response key was non-numeric —
        // a `default` or a named `components.responses` entry).
        let enclosing = |i: usize, c: usize| -> (Option<i64>, bool) {
            let mut level = c;
            let mut k = i;
            let mut in_example = false;
            while k > 0 {
                k -= 1;
                let l = lines[k];
                if l.trim().is_empty() {
                    continue;
                }
                let li = indent(l);
                if li < level {
                    let key = l
                        .trim_start()
                        .split_once(':')
                        .map(|(x, _)| x.trim())
                        .unwrap_or("");
                    if key == "example" || key == "examples" || key == "value" {
                        in_example = true;
                    }
                    if key == "responses" {
                        return (None, in_example);
                    }
                    let bare = key.trim_matches('"').trim_matches('\'');
                    if bare.len() == 3 && bare.chars().all(|ch| ch.is_ascii_digit()) {
                        return (bare.parse::<i64>().ok(), in_example);
                    }
                    level = li;
                    if li == 0 {
                        break;
                    }
                }
            }
            (None, in_example)
        };
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some(sval) = status_int(line) else {
                continue;
            };
            let c = indent(line);
            let (key, in_example) = enclosing(i, c);
            if !in_example {
                continue; // a `status` field outside any example payload
            }
            if let Some(kval) = key {
                if sval != kval {
                    out.push(i + 1);
                }
            }
        }
        out
    }

    #[test]
    fn every_error_example_status_matches_its_response_key() {
        // Contract-harness invariant (CAMARA error model, DESIGN §7/§8): the CamaraError
        // body carries the HTTP status into a `status` field, so where a Response Object
        // keyed by a numeric HTTP status code declares an example whose value has an
        // integer `status` field, that field MUST equal the response's own status-code
        // key. A sample under `"404"` that shows `status: 400` — an error example pasted
        // from a sibling status and left un-retargeted — is a self-contradictory spec:
        // the documented body disagrees with the response it illustrates, so a
        // Redoc/Swagger "try it" prefill and a codegen sample hand a caller a `status`
        // the response can never legally carry. Verified true across all mounted specs
        // before asserting.
        for api in APIS {
            let bad = error_example_status_mismatches(api.body);
            assert!(
                bad.is_empty(),
                "{} spec declares a response example whose body `status` field disagrees \
                 with the numeric HTTP status-code key of the response that encloses it \
                 (a sample the response's own status contradicts) at `status:` line(s): \
                 {:?}",
                api.name,
                bad
            );
        }
    }

    #[test]
    fn error_example_status_match_extraction_rules() {
        // Unit-cover `error_example_status_mismatches` so the contract test above can't
        // pass vacuously and its accept/reject boundary is pinned: a matching example
        // `status` (under a numeric key or a named `examples` entry) passes; a
        // mismatching `status` under a single `example:` and one under a named
        // `examples`/`value:` entry are flagged in document order; a non-integer
        // `status` (a lifecycle enum) is skipped; a `status` under a `default:` response
        // (no numeric key) is skipped; a `status:` schema *property* opening a block and
        // a schema-level `example.status` outside any `responses:` block are skipped.
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
          content:
            application/json:
              examples:
                ok:
                  value:
                    status: 200
        '404':
          description: nf
          content:
            application/json:
              example:
                status: 400
                code: NOT_FOUND
        '409':
          description: c
          content:
            application/json:
              examples:
                conflict:
                  value:
                    status: 409
                wrong:
                  value:
                    status: 422
        default:
          description: d
          content:
            application/json:
              example:
                status: 500
  /b:
    post:
      operationId: postB
      responses:
        '200':
          description: ok
          content:
            application/json:
              example:
                status: ACTIVE
components:
  schemas:
    Info:
      type: object
      properties:
        status:
          type: string
      example:
        status: 200
";
        // Flagged, in document order: line 23 (`status: 400` under the `'404'` response)
        // and line 35 (`status: 422` under the `'409'` response's `wrong` example). Not
        // flagged: line 17 (`status: 200` under `'200'`, a match) and line 32
        // (`status: 409` under `'409'`, a match); line 41 (`status: 500` under
        // `default:` — no numeric key, the up-walk halts at `responses:`); line 51
        // (`status: ACTIVE` — not an integer); line 57 (a `status:` schema property
        // opening a block, no inline value); and line 60 (`status: 200` under a schema's
        // own `example`, outside any `responses:` block).
        assert_eq!(error_example_status_mismatches(body), vec![23, 35]);

        // Non-vacuous floor: across every registered spec every numeric-status response
        // example agrees with its response key (the invariant the contract test
        // asserts), and the corpus actually declares many such example+response-key
        // pairs — so the status-comparison path runs on real data and a broken
        // (always-empty) extractor can't hide behind a corpus that never pairs an
        // example status with a numeric response key. Count pairs with an independent
        // ancestor walk that never performs the extractor's equality comparison.
        let mut status_pairs = 0usize;
        for api in APIS {
            assert!(
                error_example_status_mismatches(api.body).is_empty(),
                "{}: every response example's `status` must equal its response key",
                api.name
            );
            let lines: Vec<&str> = api.body.lines().collect();
            let indent = |l: &str| l.len() - l.trim_start().len();
            for (i, l) in lines.iter().enumerate() {
                let Some((k, v)) = l.trim_start().split_once(':') else {
                    continue;
                };
                if k.trim() != "status" {
                    continue;
                }
                let v = v.split('#').next().unwrap_or(v).trim().trim_matches('"').trim_matches('\'');
                if v.parse::<i64>().is_err() {
                    continue;
                }
                // Independent up-walk: numeric enclosing response key + an example ancestor.
                let c = indent(l);
                let mut level = c;
                let mut j = i;
                let mut in_example = false;
                let mut numeric_key = false;
                while j > 0 {
                    j -= 1;
                    let x = lines[j];
                    if x.trim().is_empty() {
                        continue;
                    }
                    let li = indent(x);
                    if li < level {
                        let key = x.trim_start().split_once(':').map(|(a, _)| a.trim()).unwrap_or("");
                        if key == "example" || key == "examples" || key == "value" {
                            in_example = true;
                        }
                        if key == "responses" {
                            break;
                        }
                        let bare = key.trim_matches('"').trim_matches('\'');
                        if bare.len() == 3 && bare.chars().all(|ch| ch.is_ascii_digit()) {
                            numeric_key = true;
                            break;
                        }
                        level = li;
                        if li == 0 {
                            break;
                        }
                    }
                }
                if in_example && numeric_key {
                    status_pairs += 1;
                }
            }
        }
        assert!(
            status_pairs >= 100,
            "expected many response example status+key pairs across specs, got {status_pairs}"
        );
    }
}
