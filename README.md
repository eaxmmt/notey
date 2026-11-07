# Folio

A Linux-first Markdown editor prototype built with [Rune](https://github.com/anomalyco/rune). Folio is a working name, not a settled product name.

The Markdown string is the document. Live preview adds visual decorations; it never converts the note into a rich-text storage format. Files remain ordinary UTF-8 `.md` files.

## Run

From this directory:

```sh
cargo run
cargo run -- examples/Welcome.md
cargo run -- /absolute/path/to/your-note.md
```

The first command opens an in-memory demo. It does not write a note until you choose Save.

Rune is pinned to `ccb4823f55749dfa667891ef7f8427017e960056`, and `Cargo.lock` locks the dependency graph. The first build downloads and compiles dependencies. The project was built and tested with Rust 1.99.1 on Linux.

Linux builds require the development libraries for XKB and Wayland, a linker/C compiler, and a working GPU driver. On Arch, the relevant packages are `base-devel`, `libxkbcommon`, and `wayland`. File choosers use the XDG desktop portal, so a functioning portal backend must be running in your desktop session.

## What works

- A native Rune window with light and dark appearances.
- Live formatting for ATX headings, bold, italic, inline code, fenced code, and inline links; quotes receive an italic/muted style.
- Markdown syntax revealed on the caret's line and on every line touched by a selection.
- A complete, unmodified Markdown source view.
- A separate reading view using rune-kit's Markdown renderer.
- Standard selection, clipboard operations, undo/redo, and Rune's text input support.
- New, Open, Save, and Save copy actions.
- Unsaved-change confirmation before creating/opening another note or quitting.
- Visible save/open errors and unsaved state.
- Atomic file replacement and checks for detected external modifications.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| Ctrl+N | New note |
| Ctrl+O | Open a Markdown note |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save a copy to a new filename |
| Ctrl+E | Toggle Markdown source / live preview |
| Ctrl+R | Toggle reading / live preview |
| Ctrl+Q | Quit, confirming unsaved changes |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z | Redo |
| Ctrl+A / C / X / V | Select all / copy / cut / paste |

Buttons provide the same app actions. Escape cancels the unsaved-change dialog. In reading mode, switch back to Live preview or Markdown to edit.

## Saving and file ownership

There is deliberately no autosave yet. Ctrl+S saves an existing note; an untitled note opens the save chooser. Save copy requires a new filename and refuses to overwrite another existing file, even if the chooser allows selecting it.

Before replacing an opened note, Folio checks that its contents still match the version originally opened or last saved. A detected external change or deletion blocks saving and leaves your draft in memory. Use Save copy to keep both versions. There is no live filesystem watcher or merge interface yet.

Writes use a temporary file beside the destination, flush its contents, and rename it into place. Existing file permissions are preserved. New-file creation refuses a pre-existing destination. Symlinks are resolved when opening a note; Save copy refuses symlink destinations.

The external-change checks are not a cross-process lock or a transactional compare-and-swap: another program can still race the final check and rename. Atomic replacement also does not preserve hard-link relationships or every filesystem-specific attribute. Notes are limited to 2 MiB in this prototype. Invalid UTF-8 is rejected rather than silently replaced.

## Live preview: the experiment

`pulldown-cmark` identifies source byte ranges. Folio maps them to Rune text decorations. Formatting markers outside active lines are transparent and use a near-zero font size; their bytes remain in the editor model. On active lines, markers return in a muted color.

This proves that a Markdown-backed live editor can be built on Rune without adding a webview or rewriting the stored Markdown. It is not yet an Obsidian-equivalent editing engine:

- Concealment uses near-zero-width text, not a dedicated hidden-source-range primitive. Caret movement, pointer placement, wrapping changes, long selections, accessibility, and IME behavior need more interactive testing.
- Blank lines, list markers, quote markers, and spacing remain source-like. There is no automatic list continuation or indent/outdent logic yet.
- Live formatting is a subset of the reading renderer. Images, tables, task checkboxes, strikethrough, setext headings, reference links, and embedded content do not have full live-preview behavior.
- Links are styled while editing. Use the reading view to follow rendered links.
- Parsing currently runs during view construction; there is no incremental parsing or large-document optimization.
- File reads and saves are synchronous and bounded in size.
- Closing a dirty native window reopens it with the confirmation dialog because Rune's close hook cannot directly veto closing. This can briefly unmap/remap a Wayland window.
- There is no crash-recovery journal. Save regularly; a force-kill cannot show the confirmation dialog.

The next decision should be whether this editing behavior feels right on your Linux desktop. Workspace navigation, search, tags, backlinks, and attachments come after that decision—not before.

## Code map

- `src/main.rs`: Rune window, application state, views, shortcuts, dialogs, and mode switching.
- `src/markdown.rs`: Markdown parsing, source-range decorations, and active-line syntax reveal.
- `src/document.rs`: exact text loading, conflict checks, and atomic saving.
- `examples/Welcome.md`: the bundled demo note.
- `tests/smoke.py`: display-free end-to-end UI scenarios using Rune's scripted input.

No explanatory comments were added to the source; the design is explained here instead.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
python3 tests/smoke.py
```

The Rust tests cover source ranges, syntax concealment/reveal, exact-byte file round trips, external changes, deleted originals, existing destinations, symlinks, size/encoding validation, typing/undo, mode switching, and unsaved-change cancellation.

The smoke script tests saving through the real UI handlers and scripted file chooser, reopening, cancelling a destructive action, and refusing an existing save-copy destination. It uses only temporary files. GPU-backed headless runs require an appropriate graphics driver; they do not require a visible window.

To generate snapshots and an element tree:

```sh
mkdir -p artifacts
target/debug/folio --snap artifacts/live-preview.png --tree artifacts/live-tree.json
target/debug/folio --scene source --snap artifacts/source.png
target/debug/folio --scene read --snap artifacts/read.png
target/debug/folio --scene light --snap artifacts/light.png
```

## Scope

This is the editor feasibility prototype, not yet the full Notion/Obsidian alternative. There are no accounts, servers, proprietary note formats, workspace databases, or desktop configuration changes.
