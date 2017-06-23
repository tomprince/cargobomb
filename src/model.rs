use errors::*;
use ex::{ExMode, Experiment};
use file;
use lists::Crate;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use toolchain::Toolchain;

pub trait Model {
    fn load_experiment(&self, ex_name: &str) -> Result<Experiment>;
    fn create_experiment(
        &self,
        ex_name: &str,
        tcs: Vec<Toolchain>,
        crates: Vec<Crate>,
        mode: ExMode,
    ) -> Result<()>;

    fn write_shas(&self, ex_name: &str, shas: &HashMap<String, String>) -> Result<()>;
    fn read_shas(&self, ex_name: &str) -> Result<HashMap<String, String>>;
}


pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn open(root: PathBuf) -> FsStore {
        FsStore { root }
    }

    fn config_file(&self, ex_name: &str) -> PathBuf {
        self.ex_dir(ex_name).join("config.json")
    }
    fn sha_file(&self, ex_name: &str) -> PathBuf {
        self.ex_dir(ex_name).join("shas.json")
    }
    fn ex_dir(&self, ex_name: &str) -> PathBuf {
        self.root.join(ex_name)
    }
}

impl Model for FsStore {
    fn load_experiment(&self, ex_name: &str) -> Result<Experiment> {
        let config = file::read_string(&self.config_file(ex_name))?;
        Ok(serde_json::from_str(&config)?)
    }
    fn create_experiment(
        &self,
        ex_name: &str,
        tcs: Vec<Toolchain>,
        crates: Vec<Crate>,
        mode: ExMode,
    ) -> Result<()> {
        info!(
            "defining experiment {} for {} crates",
            ex_name,
            crates.len()
        );
        let ex = Experiment {
            name: ex_name.to_string(),
            crates: crates,
            toolchains: tcs,
            mode: mode,
        };
        fs::create_dir_all(&self.ex_dir(&ex.name))?;
        let json = serde_json::to_string(&ex)?;
        info!(
            "writing ex config to {}",
            self.config_file(ex_name).display()
        );
        file::write_string(&self.config_file(ex_name), &json)?;
        Ok(())
    }

    fn write_shas(&self, ex_name: &str, shas: &HashMap<String, String>) -> Result<()> {
        if !self.ex_dir(ex_name).exists() {
            Err(ErrorKind::ExperimentMissing(ex_name.into()))?
        }
        let shajson = serde_json::to_string(&shas)?;
        let sha_file = self.sha_file(ex_name);
        info!("writing shas to {}", sha_file.display());
        file::write_string(&sha_file, &shajson)?;
        Ok(())
    }

    fn read_shas(&self, ex_name: &str) -> Result<HashMap<String, String>> {
        let shas = file::read_string(&self.sha_file(ex_name))?;
        let shas = serde_json::from_str(&shas)?;
        Ok(shas)
    }
}
