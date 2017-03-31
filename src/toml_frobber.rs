
use FROB_DIR;
use crates;
use errors::*;
use file;
use lists::Crate;
use std::fs;
use std::path::{Path, PathBuf};
use toml::{Parser, Value};
use util;

pub fn frob_toml(dir: &Path, name: &str, vers: &str, out: &Path) -> Result<()> {
    log!("frobbing {}-{}", name, vers);
    let toml_str = file::read_string(&dir.join("Cargo.toml")).chain_err(|| "no cargo.toml?")?;
    let mut parser = Parser::new(&toml_str);
    let mut toml =
        parser.parse()
            .ok_or(Error::from(format!("unable to parse Cargo.toml at {}", dir.display())))?;

    let mut changed = false;

    // Convert path dependencies to registry dependencies
    for section in &["dependencies", "dev-dependencies", "build-dependencies"] {
        let maybe_deps = toml.get_mut(*section);
        match maybe_deps {
            Some(&mut Value::Table(ref mut deps)) => {
                // Iterate through the "name = { ... }", removing any "path"
                // keys in the dependency definition
                for (dep_name, v) in deps.iter_mut() {
                    match v {
                        &mut Value::Table(ref mut dep_props) => {
                            if dep_props.contains_key("path") {
                                log!("removing path from {} in {}-{}", dep_name, name, vers);
                            }
                            if dep_props.remove("path").is_some() {
                                changed = true;
                            }
                        }
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }

    // Eliminate workspaces
    if toml.remove("workspace").is_some() {
        log!("removing workspace from {}-{}", name, vers);
        changed = true;
    }

    if changed {
        let toml = Value::Table(toml);
        file::write_string(out, &format!("{}", toml))?;

        log!("frobbed toml written to {}", out.display());
    }

    Ok(())
}
