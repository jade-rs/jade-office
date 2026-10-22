//! One app for every format.
//!
//! Word, Excel and PowerPoint files each get the view that suits them — the
//! document view, the workbook view, the deck view — and all of them open as
//! tabs of one workspace window (see `workspace`), the frame jade-ide puts
//! round its editors. This module is what sits outside that window: which
//! format a path is, finding or making the workspace to open it in, the Open…
//! prompt, and the recent-files list.

use gpui::{
    point, prelude::*, px, size, App, Bounds, Focusable, PathPromptOptions, SharedString,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowOptions,
};
use std::path::{Path, PathBuf};

use crate::theme;
use crate::workspace::Workspace;

/// What a file opens as, decided by its extension. The macro-enabled variants
/// read the same: their macros are a part the readers never run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Document,
    Workbook,
    Deck,
}

impl Kind {
    pub fn of(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_string_lossy().to_lowercase();
        match extension.as_str() {
            "docx" | "docm" | "dotx" | "dotm" => Some(Kind::Document),
            // Word's other formats, converted as they open (`word_files`).
            // A PDF opens converted to an editable document, as in Word.
            "doc" | "dot" | "wbk" | "rtf" | "txt" | "odt" | "fodt" | "htm" | "html" | "pdf" => {
                Some(Kind::Document)
            }
            // A CSV opens as a workbook of one sheet, as in Excel.
            "xlsx" | "xlsm" | "xltx" | "xltm" | "csv" => Some(Kind::Workbook),
            "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => Some(Kind::Deck),
            _ => None,
        }
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1440.0), px(920.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::from("jade")),
            // Drawn over by the workspace's own title bar; the traffic lights
            // are placed to sit in it.
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.0), px(13.0))),
        }),
        window_background: WindowBackgroundAppearance::Opaque,
        ..Default::default()
    }
}

/// The workspace window to open files in: the frontmost one, or a new one.
pub fn workspace(cx: &mut App) -> Result<WindowHandle<Workspace>, String> {
    let front = cx
        .active_window()
        .and_then(|window| window.downcast::<Workspace>());
    if let Some(handle) = front.or_else(|| {
        cx.windows()
            .into_iter()
            .find_map(|window| window.downcast::<Workspace>())
    }) {
        return Ok(handle);
    }
    let handle = cx
        .open_window(window_options(cx), |_window, cx| cx.new(Workspace::new))
        .map_err(|error| format!("cannot open a window: {error}"))?;
    handle
        .update(cx, |workspace, window, cx| {
            follow_appearance(window);
            window.focus(&workspace.focus_handle(cx))
        })
        .ok();
    cx.activate(true);
    Ok(handle)
}

/// Repaints every window in the system's palette when macOS switches
/// between light and dark, for as long as `window` is open.
pub fn follow_appearance(window: &mut Window) {
    window
        .observe_window_appearance(|window, cx| {
            let was_dark = theme::is_dark();
            theme::set_system(window.appearance());
            if theme::is_dark() != was_dark {
                cx.refresh_windows();
            }
        })
        .detach();
}

/// Opens each path as a tab of the workspace, making the workspace if there
/// is none. A folder becomes the explorer's root.
pub fn open_paths(paths: &[PathBuf], cx: &mut App) {
    let handle = match workspace(cx) {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    let paths = paths.to_vec();
    handle
        .update(cx, |workspace, window, cx| {
            for path in &paths {
                if path.is_dir() {
                    workspace.set_root(path.clone());
                    cx.notify();
                } else if let Err(error) = workspace.open(path, window, cx) {
                    eprintln!("{error}");
                }
            }
            window.activate_window();
        })
        .ok();
}

/// A new document, `bytes`, as a tab titled `name` until it is saved.
pub fn open_untitled(name: String, bytes: Vec<u8>, cx: &mut App) {
    let handle = match workspace(cx) {
        Ok(handle) => handle,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    handle
        .update(cx, |workspace, window, cx| {
            if let Err(error) = workspace.open_untitled(&name, &bytes, window, cx) {
                eprintln!("{error}");
            }
            window.activate_window();
        })
        .ok();
}

/// The next new document's name: 文書 1, 文書 2, … (Document1, … in
/// English), as Word numbers them in a session.
pub fn untitled_name(extension: &str) -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    format!(
        "{}.{extension}",
        crate::i18n::format("doc.untitled", &[("n", &n)])
    )
}

/// ⌘N: a blank document, as this language's Word makes one.
pub fn open_blank(cx: &mut App) {
    use jade_office_model::word::new::{blank_document, Blank};
    let blank = match crate::i18n::language() {
        crate::i18n::Language::English => Blank::English,
        _ => Blank::Japanese,
    };
    // A new document starts from the Normal template, once one is saved.
    let document = crate::word_templates::normal_template()
        .and_then(|normal| jade_office_model::word::new::document_from_template(&normal).ok())
        .unwrap_or_else(|| blank_document(blank));
    open_untitled(untitled_name("docx"), document, cx);
}

/// 新規テンプレートから: asks for a Word template, and opens a new document
/// made from it.
pub fn prompt_for_template(cx: &mut App) {
    use jade_office_model::word::new::{document_from_template, is_macro_enabled};
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(SharedString::from(crate::i18n::text(
            "menu.file.new_from_template",
        ))),
    });
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = paths.await else {
            return;
        };
        let Some(path) = paths.first() else {
            return;
        };
        let made = std::fs::read(path)
            .map_err(|error| format!("{}: {error}", path.display()))
            .and_then(|template| {
                let extension = if is_macro_enabled(&template) {
                    "docm"
                } else {
                    "docx"
                };
                Ok((extension, document_from_template(&template)?))
            });
        match made {
            Ok((extension, document)) => {
                cx.update(|cx| open_untitled(untitled_name(extension), document, cx))
                    .ok();
            }
            Err(error) => eprintln!("{error}"),
        }
    })
    .detach();
}

/// Asks for files of any supported kind and opens each as a tab.
pub fn prompt_and_open(cx: &mut App) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: Some(SharedString::from("開く")),
    });
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = paths.await else {
            // Cancelled, or the platform had nothing to say.
            return;
        };
        cx.update(|cx| open_paths(&paths, cx)).ok();
    })
    .detach();
}

/// The files opened most recently, newest first, kept in a plain text file
/// with one path per line; a pinned one (ピン留め) is marked, stays at the
/// top and is never pushed off the list.
pub mod recent {
    use std::path::{Path, PathBuf};

    const LIMIT: usize = 12;
    const PINNED: &str = "pinned\t";

    /// A file on the list.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Entry {
        pub path: PathBuf,
        pub pinned: bool,
    }

    fn store() -> Option<PathBuf> {
        // Tests open files too, and must not rewrite the user's list.
        if cfg!(test) {
            return None;
        }
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/Application Support/jade-office/recent.txt"))
    }

    /// The files on the list that are still there, pinned ones first.
    pub fn entries() -> Vec<Entry> {
        let Some(contents) = store().and_then(|store| std::fs::read_to_string(store).ok()) else {
            return Vec::new();
        };
        parse(&contents)
            .into_iter()
            .filter(|entry| entry.path.exists())
            .collect()
    }

    fn write(entries: &[Entry]) {
        let Some(store) = store() else {
            return;
        };
        if let Some(directory) = store.parent() {
            std::fs::create_dir_all(directory).ok();
        }
        std::fs::write(store, format(entries)).ok();
    }

    fn read_all() -> Vec<Entry> {
        store()
            .and_then(|store| std::fs::read_to_string(store).ok())
            .map(|contents| parse(&contents))
            .unwrap_or_default()
    }

    pub fn remember(path: &Path) {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        write(&push(read_all(), path));
    }

    /// Pins `path` to the top of the list, or unpins it.
    pub fn set_pinned(path: &Path, pinned: bool) {
        write(&pin(read_all(), path, pinned));
    }

    fn parse(contents: &str) -> Vec<Entry> {
        let mut entries: Vec<Entry> = contents
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .map(|line| match line.strip_prefix(PINNED) {
                Some(path) => Entry {
                    path: PathBuf::from(path),
                    pinned: true,
                },
                None => Entry {
                    path: PathBuf::from(line.trim()),
                    pinned: false,
                },
            })
            .collect();
        entries.sort_by_key(|entry| !entry.pinned);
        entries
    }

    fn format(entries: &[Entry]) -> String {
        entries
            .iter()
            .map(|entry| {
                let path = entry.path.to_string_lossy();
                if entry.pinned {
                    format!("{PINNED}{path}")
                } else {
                    path.into_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `path` moved to the front of its group, without duplicates, the
    /// unpinned ones cut to the limit.
    fn push(mut entries: Vec<Entry>, path: PathBuf) -> Vec<Entry> {
        let pinned = entries
            .iter()
            .any(|entry| entry.path == path && entry.pinned);
        entries.retain(|entry| entry.path != path);
        entries.insert(0, Entry { path, pinned });
        entries.sort_by_key(|entry| !entry.pinned);
        let mut unpinned = 0;
        entries.retain(|entry| {
            if entry.pinned {
                return true;
            }
            unpinned += 1;
            unpinned <= LIMIT
        });
        entries
    }

    fn pin(mut entries: Vec<Entry>, path: &Path, pinned: bool) -> Vec<Entry> {
        match entries.iter_mut().find(|entry| entry.path == path) {
            Some(entry) => entry.pinned = pinned,
            None => entries.push(Entry {
                path: path.to_path_buf(),
                pinned,
            }),
        }
        entries.sort_by_key(|entry| !entry.pinned);
        entries
    }

    /// The folders the recent files are in, most recent first: where Word
    /// suggests saving and opening (最近使ったフォルダー).
    pub fn folders(entries: &[Entry]) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        for entry in entries {
            if let Some(parent) = entry.path.parent() {
                if !out.iter().any(|folder| folder == parent) && parent.is_dir() {
                    out.push(parent.to_path_buf());
                }
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn paths(entries: &[Entry]) -> Vec<PathBuf> {
            entries.iter().map(|entry| entry.path.clone()).collect()
        }

        #[test]
        fn a_reopened_file_moves_to_the_front_once() {
            let entries = parse("/a.docx\n/b.xlsx\n\n/c.pptx\n");
            let entries = push(entries, PathBuf::from("/b.xlsx"));
            assert_eq!(
                paths(&entries),
                ["/b.xlsx", "/a.docx", "/c.pptx"]
                    .map(PathBuf::from)
                    .to_vec()
            );
        }

        #[test]
        fn the_list_is_capped_but_pinned_files_stay() {
            let mut entries = pin(Vec::new(), Path::new("/kept.docx"), true);
            for index in 0..20 {
                entries = push(entries, PathBuf::from(format!("/{index}.docx")));
            }
            assert_eq!(entries.len(), LIMIT + 1);
            assert_eq!(entries[0].path, PathBuf::from("/kept.docx"));
            assert!(entries[0].pinned);
            assert_eq!(entries[1].path, PathBuf::from("/19.docx"));
        }

        #[test]
        fn pins_survive_the_file_and_can_be_taken_off() {
            let entries = push(parse("/a.docx\n/b.docx"), PathBuf::from("/c.docx"));
            let entries = pin(entries, Path::new("/b.docx"), true);
            let read = parse(&format(&entries));
            assert_eq!(
                read[0],
                Entry {
                    path: PathBuf::from("/b.docx"),
                    pinned: true
                }
            );
            // Reopening a pinned file keeps it pinned.
            let read = push(read, PathBuf::from("/b.docx"));
            assert!(read[0].pinned);
            let read = pin(read, Path::new("/b.docx"), false);
            assert!(read.iter().all(|entry| !entry.pinned));
        }
    }
}

/// Whether `path` is an OLE compound file rather than an Open XML package:
/// what Office writes for a password-protected file (the package encrypted
/// inside), and what the old .doc, .xls and .ppt formats are.
pub fn is_compound_file(path: &Path) -> bool {
    use std::io::Read;
    const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    let mut head = [0u8; 8];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_ok_and(|()| head == SIGNATURE)
}

/// Writes `bytes` over `path` as one step: into a temporary file beside
/// it, flushed to disk, then renamed over it. A save that fails part-way
/// leaves the file as it was rather than half-written.
pub fn write_in_place(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = directory.join(format!(".{name}.jade-saving-{}", std::process::id()));
    let result = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        std::fs::remove_file(&temporary).ok();
    }
    result
}

/// A copy of `source` named `name` in a folder of its own, for a test that
/// saves over what it opened.
#[cfg(test)]
pub fn scratch_copy(source: &Path, name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "jade-save-{}-{}",
        std::process::id(),
        name.replace('.', "-")
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(name);
    std::fs::copy(source, &path).unwrap();
    path
}

/// The files beside `path`, by name.
#[cfg(test)]
pub fn siblings(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::Kind;
    use std::path::Path;

    #[test]
    fn every_office_extension_has_a_view() {
        assert_eq!(Kind::of(Path::new("a.docx")), Some(Kind::Document));
        assert_eq!(Kind::of(Path::new("a.DOCX")), Some(Kind::Document));
        assert_eq!(Kind::of(Path::new("a.xlsx")), Some(Kind::Workbook));
        assert_eq!(Kind::of(Path::new("a.xlsm")), Some(Kind::Workbook));
        assert_eq!(Kind::of(Path::new("a.CSV")), Some(Kind::Workbook));
        assert_eq!(Kind::of(Path::new("a.pptx")), Some(Kind::Deck));
        // A text file opens as a Word document, as in Word.
        assert_eq!(Kind::of(Path::new("a.txt")), Some(Kind::Document));
        assert_eq!(Kind::of(Path::new("a.zip")), None);
        assert_eq!(Kind::of(Path::new("pptx")), None);
    }
}
