use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use wait_timeout::ChildExt;

use crate::store::{RunRecord, Verdict};

pub const TLE_SECS: u64 = 5;
const IO_CAP: u64 = 1_048_576;

#[derive(Debug)]
pub struct CompileOutcome {
    pub binary: PathBuf,
    pub duration_ns: u64,
}

pub const HOMEBREW_GXX12: &str = "/opt/homebrew/bin/g++-12";

pub fn discover_compiler() -> Result<String> {
    if let Ok(cxx) = std::env::var("FLASHBOY_CXX") {
        return Ok(cxx);
    }
    for cand in [
        HOMEBREW_GXX12,
        "g++-12",
        "/usr/local/bin/g++-12",
        "g++",
        "c++",
        "clang++",
        "gcc",
    ] {
        if compiler_exists(cand) {
            return Ok(cand.to_string());
        }
    }
    Err(anyhow!(
        "Homebrew g++-12 not found at {HOMEBREW_GXX12}. Install gcc@12 or set FLASHBOY_CXX."
    ))
}

fn compiler_exists(cand: &str) -> bool {
    Command::new(cand)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn compile(compiler: &str, src: &Path) -> Result<CompileOutcome> {
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("prog");
    let binary = std::env::temp_dir().join(format!("flashboy-{stem}-{}", std::process::id()));

    let mut cmd = Command::new(compiler);
    cmd.args(["-std=c++17", "-O2", "-pipe"]);
    let base = Path::new(compiler)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(compiler);
    let is_gcc = base == "gcc" || base.starts_with("gcc-");
    if is_gcc {
        cmd.args(["-x", "c++"]);
    }
    cmd.arg(src).arg("-o").arg(&binary);
    if is_gcc {
        cmd.args(["-lstdc++", "-lm"]);
    }

    let started = Instant::now();
    let out = cmd.output().with_context(|| format!("spawn {compiler}"))?;
    let duration_ns = started.elapsed().as_nanos() as u64;

    if !out.status.success() {
        let mut msg = String::from_utf8_lossy(&out.stderr).into_owned();
        if msg.trim().is_empty() {
            msg = String::from_utf8_lossy(&out.stdout).into_owned();
        }
        return Err(anyhow!(msg.trim().to_string()));
    }
    if !binary.exists() {
        return Err(anyhow!("compiler produced no binary"));
    }
    Ok(CompileOutcome {
        binary,
        duration_ns,
    })
}

pub fn run_case(binary: &Path, input: &str, expected: Option<&str>) -> RunRecord {
    run_case_with_timeout(binary, input, expected, Duration::from_secs(TLE_SECS))
}

pub fn run_case_with_timeout(
    binary: &Path,
    input: &str,
    expected: Option<&str>,
    limit: Duration,
) -> RunRecord {
    let mut child = match Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return RunRecord {
                verdict: Verdict::Runtime,
                duration_ns: 0,
                stdout: String::new(),
                stderr: format!("failed to spawn binary: {e}"),
                exit_code: None,
            };
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = io::Write::write_all(&mut stdin, input.as_bytes());
    }

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let t_out = thread::spawn(move || drain(stdout_pipe));
    let t_err = thread::spawn(move || drain(stderr_pipe));

    let started = Instant::now();
    match child.wait_timeout(limit) {
        Ok(Some(status)) => {
            let duration_ns = started.elapsed().as_nanos() as u64;
            let stdout = t_out.join().unwrap_or_default();
            let stderr = t_err.join().unwrap_or_default();
            let exit_code = status.code();
            if !status.success() {
                return RunRecord {
                    verdict: Verdict::Runtime,
                    duration_ns,
                    stdout,
                    stderr,
                    exit_code,
                };
            }
            let verdict = match expected {
                None => Verdict::Ran,
                Some(exp) => {
                    if normalize(&stdout) == normalize(exp) {
                        Verdict::Pass
                    } else {
                        Verdict::Fail
                    }
                }
            };
            RunRecord {
                verdict,
                duration_ns,
                stdout,
                stderr,
                exit_code,
            }
        }
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = t_out.join();
            let _ = t_err.join();
            RunRecord {
                verdict: Verdict::Tle,
                duration_ns: limit.as_nanos() as u64,
                stdout: String::new(),
                stderr: format!("time limit exceeded ({}s)", limit.as_secs_f64()),
                exit_code: None,
            }
        }
        Err(e) => {
            let _ = child.kill();
            RunRecord {
                verdict: Verdict::Runtime,
                duration_ns: started.elapsed().as_nanos() as u64,
                stdout: String::new(),
                stderr: format!("wait failed: {e}"),
                exit_code: None,
            }
        }
    }
}

fn drain(pipe: Option<impl Read>) -> String {
    let Some(r) = pipe else {
        return String::new();
    };
    let mut buf = Vec::new();
    let _ = r.take(IO_CAP).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

pub fn normalize(s: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = s.lines().map(|l| l.trim_end()).collect();
    while matches!(lines.last(), Some(l) if l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}
