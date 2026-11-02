//! What the workspace offers to open: 最近使ったファイル with their pins,
//! 最近使ったフォルダー (where files were opened and saved, to open the
//! explorer on), and the documents 自動回復 kept that were never saved —
//! offered back after a crash, as Word's ドキュメントの回復 does.

use gpui::{anchored, deferred, div, prelude::*, px, svg, AnyElement, Context, SharedString};
use std::path::{Path, PathBuf};

use crate::autorecover::{self, Recovered};
use crate::shell::{self, recent};
use crate::tooltip::Tooltip;
use crate::workspace::Workspace;
use crate::{i18n, theme};

/// The rows of the recent files: a pin on each, and on hover a "…" button
/// whose menu pins or unpins the file and takes it off the list (一覧から削
/// 除), as Word's start page does. `menu` is the row whose menu is open.
pub fn recent_rows(
    entries: &[recent::Entry],
    limit: usize,
    id: &'static str,
    menu: Option<(&'static str, usize)>,
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
            let menu_open = menu == Some((id, index));
            div()
                .id((id, index))
                .group("recent-row")
                .relative()
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
                .when(menu_open, |row| row.bg(theme::hovered()))
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
                .child(more_button(id, index, menu_open, cx))
                .children(menu_open.then(|| recent_menu(&entry.path, pinned, cx)))
                .on_click(cx.listener(move |workspace, _event, window, cx| {
                    workspace.open_path_from_ui(path.clone(), window, cx)
                }))
                .into_any_element()
        })
        .collect()
}

/// The row's "…" button: shown on hover, or while its menu is open.
fn more_button(
    id: &'static str,
    index: usize,
    open: bool,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    div()
        .id(("recent-more", index))
        .p_0p5()
        .rounded_sm()
        .tooltip(Tooltip::text(i18n::text("recent.more")))
        .when(!open, |more| {
            more.invisible()
                .group_hover("recent-row", |more| more.visible())
        })
        .when(open, |more| more.bg(theme::selected()))
        .hover(|more| more.bg(theme::selected()))
        .child(
            svg()
                .path("icons/ellipsis.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .on_click(cx.listener(move |workspace, _event, _window, cx| {
            cx.stop_propagation();
            let menu = (!open).then_some((id, index));
            workspace.set_recent_menu(menu);
            cx.notify();
        }))
}

/// The "…" menu: ピン留め (or ピン留めを外す) and 一覧から削除.
fn recent_menu(path: &Path, pinned: bool, cx: &mut Context<Workspace>) -> AnyElement {
    let rows = recent::menu(pinned).into_iter().map(|action| {
        let path = path.to_path_buf();
        div()
            .id(action.label_key())
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .h(px(24.0))
            .rounded_sm()
            .whitespace_nowrap()
            .cursor_pointer()
            .text_color(theme::text())
            .hover(|row| row.bg(theme::hovered()))
            .child(
                svg()
                    .path(action.icon())
                    .size_3()
                    .text_color(theme::text_dim()),
            )
            .child(i18n::text(action.label_key()))
            .on_click(cx.listener(move |workspace, _event, _window, cx| {
                cx.stop_propagation();
                recent::perform(&path, action);
                workspace.reload_recent();
                cx.notify();
            }))
    });
    div()
        .absolute()
        .top_full()
        .right_0()
        .child(
            deferred(
                anchored().snap_to_window_with_margin(px(8.0)).child(
                    div()
                        .id("recent-menu")
                        .occlude()
                        .on_mouse_down_out(cx.listener(|workspace, _, _, cx| {
                            workspace.set_recent_menu(None);
                            cx.notify();
                        }))
                        .mt_1()
                        .p_1()
                        .min_w(px(160.0))
                        .flex()
                        .flex_col()
                        .rounded_md()
                        .bg(theme::panel())
                        .border_1()
                        .border_color(theme::border())
                        .shadow_lg()
                        .font_family(crate::UI_FONT_FAMILY)
                        .text_xs()
                        .children(rows),
                ),
            )
            .with_priority(4),
        )
        .into_any_element()
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
                recent_rows(&entries, 10, "start-recent", workspace.recent_menu(), cx),
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
