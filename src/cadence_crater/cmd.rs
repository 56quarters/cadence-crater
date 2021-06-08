//

use crate::toml::{LocalOverride, LocalVersion};
use crate::types::CraterError;
use crate::vcs::RemoteRepo;
use clap::{crate_version, Clap};
use serde_derive::Deserialize;
use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

/// Fetch and patch projects to use the local Cadence version
#[derive(Debug, Clap)]
#[clap(name = "cadence-crater", version = crate_version ! ())]
pub struct CraterApplication {
    #[clap(long = "dest")]
    destination: Option<PathBuf>,
    cadence: PathBuf,
    config: PathBuf,
}

impl CraterApplication {
    pub fn run(self) -> Result<(), CraterError> {
        let local_cadence = LocalVersion::new(self.cadence.clone());
        let local_version = local_cadence.version()?;
        let local_path = local_cadence.path()?;

        let cfg = self.config()?;
        let downloads = self.destination()?;

        for project in cfg.projects.iter() {
            let remote = RemoteRepo::new(project.repo.clone());
            let repo = remote.download(&downloads)?;

            let root = repo.join(&project.root).join("Cargo.toml");
            let crates: Vec<PathBuf> = project
                .subprojects
                .iter()
                .map(|subproject| repo.join(&project.root).join(subproject).join("Cargo.toml"))
                .collect();

            println!("CRATES: {:?}", crates);

            let patch = LocalOverride::new(root, crates);
            patch.patch(&local_version, &local_path)?;
        }

        Ok(())
    }

    fn destination(&self) -> Result<PathBuf, CraterError> {
        let dest = self.destination.clone().unwrap_or_else(env::temp_dir);

        fs::create_dir_all(&dest)
            .map(|_| dest)
            .and_then(|p| p.canonicalize())
            .map_err(|e| CraterError::new_err("unable to determine repository destination", e))
    }

    fn config(&self) -> Result<RunConfig, CraterError> {
        let mut buf = String::new();

        let _ = fs::File::open(&self.config)
            .and_then(|mut fd| fd.read_to_string(&mut buf))
            .map_err(|e| {
                CraterError::new_err(
                    format!("unable to open configuration from {:?}", self.config),
                    e,
                )
            })?;

        toml::from_str(&buf).map_err(|e| {
            CraterError::new_err(
                format!("unable to parse configuration from {:?}", self.config),
                e,
            )
        })
    }
}

#[derive(Deserialize, Debug)]
struct RunConfig {
    projects: Vec<RunProject>,
}

#[derive(Deserialize, Debug)]
struct RunProject {
    repo: String,
    root: String,
    subprojects: Vec<String>,
}
