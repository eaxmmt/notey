#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable
    )
)]

mod document;
mod markdown;

use std::path::Path;

use document::Document;
use rune::tok::TextStyle;
use rune::{
    App, AppHandle, AppSpec, Cx, El, FileDialogOptions, KeyEvent, ScrollSpec, TextEdit, Tx, col,
    el, row, text, text_input,
};
use rune_kit::{Button, ButtonVariant, Kit, Markdown};

const WELCOME: &str = include_str!("../examples/Welcome.md");

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Live,
    Source,
    Read,
}

#[derive(Clone, Copy)]
enum Action {
    New,
    Open,
    Quit,
}

struct State {
    edit: TextEdit,
    document: Document,
    mode: Mode,
    dark: bool,
    message: String,
    error: bool,
    pending: Option<Action>,
    confirm: bool,
    dialog_open: bool,
    handle: Option<AppHandle<State>>,
}

impl Default for State {
    fn default() -> Self {
        let mut edit = TextEdit::new(WELCOME);
        edit.focused = true;
        edit.set_caret(WELCOME.len());
        Self {
            edit,
            document: Document {
                path: None,
                saved: WELCOME.to_owned(),
            },
            mode: Mode::Live,
            dark: true,
            message: "Demo note · Save a copy to make it yours".into(),
            error: false,
            pending: None,
            confirm: false,
            dialog_open: false,
            handle: None,
        }
    }
}

impl State {
    fn report(&mut self, message: impl Into<String>, error: bool) {
        self.message = message.into();
        self.error = error;
    }

    fn dirty(&self) -> bool {
        self.document.dirty(&self.edit.text)
    }

    fn load(&mut self, path: &Path) {
        match Document::open(path) {
            Ok((document, content)) => {
                self.document = document;
                self.edit = TextEdit::new(content);
                self.edit.focused = true;
                self.report("Opened · Markdown kept exactly as written", false);
            }
            Err(e) => self.report(format!("Could not open: {e}"), true),
        }
    }

    fn request(&mut self, action: Action, tx: &mut Tx) {
        if self.dialog_open {
            return;
        }
        self.pending = Some(action);
        if self.dirty() {
            self.confirm = true;
        } else {
            self.continue_action(tx);
        }
    }

    fn continue_action(&mut self, tx: &mut Tx) {
        self.confirm = false;
        match self.pending.take() {
            Some(Action::New) => {
                self.document = Document::untitled();
                self.edit = TextEdit::new("");
                self.edit.focused = true;
                self.mode = Mode::Live;
                self.report("New note · Not yet saved", false);
            }
            Some(Action::Open) => {
                self.dialog_open = true;
                tx.open_file(
                    FileDialogOptions {
                        title: "Open Markdown note".into(),
                        ..Default::default()
                    }
                    .filter("Markdown", &["md"]),
                    |s: &mut State, result, _| {
                        s.dialog_open = false;
                        match result {
                            Ok(Some(path)) => s.load(&path),
                            Ok(None) => {}
                            Err(e) => s.report(format!("Could not open file chooser: {e}"), true),
                        }
                    },
                );
            }
            Some(Action::Quit) => tx.quit(),
            None => {}
        }
    }

    fn save(&mut self, copy: bool, tx: &mut Tx) {
        if self.dialog_open {
            return;
        }
        self.confirm = false;
        if copy || self.document.path.is_none() {
            self.dialog_open = true;
            tx.save_file(
                FileDialogOptions {
                    title: "Save Markdown note to a new filename".into(),
                    file_name: Some("Untitled.md".into()),
                    ..Default::default()
                }
                .filter("Markdown", &["md"]),
                |s: &mut State, result, tx| {
                    s.dialog_open = false;
                    match result {
                        Ok(Some(mut path)) => {
                            if path.extension().is_none() {
                                path.set_extension("md");
                            }
                            s.write(Some(&path), tx);
                        }
                        Ok(None) => {
                            s.pending = None;
                        }
                        Err(e) => {
                            s.pending = None;
                            s.report(format!("Could not open save chooser: {e}"), true);
                        }
                    }
                },
            );
        } else {
            self.write(None, tx);
        }
    }

    fn write(&mut self, path: Option<&Path>, tx: &mut Tx) {
        match self.document.save(&self.edit.text, path) {
            Ok(()) => {
                self.report("Saved · Plain Markdown, no conversion", false);
                self.continue_action(tx);
            }
            Err(e) => {
                self.pending = None;
                self.report(format!("Could not save: {e}"), true);
            }
        }
    }
}

fn key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if s.confirm || s.dialog_open {
        return false;
    }
    if event.is_command('s') && !event.mods.alt {
        s.save(event.mods.shift, tx);
    } else if event.is_command('o') {
        s.request(Action::Open, tx);
    } else if event.is_command('n') {
        s.request(Action::New, tx);
    } else if event.is_command('q') {
        s.request(Action::Quit, tx);
    } else if event.is_command('e') {
        s.mode = if s.mode == Mode::Source {
            Mode::Live
        } else {
            Mode::Source
        };
    } else if event.is_command('r') {
        s.mode = if s.mode == Mode::Read {
            Mode::Live
        } else {
            Mode::Read
        };
    } else {
        return false;
    }
    true
}

fn button(k: &Kit, label: &str, id: &str, action: impl Fn(&mut State, &mut Tx) + 'static) -> El {
    Button::new(label)
        .variant(ButtonVariant::Ghost)
        .on_press_tx(action)
        .build(k)
        .key(id)
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let k = Kit::neutral(s.dark);
    let c = &k.c;
    let name = s
        .document
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled.md".into());
    let title = if s.dirty() {
        format!("{name}  •")
    } else {
        name
    };
    let header = row()
        .w_full()
        .min_h(64.0)
        .px(24.0)
        .gap(12.0)
        .center_y()
        .child(
            text("folio")
                .font(TextStyle::new(22.0, 650.0))
                .color(c.text_base),
        )
        .child(
            text("/  a space for your thoughts")
                .font(TextStyle::new(12.0, 400.0))
                .color(c.text_faint),
        )
        .child(el().grow(1.0))
        .child(button(&k, "New", "new", |s, tx| s.request(Action::New, tx)))
        .child(button(&k, "Open…", "open", |s, tx| {
            s.request(Action::Open, tx)
        }))
        .child(button(&k, "Save", "save", |s, tx| s.save(false, tx)))
        .child(button(&k, "Save copy…", "save-copy", |s, tx| {
            s.save(true, tx)
        }))
        .child(button(
            &k,
            if s.dark { "Light" } else { "Dark" },
            "theme",
            |s, _| s.dark = !s.dark,
        ));
    let mut modes = row().gap(4.0);
    for (mode, label, id) in [
        (Mode::Live, "Live preview", "live"),
        (Mode::Source, "Markdown", "source"),
        (Mode::Read, "Read", "read"),
    ] {
        modes = modes.child(
            Button::new(label)
                .variant(if s.mode == mode {
                    ButtonVariant::Contrast
                } else {
                    ButtonVariant::Ghost
                })
                .on_press(move |s: &mut State| s.mode = mode)
                .build(&k)
                .key(id),
        );
    }
    let toolbar = row()
        .w_full()
        .px(24.0)
        .py(12.0)
        .gap(12.0)
        .center_y()
        .child(
            text(title)
                .font(TextStyle::new(13.0, 500.0))
                .color(c.text_muted)
                .grow(1.0),
        )
        .child(modes);
    let editor = if s.mode == Mode::Read {
        Markdown::new(&s.edit.text)
            .selectable(true)
            .on_link_tx(|s: &mut State, url, tx| {
                if let Err(e) = tx.open_url(url) {
                    s.report(format!("Could not open link: {e}"), true);
                }
            })
            .build(&k)
            .key("rendered-note")
    } else {
        let style = TextStyle::new(17.0, 400.0).lh(32.0);
        let style = if s.mode == Mode::Source {
            style.mono()
        } else {
            style
        };
        let spans = if s.mode == Mode::Live {
            markdown::spans(
                &s.edit.text,
                s.edit.range(),
                markdown::Palette {
                    text: c.text_base,
                    muted: c.text_faint,
                    accent: c.text_accent,
                    code_bg: c.layer_02,
                },
            )
        } else {
            Vec::new()
        };
        text_input(&s.edit, style)
            .key("editor")
            .label("Markdown note")
            .w_full()
            .min_h(440.0)
            .color(c.text_base)
            .caret(c.text_accent, k.selection())
            .placeholder("Start with a thought…", c.text_faint)
            .decorations(spans)
            .on_edit(|s: &mut State| &mut s.edit)
            .on_input_key(key)
            .on_input_change(|s: &mut State, _| {
                s.report("Unsaved changes · Ctrl+S to save", false);
            })
    };
    let page = col()
        .w_full()
        .max_w(820.0)
        .self_center()
        .px(36.0)
        .pt(36.0)
        .pb(80.0)
        .gap(16.0)
        .child(editor);
    let content = col()
        .key("note-scroll")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(page);
    let status_color = if s.error { c.danger.fg } else { c.text_faint };
    let footer = col()
        .w_full()
        .px(24.0)
        .py(12.0)
        .gap(6.0)
        .child(
            text(&s.message)
                .key("status")
                .font(TextStyle::new(12.0, 400.0))
                .color(status_color)
                .wrap(),
        )
        .child(
            row()
                .w_full()
                .gap(12.0)
                .child(
                    text(format!(
                        "{} words  ·  UTF-8 Markdown",
                        s.edit.text.split_whitespace().count()
                    ))
                    .font(TextStyle::new(11.0, 400.0))
                    .color(c.text_faint),
                )
                .child(el().grow(1.0))
                .child(
                    text("Ctrl+S save   Ctrl+E source   Ctrl+R read")
                        .font(TextStyle::new(11.0, 400.0))
                        .color(c.text_faint),
                ),
        );
    let mut root = col()
        .w_full()
        .h_full()
        .fill(c.bg_base)
        .color(c.text_base)
        .child(header)
        .child(el().w_full().h(1.0).fill(c.border_base))
        .child(toolbar)
        .child(content)
        .child(el().w_full().h(1.0).fill(c.border_base))
        .child(footer);
    if s.confirm {
        let modal = col().key("unsaved-dialog").w(420.0).padding(24.0).gap(18.0)
            .fill(c.layer_01).border(1.0, c.border_base)
            .child(text("Keep your changes?").font(TextStyle::new(22.0, 600.0)).color(c.text_base))
            .child(text("This note has unsaved changes. Save them before continuing, or explicitly discard them.").font(TextStyle::new(14.0, 400.0)).color(c.text_muted).wrap())
            .child(row().gap(8.0)
                .child(button(&k, "Cancel", "cancel", |s, _| { s.confirm = false; s.pending = None; }))
                .child(button(&k, "Discard", "discard", |s, tx| s.continue_action(tx)))
                .child(button(&k, "Save changes", "save-changes", |s, tx| s.save(false, tx))));
        root = root.child(
            el().key("dialog-backdrop")
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .center()
                .fill(rune::oklcha(0.0, 0.0, 0.0, 0.55))
                .modal(false, |s: &mut State| {
                    s.confirm = false;
                    s.pending = None;
                })
                .child(modal),
        );
    }
    root
}

fn main() -> Result<(), String> {
    rune::set_app_id("folio");
    let path = rune::extra_args().first().cloned();
    let spec = AppSpec {
        min_size: Some((760.0, 540.0)),
        families: ("Inter", "ui-monospace"),
        on_key: Some(key),
        scenes: &[
            ("light", |s| s.dark = false),
            ("source", |s| s.mode = Mode::Source),
            ("read", |s| s.mode = Mode::Read),
        ],
        ..AppSpec::new(
            "Folio — Markdown notes",
            (1040.0, 800.0),
            State::default,
            view,
        )
    };
    let app = App::new(spec).keep_running().on_close(|s, tx| {
        if s.dirty() || s.dialog_open {
            s.pending = Some(Action::Quit);
            s.confirm = !s.dialog_open;
            if let Some(handle) = &s.handle {
                handle.open();
            }
        } else {
            tx.quit();
        }
    });
    let handle = app.handle();
    app.init(move || {
        let mut state = State {
            handle: Some(handle.clone()),
            ..State::default()
        };
        if let Some(path) = &path {
            state.load(Path::new(path));
        }
        state
    })
    .try_run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Key, Mods, Runtime};

    fn runtime() -> Runtime<State> {
        let mut rt = Runtime::new(
            State::default(),
            view,
            (1040.0, 800.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.frame(0.0);
        rt
    }

    fn click(rt: &mut Runtime<State>, key: &str) {
        let (x, y) = rt
            .find(key)
            .unwrap_or_else(|| panic!("expected clickable UI element {key:?}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
    }

    #[test]
    fn switching_modes_preserves_the_markdown() {
        let mut rt = runtime();
        let source = rt.state.edit.text.clone();
        click(&mut rt, "source");
        assert!(rt.state.mode == Mode::Source);
        click(&mut rt, "read");
        assert!(rt.state.mode == Mode::Read);
        click(&mut rt, "live");
        assert!(rt.state.mode == Mode::Live);
        assert_eq!(rt.state.edit.text, source);
    }

    #[test]
    fn reading_links_open_safe_urls_through_the_ui() {
        for url in [
            "https://example.com/docs",
            "http://example.com/",
            "mailto:hello@example.com",
        ] {
            let mut rt = runtime();
            rt.update(|s, _| {
                s.edit = TextEdit::new(format!("[Example]({url})"));
                s.mode = Mode::Read;
            });
            rt.frame(1.0 / 60.0);
            click(&mut rt, url);
            assert_eq!(rt.take_url_requests(), [url]);
            assert!(!rt.state.error);
        }
    }

    #[test]
    fn reading_links_reject_unsafe_and_relative_destinations() {
        for url in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "custom:run",
            "other.md",
        ] {
            let mut rt = runtime();
            rt.update(|s, _| {
                s.edit = TextEdit::new(format!("[Example]({url})"));
                s.mode = Mode::Read;
            });
            rt.frame(1.0 / 60.0);
            click(&mut rt, url);
            assert!(rt.take_url_requests().is_empty());
            assert!(rt.state.error);
            assert!(rt.state.message.starts_with("Could not open link:"));
        }
    }

    #[test]
    fn dirty_note_requires_explicit_discard_before_new() {
        let mut rt = runtime();
        rt.update(|s, _| s.edit.insert("changed"));
        click(&mut rt, "new");
        assert!(rt.state.confirm);
        click(&mut rt, "cancel");
        assert!(rt.state.edit.text.contains("changed"));
        click(&mut rt, "new");
        click(&mut rt, "discard");
        assert!(rt.state.edit.text.is_empty());
        assert!(!rt.state.confirm);
    }

    #[test]
    fn typing_and_undo_keep_source_authoritative() {
        let mut rt = runtime();
        let original = rt.state.edit.text.clone();
        rt.key(KeyEvent {
            key: Key::Text(" **new**".into()),
            mods: Mods::default(),
        });
        rt.frame(1.0 / 60.0);
        assert!(rt.state.edit.text.ends_with(" **new**"));
        rt.key(KeyEvent {
            key: Key::Command('z'),
            mods: Mods {
                cmd: true,
                ctrl: true,
                ..Mods::default()
            },
        });
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.edit.text, original);
    }
}
