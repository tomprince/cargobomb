
use errors::*;
use run;
use std::fs;
use std::path::Path;
use util;

pub fn shallow_clone_or_pull(url: &str, dir: &Path) -> Result<()> {
    let ref url = frob_url(url);

    if !dir.exists() {
        log!("cloning {} into {}", url, dir.display());
        let r = run::run("git",
                         &["clone", "--depth", "1", url, &dir.to_string_lossy()],
                         &[])
                .chain_err(|| format!("unable to clone {}", url));

        if r.is_err() {
            if dir.exists() {
                fs::remove_dir_all(dir)?;
            }
        }

        r
    } else {
        log!("pulling existing url {} into {}", url, dir.display());
        run::cd_run(&dir, "git", &["pull"], &[]).chain_err(|| format!("unable to pull {}", url))
    }
}

/// Ensure that a commit exists locally in a shallow-cloned repo. This will
/// first check whether it does, and if not do increasingly deep clones until it
/// finds the commit.
pub fn shallow_fetch_sha(url: &str, dir: &Path, sha: &str) -> Result<()> {
    let ref url = frob_url(url);

    log!("ensuring sha {} in {}", sha, url);
    let depths = &[1, 10, 100, 1000];

    let exists = || if dir.exists() {
        let r = run::cd_run(dir, "git", &["log", sha], &[]);
        if r.is_ok() { true } else { false }
    } else {
        false
    };

    if exists() {
        return Ok(());
    }

    for depth in depths {
        util::try_hard(|| {
            run::run("git",
                     &["clone", "--depth", &format!("{}", depth), url, &dir.to_string_lossy()],
                     &[])
        }).chain_err(|| format!("unable to clone {}", url))?;

        if exists() {
            return Ok(());
        }
    }

    util::try_hard(|| {
            run::run("git",
                     &["clone", url, &dir.to_string_lossy()],
                     &[])
    }).chain_err(|| format!("unable to clone {}", url))?;

    if !exists() {
        Err(format!("commit {} does not exist in {}", sha, url).into())
    } else {
        Ok(())
    }
}

pub fn reset_to_sha(dir: &Path, sha: &str) -> Result<()> {
    run::cd_run(dir,
                "git",
                &["reset", "--hard", sha],
                &[])
        .chain_err(|| format!("unable to reset {} to {}", dir.display(), sha))
}

fn frob_url(url: &str) -> String {
    // With https git will interactively ask for a password for private repos.
    // Switch to the unauthenticated git protocol to just generate an error instead.
    url.replace("https://", "git://")
}
