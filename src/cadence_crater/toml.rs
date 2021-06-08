//

use crate::types::CraterError;
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

/// Determine the version and path to a local Cadence checkout
#[derive(Debug)]
pub struct LocalVersion {
    cargo_toml: PathBuf,
}

impl LocalVersion {
    /// Create a new `LocalVersion` given a path to a `cadence` crate Cargo.toml
    pub fn new<P: Into<PathBuf>>(cargo_toml: P) -> Self {
        LocalVersion {
            cargo_toml: cargo_toml.into(),
        }
    }

    /// Determine the version of the local Cadence crate or return an error
    ///
    /// Errors will be returned if
    /// * The Cargo.toml file cannot be read
    /// * The Cargo.toml file isn't syntactically valid
    /// * The the "version" key is missing from the Cargo.toml file
    pub fn version(&self) -> Result<String, CraterError> {
        let root = match load_cargo_toml(&self.cargo_toml) {
            Err(e) => {
                return Err(CraterError::new_err(
                    format!(
                        "unable to open {:?} to determine Cadence version",
                        &self.cargo_toml
                    ),
                    e,
                ));
            }
            Ok(v) => v,
        };

        root.as_table()
            .and_then(|v| v.get("package"))
            .and_then(|v| v.as_table())
            .and_then(|v| v.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .ok_or_else(|| {
                CraterError::new(format!(
                    "unable to determine Cadence version from {:?}",
                    &self.cargo_toml
                ))
            })
    }

    /// Get the path to a local Cadence crate as a string or return an error
    ///
    /// Errors will be returned if
    /// * The canonical path to the crate could not be determined
    pub fn path(&self) -> Result<String, CraterError> {
        self.cargo_toml
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .and_then(|p| p.to_str().map(|s| s.to_owned()))
            .ok_or_else(|| {
                CraterError::new(format!(
                    "unable to determine crate path to Cadence from {:?}",
                    &self.cargo_toml
                ))
            })
    }
}

/// Patch a project's Cargo.toml to use a local Cadence crate
#[derive(Debug)]
pub struct LocalOverride {
    root: PathBuf,
    crates: Vec<PathBuf>,
}

impl LocalOverride {
    /// Create a new `LocalOverride` to modify the given project root and sub-crates
    ///
    /// Multi-crate workspaces should pass the workspace Cargo.toml for `root` and
    /// the Cargo.toml of each crate contained in the workspace to be patched in the
    /// `crates` vector
    pub fn new(root: PathBuf, crates: Vec<PathBuf>) -> Self {
        LocalOverride { root, crates }
    }

    /// Patch the root and each sub-crate to use the provided local Cadence version
    ///
    /// Patching may fail:
    /// * If the local Cadence Cargo.toml can't be read or parsed
    /// * If the project Cargo.toml can't be read or parsed
    /// * If the project Cargo.toml can't be written after being modified
    pub fn patch(&self, version: &str, path: &str) -> Result<(), CraterError> {
        let mut root = load_cargo_toml(&self.root)?;
        let root_table = root.as_table_mut().unwrap();

        // patch the source for Cadence in the root Cargo.toml
        override_source(root_table, path);

        if self.crates.is_empty() {
            // there are no subprojects so just update the version required in the root
            override_version(root_table, version);
        } else {
            // open each Cargo.toml for the subprojects and update the version required
            for crate_path in self.crates.iter() {
                let mut crate_root = load_cargo_toml(crate_path)?;
                let crate_root_table = crate_root.as_table_mut().unwrap();
                override_version(crate_root_table, version);
                write_cargo_toml(crate_path, crate_root)?;
            }
        }

        write_cargo_toml(&self.root, root)
    }
}

/// Change Cadence dependencies to a local checkout for the given Cargo.toml structure
fn override_source<S: Into<String>>(table: &mut Table, path: S) {
    table.insert(
        "patch".to_owned(),
        Value::Table(toml_map!["crates-io" => Value::Table(
            toml_map!["cadence" => Value::Table(
                toml_map!["path" => Value::String(path.into())]
            )]
        )]),
    );
}

/// Change the version of Cadence required for the given Cargo.toml structure
fn override_version<S: Into<String>>(table: &mut Table, version: S) -> bool {
    table
        .get_mut("dependencies")
        .and_then(|t| t.as_table_mut())
        .and_then(|t| t.insert("cadence".to_owned(), Value::String(version.into())))
        .is_some()
}

/// Serialize and write a TOML structure to the given file
fn write_cargo_toml<P>(path: P, root: Value) -> Result<(), CraterError>
where
    P: AsRef<Path> + fmt::Debug,
{
    let contents = toml::to_string(&root).map_err(|e| {
        CraterError::new_err(
            format!("unable to serialize TOML for writing to {:?}", &path),
            e,
        )
    })?;

    // Wrap this section in a closure so we can use short-circuiting via the `?`
    // operator but only do a single `.map_err()` call to convert to a meaningful
    // crater error.
    let write_and_rename = move |p: &P| {
        let tmp_path = p.as_ref().parent().unwrap().join(".cadence-rename");
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

        Ok(fs::rename(tmp_path, &p)?)
    };

    write_and_rename(&path).map_err(|e: io::Error| {
        CraterError::new_err(format!("unable to write to TOML file {:?}", &path), e)
    })
}

/// Load and parse the contents of a Cargo.toml file
fn load_cargo_toml<P>(path: P) -> Result<Value, CraterError>
where
    P: AsRef<Path> + fmt::Debug,
{
    let mut buf = String::new();

    fs::File::open(&path)
        .and_then(|mut f| f.read_to_string(&mut buf))
        .map_err(|e| CraterError::new_err(format!("unable to read TOML file {:?}", &path), e))?;

    match buf.parse() {
        Ok(v) => Ok(v),
        Err(e) => Err(CraterError::new_err(
            format!("unable to parse TOML file {:?}", &path),
            e,
        )),
    }
}
