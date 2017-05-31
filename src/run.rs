#![deny(unused_must_use)]

use errors::*;
use futures::{Future, Stream};
use futures::stream::MergedItem;
use libc::{SIGKILL, kill, pid_t};
use slog_scope;
use std::io::{self, BufReader};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;
use tokio_core::reactor::Core;
use tokio_io::io::lines;
use tokio_process::CommandExt as TokioCommand;
use tokio_timer;

pub fn run(name: &str, args: &[&str], env: &[(&str, &str)]) -> Result<()> {
    run_full(None, name, args, env)?;
    Ok(())
}

pub fn cd_run(cd: &Path, name: &str, args: &[&str], env: &[(&str, &str)]) -> Result<()> {
    run_full(Some(cd), name, args, env)?;
    Ok(())
}

pub fn run_full(cd: Option<&Path>, name: &str, args: &[&str], env: &[(&str, &str)]) -> Result<()> {
    let cmdstr = make_cmdstr(name, args);
    let mut cmd = Command::new(name);

    cmd.args(args);
    for &(k, v) in env {
        cmd.env(k, v);
    }

    if let Some(cd) = cd {
        cmd.current_dir(cd);
    }

    info!("running `{}`", cmdstr);
    let out = log_command(cmd)?;

    if out.status.success() {
        Ok(())
    } else {
        Err(format!("command `{}` failed", cmdstr).into())
    }
}

pub fn run_capture(cd: Option<&Path>,
                   name: &str,
                   args: &[&str],
                   env: &[(&str, &str)])
                   -> Result<(Vec<String>, Vec<String>)> {
    let cmdstr = make_cmdstr(name, args);
    let mut cmd = Command::new(name);

    cmd.args(args);
    for &(k, v) in env {
        cmd.env(k, v);
    }

    if let Some(cd) = cd {
        cmd.current_dir(cd);
    }

    info!("running `{}`", cmdstr);
    let out = log_command_capture(cmd)?;

    if out.status.success() {
        Ok((out.stdout, out.stderr))
    } else {
        Err(format!("command `{}` failed", cmdstr).into())
    }
}

fn make_cmdstr(name: &str, args: &[&str]) -> String {
    assert!(!args.is_empty(), "case not handled");
    format!("{} {}", name, args.join(" "))
}

struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<String>,
    stderr: Vec<String>,
}

fn log_command(cmd: Command) -> Result<ProcessOutput> {
    log_command_(cmd, false)
}

fn log_command_capture(cmd: Command) -> Result<ProcessOutput> {
    log_command_(cmd, true)
}

const MAX_TIMEOUT_SECS: u64 = 60 * 10 * 2;
const HEARTBEAT_TIMEOUT_SECS: u64 = 60 * 2;

fn log_command_(mut cmd: Command, capture: bool) -> Result<ProcessOutput> {
    let mut core = Core::new().unwrap();
    let timer = tokio_timer::wheel()
        .max_timeout(Duration::from_secs(MAX_TIMEOUT_SECS * 2))
        .build();
    let mut child = cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .before_exec(|| {
                         ::nix::unistd::setsid()?;
                         Ok(())
                     })
        .spawn_async(&core.handle())?;

    let stdout = child.stdout().take().expect("");
    let stderr = child.stderr().take().expect("");

    // Needed for killing after timeout
    let child_id = child.id() as pid_t;

    let heartbeat_timeout = Duration::from_secs(HEARTBEAT_TIMEOUT_SECS);

    let logger = slog_scope::logger();
    let stdout = lines(BufReader::new(stdout)).map({
                                                       let logger = logger.clone();
                                                       move |line| {
                                                           slog_info!(logger, "blam! {}", line);
                                                           line
                                                       }
                                                   });
    let stderr = lines(BufReader::new(stderr)).map({
                                                       let logger = logger.clone();
                                                       move |line| {
                                                           slog_info!(logger, "kablam! {}", line);
                                                           line
                                                       }
                                                   });
    let output = Stream::merge(stdout, stderr);
    let output = timer
        .timeout_stream(output, heartbeat_timeout)
        .map_err(move |e| if e.kind() == io::ErrorKind::TimedOut {
                     kill_process(child_id);
                     Error::from(ErrorKind::Timeout("not generating output for ",
                                                    HEARTBEAT_TIMEOUT_SECS))
                 } else {
                     e.into()
                 });
    let output: Box<Future<Item = _, Error = Error>> = if capture {
        unmerge(output)
    } else {
        Box::new(output
                     .for_each(|_| Ok(()))
                     .and_then(|_| Ok((Vec::new(), Vec::new()))))
    };

    #[cfg(unix)]
    fn kill_process(id: pid_t) {
        let r = unsafe { kill(-id, SIGKILL) };
        if r != 0 {
            // Something went wrong...
        }
    }
    #[cfg(windows)]
    fn kill_process(id: u32) {
        unsafe {
            let handle = kernel32::OpenProcess(winapi::winnt::PROCESS_TERMINATE, 0, id);
            kernel32::TerminateProcess(handle, 101);
            if kernel32::CloseHandle(handle) == 0 {
                panic!("CloseHandle for process {} failed", id);
            }
        };
    }

    let child = timer
        .timeout(child, Duration::from_secs(MAX_TIMEOUT_SECS))
        .map_err(|e| if e.kind() == io::ErrorKind::TimedOut {
                     kill_process(child_id);
                     ErrorKind::Timeout("max time of", MAX_TIMEOUT_SECS).into()
                 } else {
                     e.into()
                 });


    // TODO: Handle errors from tokio_timer better, in particular TimerError::TooLong
    let (status, (stdout, stderr)) = core.run(Future::join(child, output))?;

    Ok(ProcessOutput {
           status: status,
           stdout: stdout,
           stderr: stderr,
       })
}

#[cfg_attr(feature = "cargo-clippy", allow(type_complexity))]
fn unmerge<T1, T2, S>(reader: S) -> Box<Future<Item = (Vec<T1>, Vec<T2>), Error = S::Error>>
    where S: Stream<Item = MergedItem<T1, T2>> + 'static,
          T1: 'static,
          T2: 'static
{
    Box::new(reader
                 .map(|i| match i {
                          MergedItem::First(l) => (Some(l), None),
                          MergedItem::Second(r) => (None, Some(r)),
                          MergedItem::Both(l, r) => (Some(l), Some(r)),
                      })
                 .fold((Vec::new(), Vec::new()), |mut v, i| {
        i.0.map(|i| v.0.push(i));
        i.1.map(|i| v.1.push(i));
        Ok(v)
    }))
}
