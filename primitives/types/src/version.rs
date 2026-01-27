//! Protocol Version representation for Ethos artifact generation.
//!
//! Ethos stores protocol version information for artifact generation.
//! The canonical protocol schemas (e.g., `bitcoin-api.json`,
//! `core-lightning-api.json`) are versionless.
use std::cmp::Ordering;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A parsed protocol version.
///
/// Accepts formats like:
/// - `30.99.0` or `v30.99.0` (Bitcoin Core SemVer - 'v' prefix optional)
/// - `25.09` or `v25.09` (Core Lightning CalVer - 'v' prefix optional)
/// - `0.1.0` or `v0.1.0` (ProtocolVersion format - 'v' prefix optional)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolVersion {
    /// Original version string as provided by the version metadata or configuration.
    pub version_string: String,
    /// Major version component (e.g., `30` in `v30.1.2`).
    pub major: u32,
    /// Minor version component (e.g., `1` in `v30.1.2`).
    pub minor: u32,
    /// Patch component (e.g., `2` in `v30.1.2`).
    pub patch: u32,
    /// Protocol name for module naming (e.g., "bitcoin_core", "core_lightning").
    pub protocol: Option<String>,
}

impl PartialOrd for ProtocolVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for ProtocolVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

/// Errors that can occur while parsing or handling versions.
#[derive(Error, Debug)]
pub enum VersionError {
    /// The provided string did not match the expected version format.
    #[error("Invalid version format: {0}")]
    InvalidFormat(String),
    /// A numeric parse or regex error occurred.
    #[error("Parse error: {0}")]
    Parse(String),
}

impl ProtocolVersion {
    /// Parse a `ProtocolVersion` from a string in the `[v]MAJOR.MINOR[.PATCH]` format.
    pub fn from_string(s: &str) -> std::result::Result<Self, VersionError> {
        Self::from_string_with_protocol(s, None)
    }

    /// Parse a `ProtocolVersion` from a string with an optional protocol name.
    pub fn from_string_with_protocol(
        s: &str,
        protocol: Option<String>,
    ) -> std::result::Result<Self, VersionError> {
        // Expected formats (v prefix is optional):
        // 30.99.0 or v30.99.0
        // 0.1.0 or v0.1.0
        let re = Regex::new(r"^(?:v)?(\d+)\.(\d+)(?:\.(\d+))?$")
            .map_err(|e: regex::Error| VersionError::Parse(e.to_string()))?;
        let caps = re.captures(s).ok_or_else(|| VersionError::InvalidFormat(s.to_string()))?;

        // Store original version string without normalization
        let version_string = s.to_string();

        Ok(Self {
            version_string,
            major: caps[1]
                .parse()
                .map_err(|e: std::num::ParseIntError| VersionError::Parse(e.to_string()))?,
            minor: caps[2]
                .parse()
                .map_err(|e: std::num::ParseIntError| VersionError::Parse(e.to_string()))?,
            patch: caps.get(3).map(|m| m.as_str().parse().unwrap_or(0)).unwrap_or(0),
            protocol,
        })
    }

    /// Return the original version string.
    pub fn as_str(&self) -> &str { &self.version_string }

    /// Get the major version component.
    pub fn major(&self) -> u32 { self.major }

    /// Get the minor version component.
    pub fn minor(&self) -> u32 { self.minor }

    /// Render as a documentation version: `MAJOR.MINOR`.
    pub fn as_doc_version(&self) -> String { format!("{}.{}", self.major(), self.minor()) }

    /// Render as a short `vMAJOR.MINOR` string.
    pub fn short(&self) -> String { format!("v{}.{}", self.major, self.minor) }

    /// Render as a crate-compatible version string: `MAJOR.MINOR.PATCH`.
    pub fn crate_version(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// Render as a module name segment used in generated code.
    ///
    /// # Returns
    ///
    /// Returns a module name string, or an error if no protocol is specified.
    pub fn as_module_name(&self) -> Result<String, VersionError> {
        let protocol = self.protocol.as_deref().ok_or_else(|| {
            VersionError::InvalidFormat("Protocol name is required for module naming".to_string())
        })?;
        let patch = self.patch;

        // For CalVer versions (like Core Lightning), preserve the original minor version format
        // Extract the minor version from the original version string to preserve leading zeros
        let minor_str = if let Some(dot_pos) = self.version_string.find('.') {
            let after_dot = &self.version_string[dot_pos + 1..];
            if let Some(next_dot) = after_dot.find('.') {
                &after_dot[..next_dot]
            } else {
                after_dot
            }
        } else {
            &self.minor.to_string()
        };

        Ok(format!("{}_v{}_{}_{}", protocol, self.major, minor_str, patch))
    }

    /// Render as an identifier suitable for use in generated code.
    ///
    /// # Returns
    ///
    /// Returns a formatted version string suitable for use in generated code.
    /// Removes the 'v' prefix and replaces dots with dashes (e.g., "v30.1.0" -> "30-1-0").
    pub fn identifier(&self) -> String { self.as_str().replace('v', "").replace('.', "-") }

    /// Format version string for filename (e.g., "30.2" -> "30_2", "v30.2.1" -> "30_2_1").
    ///
    /// Replaces dots with underscores and removes the 'v' prefix to create filesystem-safe version strings.
    /// Preserves the original version format (including CalVer formatting like "25.09" -> "25_09").
    pub fn as_filename_version(&self) -> String {
        self.version_string.trim_start_matches('v').trim_start_matches('V').replace('.', "_")
    }

    /// Check if this version matches a target version with flexible matching.
    ///
    /// Supports different levels of precision:
    /// - v25 → matches any 25.x.x
    /// - v25.0 → matches any 25.0.x
    /// - v25.0.0 → exact match
    ///
    /// # Arguments
    ///
    /// * `target` - The target version to match against
    ///
    /// # Returns
    ///
    /// Returns `true` if this version is compatible with the target version
    pub fn matches_target(&self, target: &Self) -> bool {
        // If major versions don't match, no compatibility
        if self.major != target.major {
            return false;
        }

        // If target has no minor version specified (e.g., v25), match any minor
        // This is determined by checking if the target version string ends with just the major
        if target.version_string.ends_with(&format!("{}", target.major)) {
            return true;
        }

        // If minor versions don't match, no compatibility
        if self.minor != target.minor {
            return false;
        }

        // Check if either version has no patch specified
        // If target has no patch (e.g., v25.09), match any patch of the same major.minor
        if target.version_string.contains('.') {
            let target_parts: Vec<&str> = target.version_string.split('.').collect();
            if target_parts.len() == 2 {
                // Target has no patch, so any patch should match
                return true;
            }
        }

        // If self has no patch (e.g., v25.09), match any patch of the same major.minor
        if self.version_string.contains('.') {
            let self_parts: Vec<&str> = self.version_string.split('.').collect();
            if self_parts.len() == 2 {
                // Self has no patch, so any patch should match
                return true;
            }
        }

        // For exact match (e.g., v25.0.0), patch versions must match
        self.patch == target.patch
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.version_string)
    }
}

impl std::str::FromStr for ProtocolVersion {
    type Err = VersionError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> { Self::from_string(s) }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self { version_string: "0.0.0".to_string(), major: 0, minor: 0, patch: 0, protocol: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_string() {
        let version = ProtocolVersion::from_string("v1.2.3").unwrap();
        assert_eq!(version.version_string, "v1.2.3");
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
        assert_eq!(version.protocol, None);
    }

    #[test]
    fn test_from_string_with_protocol() {
        let with_patch =
            ProtocolVersion::from_string_with_protocol("v30.1.2", Some("bitcoin_core".to_string()))
                .unwrap();
        assert_eq!(with_patch.version_string, "v30.1.2");
        assert_eq!(with_patch.major, 30);
        assert_eq!(with_patch.minor, 1);
        assert_eq!(with_patch.patch, 2);
        assert_eq!(with_patch.protocol.as_deref(), Some("bitcoin_core"));

        let without_patch = ProtocolVersion::from_string_with_protocol("v25.09", None).unwrap();
        assert_eq!(without_patch.version_string, "v25.09");
        assert_eq!(without_patch.major, 25);
        assert_eq!(without_patch.minor, 9);
        assert_eq!(without_patch.patch, 0);
        assert_eq!(without_patch.protocol, None);

        let error = ProtocolVersion::from_string_with_protocol("invalid", None).unwrap_err();
        match error {
            VersionError::InvalidFormat(s) => assert_eq!(s, "invalid"),
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn test_as_str() {
        let version = ProtocolVersion {
            version_string: "v1.2.3".to_string(),
            major: 1,
            minor: 2,
            patch: 3,
            protocol: None,
        };
        assert_eq!(version.as_str(), "v1.2.3");
    }

    #[test]
    fn test_major() {
        let version = ProtocolVersion {
            version_string: "v2.3.4".to_string(),
            major: 2,
            minor: 3,
            patch: 4,
            protocol: None,
        };
        assert_eq!(version.major(), 2);
    }

    #[test]
    fn test_minor() {
        let version = ProtocolVersion {
            version_string: "v3.4.5".to_string(),
            major: 3,
            minor: 4,
            patch: 5,
            protocol: None,
        };
        assert_eq!(version.minor(), 4);
    }

    #[test]
    fn test_as_doc_version() {
        let version = ProtocolVersion {
            version_string: "v4.5.6".to_string(),
            major: 4,
            minor: 5,
            patch: 6,
            protocol: None,
        };
        assert_eq!(version.as_doc_version(), "4.5");
    }

    #[test]
    fn test_short() {
        let version = ProtocolVersion {
            version_string: "v5.6.7".to_string(),
            major: 5,
            minor: 6,
            patch: 7,
            protocol: None,
        };
        assert_eq!(version.short(), "v5.6");
    }

    #[test]
    fn test_crate_version() {
        let version = ProtocolVersion {
            version_string: "v6.7.8".to_string(),
            major: 6,
            minor: 7,
            patch: 8,
            protocol: None,
        };
        assert_eq!(version.crate_version(), "6.7.8");
    }

    #[test]
    fn test_as_module_name() {
        let without_protocol = ProtocolVersion {
            version_string: "1.2.3".to_string(),
            major: 1,
            minor: 2,
            patch: 3,
            protocol: None,
        };
        let error = without_protocol.as_module_name().unwrap_err();
        match error {
            VersionError::InvalidFormat(message) => {
                assert!(message.contains("Protocol name is required for module naming"));
            }
            _ => panic!("unexpected error variant"),
        }

        let with_protocol = ProtocolVersion {
            version_string: "v1.2.3".to_string(),
            major: 1,
            minor: 2,
            patch: 3,
            protocol: Some("bitcoin_core".to_string()),
        };
        let module_name = with_protocol.as_module_name().unwrap();
        assert_eq!(module_name, "bitcoin_core_v1_2_3");
    }

    #[test]
    fn test_identifier() {
        let version = ProtocolVersion {
            version_string: "v30.1.0".to_string(),
            major: 30,
            minor: 1,
            patch: 0,
            protocol: None,
        };
        assert_eq!(version.identifier(), "30-1-0");
    }

    #[test]
    fn test_as_filename_version() {
        let version1 = ProtocolVersion {
            version_string: "v30.2".to_string(),
            major: 30,
            minor: 2,
            patch: 0,
            protocol: None,
        };
        assert_eq!(version1.as_filename_version(), "30_2");

        let version2 = ProtocolVersion {
            version_string: "v30.2.1".to_string(),
            major: 30,
            minor: 2,
            patch: 1,
            protocol: None,
        };
        assert_eq!(version2.as_filename_version(), "30_2_1");

        let version3 = ProtocolVersion {
            version_string: "v25.09".to_string(),
            major: 25,
            minor: 9,
            patch: 0,
            protocol: None,
        };
        assert_eq!(version3.as_filename_version(), "25_09");

        let version4 = ProtocolVersion {
            version_string: "30.2".to_string(),
            major: 30,
            minor: 2,
            patch: 0,
            protocol: None,
        };
        assert_eq!(version4.as_filename_version(), "30_2");
    }

    #[test]
    fn test_matches_target() {
        let different_major_self = ProtocolVersion {
            version_string: "v24.0.0".to_string(),
            major: 24,
            minor: 0,
            patch: 0,
            protocol: None,
        };
        let different_major_target = ProtocolVersion {
            version_string: "v25.0.0".to_string(),
            major: 25,
            minor: 0,
            patch: 0,
            protocol: None,
        };
        assert!(!different_major_self.matches_target(&different_major_target));

        let self_for_major_only = ProtocolVersion {
            version_string: "v25.0.0".to_string(),
            major: 25,
            minor: 0,
            patch: 0,
            protocol: None,
        };
        let target_major_only = ProtocolVersion {
            version_string: "25".to_string(),
            major: 25,
            minor: 0,
            patch: 0,
            protocol: None,
        };
        assert!(self_for_major_only.matches_target(&target_major_only));

        let self_different_minor = ProtocolVersion {
            version_string: "v25.1.0".to_string(),
            major: 25,
            minor: 1,
            patch: 0,
            protocol: None,
        };
        let target_different_minor = ProtocolVersion {
            version_string: "v25.2.0".to_string(),
            major: 25,
            minor: 2,
            patch: 0,
            protocol: None,
        };
        assert!(!self_different_minor.matches_target(&target_different_minor));

        let self_target_minor_only_self = ProtocolVersion {
            version_string: "v25.0.1".to_string(),
            major: 25,
            minor: 0,
            patch: 1,
            protocol: None,
        };
        let target_minor_only = ProtocolVersion {
            version_string: "v25.0".to_string(),
            major: 25,
            minor: 0,
            patch: 0,
            protocol: None,
        };
        assert!(self_target_minor_only_self.matches_target(&target_minor_only));

        let self_minor_only = ProtocolVersion {
            version_string: "v25.0".to_string(),
            major: 25,
            minor: 0,
            patch: 0,
            protocol: None,
        };
        let target_with_patch = ProtocolVersion {
            version_string: "v25.0.2".to_string(),
            major: 25,
            minor: 0,
            patch: 2,
            protocol: None,
        };
        assert!(self_minor_only.matches_target(&target_with_patch));

        let self_exact_true = ProtocolVersion {
            version_string: "v25.0.3".to_string(),
            major: 25,
            minor: 0,
            patch: 3,
            protocol: None,
        };
        let target_exact_true = ProtocolVersion {
            version_string: "v25.0.3".to_string(),
            major: 25,
            minor: 0,
            patch: 3,
            protocol: None,
        };
        assert!(self_exact_true.matches_target(&target_exact_true));

        let self_exact_false = ProtocolVersion {
            version_string: "v25.0.3".to_string(),
            major: 25,
            minor: 0,
            patch: 3,
            protocol: None,
        };
        let target_exact_false = ProtocolVersion {
            version_string: "v25.0.4".to_string(),
            major: 25,
            minor: 0,
            patch: 4,
            protocol: None,
        };
        assert!(!self_exact_false.matches_target(&target_exact_false));
    }
}
