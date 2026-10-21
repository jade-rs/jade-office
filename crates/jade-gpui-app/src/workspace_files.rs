//! What the workspace offers to open: 最近使ったファイル with their pins,
//! 最近使ったフォルダー (where files were opened and saved, to open the
//! explorer on), and the documents 自動回復 kept that were never saved —
//! offered back after a crash, as Word's ドキュメントの回復 does.

use gpui::{div, prelude::*, px, svg, AnyElement, Context, SharedString};
use std::path::{Path, PathBuf};

use crate::autorecover::{self, Recovered};
use crate::shell::{self, recent};
use crate::tooltip::Tooltip;
use crate::workspace::Workspace;
use crate::{i18n, theme};

/// The rows of the recent files, a pin on each.
pub fn recent_rows(
    entries: &[recent::Entry],
    limit: usize,
    id: &'static str,
    cx: &mut Context<Workspace>,
) -> Vec<AnyElement> {
    entries
        .iter()
        .take(limit)
        .enumerate()
        .map(|(index, entry)| {
            let kind = shell::Kind::of(&entry.path);
            let path = entry.path.clone();
            let pin_path = entry.path.clone();
            let pinned = entry.pinned;
            div()
                .id((id, index))
                .group("recent-row")
                .flex()
                .items_center()
                .gap_1p5()
                .mx_1()
                .px_2()
                .py(px(4.0))
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .text_color(theme::text_dim())
                .hover(|row| row.bg(theme::hovered()).text_color(theme::text()))
                .children(kind.map(|kind| {
                    svg()
                        .path(crate::workspace::kind_icon(kind))
                        .size_3p5()
                        .text_color(crate::workspace::kind_color(kind))
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(shell::file_name(&path)),
                )
                .child(
                    div()
                        .id((if pinned { "recent-unpin" } else { "recent-pin" }, index))
                        .p_0p5()
                        .rounded_sm()
                        .tooltip(Tooltip::text(i18n::text(if pinned {
                            "recent.unpin"
                        } else {
                            "recent.pin"
                        })))
                        .when(!pinned, |pin| {
                            pin.invisible()
                                .group_hover("recent-row", |pin| pin.visible())
                        })
                        .hover(|pin| pin.bg(theme::selected()))
                        .child(svg().path("icons/pin.svg").size_3().text_color(if pinned {
                            theme::accent()
                        } else {
                            theme::text_faint()
                        }))
                        .on_click(cx.listener(move |workspace, _event, _window, cx| {
                            cx.stop_propagation();
                            recent::set_pinned(&pin_path, !pinned);
                            workspace.reload_recent();
                            cx.notify();
                        })),
                )
                .on_click(cx.listener(move |workspace, _event, window, cx| {
                    workspace.open_path_from_ui(path.clone(), window, cx)
                }))
                .into_any_element()
        })
        .collect()
}

/// The recovered documents, each with 開く and 削除.
pub fn recovered_rows(recovered: &[Recovered], cx: &mut Context<Workspace>) -> Vec<AnyElement> {
    recovered
        .iter()
        .enumerate()
        .map(|(index, copy)| {
            let open = copy.clone();
            let delete = copy.copy.clone();
            let name = Path::new(&copy.original)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| copy.original.clone());
            div()
                .id(("recovered-row", index))
                .flex()
                .items_center()
                .gap_1p5()
                .mx_1()
                .px_2()
                .py(px(4.0))
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .text_color(theme::text_dim())
                .hover(|row| row.bg(theme::hovered()).text_color(theme::text()))
                .tooltip(Tooltip::text(SharedString::from(copy.original.clone())))
                .child(
                    svg()
                        .path("icons/life-buoy.svg")
                        .size_3p5()
                        .text_color(theme::accent()),
                )
                .child(div().flex_1().min_w_0().truncate().child(name))
                .child(
                    div()
                        .id(("recovered-delete", index))
                        .p_0p5()
                        .rounded_sm()
                        .tooltip(Tooltip::text(i18n::text("recovered.delete")))
                        .hover(|button| button.bg(theme::selected()))
                        .child(
                            svg()
                                .path("icons/trash-2.svg")
                                .size_3()
                                .text_color(theme::text_faint()),
                        )
                        .on_click(cx.listener(move |workspace, _event, _window, cx| {
                            cx.stop_propagation();
                            autorecover::discard(&delete);
                            workspace.reload_recovered();
                            cx.notify();
                        })),
                )
                .on_click(cx.listener(move |workspace, _event, window, cx| {
                    workspace.open_recovered(&open, window, cx)
                }))
                .into_any_element()
        })
        .collect()
}

/// The folders recent files are in: a click opens the explorer there.
pub fn folder_rows(folders: &[PathBuf], cx: &mut Context<Workspace>) -> Vec<AnyElement> {
    folders
        .iter()
        .take(5)
        .enumerate()
        .map(|(index, folder)| {
            let open = folder.clone();
            div()
                .id(("recent-folder", index))
                .flex()
                .items_center()
                .gap_1p5()
                .mx_1()
                .px_2()
                .py(px(4.0))
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .text_color(theme::text_dim())
                .hover(|row| row.bg(theme::hovered()).text_color(theme::text()))
                .tooltip(Tooltip::text(SharedString::from(
                    folder.to_string_lossy().into_owned(),
                )))
                .child(
                    svg()
                        .path("icons/folder.svg")
                        .size_3p5()
                        .text_color(theme::accent()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(shell::file_name(folder)),
                )
                .on_click(cx.listener(move |workspace, _event, _window, cx| {
                    workspace.set_root(open.clone());
                    cx.notify();
                }))
                .into_any_element()
        })
        .collect()
}

/// A titled list for the start page.
pub fn section(title: &'static str, icon: &'static str, rows: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .w(px(300.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .px_3()
                .pb_1()
                .text_xs()
                .text_color(theme::text_faint())
                .child(svg().path(icon).size_3().text_color(theme::text_faint()))
                .child(i18n::text(title)),
        )
        .children(rows)
        .into_any_element()
}

/// The start page's lists, side by side: what is recovered, recent files,
/// recent folders.
pub fn start_lists(workspace: &Workspace, cx: &mut Context<Workspace>) -> AnyElement {
    let recovered = workspace.recovered().to_vec();
    let entries = workspace.recent_entries().to_vec();
    let folders = recent::folders(&entries);
    div()
        .flex()
        .gap_6()
        .mt_4()
        .when(!recovered.is_empty(), |lists| {
            lists.child(section(
                "recovered.title",
                "icons/life-buoy.svg",
                recovered_rows(&recovered, cx),
            ))
        })
        .when(!entries.is_empty(), |lists| {
            lists.child(section(
                "workspace.recent",
                "icons/clock.svg",
                recent_rows(&entries, 10, "start-recent", cx),
            ))
        })
        .when(!folders.is_empty(), |lists| {
            lists.child(section(
                "recent.folders",
                "icons/folder.svg",
                folder_rows(&folders, cx),
            ))
        })
        .into_any_element()
}

/// The name a recovered copy opens under: the document's own, marked.
pub fn recovered_name(copy: &Recovered) -> String {
    let stem = Path::new(&copy.original)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into());
    format!(
        "{}.docx",
        i18n::format("recovered.name", &[("name", &stem)])
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document 自動回復 kept is listed when the workspace opens, and
    /// opens as a new tab under its name marked 回復済み; its copy goes.
    #[gpui::test]
    fn a_recovered_document_is_offered_and_opens(cx: &mut gpui::TestAppContext) {
        let folder = std::env::temp_dir().join(format!("jade-recovered-{}", std::process::id()));
        std::fs::remove_dir_all(&folder).ok();
        let docx = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ))
        .unwrap();
        autorecover::write_copy(&folder, "test", "/Users/me/契約書.docx", &docx).unwrap();
        autorecover::set_settings(autorecover::Settings {
            location: Some(folder.clone()),
            ..Default::default()
        });
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        let copy = workspace.read_with(cx, |workspace, _| {
            assert_eq!(workspace.recovered().len(), 1);
            workspace.recovered()[0].clone()
        });
        assert_eq!(
            recovered_name(&copy),
            format!(
                "{}.docx",
                i18n::format("recovered.name", &[("name", &"契約書")])
            )
        );
        cx.update(|window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.open_recovered(&copy, window, cx)
            })
        });
        cx.run_until_parked();
        workspace.read_with(cx, |workspace, _| assert!(workspace.recovered().is_empty()));
        assert!(autorecover::recovered_in(&folder).is_empty());
        autorecover::set_settings(autorecover::Settings::default());
        std::fs::remove_dir_all(folder).ok();
    }

    #[test]
    fn word_s_other_formats_open_as_documents() {
        for name in [
            "a.doc", "a.rtf", "a.txt", "a.odt", "a.html", "a.htm", "a.wbk",
        ] {
            assert_eq!(
                shell::Kind::of(Path::new(name)),
                Some(shell::Kind::Document),
                "{name}"
            );
        }
    }
}
