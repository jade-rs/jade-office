//! Word's right-click menus: what Word for Mac offers over text, a table, a
//! picture, a shape and a header or footer, in its order. The shells draw
//! the menu and run each command with the tools they already have.

use super::picture::Wrap;

/// What the pointer was over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuTarget {
    Text,
    Table,
    Picture,
    Shape,
    /// The header or footer band; `true` when the story is open for editing.
    Header(bool),
    Footer(bool),
}

/// One command of a right-click menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCommand {
    Cut,
    Copy,
    Paste,
    PasteSpecial,
    Font,
    Paragraph,
    Bullets,
    Numbering,
    Hyperlink,
    NewComment,
    InsertRowAbove,
    InsertRowBelow,
    InsertColumnLeft,
    InsertColumnRight,
    DeleteRow,
    DeleteColumn,
    DeleteTable,
    MergeCells,
    SplitCells,
    TableProperties,
    Wrap(Wrap),
    EditHeader,
    EditFooter,
    CloseHeaderFooter,
}

/// One row of a menu: a command and its label key, or a separator.
pub type MenuRow = Option<(MenuCommand, &'static str)>;

fn clipboard() -> Vec<MenuRow> {
    vec![
        Some((MenuCommand::Cut, "word.ctx.cut")),
        Some((MenuCommand::Copy, "word.ctx.copy")),
        Some((MenuCommand::Paste, "word.ctx.paste")),
        Some((MenuCommand::PasteSpecial, "word.ctx.paste_special")),
    ]
}

fn text_formatting() -> Vec<MenuRow> {
    vec![
        Some((MenuCommand::Font, "word.ctx.font")),
        Some((MenuCommand::Paragraph, "word.ctx.paragraph")),
    ]
}

fn wraps() -> Vec<MenuRow> {
    [
        (Wrap::Inline, "picture.wrap.inline"),
        (Wrap::Square, "picture.wrap.square"),
        (Wrap::Tight, "picture.wrap.tight"),
        (Wrap::TopAndBottom, "picture.wrap.top_bottom"),
        (Wrap::BehindText, "picture.wrap.behind"),
        (Wrap::InFrontOfText, "picture.wrap.front"),
    ]
    .into_iter()
    .map(|(wrap, label)| Some((MenuCommand::Wrap(wrap), label)))
    .collect()
}

/// The menu for `target`, separators as `None`.
pub fn menu_rows(target: MenuTarget) -> Vec<MenuRow> {
    let mut rows = clipboard();
    match target {
        MenuTarget::Text => {
            rows.push(None);
            rows.extend(text_formatting());
            rows.extend([
                Some((MenuCommand::Bullets, "word.ctx.bullets")),
                Some((MenuCommand::Numbering, "word.ctx.numbering")),
                None,
                Some((MenuCommand::Hyperlink, "word.ctx.hyperlink")),
                Some((MenuCommand::NewComment, "word.ctx.new_comment")),
            ]);
        }
        MenuTarget::Table => {
            rows.extend([
                None,
                Some((MenuCommand::InsertRowAbove, "word.ctx.insert_row_above")),
                Some((MenuCommand::InsertRowBelow, "word.ctx.insert_row_below")),
                Some((MenuCommand::InsertColumnLeft, "word.ctx.insert_column_left")),
                Some((
                    MenuCommand::InsertColumnRight,
                    "word.ctx.insert_column_right",
                )),
                None,
                Some((MenuCommand::DeleteRow, "word.ctx.delete_row")),
                Some((MenuCommand::DeleteColumn, "word.ctx.delete_column")),
                Some((MenuCommand::DeleteTable, "word.ctx.delete_table")),
                None,
                Some((MenuCommand::MergeCells, "word.ctx.merge_cells")),
                Some((MenuCommand::SplitCells, "word.ctx.split_cells")),
                None,
                Some((MenuCommand::TableProperties, "word.ctx.table_properties")),
                None,
            ]);
            rows.extend(text_formatting());
        }
        MenuTarget::Picture | MenuTarget::Shape => {
            rows.push(None);
            rows.extend(wraps());
            if target == MenuTarget::Picture {
                rows.extend([None, Some((MenuCommand::Hyperlink, "word.ctx.hyperlink"))]);
            }
        }
        MenuTarget::Header(open) | MenuTarget::Footer(open) => {
            let header = matches!(target, MenuTarget::Header(_));
            rows.push(None);
            if open {
                rows.extend(text_formatting());
                rows.extend([
                    None,
                    Some((
                        MenuCommand::CloseHeaderFooter,
                        "word.ctx.close_header_footer",
                    )),
                ]);
            } else if header {
                rows.push(Some((MenuCommand::EditHeader, "word.ctx.edit_header")));
            } else {
                rows.push(Some((MenuCommand::EditFooter, "word.ctx.edit_footer")));
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commands(target: MenuTarget) -> Vec<MenuCommand> {
        menu_rows(target)
            .into_iter()
            .flatten()
            .map(|(command, _)| command)
            .collect()
    }

    #[test]
    fn every_menu_starts_with_the_clipboard_and_offers_its_targets_tools() {
        for target in [
            MenuTarget::Text,
            MenuTarget::Table,
            MenuTarget::Picture,
            MenuTarget::Shape,
            MenuTarget::Header(false),
            MenuTarget::Footer(true),
        ] {
            assert_eq!(
                commands(target)[..3],
                [MenuCommand::Cut, MenuCommand::Copy, MenuCommand::Paste]
            );
        }
        assert!(commands(MenuTarget::Text).contains(&MenuCommand::NewComment));
        assert!(commands(MenuTarget::Table).contains(&MenuCommand::InsertRowBelow));
        assert!(commands(MenuTarget::Table).contains(&MenuCommand::TableProperties));
        assert!(commands(MenuTarget::Picture).contains(&MenuCommand::Wrap(Wrap::BehindText)));
        assert!(commands(MenuTarget::Shape).contains(&MenuCommand::Wrap(Wrap::InFrontOfText)));
        assert!(commands(MenuTarget::Header(false)).contains(&MenuCommand::EditHeader));
        assert!(commands(MenuTarget::Footer(false)).contains(&MenuCommand::EditFooter));
        assert!(commands(MenuTarget::Header(true)).contains(&MenuCommand::CloseHeaderFooter));
        // No menu opens or closes on a separator.
        for target in [MenuTarget::Text, MenuTarget::Table, MenuTarget::Picture] {
            let rows = menu_rows(target);
            assert!(rows.first().unwrap().is_some() && rows.last().unwrap().is_some());
        }
    }
}
