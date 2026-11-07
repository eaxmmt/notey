use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn run(directory: &Path, name: &str, events: Value, note: Option<&Path>) -> String {
    let script = directory.join(format!("{name}.json"));
    let tree = directory.join(format!("{name}-tree.json"));
    fs::write(
        &script,
        serde_json::to_vec(&json!({"events": events})).unwrap(),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_folio"));
    command.current_dir(env!("CARGO_MANIFEST_DIR"));
    if let Some(note) = note {
        command.arg(note);
    }
    command
        .arg("--script")
        .arg(&script)
        .arg("--tree")
        .arg(&tree)
        .args(["--at", "1.5"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let mut child = command.spawn().expect("launch app");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("check app status") {
            assert!(status.success(), "scenario {name} failed: {status}");
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("scenario {name} timed out after 30 seconds");
        }
        thread::sleep(Duration::from_millis(20));
    }
    fs::read_to_string(tree).expect("read UI tree")
}

#[test]
#[ignore = "requires a working GPU driver; run with cargo test --test smoke -- --ignored"]
fn headless_ui_smoke_scenarios() {
    let temporary = tempfile::Builder::new()
        .prefix("notey-smoke-")
        .tempdir()
        .unwrap();
    let directory = temporary.path();
    let note = directory.join("smoke.md");
    let content = "# Smoke test\n\nA **bold** thought, café ☕.\n";
    let tree = run(
        directory,
        "save",
        json!([
            {"t": 0.1, "type": "key", "name": "a", "cmd": true},
            {"t": 0.2, "paste": content},
            {"t": 0.3, "type": "key", "name": "s", "cmd": true},
            {"t": 0.5, "type": "file-dialog", "path": note},
        ]),
        None,
    );
    assert_eq!(fs::read_to_string(&note).unwrap(), content);
    assert!(tree.contains("Saved · Plain Markdown"));

    let tree = run(directory, "reopen", json!([]), Some(&note));
    assert!(tree.contains("Opened · Markdown kept exactly as written"));
    assert!(tree.contains("Smoke test"));

    let tree = run(
        directory,
        "cancel-new",
        json!([
            {"t": 0.1, "paste": "unsaved"},
            {"t": 0.2, "type": "key", "name": "n", "cmd": true},
            {"t": 0.4, "type": "tap", "key": "cancel"},
        ]),
        Some(&note),
    );
    assert!(tree.contains("unsaved"));
    assert!(!tree.contains("\"key\": \"unsaved-dialog\""));
    assert_eq!(fs::read_to_string(&note).unwrap(), content);

    let tree = run(
        directory,
        "refuse-overwrite",
        json!([
            {"t": 0.1, "paste": "another note"},
            {"t": 0.2, "type": "key", "name": "s", "cmd": true},
            {"t": 0.5, "type": "file-dialog", "path": note},
        ]),
        None,
    );
    assert!(tree.contains("That file already exists"));
    assert_eq!(fs::read_to_string(&note).unwrap(), content);

    let tree = run(
        directory,
        "refuse-copy-over-original",
        json!([
            {"t": 0.1, "paste": "unsaved draft"},
            {"t": 0.2, "type": "tap", "key": "save-copy"},
            {"t": 0.5, "type": "file-dialog", "path": note},
        ]),
        Some(&note),
    );
    assert!(tree.contains("That file already exists"));
    assert!(tree.contains("unsaved draft"));
    assert_eq!(fs::read_to_string(&note).unwrap(), content);

    let copy = directory.join("copy.md");
    let tree = run(
        directory,
        "save-copy",
        json!([
            {"t": 0.1, "paste": "copied draft"},
            {"t": 0.2, "type": "tap", "key": "save-copy"},
            {"t": 0.5, "type": "file-dialog", "path": copy},
        ]),
        Some(&note),
    );
    assert!(tree.contains("Saved · Plain Markdown"));
    assert_eq!(
        fs::read_to_string(&copy).unwrap(),
        format!("{content}copied draft")
    );
    assert_eq!(fs::read_to_string(&note).unwrap(), content);

    let fifo = directory.join("pipe.md");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let tree = run(directory, "reject-fifo", json!([]), Some(&fifo));
    assert!(tree.contains("Choose a regular Markdown file"));
}
