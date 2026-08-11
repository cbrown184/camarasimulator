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
