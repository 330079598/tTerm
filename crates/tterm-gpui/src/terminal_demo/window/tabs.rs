//! A workspace always owns its active tab; switching moves resources, not sessions.

use std::collections::BTreeMap;

pub(super) struct Tabs<T> {
    active: (usize, T),
    hidden: BTreeMap<usize, T>,
    last_id: usize,
}

impl<T> Tabs<T> {
    pub(super) fn new(first: T) -> Self {
        Self {
            active: (1, first),
            hidden: BTreeMap::new(),
            last_id: 1,
        }
    }

    pub(super) fn active(&self) -> &T {
        &self.active.1
    }
    pub(super) fn active_id(&self) -> usize {
        self.active.0
    }
    pub(super) fn next_id(&self) -> Option<usize> {
        self.last_id.checked_add(1)
    }

    pub(super) fn push(&mut self, build: impl FnOnce(usize) -> T) {
        if let Some(id) = self.next_id() {
            self.last_id = id;
            let old = std::mem::replace(&mut self.active, (id, build(id)));
            self.hidden.insert(old.0, old.1);
        }
    }

    pub(super) fn ordered(&self) -> Vec<(usize, &T)> {
        let mut tabs: Vec<_> = self.hidden.iter().map(|(&id, tab)| (id, tab)).collect();
        let index = tabs.partition_point(|(id, _)| *id < self.active.0);
        tabs.insert(index, (self.active.0, &self.active.1));
        tabs
    }

    pub(super) fn neighbour(&self, forward: bool) -> Option<usize> {
        use std::ops::Bound::{Excluded, Unbounded};
        let id = self.active.0;
        let next = if forward {
            self.hidden
                .range((Excluded(id), Unbounded))
                .next()
                .or_else(|| self.hidden.first_key_value())
        } else {
            self.hidden
                .range(..id)
                .next_back()
                .or_else(|| self.hidden.last_key_value())
        };
        next.map(|(&id, _)| id)
    }

    pub(super) fn select(&mut self, id: usize) -> bool {
        let Some(next) = self.hidden.remove(&id) else {
            return false;
        };
        let old = std::mem::replace(&mut self.active, (id, next));
        self.hidden.insert(old.0, old.1);
        true
    }

    pub(super) fn close(&mut self, id: usize) -> Option<T> {
        if id != self.active_id() {
            return self.hidden.remove(&id);
        }
        let old = self.active.0;
        let next = self
            .hidden
            .range((std::ops::Bound::Excluded(old), std::ops::Bound::Unbounded))
            .next()
            .or_else(|| self.hidden.last_key_value())
            .map(|(&id, _)| id)?;
        self.select(next);
        self.hidden.remove(&old)
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
        assert_eq!(
            tabs.ordered().iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            [2, 3]
        );
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
        assert_eq!(tabs.ordered(), vec![(2, &"second"), (3, &"third")]);
    }
}
