//! Fixed SSH replies that wait for their position in a synchronized update.
//!
//! The model parser holds every byte between BSU and ESU, including the
//! replies its cursor and status queries would produce. A fixed reply comes
//! from the SSH adapter instead of the parser, so writing it as soon as the
//! adapter sees it would put it ahead of those buffered model replies.
//!
//! The session inserts a model-only ordering marker at the fixed reply's
//! position. The model reports that marker as an `Intent::OrderedReply` in
//! stream order, carrying the number reserved here. The marker cannot be
//! confused with a program's own query because the model records whether each
//! occurrence was synthetic before its handler runs.

use std::collections::HashMap;

/// Fixed replies waiting for their ordering marker to reach the model.
///
/// A marker is only inserted when the parser already holds synchronized
/// output, so the parser's own buffer limit bounds the number of waiting
/// replies.
#[derive(Default)]
pub(super) struct DeferredReplies {
    next: u64,
    waiting: HashMap<u64, Vec<u8>>,
}

impl DeferredReplies {
    /// Reserves an order number for `reply`.
    pub(super) fn defer(&mut self, reply: Vec<u8>) -> u64 {
        let order = self.next;
        self.next = self.next.wrapping_add(1);
        self.waiting.insert(order, reply);
        order
    }

    /// Takes the fixed reply the model reached at this ordering marker.
    pub(super) fn take(&mut self, order: u64) -> Option<Vec<u8>> {
        self.waiting.remove(&order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deferred_reply_comes_back_with_its_order() {
        let mut deferred = DeferredReplies::default();
        let order = deferred.defer(b"fixed".to_vec());
        assert_eq!(deferred.take(order), Some(b"fixed".to_vec()));
        assert_eq!(deferred.take(order), None);
    }

    #[test]
    fn order_numbers_keep_their_replies_apart() {
        let mut deferred = DeferredReplies::default();
        let first = deferred.defer(b"first".to_vec());
        let second = deferred.defer(b"second".to_vec());
        assert_ne!(first, second);
        assert_eq!(deferred.take(second), Some(b"second".to_vec()));
        assert_eq!(deferred.take(first), Some(b"first".to_vec()));
    }
}
