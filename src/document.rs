use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub const MAX_BYTES: u64 = 2 * 1024 * 1024;

pub struct Document {
    pub path: Option<PathBuf>,
    pub saved: String,
}

impl Document {
    pub fn untitled() -> Self {
        Self {
            path: None,
            saved: String::new(),
        }
    }

    pub fn open(path: &Path) -> io::Result<(Self, String)> {
        let path = fs::canonicalize(path)?;
        let text = read_note(&path)?;
        Ok((
            Self {
                path: Some(path),
                saved: text.clone(),
            },
            text,
        ))
    }

    pub fn dirty(&self, text: &str) -> bool {
        self.saved != text
    }

    pub fn save(&mut self, text: &str, new_path: Option<&Path>) -> io::Result<()> {
        if text.len() as u64 > MAX_BYTES {
            return Err(io::Error::other(
                "This prototype supports notes up to 2 MiB.",
            ));
        }
        let path = new_path
            .or(self.path.as_deref())
            .ok_or_else(|| io::Error::other("Choose a file before saving."))?;
        if path
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("md"))
        {
            return Err(io::Error::other("Choose a filename ending in .md."));
        }
        let parent = fs::canonicalize(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )?;
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::other("Choose a filename."))?;
        let target = parent.join(name);
        let existing = fs::symlink_metadata(&target);
        let expected = if new_path.is_none() && self.path.as_deref() == Some(target.as_path()) {
            Some(self.saved.as_str())
        } else {
            None
        };
        match existing {
            Ok(meta) => {
                if meta.file_type().is_symlink() || !meta.is_file() {
                    return Err(io::Error::other(
                        "Refusing to replace a symlink or non-file.",
                    ));
                }
                match expected {
                    Some(saved) if read_note(&target)? == saved => {}
                    Some(_) => {
                        return Err(io::Error::other(
                            "The file changed outside Folio. Save a copy instead to keep both versions.",
                        ));
                    }
                    None => {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "That file already exists. Choose a new filename.",
                        ));
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound && expected.is_none() => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(io::Error::other(
                    "The original file was removed. Save a copy to a new filename.",
                ));
            }
            Err(e) => return Err(e),
        }
        let mut temp = tempfile::NamedTempFile::new_in(&parent)?;
        if expected.is_some() {
            temp.as_file()
                .set_permissions(fs::metadata(&target)?.permissions())?;
        }
        temp.write_all(text.as_bytes())?;
        temp.as_file().sync_all()?;
        if let Some(saved) = expected {
            if read_note(&target)? != saved {
                return Err(io::Error::other(
                    "The file changed while saving. Your edits are still in memory.",
                ));
            }
            temp.persist(&target).map_err(|e| e.error)?;
        } else {
            temp.persist_noclobber(&target).map_err(|e| e.error)?;
        }
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "The file was written, but its directory could not be synced: {error}. \
                         Durability is uncertain; your draft is still in memory. Save a copy to keep it."
                    ),
                )
            })?;
        self.path = Some(target);
        self.saved = text.to_owned();
        Ok(())
    }
}

fn read_note(path: &Path) -> io::Result<String> {
    if !fs::metadata(path)?.is_file() {
        return Err(io::Error::other("Choose a regular Markdown file."));
    }
    let file = File::from(rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::NOCTTY,
        rustix::fs::Mode::empty(),
    )?);
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Choose a regular Markdown file."));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::other(
            "This prototype supports notes up to 2 MiB.",
        ));
    }
    String::from_utf8(bytes).map_err(|_| io::Error::other("This file is not UTF-8 Markdown."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_every_byte() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        let source = "# Café\r\n\r\n**bold**  \r\n[link](other.md)\r\n";
        let mut doc = Document::untitled();
        doc.save(source, Some(&path)).unwrap();
        let (mut doc, loaded) = Document::open(&path).unwrap();
        assert_eq!(loaded, source);
        doc.save(&(loaded + "\nnew"), None).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), format!("{source}\nnew"));
    }

    #[test]
    fn failed_saves_preserve_the_document_state() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.md");
        fs::write(&original, "original").unwrap();
        let (mut doc, _) = Document::open(&original).unwrap();
        for target in [dir.path().join("invalid.txt"), original.clone()] {
            assert!(doc.save("draft", Some(&target)).is_err());
            assert_eq!(doc.path.as_deref(), Some(original.as_path()));
            assert_eq!(doc.saved, "original");
            assert!(doc.dirty("draft"));
        }
        let oversized = "x".repeat(MAX_BYTES as usize + 1);
        assert!(doc.save(&oversized, None).is_err());
        assert_eq!(doc.saved, "original");
        assert_eq!(fs::read_to_string(&original).unwrap(), "original");
    }

    #[test]
    fn existing_file_permissions_survive_atomic_replacement() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "original").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let (mut doc, _) = Document::open(&path).unwrap();
        doc.save("updated", None).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[test]
    fn external_change_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "original").unwrap();
        let (mut doc, _) = Document::open(&path).unwrap();
        fs::write(&path, "external edit").unwrap();
        assert!(doc.save("my edit", None).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "external edit");
        assert!(doc.dirty("my edit"));
    }

    #[test]
    fn save_copy_never_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "keep me").unwrap();
        assert!(Document::untitled().save("replace", Some(&path)).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "keep me");
    }

    #[test]
    fn save_copy_rejects_the_current_file_and_keeps_the_draft_dirty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "original").unwrap();
        let (mut doc, _) = Document::open(&path).unwrap();
        let alias = dir.path().join(".").join("note.md");
        let error = doc.save("my edit", Some(&alias)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
        assert_eq!(doc.path.as_deref(), Some(path.as_path()));
        assert!(doc.dirty("my edit"));
        doc.save("my edit", None).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "my edit");
    }

    #[test]
    fn save_copy_to_a_new_file_preserves_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.md");
        let copy = dir.path().join("copy.md");
        fs::write(&original, "original").unwrap();
        let (mut doc, _) = Document::open(&original).unwrap();
        doc.save("my edit", Some(&copy)).unwrap();
        assert_eq!(fs::read_to_string(&original).unwrap(), "original");
        assert_eq!(fs::read_to_string(&copy).unwrap(), "my edit");
        assert_eq!(doc.path.as_deref(), Some(copy.as_path()));
        assert!(!doc.dirty("my edit"));
    }

    #[test]
    fn named_pipes_are_rejected_without_waiting_for_a_writer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pipe.md");
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &path,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            send.send(Document::open(&path).err().map(|e| e.to_string()))
                .unwrap();
        });
        let error = receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("opening a named pipe must not block");
        assert_eq!(error.as_deref(), Some("Choose a regular Markdown file."));
        reader.join().unwrap();
    }

    #[test]
    fn deleted_original_requires_save_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "original").unwrap();
        let (mut doc, _) = Document::open(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(doc.save("my edit", None).is_err());
    }

    #[test]
    fn invalid_utf8_and_large_files_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, [0xff]).unwrap();
        assert!(Document::open(&path).is_err());
        fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        assert!(Document::open(&path).is_err());
    }

    #[test]
    fn symlinks_are_resolved_on_open_and_rejected_on_save_copy() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.md");
        let link = dir.path().join("link.md");
        fs::write(&target, "original").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let (mut doc, _) = Document::open(&link).unwrap();
        doc.save("updated", None).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "updated");
        assert!(Document::untitled().save("replace", Some(&link)).is_err());
    }
}
