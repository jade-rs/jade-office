//! 自動保存 and バージョン履歴, as Office for Mac has them, for any of the
//! three apps (Word is the only one that uses them so far):
//!
//! - **自動保存** — the switch in the title bar. While it is on, a document
//!   with edits is saved to its own file a moment after the edits stop
//!   ([`AUTOSAVE_DELAY_MS`]), never while a dialog is being filled in, so
//!   a half-made change is not what lands on disk. [`AutoSave`] decides
//!   when; the shell does the saving.
//! - **バージョン履歴** — every save leaves a copy of what was written in
//!   the app's own data folder (never beside the user's file), so ファイル
//!   › バージョン履歴 can list them, open one, or put one back (復元).
//!   Autosaves a few minutes apart fold into one version, as Office's own
//!   history shows a run of autosaves as one entry; an explicit save always
//!   makes a new one. [`VersionStore`] keeps them.
//!
//! Nothing here knows what a document is: a version is the bytes a save
//! wrote, and a document is the path it was saved to.

use std::io;
use std::path::{Path, PathBuf};

/// How long after the last edit 自動保存 waits before saving.
pub const AUTOSAVE_DELAY_MS: f64 = 2_000.0;

/// Autosaves this close to the version before them replace it rather than
/// adding another.
pub const MERGE_WINDOW_SECS: u64 = 10 * 60;

/// How many versions of one document are kept; the oldest go first.
pub const KEEP: usize = 50;

// ---- 自動保存 -------------------------------------------------------------------

/// The title bar's 自動保存 switch for one document, and when its next save
/// is due.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AutoSave {
    /// The switch.
    pub on: bool,
    /// The edit counter as last seen, to notice a new edit.
    seen: u64,
    /// When the last edit was noticed, in milliseconds.
    last_edit: Option<f64>,
}

impl AutoSave {
    pub fn new(on: bool) -> Self {
        Self {
            on,
            ..Self::default()
        }
    }

    /// Called on a clock: `edits` is a counter the document bumps on every
    /// change, `unsaved` whether it has changes not yet saved, `busy`
    /// whether a dialog is open over it, `now` in milliseconds. Whether to
    /// save now.
    ///
    /// The save waits for [`AUTOSAVE_DELAY_MS`] without a new edit, and
    /// for the dialog to close, however long that takes.
    pub fn poll(&mut self, edits: u64, unsaved: bool, busy: bool, now: f64) -> bool {
        if edits != self.seen {
            self.seen = edits;
            self.last_edit = Some(now);
        }
        if !self.on || !unsaved || busy {
            return false;
        }
        match self.last_edit {
            Some(last) => now - last >= AUTOSAVE_DELAY_MS,
            // Changes from before the clock started: due now.
            None => true,
        }
    }

    /// Turns the switch, as a click does. Turning it on with changes
    /// unsaved saves them at the next poll.
    pub fn toggle(&mut self) -> bool {
        self.on = !self.on;
        self.last_edit = None;
        self.on
    }

    /// After a save, whichever made it.
    pub fn saved(&mut self) {
        self.last_edit = None;
    }
}

// ---- バージョン履歴 ----------------------------------------------------------------

/// What made a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionKind {
    /// The file as it was before this app first saved over it.
    Original,
    /// 保存 (⌘S, 名前を付けて保存).
    Saved,
    /// 自動保存.
    AutoSaved,
    /// 復元: an earlier version put back.
    Restored,
}

impl VersionKind {
    const ALL: [VersionKind; 4] = [
        VersionKind::Original,
        VersionKind::Saved,
        VersionKind::AutoSaved,
        VersionKind::Restored,
    ];

    fn tag(self) -> &'static str {
        match self {
            VersionKind::Original => "original",
            VersionKind::Saved => "saved",
            VersionKind::AutoSaved => "autosaved",
            VersionKind::Restored => "restored",
        }
    }

    fn of_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.tag() == tag)
    }

    /// The string key of the note the history shows under the time.
    pub fn label_key(self) -> &'static str {
        match self {
            VersionKind::Original => "versions.kind.original",
            VersionKind::Saved => "versions.kind.saved",
            VersionKind::AutoSaved => "versions.kind.autosaved",
            VersionKind::Restored => "versions.kind.restored",
        }
    }
}

/// One version of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    /// The copy in the store.
    pub file: PathBuf,
    /// When it was saved, seconds since the epoch.
    pub saved_at: u64,
    pub kind: VersionKind,
    /// Its size in bytes.
    pub size: u64,
}

/// The versions of every document, each document's in a folder of its own
/// under `root`: `<name>-<hash of its path>/<seconds>-<sequence>-<kind>.<ext>`,
/// with `document.txt` naming the file they are versions of.
#[derive(Clone, Debug)]
pub struct VersionStore {
    root: PathBuf,
}

/// FNV-1a: a hash that stays the same from run to run, unlike std's.
fn stable_hash(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

impl VersionStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder `document`'s versions are kept in.
    pub fn folder(&self, document: &Path) -> PathBuf {
        let key = document.to_string_lossy();
        let stem: String = document
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document".into())
            .chars()
            .filter(|c| !matches!(c, '/' | '\\' | ':'))
            .take(40)
            .collect();
        self.root.join(format!("{stem}-{:016x}", stable_hash(&key)))
    }

    /// `document`'s versions, newest first.
    pub fn list(&self, document: &Path) -> Vec<Version> {
        let Ok(entries) = std::fs::read_dir(self.folder(document)) else {
            return Vec::new();
        };
        let mut versions: Vec<(u64, u64, Version)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let file = entry.path();
                let name = file.file_stem()?.to_string_lossy().into_owned();
                let mut parts = name.splitn(3, '-');
                let saved_at: u64 = parts.next()?.parse().ok()?;
                let sequence: u64 = parts.next()?.parse().ok()?;
                let kind = VersionKind::of_tag(parts.next()?)?;
                let size = entry.metadata().ok()?.len();
                Some((
                    saved_at,
                    sequence,
                    Version {
                        file,
                        saved_at,
                        kind,
                        size,
                    },
                ))
            })
            .collect();
        versions.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
        versions
            .into_iter()
            .map(|(_, _, version)| version)
            .collect()
    }

    /// Keeps `bytes` as the newest version of `document`, made by `kind` at
    /// `now` (seconds since the epoch). The version it made, or `None` when
    /// the newest one already holds exactly these bytes.
    ///
    /// An autosave within [`MERGE_WINDOW_SECS`] of an autosaved newest
    /// version replaces it; past [`KEEP`] versions the oldest go.
    pub fn record(
        &self,
        document: &Path,
        bytes: &[u8],
        kind: VersionKind,
        now: u64,
    ) -> io::Result<Option<Version>> {
        let folder = self.folder(document);
        std::fs::create_dir_all(&folder)?;
        std::fs::write(
            folder.join("document.txt"),
            format!("{}\n", document.to_string_lossy()),
        )?;
        let existing = self.list(document);
        if let Some(newest) = existing.first() {
            if std::fs::read(&newest.file).is_ok_and(|kept| kept == bytes) {
                return Ok(None);
            }
            if kind == VersionKind::AutoSaved
                && newest.kind == VersionKind::AutoSaved
                && now.saturating_sub(newest.saved_at) < MERGE_WINDOW_SECS
            {
                std::fs::remove_file(&newest.file)?;
            }
        }
        let sequence = existing
            .iter()
            .filter(|version| version.saved_at == now)
            .count();
        let extension = document
            .extension()
            .map(|extension| extension.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bin".into());
        let file = folder.join(format!("{now}-{sequence:04}-{}.{extension}", kind.tag()));
        std::fs::write(&file, bytes)?;
        for old in self.list(document).into_iter().skip(KEEP) {
            std::fs::remove_file(old.file).ok();
        }
        Ok(Some(Version {
            file,
            saved_at: now,
            kind,
            size: bytes.len() as u64,
        }))
    }

    /// Before the first save over `document` in a session: what is on disk
    /// now, kept as its 元のバージョン unless a version already holds it.
    pub fn keep_original(&self, document: &Path, now: u64) -> io::Result<Option<Version>> {
        let Ok(bytes) = std::fs::read(document) else {
            return Ok(None);
        };
        let held = self
            .list(document)
            .iter()
            .any(|version| std::fs::read(&version.file).is_ok_and(|kept| kept == bytes));
        if held {
            return Ok(None);
        }
        self.record(document, &bytes, VersionKind::Original, now)
    }

    /// The bytes of `version`.
    pub fn read(&self, version: &Version) -> io::Result<Vec<u8>> {
        std::fs::read(&version.file)
    }
}

// ---- how a version is shown ------------------------------------------------------

/// Days since 1970-01-01 as a civil date (Howard Hinnant's algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// When a version was saved, as the history lists it: `2026年11月3日
/// 14:05` in Japanese, `Nov 3, 2026, 2:05 PM` in English. `offset` is the
/// local zone's offset from UTC in seconds.
pub fn saved_label(saved_at: u64, offset: i64, japanese: bool) -> String {
    let local = saved_at as i64 + offset;
    let (year, month, day) = civil(local.div_euclid(86_400));
    let seconds = local.rem_euclid(86_400);
    let (hour, minute) = (seconds / 3600, (seconds % 3600) / 60);
    if japanese {
        format!("{year}年{month}月{day}日 {hour}:{minute:02}")
    } else {
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let (hour12, half) = match hour {
            0 => (12, "AM"),
            1..=11 => (hour, "AM"),
            12 => (12, "PM"),
            _ => (hour - 12, "PM"),
        };
        format!(
            "{} {day}, {year}, {hour12}:{minute:02} {half}",
            MONTHS[(month - 1) as usize]
        )
    }
}

/// The name an opened version goes by: `<stem> (<when>).<ext>`, a new
/// document of its own until it is saved.
pub fn opened_name(document: &Path, label: &str) -> String {
    let stem = document
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into());
    let extension = document
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
        .unwrap_or_else(|| "docx".into());
    let when = label.replace([':', '/'], "-");
    format!("{stem} ({when}).{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_waits_for_the_edits_to_stop_and_for_the_dialog() {
        let mut auto = AutoSave::new(true);
        // Nothing to save.
        assert!(!auto.poll(0, false, false, 0.0));
        // An edit: not yet, then due once the delay has gone by quietly.
        assert!(!auto.poll(1, true, false, 100.0));
        assert!(!auto.poll(1, true, false, 1_500.0));
        // Another edit restarts the wait.
        assert!(!auto.poll(2, true, false, 1_900.0));
        assert!(!auto.poll(2, true, false, 3_800.0));
        // A dialog open holds it, however long.
        assert!(!auto.poll(2, true, true, 9_000.0));
        assert!(auto.poll(2, true, false, 9_100.0));
        auto.saved();
        assert!(!auto.poll(2, false, false, 9_200.0));
        // Off, nothing is saved.
        auto.toggle();
        assert!(!auto.poll(3, true, false, 20_000.0));
        // Turned on with changes waiting: saved at the next poll.
        assert!(auto.toggle());
        assert!(auto.poll(3, true, false, 20_001.0));
    }

    #[test]
    fn labels_follow_the_language() {
        // 2026-11-03 05:05 UTC, shown in Tokyo (+9).
        let at = 1_793_682_300;
        assert_eq!(saved_label(at, 9 * 3600, true), "2026年11月3日 14:05");
        assert_eq!(saved_label(at, 9 * 3600, false), "Nov 3, 2026, 2:05 PM");
        assert_eq!(saved_label(at, 0, false), "Nov 3, 2026, 5:05 AM");
        assert_eq!(
            opened_name(Path::new("/docs/契約書.docx"), "2026年11月3日 14:05"),
            "契約書 (2026年11月3日 14-05).docx"
        );
    }
}
