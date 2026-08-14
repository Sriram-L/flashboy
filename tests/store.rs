use std::path::Path;

use flashboy::store::{self, Bank};

#[test]
fn sidecar_keeps_cpp_stem() {
    let p = store::sidecar_path(Path::new("/tmp/solve.cpp"));
    assert!(p.ends_with("solve.cpp.fbk"));
}

#[test]
fn roundtrip_bank() {
    let dir = std::env::temp_dir().join(format!("flashboy-store-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a.cpp.fbk");
    let mut bank = Bank::new("a.cpp");
    bank.add("sample".into(), "1 2\n".into(), Some("3\n".into()));
    store::save(&path, &bank).unwrap();
    let loaded = store::load(&path).unwrap();
    assert_eq!(loaded.source_name, "a.cpp");
    assert_eq!(loaded.cases.len(), 1);
    assert_eq!(loaded.cases[0].name, "sample");
    assert_eq!(loaded.cases[0].input, "1 2\n");
    assert_eq!(loaded.cases[0].expected.as_deref(), Some("3\n"));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn update_and_delete_cases() {
    let mut bank = Bank::new("p.cpp");
    let id = bank.add("a".into(), "1".into(), None);
    bank.add("b".into(), "2".into(), Some("x".into()));
    bank.get_mut(id).unwrap().input = "changed".into();
    assert_eq!(bank.cases[0].input, "changed");
    assert!(bank.remove(id));
    assert_eq!(bank.cases.len(), 1);
    assert_eq!(bank.cases[0].name, "b");
}
