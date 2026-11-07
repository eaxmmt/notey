# Project instructions

These instructions apply only to this repository and its descendants. Do not install them as global agent instructions or change desktop/system configuration as part of project work.

## Rust practices

- Preserve the production safeguards in `src/main.rs`: unsafe Rust is forbidden, and Clippy denies `unwrap_used`, `expect_used`, `panic`, and `unreachable` outside tests. Do not weaken these checks to make a change pass.
- Handle recoverable failures with `Result`, `?`, or explicit matching. File I/O, user input, file dialogs, URLs, and startup failures must not crash the app. Provide actionable error messages and preserve unsaved work.
- Replacing `unwrap()` with `expect()` does not make production error handling safe; both panic. `unwrap_or`, `unwrap_or_else`, and similar fallback methods are fine when the fallback is semantically correct.
- Tests may use unwraps, expects, assertions, and panics to fail on unexpected conditions. Give useful context where a bare failure would be hard to diagnose.
- Avoid unchecked indexing and string slicing for offsets that have not been validated. Rust string offsets are UTF-8 byte offsets, not character counts. Clamp editor selections to character boundaries and test multibyte text, reversed selections, empty input, and out-of-range offsets.
- Prefer straightforward, idiomatic Rust: borrowed arguments when ownership is unnecessary, exhaustive enum matching, standard library facilities, and small focused functions. Avoid unnecessary cloning, eager fallback allocations, speculative abstractions, and new dependencies without a concrete benefit.
- Do not silently discard meaningful errors. Any intentionally ignored failure must have a clear reason, such as best-effort cleanup in a test.
- Add regression tests for fixes. Passing Clippy alone is not proof of correctness, panic freedom, or security.

## Document and UI invariants

- Plain UTF-8 Markdown is authoritative. Live preview must not rewrite source text or convert it to a rich-text storage format.
- Preserve exact file contents, including line endings and whitespace. Reject invalid UTF-8 rather than replacing bytes silently.
- Preserve unsaved-change confirmation before destructive actions. Failed or cancelled operations must retain the draft and leave application state consistent.
- Keep file reads bounded by `MAX_BYTES`. Reject non-regular files without blocking on named pipes or devices.
- Preserve same-directory temporary writes, data sync, atomic replacement, permission preservation, conflict checks, and no-clobber creation for Save copy. Resolve symlinks on open and reject symlink destinations for Save copy.
- Mark a document saved only after the required write and directory sync succeed. A directory-sync failure after replacement is a partial success: explain that the file was written but durability is uncertain, and preserve the draft/state rather than claiming a clean save.
- External-change checks are not a cross-process lock or transactional compare-and-swap. Do not claim that the final check/rename race is eliminated.
- File I/O and Markdown parsing currently run synchronously during UI work. Avoid increasing that work; use an appropriate background-work design if operations become substantially heavier.

## Dependency boundaries

- Rune is pinned in `Cargo.toml`; keep dependency changes deliberate and keep `Cargo.lock` consistent. Do not edit Cargo's cached dependency sources as a project fix.
- Use Rune's `App::try_run()` for startup error handling, not its panic-on-error `App::run()` convenience API. `try_run()` does not catch arbitrary internal dependency panics.
- The reviewed Rune revision contains internal mutex-poisoning unwraps and GPU-readback expects. Distinguish upstream behavior from application bugs; invariant checks are not automatically defects. If a dependency prevents recovery, report the relevant version, source location, and concrete behavior to the user.
- Rune's native close hook has no direct close-veto return value. The app currently reopens a dirty window to show confirmation; do not remove that protection without an equivalent mechanism.
- `pulldown-cmark` 0.13.6 recognizes spaces, but not tabs, after closing code fences. Live decorations must follow parser-recognized ranges rather than hide text the reading renderer treats as code. Keep the regression test and reassess this note when upgrading the parser. This is a parser limitation, not a Rune issue.
- Do not claim a full dependency security audit based only on builds, tests, or Clippy.

## Validation

Run the checks relevant to a code change, normally:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build
```

For UI, file-dialog, or end-to-end changes, also run:

```sh
cargo test --test smoke -- --ignored
```

The smoke test needs a working GPU driver but not a visible window. Use temporary files for test data. If a check cannot run, report the limitation rather than claiming it passed.

## Documentation

Keep the README focused on setup, usage, user-visible behavior, and limitations. Keep agent/contributor coding rules and implementation-specific dependency notes here. Update tests and relevant documentation when behavior changes.
