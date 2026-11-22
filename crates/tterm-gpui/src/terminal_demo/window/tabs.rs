//! A workspace always owns its active tab; switching moves resources, not sessions.

pub(super) struct Tabs<T> {
    entries: Vec<(usize, T)>,
    active_index: usize,
    last_id: usize,
}

impl<T> Tabs<T> {
    pub(super) fn new(first: T) -> Self {
        Self {
            entries: vec![(1, first)],
            active_index: 0,
            last_id: 1,
        }
    }

    /// Restores tabs in their saved order. The caller keeps each tab's
    /// persisted id, so the internal keys and those ids stay independent.
    pub(super) fn restore(entries: Vec<T>, active: usize) -> Option<Self> {
        if entries.is_empty() {
            return None;
        }
        let count = entries.len();
        Some(Self {
            entries: entries
                .into_iter()
                .enumerate()
                .map(|(index, tab)| (index + 1, tab))
                .collect(),
            active_index: active.saturating_sub(1).min(count - 1),
            last_id: count,
        })
    }

    pub(super) fn active(&self) -> &T {
        &self.entries[self.active_index].1
    }

    pub(super) fn get(&self, id: usize) -> Option<&T> {
        self.entries
            .iter()
            .find(|entry| entry.0 == id)
            .map(|entry| &entry.1)
    }
    pub(super) fn get_mut(&mut self, id: usize) -> Option<&mut T> {
        self.entries
            .iter_mut()
            .find(|entry| entry.0 == id)
            .map(|entry| &mut entry.1)
    }
    pub(super) fn active_id(&self) -> usize {
        self.entries[self.active_index].0
    }
    pub(super) fn active_index(&self) -> usize {
        self.active_index
    }
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn next_id(&self) -> Option<usize> {
        self.last_id.checked_add(1)
    }

    pub(super) fn push(&mut self, build: impl FnOnce(usize) -> T) {
        if let Some(id) = self.next_id() {
            self.last_id = id;
            self.entries.push((id, build(id)));
            self.active_index = self.entries.len() - 1;
        }
    }

    pub(super) fn ordered(&self) -> impl ExactSizeIterator<Item = (usize, &T)> {
        self.entries.iter().map(|(id, tab)| (*id, tab))
    }

    fn position(&self, id: usize) -> Option<usize> {
        self.entries.iter().position(|entry| entry.0 == id)
    }

    pub(super) fn move_to(&mut self, source: usize, target: usize) -> bool {
        let (Some(from), Some(to)) = (self.position(source), self.position(target)) else {
            return false;
        };
        if from == to {
            return false;
        }
        let selected = self.active_id();
        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        self.select(selected);
        true
    }

    pub(super) fn neighbour(&self, forward: bool) -> Option<usize> {
        let count = self.entries.len();
        let index = if forward {
            (self.active_index + 1) % count
        } else {
            (self.active_index + count - 1) % count
        };
        (count > 1).then(|| self.entries[index].0)
    }

    pub(super) fn select(&mut self, id: usize) -> bool {
        let Some(index) = self.position(id) else {
            return false;
        };
        std::mem::replace(&mut self.active_index, index) != index
    }

    pub(super) fn close(&mut self, id: usize) -> Option<T> {
        if self.entries.len() == 1 {
            return None;
        }
        let index = self.position(id)?;
        let (_, entry) = self.entries.remove(index);
        if index < self.active_index || self.active_index == self.entries.len() {
            self.active_index -= 1;
        }
        Some(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::Tabs;

    #[test]
    fn switching_and_closing_keep_the_other_resource_and_never_reuse_ids() {
        let mut tabs = Tabs::new(Box::new(10));
        let first = std::ptr::from_ref(&**tabs.active());
        tabs.push(|_| Box::new(20));
        assert_eq!(tabs.neighbour(true), Some(1));
        assert!(tabs.select(1));
        assert_eq!(std::ptr::from_ref(&**tabs.active()), first);
        assert_eq!(tabs.close(1).as_deref(), Some(&10));
        assert_eq!(tabs.active_id(), 2);
        assert_eq!(tabs.close(2), None);
        tabs.push(Box::new);
        assert_eq!(tabs.ordered().map(|(id, _)| id).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(tabs.neighbour(false), Some(2));
        assert_eq!(tabs.close(3).as_deref(), Some(&3));
    }

    #[test]
    fn closing_a_background_tab_or_stale_id_preserves_the_selected_session() {
        let mut tabs = Tabs::new("first");
        tabs.push(|_| "second");
        tabs.push(|_| "third");
        tabs.select(2);
        assert_eq!(tabs.close(1), Some("first"));
        assert_eq!(tabs.close(1), None);
        assert_eq!(tabs.active_id(), 2);
        assert_eq!(
            tabs.ordered().collect::<Vec<_>>(),
            [(2, &"second"), (3, &"third")]
        );
    }

    #[test]
    fn reordering_keeps_the_session_and_uses_visual_order_for_navigation_and_close() {
        let mut tabs = Tabs::new(Box::new(1));
        for _ in 0..3 {
            tabs.push(Box::new);
        }
        tabs.select(2);
        let session = std::ptr::from_ref(&**tabs.active());
        assert!(tabs.move_to(4, 1));
        assert_eq!(tabs.active_index(), 2);
        assert!(tabs.move_to(2, 3));
        assert_eq!(tabs.active_index(), 3);
        assert_eq!(std::ptr::from_ref(&**tabs.active()), session);
        assert!(!tabs.move_to(1, 1));
        assert!(!tabs.move_to(5, 2));
        assert!(!tabs.move_to(2, 5));
        assert_eq!(
            tabs.ordered().map(|(id, _)| id).collect::<Vec<_>>(),
            [4, 1, 3, 2]
        );
        assert_eq!(tabs.neighbour(true), Some(4));
        assert_eq!(tabs.neighbour(false), Some(3));
        assert_eq!(tabs.close(2).as_deref(), Some(&2));
        assert_eq!(tabs.active_id(), 3);
        tabs.select(4);
        tabs.close(4);
        assert_eq!(tabs.active_id(), 1);
    }
}
