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

    /// Render as a version module name segment for use in generated code.
    ///
    /// # Returns
    ///
    /// Returns a version string formatted for use as a module name segment.
    /// Preserves CalVer formatting and uses underscores (e.g., "v30.1.0" -> "v30_1_0", "v25.09" -> "v25_09_0").
    pub fn as_version_module_name(&self) -> String {
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

        format!("v{}_{}_{}", self.major, minor_str, self.patch)
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
