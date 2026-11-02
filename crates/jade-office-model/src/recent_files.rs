//! 最近使ったファイル: the list both shells keep, and the "…" menu each of its
//! rows has, as Word's start page has — ピン留め / ピン留めを外す and
//! 一覧から削除. Pinned files sit at the top, kept past the limit; the
//! others are the most recent first, cut to the limit.
//!
//! The native shell stores paths, the browser shell file names: the list is
//! generic over the key.

/// A file on the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentEntry<K> {
    pub key: K,
    pub pinned: bool,
}

/// What the row's "…" menu offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecentAction {
    /// 一覧にピン留めする.
    Pin,
    /// ピン留めを外す.
    Unpin,
    /// 一覧から削除.
    Remove,
}

impl RecentAction {
    /// The menu entry's string key.
    pub fn label_key(self) -> &'static str {
        match self {
            RecentAction::Pin => "recent.pin",
            RecentAction::Unpin => "recent.unpin",
            RecentAction::Remove => "recent.remove",
        }
    }

    /// The menu entry's icon.
    pub fn icon(self) -> &'static str {
        match self {
            RecentAction::Pin | RecentAction::Unpin => "icons/pin.svg",
            RecentAction::Remove => "icons/trash-2.svg",
        }
    }
}

/// The "…" menu of a row: the pin to put on or take off, then 一覧から削除.
pub fn menu(pinned: bool) -> [RecentAction; 2] {
    [
        if pinned {
            RecentAction::Unpin
        } else {
            RecentAction::Pin
        },
        RecentAction::Remove,
    ]
}

/// Pinned entries first, each group keeping its order.
fn pinned_first<K>(entries: &mut [RecentEntry<K>]) {
    entries.sort_by_key(|entry| !entry.pinned);
}

/// `key` opened: moved to the front of its group, without duplicates, the
/// unpinned ones cut to `limit`.
pub fn push<K: PartialEq>(
    mut entries: Vec<RecentEntry<K>>,
    key: K,
    limit: usize,
) -> Vec<RecentEntry<K>> {
    let pinned = entries.iter().any(|entry| entry.key == key && entry.pinned);
    entries.retain(|entry| entry.key != key);
    entries.insert(0, RecentEntry { key, pinned });
    pinned_first(&mut entries);
    let mut unpinned = 0;
    entries.retain(|entry| {
        if entry.pinned {
            return true;
        }
        unpinned += 1;
        unpinned <= limit
    });
    entries
}

/// `key` pinned to the top, or unpinned.
pub fn pin<K: PartialEq + Clone>(
    mut entries: Vec<RecentEntry<K>>,
    key: &K,
    pinned: bool,
) -> Vec<RecentEntry<K>> {
    match entries.iter_mut().find(|entry| entry.key == *key) {
        Some(entry) => entry.pinned = pinned,
        None => entries.push(RecentEntry {
            key: key.clone(),
            pinned,
        }),
    }
    pinned_first(&mut entries);
    entries
}

/// 一覧から削除: `key` off the list, pinned or not.
pub fn remove<K: PartialEq>(mut entries: Vec<RecentEntry<K>>, key: &K) -> Vec<RecentEntry<K>> {
    entries.retain(|entry| entry.key != *key);
    entries
}

/// What a menu entry does to the list.
pub fn apply<K: PartialEq + Clone>(
    entries: Vec<RecentEntry<K>>,
    key: &K,
    action: RecentAction,
) -> Vec<RecentEntry<K>> {
    match action {
        RecentAction::Pin => pin(entries, key, true),
        RecentAction::Unpin => pin(entries, key, false),
        RecentAction::Remove => remove(entries, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(entries: &[RecentEntry<&'static str>]) -> Vec<&'static str> {
        entries.iter().map(|entry| entry.key).collect()
    }

    fn list(names: &[&'static str]) -> Vec<RecentEntry<&'static str>> {
        names
            .iter()
            .rev()
            .fold(Vec::new(), |entries, name| push(entries, *name, 8))
    }

    #[test]
    fn the_menu_offers_the_pin_it_does_not_have_then_removal() {
        assert_eq!(menu(false), [RecentAction::Pin, RecentAction::Remove]);
        assert_eq!(menu(true), [RecentAction::Unpin, RecentAction::Remove]);
        assert_eq!(RecentAction::Remove.label_key(), "recent.remove");
    }

    #[test]
    fn removing_takes_the_file_off_and_keeps_the_rest_in_order() {
        let entries = list(&["a.docx", "b.xlsx", "c.pptx"]);
        let entries = apply(entries, &"b.xlsx", RecentAction::Remove);
        assert_eq!(keys(&entries), ["a.docx", "c.pptx"]);
    }

    #[test]
    fn a_pinned_file_can_be_removed_too() {
        let entries = list(&["a.docx", "b.xlsx", "c.pptx"]);
        let entries = apply(entries, &"c.pptx", RecentAction::Pin);
        assert_eq!(keys(&entries), ["c.pptx", "a.docx", "b.xlsx"]);
        let entries = apply(entries, &"c.pptx", RecentAction::Remove);
        assert_eq!(keys(&entries), ["a.docx", "b.xlsx"]);
        assert!(entries.iter().all(|entry| !entry.pinned));
    }

    #[test]
    fn removing_what_is_not_there_changes_nothing() {
        let entries = list(&["a.docx", "b.xlsx"]);
        assert_eq!(remove(entries.clone(), &"z.docx"), entries);
    }

    #[test]
    fn a_removed_file_comes_back_at_the_front_when_opened_again() {
        let entries = list(&["a.docx", "b.xlsx"]);
        let entries = remove(entries, &"b.xlsx");
        let entries = push(entries, "b.xlsx", 8);
        assert_eq!(keys(&entries), ["b.xlsx", "a.docx"]);
        assert!(!entries[0].pinned);
    }

    #[test]
    fn unpinning_puts_the_file_back_among_the_others() {
        let entries = list(&["a.docx", "b.xlsx"]);
        let entries = apply(entries, &"b.xlsx", RecentAction::Pin);
        let entries = apply(entries, &"b.xlsx", RecentAction::Unpin);
        assert!(entries.iter().all(|entry| !entry.pinned));
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn the_limit_cuts_unpinned_files_only() {
        let mut entries = pin(Vec::new(), &"kept", true);
        for name in ["1", "2", "3", "4"] {
            entries = push(entries, name, 2);
        }
        assert_eq!(keys(&entries), ["kept", "4", "3"]);
    }
}
