import json
import os
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/debug/folio"


def run(directory, name, events, note=None):
    script = directory / f"{name}.json"
    tree = directory / f"{name}-tree.json"
    script.write_text(json.dumps({"events": events}))
    command = [str(BINARY)]
    if note:
        command.append(str(note))
    command += ["--script", str(script), "--tree", str(tree), "--at", "1.5"]
    subprocess.run(command, cwd=ROOT, check=True, timeout=30)
    return tree.read_text()


with tempfile.TemporaryDirectory(prefix="folio-smoke-") as temporary:
    directory = pathlib.Path(temporary)
    note = directory / "smoke.md"
    content = "# Smoke test\n\nA **bold** thought, café ☕.\n"
    tree = run(directory, "save", [
        {"t": 0.1, "type": "key", "name": "a", "cmd": True},
        {"t": 0.2, "paste": content},
        {"t": 0.3, "type": "key", "name": "s", "cmd": True},
        {"t": 0.5, "type": "file-dialog", "path": str(note)},
    ])
    assert note.read_text() == content, repr((note.read_text(), content))
    assert "Saved · Plain Markdown" in tree
    tree = run(directory, "reopen", [], note)
    assert "Opened · Markdown kept exactly as written" in tree
    assert "Smoke test" in tree
    tree = run(directory, "cancel-new", [
        {"t": 0.1, "paste": "unsaved"},
        {"t": 0.2, "type": "key", "name": "n", "cmd": True},
        {"t": 0.4, "type": "tap", "key": "cancel"},
    ], note)
    assert "unsaved" in tree
    assert '"key": "unsaved-dialog"' not in tree
    assert note.read_text() == content
    tree = run(directory, "refuse-overwrite", [
        {"t": 0.1, "paste": "another note"},
        {"t": 0.2, "type": "key", "name": "s", "cmd": True},
        {"t": 0.5, "type": "file-dialog", "path": str(note)},
    ])
    assert "That file already exists" in tree
    assert note.read_text() == content
    tree = run(directory, "refuse-copy-over-original", [
        {"t": 0.1, "paste": "unsaved draft"},
        {"t": 0.2, "type": "tap", "key": "save-copy"},
        {"t": 0.5, "type": "file-dialog", "path": str(note)},
    ], note)
    assert "That file already exists" in tree
    assert "unsaved draft" in tree
    assert note.read_text() == content
    copy = directory / "copy.md"
    tree = run(directory, "save-copy", [
        {"t": 0.1, "paste": "copied draft"},
        {"t": 0.2, "type": "tap", "key": "save-copy"},
        {"t": 0.5, "type": "file-dialog", "path": str(copy)},
    ], note)
    assert "Saved · Plain Markdown" in tree
    assert copy.read_text() == content + "copied draft"
    assert note.read_text() == content
    fifo = directory / "pipe.md"
    os.mkfifo(fifo)
    tree = run(directory, "reject-fifo", [], fifo)
    assert "Choose a regular Markdown file" in tree

print("7 headless UI smoke scenarios passed")
