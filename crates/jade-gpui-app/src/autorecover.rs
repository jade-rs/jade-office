//! 自動回復 and バックアップ, as Word for Mac's 環境設定 → 保存 sets them:
//!
//! - while a document has unsaved edits, a copy of it is written to the
//!   AutoRecovery folder every few minutes (自動回復用データを保存する,
//!   the interval and the folder are settings);
//! - a save, or closing a document with nothing unsaved, removes its copy;
//!   a crash, a forced quit or closing with edits unsaved leaves it, and
//!   the next launch offers it back (未保存の文書の回復);
//! - バックアップ ファイルを常に作成する keeps the file a save is about to
//!   replace, beside it, as `<name> のバックアップ.wbk`.
//!
//! Each copy is the document's `.docx` with a line of text beside it
//! naming the file it is a copy of and when it was made.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gpui::Context;

use crate::DocumentView;

/// The save settings, read once and written when changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// 自動回復用データを保存する.
    pub enabled: bool,
    /// How often, in minutes.
    pub minutes: u32,
    /// Where the copies go; the default folder when `None`.
    pub location: Option<PathBuf>,
    /// バックアップ ファイルを常に作成する.
    pub always_backup: bool,
    /// Where a new document is saved first; Documents when `None`.
    pub save_folder: Option<PathBuf>,
}

impl Default for Settings {
    /// Word's: every 10 minutes, no backups.
    fn default() -> Self {
        Self {
            enabled: true,
            minutes: 10,
            location: None,
            always_backup: false,
            save_folder: None,
        }
    }
}

impl Settings {
    pub fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "autorecover" => settings.enabled = value == "on",
                "minutes" => {
                    if let Ok(minutes) = value.parse::<u32>() {
                        settings.minutes = minutes.clamp(1, 120);
                    }
                }
                "location" if !value.is_empty() => settings.location = Some(PathBuf::from(value)),
                "backup" => settings.always_backup = value == "on",
                "save_folder" if !value.is_empty() => {
                    settings.save_folder = Some(PathBuf::from(value))
                }
                _ => {}
            }
        }
        settings
    }

    pub fn format(&self) -> String {
        let on = |value: bool| if value { "on" } else { "off" };
        let path = |path: &Option<PathBuf>| {
            path.as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        format!(
            "autorecover={}\nminutes={}\nlocation={}\nbackup={}\nsave_folder={}\n",
            on(self.enabled),
            self.minutes,
            path(&self.location),
            on(self.always_backup),
            path(&self.save_folder),
        )
    }

    /// The folder the copies go in.
    pub fn folder(&self) -> Option<PathBuf> {
        self.location
            .clone()
            .or_else(|| support().map(|support| support.join("AutoRecovery")))
    }
}

/// The app's own folder in Application Support. Tests have none, so they
/// never read or write the user's.
fn support() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/jade-office"))
}

thread_local! {
    static SETTINGS: std::cell::RefCell<Option<Settings>> = const { std::cell::RefCell::new(None) };
}

pub fn settings() -> Settings {
    SETTINGS.with(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| {
                support()
                    .and_then(|support| std::fs::read_to_string(support.join("save.txt")).ok())
                    .map(|text| Settings::parse(&text))
                    .unwrap_or_default()
            })
            .clone()
    })
}

pub fn set_settings(settings: Settings) {
    if let Some(support) = support() {
        std::fs::create_dir_all(&support).ok();
        std::fs::write(support.join("save.txt"), settings.format()).ok();
    }
    SETTINGS.with(|cell| *cell.borrow_mut() = Some(settings));
}

// ---- backups -----------------------------------------------------------------

/// The backup a save of `path` keeps: `<name> のバックアップ.wbk` beside
/// it (`Backup of <name>.wbk` in English).
pub fn backup_path(path: &Path) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let name = crate::i18n::format("files.backup_name", &[("name", &stem)]);
    Some(path.with_file_name(format!("{name}.wbk")))
}

/// Before a save replaces `path`: keeps what is there as its backup, when
/// backups are on.
pub fn back_up_before_writing(path: &Path) {
    back_up_with(&settings(), path);
}

fn back_up_with(settings: &Settings, path: &Path) {
    if !settings.always_backup || !path.is_file() {
        return;
    }
    if let Some(backup) = backup_path(path) {
        std::fs::copy(path, backup).ok();
    }
}

// ---- recovery copies ------------------------------------------------------------

/// A copy the AutoRecovery folder holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovered {
    /// The copy itself.
    pub copy: PathBuf,
    /// The file it is a copy of, or the name of a document never saved.
    pub original: String,
    /// When it was made, seconds since the epoch.
    pub saved_at: u64,
}

/// Every copy in `folder`, newest first.
pub fn recovered_in(folder: &Path) -> Vec<Recovered> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut out: Vec<Recovered> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "docx"))
        .filter_map(|copy| {
            let info = std::fs::read_to_string(copy.with_extension("info")).ok()?;
            let mut lines = info.lines();
            let original = lines.next()?.to_string();
            let saved_at = lines.next().and_then(|line| line.parse().ok()).unwrap_or(0);
            Some(Recovered {
                copy,
                original,
                saved_at,
            })
        })
        .collect();
    out.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    out
}

/// The copies the next launch offers back.
pub fn recovered() -> Vec<Recovered> {
    settings()
        .folder()
        .map(|folder| recovered_in(&folder))
        .unwrap_or_default()
}

/// Removes a copy and the line beside it.
pub fn discard(copy: &Path) {
    std::fs::remove_file(copy).ok();
    std::fs::remove_file(copy.with_extension("info")).ok();
}

/// Writes `docx` as the copy of `original` in `folder`, under `id`.
pub fn write_copy(
    folder: &Path,
    id: &str,
    original: &str,
    docx: &[u8],
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(folder)?;
    let stem = Path::new(original)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into());
    let copy = folder.join(format!("{stem} ({id}).docx"));
    crate::shell::write_in_place(&copy, docx)?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    std::fs::write(copy.with_extension("info"), format!("{original}\n{now}\n"))?;
    Ok(copy)
}

/// A document's recovery state: its copy, and when it was last made.
#[derive(Default)]
pub struct Recovery {
    pub copy: Option<PathBuf>,
    pub last: Option<std::time::Instant>,
    id: String,
    /// Whether the document has unsaved edits, as of the last tick: what
    /// decides whether closing it keeps the copy.
    unsaved: bool,
}

impl Drop for Recovery {
    /// A document closed with nothing unsaved needs no copy; one closed
    /// with edits keeps it, to be offered back.
    fn drop(&mut self) {
        if !self.unsaved {
            if let Some(copy) = self.copy.take() {
                discard(&copy);
            }
        }
    }
}

/// Starts `view`'s clock: every half minute it checks whether a copy is
/// due.
pub fn start(cx: &mut Context<DocumentView>) {
    cx.spawn(async move |view, cx| loop {
        cx.background_executor()
            .timer(Duration::from_secs(30))
            .await;
        let alive = view.update(cx, |view, _cx| tick(view)).is_ok();
        if !alive {
            break;
        }
    })
    .detach();
}

/// Writes a copy when one is due: autorecover on, edits unsaved, and the
/// interval gone by since the last.
pub fn tick(view: &mut DocumentView) {
    let settings = settings();
    view.recovery.unsaved = view.dirty;
    if !settings.enabled || !view.dirty {
        return;
    }
    let due = view
        .recovery
        .last
        .is_none_or(|last| last.elapsed() >= Duration::from_secs(settings.minutes as u64 * 60));
    if due {
        snapshot(view, &settings);
    }
}

/// Writes the copy now.
pub fn snapshot(view: &mut DocumentView, settings: &Settings) {
    let Some(folder) = settings.folder() else {
        return;
    };
    let Ok(docx) = view.document.export_docx() else {
        return;
    };
    if view.recovery.id.is_empty() {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|elapsed| elapsed.subsec_nanos())
            .unwrap_or(0);
        view.recovery.id = format!("{}-{nanos:08x}", std::process::id());
    }
    let id = view.recovery.id.clone();
    if let Ok(copy) = write_copy(&folder, &id, view.path.as_ref(), &docx) {
        view.recovery.copy = Some(copy);
        view.recovery.last = Some(std::time::Instant::now());
        view.recovery.unsaved = true;
    }
}

/// After a save: the copy is not needed.
pub fn saved(view: &mut DocumentView) {
    view.recovery.unsaved = false;
    view.recovery.last = None;
    if let Some(copy) = view.recovery.copy.take() {
        discard(&copy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("jade-autorecover-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&directory).ok();
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn settings_round_trip_through_their_file() {
        let settings = Settings {
            enabled: false,
            minutes: 3,
            location: Some(PathBuf::from("/tmp/recovery")),
            always_backup: true,
            save_folder: Some(PathBuf::from("/Users/me/Documents/Work")),
        };
        assert_eq!(Settings::parse(&settings.format()), settings);
        // Out-of-range minutes are held to Word's range.
        assert_eq!(Settings::parse("minutes=0").minutes, 1);
        assert_eq!(Settings::parse("").minutes, 10);
    }

    #[test]
    fn copies_are_listed_newest_first_and_discarded() {
        let folder = scratch("copies");
        let first = write_copy(&folder, "a", "/docs/report.docx", b"one").unwrap();
        std::fs::write(first.with_extension("info"), "/docs/report.docx\n100\n").unwrap();
        let second = write_copy(&folder, "b", "文書 1.docx", b"two").unwrap();
        std::fs::write(second.with_extension("info"), "文書 1.docx\n200\n").unwrap();
        let listed = recovered_in(&folder);
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].original, "文書 1.docx");
        assert_eq!(listed[1].original, "/docs/report.docx");
        assert_eq!(std::fs::read(&listed[1].copy).unwrap(), b"one");
        discard(&listed[0].copy);
        assert_eq!(recovered_in(&folder).len(), 1);
        std::fs::remove_dir_all(folder).ok();
    }

    #[test]
    fn a_backup_keeps_the_file_a_save_replaces() {
        let folder = scratch("backup");
        let file = folder.join("report.docx");
        std::fs::write(&file, b"before").unwrap();
        let mut settings = Settings::default();
        back_up_with(&settings, &file);
        let backup = backup_path(&file).unwrap();
        assert!(!backup.exists(), "no backup while the setting is off");
        settings.always_backup = true;
        back_up_with(&settings, &file);
        assert_eq!(std::fs::read(&backup).unwrap(), b"before");
        assert_eq!(backup.extension().unwrap(), "wbk");
        std::fs::remove_dir_all(folder).ok();
    }
}
