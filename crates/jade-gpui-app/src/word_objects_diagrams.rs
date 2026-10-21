//! 挿入 → SmartArt and グラフ, and their contextual tools.

use gpui::{AnyElement, Context};
use jade_office_model::word::objects::ObjectSort;

use crate::DocumentView;

impl DocumentView {
    /// 挿入's SmartArt and グラフ buttons.
    pub(crate) fn insert_diagram_buttons(&mut self, _cx: &mut Context<Self>) -> Vec<AnyElement> {
        Vec::new()
    }

    /// SmartArt's and a chart's own groups on the contextual tab.
    pub(crate) fn diagram_groups(&mut self, _sort: ObjectSort, _cx: &mut Context<Self>) -> Vec<AnyElement> {
        Vec::new()
    }
}
