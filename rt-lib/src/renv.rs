use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::{RVersion, RVersions};

impl RenvLock {
    /// Find the closest installed R version
    pub fn find_closest_r_ver<'a>(&self, versions: &'a RVersions) -> anyhow::Result<&'a RVersion> {
        versions.find_closest(&self.r.version)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RenvLock {
    pub r: RenvRVersion,
    // Note that each apckage should be untagged
    pub packages: HashMap<String, RenvPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RenvRVersion {
    pub version: String,
    pub repositories: Vec<RenvRepository>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenvRepository {
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "URL")]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RenvPackage {
    pub package: String,
    pub version: String,
    pub source: String,
    pub repository: String,
    pub requirements: Option<Vec<String>>,
    pub hash: String,
}

impl RenvLock {
    pub fn read<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = File::open(path.as_ref())?;
        let reader = BufReader::new(file);
        let renv_lock: RenvLock = serde_json::from_reader(reader)?;
        Ok(renv_lock)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let f = std::fs::File::create(&path)?;
        serde_json::to_writer_pretty(&f, self)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_renv_lock() {
        let path = "../tests/renv.lock";
        match RenvLock::read(path) {
            Ok(renv_lock) => {
                println!("{:#?}", renv_lock);
            }
            Err(err) => {
                panic!("Failed to parse renv.lock: {}", err);
            }
        }
    }

    #[test]
    fn round_trip_renv() -> anyhow::Result<()> {
        let lock = RenvLock::read("../tests/renv.lock")?;
        // Write the parsed RenvLock to a temporary file
        let temp_file_path = std::env::temp_dir().join("temp_renv.lock");

        lock.write(&temp_file_path)?;
        // Read the temporary file back and deserialize it
        let read_lock = RenvLock::read(&temp_file_path)?;

        // Print out the deserialized RenvLock
        println!("{:#?}", read_lock);

        // Clean up the temporary file
        std::fs::remove_file(&temp_file_path)?;

        Ok(())
    }

    #[test]
    fn match_renv_versions() -> anyhow::Result<()> {
        let mut lock = RenvLock::read("../tests/renv.lock")?;

        let versions = dbg!(RVersions::discover()?);
        let matched = lock.find_closest_r_ver(&versions)?;
        assert_eq!("4.4.1", matched.version.to_string());

        lock.r.version = "4.5.0".to_string();
        let matched = lock.find_closest_r_ver(&versions)?;
        assert_eq!("4.5.0-devel", matched.version.to_string());

        lock.r.version = "5.5.0".to_string();
        let matched = lock.find_closest_r_ver(&versions)?;
        assert_eq!("4.5.0-devel", matched.version.to_string());

        lock.r.version = "3.5.0".to_string();
        let matched = lock.find_closest_r_ver(&versions)?;
        assert_eq!("4.0.1", matched.version.to_string());

        Ok(())
    }
}
