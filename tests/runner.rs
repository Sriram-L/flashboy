use std::path::{Path, PathBuf};
use std::time::Duration;

use flashboy::runner::{
    compile, discover_compiler, normalize, run_case, run_case_with_timeout, HOMEBREW_GXX12,
};
use flashboy::store::Verdict;

fn gxx12() -> String {
    let compiler = HOMEBREW_GXX12.to_string();
    assert!(
        Path::new(&compiler).exists(),
        "Homebrew g++-12 missing at {compiler}"
    );
    compiler
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn normalize_trims_line_ends_and_trailing_blank() {
    assert_eq!(normalize("1  \r\n2\n\n"), "1\n2");
    assert_eq!(normalize("ok\n"), "ok");
}

#[test]
fn discover_prefers_homebrew_gxx12() {
    assert!(
        Path::new(HOMEBREW_GXX12).exists(),
        "Homebrew g++-12 missing at {HOMEBREW_GXX12}"
    );
    if std::env::var("FLASHBOY_CXX").is_ok() {
        return;
    }
    let found = discover_compiler().expect("g++-12 should be installed");
    assert_eq!(found, HOMEBREW_GXX12);
}

#[test]
fn compile_and_judge_with_gxx12() {
    let compiler = gxx12();
    let src = fixture("sum.cpp");
    let out = compile(&compiler, &src).expect("g++-12 compile");
    let ac = run_case(&out.binary, "2 3\n", Some("5\n"));
    assert_eq!(ac.verdict, Verdict::Pass, "stderr={}", ac.stderr);
    assert!(ac.duration_ns > 0);

    let wa = run_case(&out.binary, "2 3\n", Some("0\n"));
    assert_eq!(wa.verdict, Verdict::Fail);

    let ok = run_case(&out.binary, "10 20\n", None);
    assert_eq!(ok.verdict, Verdict::Ran);
    assert_eq!(normalize(&ok.stdout), "30");

    let _ = std::fs::remove_file(&out.binary);
}

#[test]
fn runtime_error_nonzero_exit() {
    let compiler = gxx12();
    let src = fixture("fail.cpp");
    let out = compile(&compiler, &src).expect("g++-12 compile");
    let rec = run_case(&out.binary, "", Some(""));
    assert_eq!(rec.verdict, Verdict::Runtime);
    assert_eq!(rec.exit_code, Some(1));
    let _ = std::fs::remove_file(&out.binary);
}

#[test]
fn tle_when_program_exceeds_limit() {
    let compiler = gxx12();
    let src = fixture("tle.cpp");
    let out = compile(&compiler, &src).expect("g++-12 compile");
    let rec = run_case_with_timeout(&out.binary, "", None, Duration::from_millis(250));
    assert_eq!(rec.verdict, Verdict::Tle, "stderr={}", rec.stderr);
    let _ = std::fs::remove_file(&out.binary);
}
