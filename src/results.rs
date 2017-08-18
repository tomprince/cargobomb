use errors::*;
use ex::ExCrate;
use gh_mirrors;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use toolchain::Toolchain;

fn crate_to_dir(c: &ExCrate) -> String {
    match *c {
        ExCrate::Version {
            ref name,
            ref version,
        } => format!("reg/{}-{}", name, version),
        ExCrate::Repo { ref url, ref sha } => {
            let (org, name) =
                gh_mirrors::gh_url_to_org_and_name(url).expect("malformed github repo name");
            format!("gh/{}.{}.{}", org, name, sha)
        }
    }
}

/// Return a path fragement that can be used to identify this crate and
/// toolchain.
pub fn result_path_fragement(crate_: &ExCrate, toolchain: &Toolchain) -> PathBuf {
    let tc = toolchain.rustup_name();
    PathBuf::from(tc).join(crate_to_dir(crate_))
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum TestResult {
    BuildFail,
    TestFail,
    TestPass,
}
impl Display for TestResult {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.to_string().fmt(f)
    }
}

impl FromStr for TestResult {
    type Err = Error;

    fn from_str(s: &str) -> Result<TestResult> {
        match s {
            "build-fail" => Ok(TestResult::BuildFail),
            "test-fail" => Ok(TestResult::TestFail),
            "test-pass" => Ok(TestResult::TestPass),
            _ => Err(format!("bogus test result: {}", s).into()),
        }
    }
}

impl TestResult {
    fn to_string(&self) -> String {
        match *self {
            TestResult::BuildFail => "build-fail",
            TestResult::TestFail => "test-fail",
            TestResult::TestPass => "test-pass",
        }.to_string()
    }
}
