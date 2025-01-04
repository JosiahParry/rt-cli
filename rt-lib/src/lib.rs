// we will discover them based on OS
// https://doc.rust-lang.org/reference/conditional-compilation.html#target_os
pub mod discover;
use discover::*;
pub mod renv;
use anyhow::anyhow;
use regex::Regex;
use semver::Version;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

/// Represents an installed version of R
#[derive(Debug, Clone)]
pub struct RVersion {
    /// A [semver::Version] representing the version of R
    pub version: Version,
    /// A [PathBuf] to the root of the R installation
    pub root: PathBuf,
}

impl RVersion {
    /// Finds the version of the default R executable
    pub fn default() -> anyhow::Result<Self> {
        // find the default binary
        let r_root = which::which("R")?
            .canonicalize()?
            .parent()
            .ok_or(anyhow!("Failed to navigate the R folder"))?
            .parent()
            .ok_or(anyhow!("Failed to navigate the R folder"))?
            .canonicalize()?;

        let ver = read_r_ver(&r_root)?;

        Ok(Self {
            version: ver,
            root: r_root,
        })
    }

    /// Creates a new [std::process::Command] calling `Rscript`
    /// pass addtional arguments via the `.args()` method.
    pub fn rscript(&self) -> Command {
        Command::new(self.root.join("Rscript"))
    }

    /// Creates a new [std::process::Command] calling `R`
    /// pass addtional arguments via the `.args()` method.
    /// ie.
    /// ```ignore
    /// rt_lib::RVersion::default().unwrap().r().args("-e", "print('hello world')").spawn()
    /// ```
    pub fn r(&self) -> Command {
        Command::new(self.root.join("R"))
    }
}

fn read_r_ver(path: &Path) -> anyhow::Result<Version> {
    // path to the version head
    let ver_h = path.join("include").join("Rversion.h");

    let content = std::fs::read_to_string(ver_h)?;

    // Define regex patterns for major, minor, and status
    let major_re = Regex::new(r#"#define R_MAJOR\s+"(\d+)""#).unwrap();
    let minor_re = Regex::new(r#"#define R_MINOR\s+"(\d+\.\d+)""#).unwrap();
    let status_re = Regex::new(r#"#define R_STATUS\s+"(.*?)""#).unwrap();

    // Capture the values
    let major = major_re
        .captures(&content)
        .ok_or(anyhow!("Failed to capture the major version"))?
        .get(1)
        .ok_or(anyhow!("Failed to capture the major version"))?
        .as_str();
    let minor = minor_re
        .captures(&content)
        .ok_or(anyhow!("Failed to capture the minor version"))?
        .get(1)
        .ok_or(anyhow!("Failed to capture the minor version"))?
        .as_str();
    let status = status_re
        .captures(&content)
        .ok_or(anyhow!("Failed to capture the version status"))?
        .get(1)
        .ok_or(anyhow!("Failed to capture the version status"))?
        .as_str();

    let status = if status.is_empty() { "" } else { "-devel" };

    Ok(Version::parse(&format!("{major}.{minor}{status}"))?)
}

#[derive(Debug, Clone, Default)]
pub struct RVersions {
    pub default: Option<RVersion>,
    pub versions: Vec<RVersion>,
}

impl RVersions {
    pub fn discover() -> anyhow::Result<Self> {
        if cfg!(target_os = "macos") {
            Ok(discover_mac()?)
        } else if cfg!(target_os = "linux") {
            Ok(discover_linux()?)
        } else if cfg!(target_os = "windows") {
            Ok(discover_windows()?)
        } else {
            Err(anyhow!("Unsupported OS"))
        }
    }

    pub fn find_closest(&self, target_version: &str) -> anyhow::Result<&RVersion> {
        if self.versions.is_empty() {
            return Err(anyhow!("No versions available"));
        }

        // Parse the target version
        let target = Version::parse(target_version)
            .map_err(|_| anyhow!("Invalid version format: {}", target_version))?;

        // Group versions by their major version in a BTreeMap
        let mut ver_map: BTreeMap<u64, Vec<&RVersion>> = BTreeMap::new();
        for ver in &self.versions {
            ver_map.entry(ver.version.major).or_default().push(ver);
        }

        // If the target major version is larger than the biggest key
        if let Some(biggest_key) = ver_map.keys().max() {
            if target.major > *biggest_key {
                if let Some(versions) = ver_map.get(biggest_key) {
                    // Return the largest version regardless of pre-release
                    return versions
                        .iter()
                        .max_by(|a, b| a.version.cmp(&b.version))
                        .copied()
                        .ok_or_else(|| anyhow!("No versions found in the largest group"));
                }
            }
        }

        // Ensure all version groups are sorted
        for versions in ver_map.values_mut() {
            versions.sort_by(|a, b| a.version.cmp(&b.version));
        }

        if let Some(versions) = ver_map.get(&target.major) {
            // Iterate over the versions for the target major version
            for version in versions.iter().rev() {
                // Return immediately if an exact match is found (including pre-releases)
                if version.version == target {
                    return Ok(*version);
                }

                // if they're all a match except the pre-release return it
                if version.version.major == target.major
                    && version.version.minor == target.minor
                    && version.version.patch == target.patch
                {
                    return Ok(*version);
                }
                // Otherwise, find the closest non-pre-release version
                if version.version <= target && version.version.pre.is_empty() {
                    return Ok(*version);
                }
            }
            let fallback = versions.get(versions.len()).cloned().unwrap_or(versions[0]);
            return Ok(fallback);
        }

        // If no match in the same major version, look for the next higher major version
        if let Some((_, versions)) = ver_map.range(target.major + 1..).next() {
            return versions
                .iter()
                .find(|v| v.version.pre.is_empty())
                .copied()
                .ok_or_else(|| anyhow!("No suitable version found in higher major versions"));
        }

        // Fall back to the largest version across all groups
        ver_map
            .values()
            .flat_map(|v| v.iter())
            .rev()
            .find(|v| v.version.pre.is_empty())
            .copied()
            .ok_or_else(|| anyhow!("No suitable version found"))
    }
}

#[cfg(test)]
mod tests {
    use crate::{RVersion, RVersions};
    use semver::Version;

    #[test]
    fn test_rver_matches() -> anyhow::Result<()> {
        let vers = RVersions::discover()?;
        println!("Discovered R versions {vers:#?}");
        let closest = vers.find_closest("4.3.1111");
        println!("~~\nMatched version: {closest:#?}");
        Ok(())
    }

    #[test]
    fn test_rver_matche_edge_cases() {
        let mut r_versions = RVersions {
            default: None,
            versions: vec![
                RVersion {
                    version: Version::parse("4.4.1").unwrap(),
                    root: "/Library/Frameworks/R.framework/Versions/4.4-arm64/Resources".into(),
                },
                RVersion {
                    version: Version::parse("4.5.0-devel").unwrap(),
                    root: "/Library/Frameworks/R.framework/Versions/4.5-arm64/Resources".into(),
                },
                RVersion {
                    version: Version::parse("4.3.3").unwrap(),
                    root: "/Library/Frameworks/R.framework/Versions/4.3-arm64/Resources".into(),
                },
                RVersion {
                    version: Version::parse("4.1.3").unwrap(),
                    root: "/Library/Frameworks/R.framework/Versions/4.1-arm64/Resources".into(),
                },
            ],
        };

        assert_eq!(
            r_versions.find_closest("4.4.0").unwrap().version,
            Version::parse("4.3.3").unwrap()
        );

        // find the lowest available if possible w/in minor
        assert_eq!(
            r_versions.find_closest("4.4.9").unwrap().version,
            Version::parse("4.4.1").unwrap()
        );

        // find lowest available w/in minor
        assert_eq!(
            r_versions.find_closest("4.3.9").unwrap().version,
            Version::parse("4.3.3").unwrap()
        );

        // find highest available (non-devel) from nearest major
        assert_eq!(
            r_versions.find_closest("5.0.0").unwrap().version,
            Version::parse("4.4.1").unwrap()
        );

        // go up a major if needed
        assert_eq!(
            r_versions.find_closest("3.5.0").unwrap().version,
            Version::parse("4.1.3").unwrap()
        );

        // find within same major going up
        let mut new363 = r_versions.versions[0].clone();
        new363.version = Version::parse("3.6.3").unwrap();
        r_versions.versions.push(new363);

        assert_eq!(
            r_versions.find_closest("3.5.0").unwrap().version,
            Version::parse("3.6.3").unwrap()
        );

        // going up a major versions
        assert_eq!(
            r_versions.find_closest("2.5.0").unwrap().version,
            Version::parse("3.6.3").unwrap()
        );

        // pre-release exact match
        assert_eq!(
            r_versions.find_closest("4.5.0-devel").unwrap().version,
            Version::parse("4.5.0-devel").unwrap()
        );

        // pre-release partial match
        assert_eq!(
            r_versions.find_closest("4.5.0").unwrap().version,
            Version::parse("4.5.0-devel").unwrap()
        );
    }
}
