//

use clap::{crate_version, Clap};
use git2::Repository;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use toml::value::{Table, Value};

macro_rules! toml_map (
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = toml::value::Map::new();
            $(
                m.insert($key.to_owned(), $value);
            )+
            m
        }
     };
);

/// Break stuff
#[derive(Debug, Clap)]
#[clap(name = "cadence-crater", version = crate_version ! ())]
struct CraterOptions {
    #[clap(subcommand)]
    mode: SubCommand,
}

#[derive(Debug, Clap)]
enum SubCommand {
    Fetch(FetchCommand),
    Patch(PatchCommand),
    Build(BuildCommand),
    Test(TestCommand),
}

/// Clone a repository
#[derive(Debug, Clap)]
struct FetchCommand {
    /// Destination for repository clone (not including repository name)
    #[clap(long)]
    path: Option<String>,

    /// URL of repository to clone
    url: String,
}

impl FetchCommand {
    fn run(self) -> Result<(), CraterError> {
        let into = self
            .path
            .as_ref()
            .map(|s| PathBuf::from(s))
            .unwrap_or_else(|| env::temp_dir())
            .canonicalize()?;

        let full = into.join(self.proj_name()?);
        let _repo = match Repository::clone(&self.url, &full) {
            Err(e) => {
                if e.code() == git2::ErrorCode::Exists {
                    Ok(Repository::open(&full)?)
                } else {
                    Err(e)
                }
            }
            Ok(r) => Ok(r),
        }?;

        println!("{}", full.as_os_str().to_str().unwrap());
        Ok(())
    }

    fn proj_name(&self) -> Result<String, CraterError> {
        PathBuf::from(&self.url)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| {
                CraterError::Message(format!(
                    "unable to determine project name from {:?}",
                    self.path
                ))
            })
    }
}

/// Patch a project's Cargo.toml
#[derive(Debug, Clap)]
struct PatchCommand {
    /// Path to a local Cadence Cargo.toml
    #[clap(long)]
    cadence: String,

    /// Print Cargo.toml changes to stdout instead of modifying it
    #[clap(long)]
    stdout: bool,

    /// Path to the root project Cargo.toml and child project Cargo.toml files.
    /// If only a single path is given, it is used as both the root and child
    paths: Vec<String>,
}

impl PatchCommand {
    fn run(self) -> Result<(), CraterError> {
        let cadence_version = local_crate_version(&self.cadence)?;
        let cadence_path = local_crate_path(&self.cadence)?;

        if self.paths.len() == 1 {
            let mut root = load_cargo_toml(&self.paths[0])?;
            let table = root.as_table_mut().unwrap();

            self.override_patch(table, &cadence_path)?;
            self.override_version(table, &cadence_version)?;

            let contents = toml::to_string(&root)?;
            self.write_cargo_toml(&contents, &self.paths[0])?;
        } else {
        }

        Ok(())
    }

    fn override_patch<S: Into<String>>(
        &self,
        table: &mut Table,
        path: S,
    ) -> Result<(), CraterError> {
        table.insert(
            "patch".to_owned(),
            Value::Table(toml_map!["crates-io" => Value::Table(
                toml_map!["cadence" => Value::Table(
                    toml_map!["path" => Value::String(path.into())]
                )]
            )]),
        );

        Ok(())
    }

    fn override_version<S: Into<String>>(
        &self,
        table: &mut Table,
        version: S,
    ) -> Result<(), CraterError> {
        table
            .get_mut("dependencies")
            .and_then(|t| t.as_table_mut())
            .ok_or_else(|| {
                CraterError::Message("missing or corrupt dependency section".to_owned())
            })?
            .insert("cadence".to_owned(), Value::String(version.into()));

        Ok(())
    }

    /// Write the given TOML file contents to a path, using a temporary file and rename
    fn write_cargo_toml<P: AsRef<Path>>(&self, contents: &str, path: P) -> Result<(), CraterError> {
        let tmp_path = self.tmp_path(&path)?;
        {
            let mut fd = fs::OpenOptions::new()
                .read(false)
                .write(true)
                .create(true)
                .open(&tmp_path)?;

            fd.write_all(contents.as_bytes())?;
            fd.flush()?;
            fd.sync_all()?;
        }

        Ok(fs::rename(tmp_path, &path)?)
    }

    /// Get a temporary name for a file in the same directory as the given path
    fn tmp_path<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, CraterError> {
        let path = path.as_ref();
        path.parent()
            .ok_or_else(|| {
                CraterError::Message(format!("could not determine parent of {:?}", path))
            })
            .map(|p| p.join(".cadence-rename"))
    }
}

/// Build a project
#[derive(Debug, Clap)]
struct BuildCommand {
    path: String,
}

impl BuildCommand {
    fn run(self) -> Result<(), CraterError> {
        println!("BUILD: {:?}", self);
        Ok(())
    }
}

/// Test a project
#[derive(Debug, Clap)]
struct TestCommand {
    path: String,
}

impl TestCommand {
    fn run(self) -> Result<(), CraterError> {
        println!("TEST: {:?}", self);
        Ok(())
    }
}

#[derive(Debug)]
enum CraterError {
    VcsError(git2::Error),
    DeserializeError(toml::de::Error),
    SerializeError(toml::ser::Error),
    IoError(io::Error),
    Message(String),
}

impl From<git2::Error> for CraterError {
    fn from(e: git2::Error) -> Self {
        CraterError::VcsError(e)
    }
}

impl From<io::Error> for CraterError {
    fn from(e: io::Error) -> Self {
        CraterError::IoError(e)
    }
}

impl From<toml::de::Error> for CraterError {
    fn from(e: toml::de::Error) -> Self {
        CraterError::DeserializeError(e)
    }
}

impl From<toml::ser::Error> for CraterError {
    fn from(e: toml::ser::Error) -> Self {
        CraterError::SerializeError(e)
    }
}

impl fmt::Display for CraterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CraterError::VcsError(ref e) => e.fmt(f),
            CraterError::DeserializeError(ref e) => e.fmt(f),
            CraterError::SerializeError(ref e) => e.fmt(f),
            CraterError::IoError(ref e) => e.fmt(f),
            CraterError::Message(ref e) => e.fmt(f),
        }
    }
}

impl Error for CraterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            CraterError::VcsError(ref e) => Some(e),
            CraterError::DeserializeError(ref e) => Some(e),
            CraterError::SerializeError(ref e) => Some(e),
            CraterError::IoError(ref e) => Some(e),
            CraterError::Message(_) => None,
        }
    }
}

/// Load and parse the contents of a Cargo.toml file
fn load_cargo_toml<P: AsRef<Path>>(path: P) -> Result<Value, CraterError> {
    let mut fd = fs::File::open(&path)?;
    let mut buf = String::new();
    let _ = fd.read_to_string(&mut buf)?;
    Ok(buf.parse()?)
}

/// Get the version of a local crate based on the path to its Cargo.toml file
fn local_crate_version<P: AsRef<Path> + fmt::Debug>(path: P) -> Result<String, CraterError> {
    let root = load_cargo_toml(&path)?;

    root.as_table()
        .and_then(|v| v.get("package"))
        .and_then(|v| v.as_table())
        .and_then(|v| v.get("version"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .ok_or_else(|| {
            CraterError::Message(format!(
                "unable to determine Cadence version from {:?}",
                path
            ))
        })
}

/// Get the path of a crate as a string based on the path to its Cargo.toml file
fn local_crate_path<P: AsRef<Path> + fmt::Debug>(path: P) -> Result<String, CraterError> {
    path.as_ref()
        .parent()
        .ok_or_else(|| CraterError::Message(format!("unable to determine parent of {:?}", path)))?
        .canonicalize()?
        .to_str()
        .map(|s| s.to_owned())
        .ok_or_else(|| {
            CraterError::Message(format!("unable to normalize path to Cadence {:?}", path))
        })
}

fn main() -> Result<(), CraterError> {
    let opts = CraterOptions::parse();

    match opts.mode {
        SubCommand::Fetch(cmd) => cmd.run(),
        SubCommand::Patch(cmd) => cmd.run(),
        SubCommand::Build(cmd) => cmd.run(),
        SubCommand::Test(cmd) => cmd.run(),
    }
}
