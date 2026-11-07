use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag};
use rune::{Color, Span};

#[derive(Clone, Copy)]
pub struct Palette {
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub code_bg: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Format {
    Heading(u8),
    Bold,
    Italic,
    Code,
    Link,
    Quote,
    Syntax,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decoration {
    pub range: Range<usize>,
    pub format: Format,
}

pub fn active_lines(source: &str, selection: (usize, usize)) -> Range<usize> {
    let a = selection.0.min(selection.1).min(source.len());
    let b = selection.0.max(selection.1).min(source.len());
    let start = source[..a].rfind('\n').map_or(0, |i| i + 1);
    let end = source[b..].find('\n').map_or(source.len(), |i| b + i);
    start..end
}

pub fn decorations(source: &str) -> Vec<Decoration> {
    let mut out = Vec::new();
    let mut fenced_body = None;
    for (event, range) in Parser::new(source).into_offset_iter() {
        let fragment = &source[range.clone()];
        let mut add = |range, format| out.push(Decoration { range, format });
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                add(range.clone(), Format::Heading(level));
                let trimmed = fragment.trim_start_matches(' ');
                let indent = fragment.len() - trimmed.len();
                let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
                if (1..=6).contains(&hashes) {
                    let end = range.start + indent + hashes;
                    let space = source
                        .as_bytes()
                        .get(end)
                        .is_some_and(|b| b.is_ascii_whitespace());
                    add(range.start..end + usize::from(space), Format::Syntax);
                }
            }
            Event::Start(Tag::Strong) | Event::Start(Tag::Emphasis) => {
                let width = if matches!(event, Event::Start(Tag::Strong)) {
                    2
                } else {
                    1
                };
                if range.len() >= width * 2 {
                    add(
                        range.clone(),
                        if width == 2 {
                            Format::Bold
                        } else {
                            Format::Italic
                        },
                    );
                    add(range.start..range.start + width, Format::Syntax);
                    add(range.end - width..range.end, Format::Syntax);
                }
            }
            Event::Code(_) => {
                add(range.clone(), Format::Code);
                let ticks = fragment.bytes().take_while(|&b| b == b'`').count();
                if ticks > 0 && range.len() >= ticks * 2 {
                    add(range.start..range.start + ticks, Format::Syntax);
                    add(range.end - ticks..range.end, Format::Syntax);
                }
            }
            Event::Start(Tag::Link { .. }) => {
                add(range.clone(), Format::Link);
                if fragment.starts_with('[') {
                    add(range.start..range.start + 1, Format::Syntax);
                }
            }
            Event::End(pulldown_cmark::TagEnd::Link) => {
                if fragment.starts_with('[')
                    && let Some(close) = fragment.rfind("](")
                {
                    add(range.start + close..range.end, Format::Syntax);
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                add(range.clone(), Format::Code);
                if matches!(kind, CodeBlockKind::Fenced(_)) {
                    let first_end = fragment.find('\n').unwrap_or(fragment.len());
                    add(range.start..range.start + first_end, Format::Syntax);
                    fenced_body = Some((
                        fragment.as_bytes()[0] as char,
                        (range.start + first_end + 1).min(range.end),
                    ));
                }
            }
            Event::Text(_) => {
                if let Some((_, body_end)) = &mut fenced_body {
                    *body_end = range.end;
                }
            }
            Event::End(pulldown_cmark::TagEnd::CodeBlock) => {
                if let Some((marker, body_end)) = fenced_body.take()
                    && let Some(fence) = source[body_end..range.end].find(marker)
                {
                    add(body_end + fence..range.end, Format::Syntax);
                }
            }
            Event::Start(Tag::BlockQuote(_)) => add(range, Format::Quote),
            _ => {}
        }
    }
    out
}

pub fn spans(source: &str, selection: (usize, usize), palette: Palette) -> Vec<Span> {
    let active = active_lines(source, selection);
    let decorations = decorations(source);
    let mut spans = Vec::new();
    for d in decorations.iter().filter(|d| d.format != Format::Syntax) {
        let span = Span::new(d.range.start, d.range.end);
        spans.push(match d.format {
            Format::Heading(level) => span
                .bold()
                .size(match level {
                    1 => 32.0,
                    2 => 24.0,
                    _ => 19.0,
                })
                .color(palette.text),
            Format::Bold => span.bold(),
            Format::Italic => span.italic(),
            Format::Code => span.mono().bg(palette.code_bg, 2.0),
            Format::Link => span.color(palette.accent).underline(palette.accent),
            Format::Quote => span.italic().color(palette.muted),
            Format::Syntax => unreachable!(),
        });
    }
    for d in decorations.iter().filter(|d| d.format == Format::Syntax) {
        let mut start = d.range.start;
        for line in source[d.range.clone()].split_inclusive('\n') {
            let end = start + line.len();
            let visible = start <= active.end && end > active.start;
            let span = Span::new(start, end).color(palette.muted);
            spans.push(if visible {
                span
            } else {
                span.size(0.01).color(palette.text.alpha(0.0))
            });
            start = end;
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_nested_emphasis_without_changing_source() {
        let source = "# Café\n\n**bold and *soft*** with `code`";
        let before = source.to_owned();
        let ds = decorations(source);
        assert!(ds.iter().any(|d| d.format == Format::Heading(1)));
        assert!(ds.iter().any(|d| d.format == Format::Bold));
        assert!(ds.iter().any(|d| d.format == Format::Italic));
        assert!(ds.iter().any(|d| d.format == Format::Code));
        for d in ds {
            assert!(source.is_char_boundary(d.range.start));
            assert!(source.is_char_boundary(d.range.end));
        }
        assert_eq!(source, before);
    }

    #[test]
    fn selection_reveals_all_touched_lines() {
        assert_eq!(active_lines("one\ntwo\nthree", (5, 10)), 4..13);
        assert_eq!(active_lines("one\ntwo\n", (8, 8)), 8..8);
        assert_eq!(active_lines("", (0, 0)), 0..0);
    }

    #[test]
    fn code_contents_are_not_parsed_as_emphasis() {
        let ds = decorations("```md\n**literal**\n```\n");
        assert!(!ds.iter().any(|d| d.format == Format::Bold));
        assert_eq!(ds.iter().filter(|d| d.format == Format::Syntax).count(), 2);
    }

    #[test]
    fn fence_like_code_contents_are_never_concealed() {
        for source in [
            "````md\n```literal",
            "````md\n```\n",
            "```sh\n~~~literal",
            "```sh\n~~~\n",
            "```\n    ```",
            "   ```\n    ```",
            "```\n```trailing text\n",
            "```\n\n",
            "```",
        ] {
            let syntax: Vec<_> = decorations(source)
                .into_iter()
                .filter(|d| d.format == Format::Syntax)
                .collect();
            assert_eq!(syntax.len(), 1, "{source:?}: {syntax:?}");
        }
    }

    #[test]
    fn only_parser_recognized_closing_fences_are_concealed() {
        for (source, closing) in [
            ("````md\n```literal\n````\n", "````"),
            ("```sh\n~~~literal\n```\n", "```"),
            ("~~~\nbody\n~~~~\n", "~~~~"),
            ("```\nbody\n  ```\t \n", "```"),
            ("```\r\nbody\r\n```\r\n", "```"),
            ("```\n```", "```"),
            ("> ```md\n> body\n> ```\n", "```"),
            ("- ```md\n  body\n  ```\n", "```"),
        ] {
            let syntax: Vec<_> = decorations(source)
                .into_iter()
                .filter(|d| d.format == Format::Syntax)
                .collect();
            assert_eq!(syntax.len(), 2, "{source:?}: {syntax:?}");
            assert_eq!(source[syntax[1].range.clone()].trim_end(), closing);
        }
    }

    #[test]
    fn escaped_and_unfinished_markers_stay_literal() {
        assert!(decorations("\\*literal\\* and **unfinished").is_empty());
    }

    #[test]
    fn markers_collapse_only_outside_the_active_line() {
        let source = "**one**\n**two**";
        let color = rune::oklch(0.8, 0.0, 0.0);
        let palette = Palette {
            text: color,
            muted: color,
            accent: color,
            code_bg: color,
        };
        let first = spans(source, (2, 2), palette);
        assert!(!first.iter().any(|s| s.start == 0 && s.size == Some(0.01)));
        assert!(first.iter().any(|s| s.start == 8 && s.size == Some(0.01)));
        let selected = spans(source, (2, 11), palette);
        assert!(!selected.iter().any(|s| s.size == Some(0.01)));
    }

    #[test]
    fn inline_links_hide_destination_but_not_label() {
        let source = "[Rune](https://example.com)";
        let ds = decorations(source);
        assert!(
            ds.iter().any(|d| d.format == Format::Syntax
                && &source[d.range.clone()] == "](https://example.com)")
        );
    }
}
