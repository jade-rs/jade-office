// Shared native/browser: the grid's right-click menu, its less used
// commands gathered under headers that open in place, so the menu fits.

/// A header of the grid's menu and the commands it holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuGroup {
    Clear,
    InsertDelete,
    Data,
    PivotTable,
    LinksValidation,
    Protection,
    PageLayout,
    CellStyles,
}

impl MenuGroup {
    /// The i18n key of its header.
    fn label(self) -> &'static str {
        match self {
            MenuGroup::Clear => "sheet.menu.group_clear",
            MenuGroup::InsertDelete => "sheet.menu.group_insert_delete",
            MenuGroup::Data => "sheet.menu.group_data",
            MenuGroup::PivotTable => "sheet.menu.group_pivot_table",
            MenuGroup::LinksValidation => "sheet.menu.group_links_validation",
            MenuGroup::Protection => "sheet.menu.group_protection",
            MenuGroup::PageLayout => "sheet.menu.group_page_layout",
            MenuGroup::CellStyles => "sheet.menu.group_cell_styles",
        }
    }
}

/// The header a grid command sits under, if any.
fn menu_group(command: MenuCommand) -> Option<MenuGroup> {
    use MenuCommand as C;
    Some(match command {
        C::Clear | C::ClearFormats | C::ClearAll => MenuGroup::Clear,
        C::InsertRows
        | C::DeleteRows
        | C::InsertColumns
        | C::DeleteColumns
        | C::ShiftCells(..) => MenuGroup::InsertDelete,
        C::SortAscending
        | C::SortDescending
        | C::SortDialog
        | C::ShowFormulas
        | C::ShowZeros
        | C::NameManager
        | C::TracePrecedents
        | C::TraceDependents
        | C::GoToOtherSheetPrecedent
        | C::RemoveArrows
        | C::Subtotal
        | C::RemoveDuplicates
        | C::TextToColumns
        | C::FromText
        | C::CalculateNow
        | C::ManualCalculation(_)
        | C::Outline(_)
        | C::AutoOutline
        | C::ClearOutline => MenuGroup::Data,
        C::CreatePivot
        | C::RefreshAllPivots
        | C::RefreshPivot
        | C::EditPivot
        | C::DeletePivot
        | C::Table => MenuGroup::PivotTable,
        C::Link | C::RemoveLinks | C::Validation | C::ClearValidation | C::CircleInvalid(_) => {
            MenuGroup::LinksValidation
        }
        C::CellLocked | C::CellHidden | C::EditRanges(_) => MenuGroup::Protection,
        C::PageSetup | C::InsertPageBreak | C::RemovePageBreak | C::ResetPageBreaks => {
            MenuGroup::PageLayout
        }
        C::CellStyle(_) => MenuGroup::CellStyles,
        _ => return None,
    })
}

/// The menu's rows: each command not in a group where it was, each group's
/// header where its first command was, and — under the open one — its
/// commands, marked to be indented.
fn grouped_menu(
    items: Vec<(MenuCommand, &'static str)>,
    open: Option<MenuGroup>,
) -> Vec<(MenuCommand, &'static str, bool)> {
    let mut rows = Vec::with_capacity(items.len());
    let mut seen: Vec<MenuGroup> = Vec::new();
    let members = |group: MenuGroup| {
        items
            .iter()
            .filter(move |(command, _)| menu_group(*command) == Some(group))
            .copied()
    };
    for &(command, label) in &items {
        match menu_group(command) {
            None => rows.push((command, label, false)),
            Some(group) if !seen.contains(&group) => {
                seen.push(group);
                // A group of one is just that command.
                if members(group).count() == 1 {
                    rows.push((command, label, false));
                    continue;
                }
                rows.push((MenuCommand::OpenGroup(group), group.label(), false));
                if open == Some(group) {
                    rows.extend(members(group).map(|(command, label)| (command, label, true)));
                }
            }
            Some(_) => {}
        }
    }
    rows
}
