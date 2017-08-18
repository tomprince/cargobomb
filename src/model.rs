use errors::*;
use ex::{ExCrate, ExMode, Experiment};
use file;
use lists::Crate;
use results::TestResult;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use toolchain::Toolchain;
use util;

pub trait Model {
    fn load_experiment(&self, ex_name: &str) -> Result<Experiment>;
    fn create_experiment(
        &self,
        ex_name: &str,
        tcs: Vec<Toolchain>,
        crates: Vec<Crate>,
        mode: ExMode,
    ) -> Result<()>;
    fn delete_experiment(&self, ex_name: &str) -> Result<()>;


    fn write_shas(&self, ex_name: &str, shas: &HashMap<String, String>) -> Result<()>;
    fn read_shas(&self, ex_name: &str) -> Result<HashMap<String, String>>;

    fn load_test_result(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<Option<TestResult>>;
    fn delete_test_result(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<()>;
    fn read_test_log(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<fs::File>;
    fn delete_all_test_results(&self, ex_name: &str) -> Result<()>;

    fn record_test_results(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
        f: &mut FnMut() -> Result<TestResult>,
    ) -> Result<TestResult>;
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
    fn result_file(&self, ex_name: &str, crate_: &ExCrate, tc: &Toolchain) -> PathBuf {

        self.result_dir(ex_name, crate_, tc).join("results.txt")
    }
    pub fn result_log(&self, ex_name: &str, crate_: &ExCrate, tc: &Toolchain) -> PathBuf {
        self.result_dir(ex_name, crate_, tc).join("log.txt")
    }
    fn result_dir(&self, ex_name: &str, crate_: &ExCrate, tc: &Toolchain) -> PathBuf {
        use results::result_path_fragement;
        self.ex_dir(ex_name).join("res").join(
            result_path_fragement(
                crate_,
                tc,
            ),
        )
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
    fn delete_experiment(&self, ex_name: &str) -> Result<()> {
        let ex_dir = self.ex_dir(ex_name);
        if ex_dir.exists() {
            util::remove_dir_all(&ex_dir)?;
        }

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

    fn load_test_result(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<Option<TestResult>> {
        let result_file = self.result_file(ex_name, crate_, toolchain);
        if result_file.exists() {
            let s = file::read_string(&result_file)?;
            let r = s.parse::<TestResult>().chain_err(|| {
                format!("invalid test result value: '{}'", s)
            })?;
            Ok(Some(r))
        } else {
            Ok(None)
        }
    }

    fn delete_test_result(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<()> {
        let result_dir = self.result_dir(ex_name, crate_, toolchain);
        if result_dir.exists() {
            util::remove_dir_all(&result_dir)?;
        }
        Ok(())
    }
    fn read_test_log(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
    ) -> Result<fs::File> {
        let log_path = self.result_log(ex_name, crate_, toolchain);
        fs::File::open(log_path).chain_err(|| "Couldn't open result file.")
    }
    fn delete_all_test_results(&self, ex_name: &str) -> Result<()> {

        let dir = self.ex_dir(ex_name).join("res");
        if dir.exists() {
            util::remove_dir_all(&dir)?;
        }

        Ok(())
    }

    fn record_test_results(
        &self,
        ex_name: &str,
        crate_: &ExCrate,
        toolchain: &Toolchain,
        f: &mut FnMut() -> Result<TestResult>,
    ) -> Result<TestResult> {
        use log;
        self.delete_test_result(ex_name, crate_, toolchain)?;
        fs::create_dir_all(&self.result_dir(ex_name, crate_, toolchain))?;

        let log_file = self.result_log(ex_name, crate_, toolchain);
        let result_file = self.result_file(ex_name, crate_, toolchain);

        let result = log::redirect(&log_file, f)?;
        file::write_string(&result_file, &result.to_string())?;

        Ok(result)

    }
}
