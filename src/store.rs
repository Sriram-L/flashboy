use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 4] = b"FBK1";
const ZSTD_LEVEL: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Pass,
    Fail,
    Tle,
    Runtime,
    Ran,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "AC",
            Self::Fail => "WA",
            Self::Tle => "TLE",
            Self::Runtime => "RE",
            Self::Ran => "OK",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub verdict: Verdict,
    pub duration_ns: u64,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: u32,
    pub name: String,
    pub input: String,
    pub expected: Option<String>,
    pub last: Option<RunRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bank {
    pub source_name: String,
    pub next_id: u32,
    pub cases: Vec<Case>,
}

impl Bank {
    pub fn new(source_name: impl Into<String>) -> Self {
        Self {
            source_name: source_name.into(),
            next_id: 1,
            cases: Vec::new(),
        }
    }

    pub fn add(&mut self, name: String, input: String, expected: Option<String>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.cases.push(Case {
            id,
            name,
            input,
            expected,
            last: None,
        });
        id
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Case> {
        self.cases.iter_mut().find(|c| c.id == id)
    }

    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.cases.len();
        self.cases.retain(|c| c.id != id);
        self.cases.len() != before
    }

    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.cases.iter().position(|c| c.id == id)
    }
}

pub fn sidecar_path(cpp: &Path) -> PathBuf {
    let mut p = cpp.to_path_buf();
    match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            p.set_extension(format!("{ext}.fbk"));
        }
        None => {
            p.set_extension("fbk");
        }
    }
    p
}

pub fn load(path: &Path) -> Result<Bank> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.len() < 4 || &bytes[..4] != MAGIC {
        return Err(anyhow!("not a flashboy bank: {}", path.display()));
    }
    let raw = zstd::decode_all(&bytes[4..]).context("zstd decompress")?;
    postcard::from_bytes(&raw).context("decode bank")
}

pub fn save(path: &Path, bank: &Bank) -> Result<()> {
    let raw = postcard::to_stdvec(bank).context("encode bank")?;
    let compressed = zstd::encode_all(&raw[..], ZSTD_LEVEL).context("zstd compress")?;
    let mut out = Vec::with_capacity(4 + compressed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&compressed);

    let tmp = path.with_extension("fbk.tmp");
    fs::write(&tmp, &out).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("rename {}", path.display()))?;
    Ok(())
}

pub fn load_or_new(cpp: &Path) -> Result<(PathBuf, Bank)> {
    let path = sidecar_path(cpp);
    let name = cpp
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("program.cpp")
        .to_string();
    if path.exists() {
        let bank = load(&path)?;
        Ok((path, bank))
    } else {
        Ok((path, Bank::new(name)))
    }
}

pub fn format_duration(ns: u64) -> String {
    if ns < 1_000 {
        format!("{ns} ns")
    } else if ns < 1_000_000 {
        format!("{:.2} µs", ns as f64 / 1_000.0)
    } else if ns < 1_000_000_000 {
        format!("{:.2} ms", ns as f64 / 1_000_000.0)
    } else {
        format!("{:.3} s", ns as f64 / 1_000_000_000.0)
    }
}
