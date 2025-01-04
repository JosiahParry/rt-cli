// we will discover them based on OS
// https://doc.rust-lang.org/reference/conditional-compilation.html#target_os
pub mod discover;
use discover::*;
pub mod renv;
use anyhow::anyhow;
use regex::Regex;
use semver::Version;
use std::{
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

        let target = Version::parse(target_version)
            .map_err(|_| anyhow!("Invalid version format: {}", target_version))?;

        // Sort the versions once
        let mut sorted_versions: Vec<&RVersion> = self.versions.iter().collect();
        sorted_versions.sort_by(|a, b| a.version.cmp(&b.version));

        // Track the best matches during a single pass
        let mut exact_match = None;
        let mut closest_within_minor = None;
        let mut highest_within_minor = None;
        let mut next_higher_minor = None;
        let mut largest_version = None;

        for r_version in &sorted_versions {
            // Update largest version (always keep track of the latest seen)
            largest_version = Some(*r_version);

            if r_version.version == target {
                exact_match = Some(*r_version);
                break; // Exact match, no need to continue
            }

            if r_version.version.major == target.major && r_version.version.minor == target.minor {
                // Closest within the same minor version
                if &r_version.version >= &target {
                    closest_within_minor = Some(*r_version);
                }
                // Track the highest version within the minor version
                highest_within_minor = Some(*r_version);
            } else if r_version.version.major == target.major
                && r_version.version.minor > target.minor
            {
                // Track the next higher minor version
                next_higher_minor = next_higher_minor.or(Some(*r_version));
            }
        }

        // Return the best match based on priority
        if let Some(exact) = exact_match {
            Ok(exact)
        } else if let Some(closest) = closest_within_minor {
            Ok(closest)
        } else if let Some(highest) = highest_within_minor {
            Ok(highest)
        } else if let Some(next_minor) = next_higher_minor {
            Ok(next_minor)
        } else {
            largest_version.ok_or_else(|| anyhow!("No versions available"))
        }
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
        let r_versions = RVersions {
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

        // find highest within the same patch
        assert_eq!(
            r_versions.find_closest("4.4.0").unwrap().version,
            Version::parse("4.4.1").unwrap()
        );
        // find the lowest version within the same minor version
        assert_eq!(
            r_versions.find_closest("4.4.9").unwrap().version,
            Version::parse("4.4.1").unwrap()
        );
        // find the lowest version within the same minor version
        assert_eq!(
            r_versions.find_closest("4.3.9").unwrap().version,
            Version::parse("4.3.3").unwrap()
        );
        // find the lowest next major version
        assert_eq!(
            r_versions.find_closest("5.0.0").unwrap().version,
            Version::parse("4.5.0-devel").unwrap()
        );
    }
}
