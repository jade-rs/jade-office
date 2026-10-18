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
            "docx" | "docm" | "dotx" => Some(Kind::Document),
            "xlsx" | "xlsm" | "xltx" => Some(Kind::Workbook),
            "pptx" | "pptm" | "potx" | "ppsx" => Some(Kind::Deck),
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
/// with one path per line.
pub mod recent {
    use std::path::{Path, PathBuf};

    const LIMIT: usize = 12;

    fn store() -> Option<PathBuf> {
        // Tests open files too, and must not rewrite the user's list.
        if cfg!(test) {
            return None;
        }
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/Application Support/jade-office/recent.txt"))
    }

    pub fn load() -> Vec<PathBuf> {
        let Some(contents) = store().and_then(|store| std::fs::read_to_string(store).ok()) else {
            return Vec::new();
        };
        parse(&contents)
            .into_iter()
            .filter(|path| path.exists())
            .collect()
    }

    pub fn remember(path: &Path) {
        let Some(store) = store() else {
            return;
        };
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let existing = std::fs::read_to_string(&store).unwrap_or_default();
        let contents = push(parse(&existing), path);
        if let Some(directory) = store.parent() {
            std::fs::create_dir_all(directory).ok();
        }
        let text: Vec<String> = contents
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        std::fs::write(store, text.join("\n")).ok();
    }

    fn parse(contents: &str) -> Vec<PathBuf> {
        contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    /// `path` moved to the front, without duplicates, cut to the limit.
    fn push(mut paths: Vec<PathBuf>, path: PathBuf) -> Vec<PathBuf> {
        paths.retain(|existing| *existing != path);
        paths.insert(0, path);
        paths.truncate(LIMIT);
        paths
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_reopened_file_moves_to_the_front_once() {
            let paths = parse("/a.docx\n/b.xlsx\n\n/c.pptx\n");
            let paths = push(paths, PathBuf::from("/b.xlsx"));
            assert_eq!(
                paths,
                ["/b.xlsx", "/a.docx", "/c.pptx"]
                    .map(PathBuf::from)
                    .to_vec()
            );
        }

        #[test]
        fn the_list_is_capped() {
            let mut paths = Vec::new();
            for index in 0..20 {
                paths = push(paths, PathBuf::from(format!("/{index}.docx")));
            }
            assert_eq!(paths.len(), LIMIT);
            assert_eq!(paths[0], PathBuf::from("/19.docx"));
        }
    }
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
        assert_eq!(Kind::of(Path::new("a.pptx")), Some(Kind::Deck));
        assert_eq!(Kind::of(Path::new("a.txt")), None);
        assert_eq!(Kind::of(Path::new("pptx")), None);
    }
}
