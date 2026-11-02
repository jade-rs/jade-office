use super::super::DocView;
use crate::{i18n, palette as theme};
use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_web::more_colors::{MoreColorsDialog, MoreColorsKey};
type View = DocView;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/more_colors_ui.rs"
));
impl DocView {
    fn more_colors_editor(&self) -> Option<&MoreColorsDialog> {
        self.sheet_ref()?.more_colors.as_ref()
    }
    /// A colour clicked on the spectrum or the strip of shades.
    pub(super) fn more_colors_pick(&mut self, rgb: &str, cx: &mut Context<Self>) {
        if let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.more_colors.as_mut())
        {
            dialog.pick(rgb);
            cx.notify();
        }
    }
    /// More Colors… from the fill (true) or font colour gallery.
    pub(super) fn open_more_colors(&mut self, fill: bool, cx: &mut Context<Self>) {
        if let Some(sheet) = self.sheet_mut() {
            let look = sheet.book.format_at(sheet.selection.active);
            let current = if fill {
                look.fill_color
            } else {
                look.font_color
            };
            sheet.popup = None;
            sheet.more_colors = Some(MoreColorsDialog::new(fill, current.as_deref()));
        }
        cx.notify();
    }
    pub(super) fn more_colors_action(&mut self, action: MoreColorsAction, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        match action {
            MoreColorsAction::Take => {
                // A code that is not a colour keeps the dialog open.
                let Some(color) = sheet.more_colors.as_ref().and_then(|dialog| dialog.color())
                else {
                    return;
                };
                let Some(dialog) = sheet.more_colors.take() else {
                    return;
                };
                let range = sheet.selection.range();
                let result = if dialog.fill {
                    sheet.book.fill_color(range, Some(&color))
                } else {
                    sheet.book.font_color(range, &color)
                };
                self.edited(result, cx);
            }
            MoreColorsAction::Cancel => {
                sheet.more_colors = None;
                cx.notify();
            }
        }
    }
    pub(super) fn more_colors_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(dialog) = self
            .sheet_mut()
            .and_then(|sheet| sheet.more_colors.as_mut())
        else {
            return false;
        };
        match dialog.key(key, command) {
            MoreColorsKey::Take => self.more_colors_action(MoreColorsAction::Take, cx),
            MoreColorsKey::Close => self.more_colors_action(MoreColorsAction::Cancel, cx),
            MoreColorsKey::Handled => cx.notify(),
            MoreColorsKey::Unhandled => return false,
        }
        true
    }
}
