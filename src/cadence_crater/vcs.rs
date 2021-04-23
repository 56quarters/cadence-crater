use crate::types::CraterError;
use git2::Repository;
use std::path::{Path, PathBuf};

/// Clone a repository
#[derive(Debug)]
pub struct RemoteRepo {
    url: String,
}

impl RemoteRepo {
    pub fn new(url: String) -> Self {
        RemoteRepo { url }
    }

    pub fn download<P: AsRef<Path>>(&self, into: P) -> Result<PathBuf, CraterError> {
        let full = into.as_ref().join(self.proj_name()?);
        let _repo = Repository::clone(&self.url, &full)
            .or_else(|e| {
                if e.code() == git2::ErrorCode::Exists {
                    Repository::open(&full)
                } else {
                    Err(e)
                }
            })
            .map_err(|e| {
                CraterError::new_err(
                    format!(
                        "unable to clone or open repository {} at {:?}",
                        self.url, full
                    ),
                    e,
                )
            })?;

        Ok(full)
    }

    fn proj_name(&self) -> Result<String, CraterError> {
        PathBuf::from(&self.url)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| {
                CraterError::new(format!(
                    "unable to determine project name from {:?}",
                    self.url
                ))
            })
    }
}
