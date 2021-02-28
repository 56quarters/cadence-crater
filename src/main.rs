//

use clap::{crate_version, Clap};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use toml::value::Value;

const DEFAULT_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    url: String,
}

impl FetchCommand {
    fn run(self) -> Result<(), CraterError> {
        println!("FETCH: {:?}", self);
        Ok(())
    }
}

/// Patch a project's Cargo.toml
#[derive(Debug, Clap)]
struct PatchCommand {
    /// Print Cargo.toml changes to stdout instead of modifying it
    #[clap(long)]
    stdout: bool,

    /// Path to the Cargo.toml to patch
    path: String,
}

impl PatchCommand {
    fn run(self) -> Result<(), CraterError> {
        let mut root = self.load_cargo_toml()?;
        let table = root.as_table_mut().unwrap();

        table
            .get_mut("dependencies")
            .and_then(|t| t.as_table_mut())
            .ok_or_else(|| CraterError::Message(format!("missing or corrupt dependency section in {}", self.path)))?
            .insert("cadence".to_owned(), Value::String(DEFAULT_VERSION.to_owned()));

        let our_crate = local_crate_path("cadence")?;
        table.insert(
            "patch".to_owned(),
            Value::Table(toml_map!["crates-io" => Value::Table(
                toml_map!["cadence" => Value::Table(
                    toml_map!["path" => Value::String(our_crate)]
                )]
            )]),
        );

        let contents = toml::to_string(table)?;
        if !self.stdout {
            self.write_cargo_toml(&contents)?;
        } else {
            println!("{}", contents);
        }

        Ok(())
    }

    fn load_cargo_toml(&self) -> Result<Value, CraterError> {
        let mut fd = std::fs::File::open(&self.path)?;
        let mut buf = String::new();
        let _ = fd.read_to_string(&mut buf)?;
        Ok(buf.parse()?)
    }

    fn write_cargo_toml(&self, contents: &str) -> Result<(), CraterError> {
        let tmp_path = self.tmp_path()?;
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

        Ok(fs::rename(tmp_path, &self.path)?)
    }

    fn tmp_path(&self) -> Result<PathBuf, CraterError> {
        let path = Path::new(&self.path);
        path.parent()
            .ok_or_else(|| CraterError::Message(format!("could not determine parent of {:?}", path)))
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
    DeserializeError(toml::de::Error),
    SerializeError(toml::ser::Error),
    IoError(io::Error),
    Message(String),
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
            CraterError::DeserializeError(ref e) => Some(e),
            CraterError::SerializeError(ref e) => Some(e),
            CraterError::IoError(ref e) => Some(e),
            CraterError::Message(_) => None,
        }
    }
}

fn local_crate_path<P: Into<String>>(project: P) -> Result<String, CraterError> {
    std::env::current_dir()?
        .join("..")
        .join(project.into())
        .canonicalize()?
        .to_str()
        .ok_or_else(|| CraterError::Message("path conversion error".to_owned()))
        .map(|s| s.to_owned())
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
