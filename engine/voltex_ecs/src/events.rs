//! Events — decoupled system-to-system messaging on top of [`World`] resources.
//!
//! A producer system calls [`World::send_event`] and a consumer system later in
//! the frame calls [`World::drain_events`]; neither needs a reference to the
//! other. Events of type `T` live in an [`Events<T>`] resource, so — like all
//! resources — any system can reach them through the `&mut World` it already
//! has, with no extra plumbing.
//!
//! ```ignore
//! struct EnemyKilled { points: u32 }
//!
//! // producer system:
//! world.send_event(EnemyKilled { points: 10 });
//!
//! // consumer system (later the same frame):
//! for ev in world.drain_events::<EnemyKilled>() {
//!     world.resource_mut::<Score>().unwrap().0 += ev.points;
//! }
//! ```
//!
//! **Ordering**: this is a single-buffer queue — [`drain_events`] removes what
//! it returns, so a consumer sees only events sent *earlier this frame*. The
//! runtime runs systems in registration order, so register producers before
//! consumers. (A double-buffered, order-independent reader is possible future
//! work; the single buffer keeps the v1 semantics obvious.)
//!
//! [`World::send_event`]: crate::World::send_event
//! [`World::drain_events`]: crate::World::drain_events
//! [`drain_events`]: crate::World::drain_events

use crate::world::World;

/// A queue of pending events of type `T`, stored as a [`World`] resource.
pub struct Events<T> {
    queue: Vec<T>,
}

impl<T> Default for Events<T> {
    fn default() -> Self {
        Self { queue: Vec::new() }
    }
}

impl<T> Events<T> {
    /// An empty event queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an event.
    pub fn send(&mut self, event: T) {
        self.queue.push(event);
    }

    /// Remove and return every queued event, in send order.
    pub fn drain(&mut self) -> Vec<T> {
        std::mem::take(&mut self.queue)
    }

    /// Number of queued events.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// True if no events are queued.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl World {
    /// Queue an event of type `T` (creating the [`Events<T>`] resource on first
    /// use). Read it later this frame with [`drain_events`](World::drain_events).
    pub fn send_event<T: 'static>(&mut self, event: T) {
        self.resource_or_insert_with::<Events<T>>(Events::new)
            .send(event);
    }

    /// Remove and return every queued event of type `T`, in send order. Empty
    /// if none were sent (or the resource doesn't exist yet).
    pub fn drain_events<T: 'static>(&mut self) -> Vec<T> {
        match self.resource_mut::<Events<T>>() {
            Some(events) => events.drain(),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Damage(u32);
    #[derive(Debug, PartialEq)]
    struct LevelUp;

    #[test]
    fn drain_returns_sent_events_in_order() {
        let mut world = World::new();
        world.send_event(Damage(5));
        world.send_event(Damage(3));
        let drained = world.drain_events::<Damage>();
        assert_eq!(drained, vec![Damage(5), Damage(3)]);
    }

    #[test]
    fn drain_clears_the_queue() {
        let mut world = World::new();
        world.send_event(Damage(1));
        assert_eq!(world.drain_events::<Damage>().len(), 1);
        // Second drain in the same frame sees nothing — drain consumes.
        assert!(world.drain_events::<Damage>().is_empty());
    }

    #[test]
    fn drain_with_no_events_is_empty() {
        let mut world = World::new();
        assert!(world.drain_events::<Damage>().is_empty());
    }

    #[test]
    fn event_types_are_independent() {
        let mut world = World::new();
        world.send_event(Damage(9));
        world.send_event(LevelUp);
        assert_eq!(world.drain_events::<LevelUp>(), vec![LevelUp]);
        // Draining LevelUp must not touch Damage.
        assert_eq!(world.drain_events::<Damage>(), vec![Damage(9)]);
    }

    #[test]
    fn events_struct_send_drain_directly() {
        let mut events = Events::new();
        assert!(events.is_empty());
        events.send(Damage(2));
        events.send(Damage(4));
        assert_eq!(events.len(), 2);
        assert_eq!(events.drain(), vec![Damage(2), Damage(4)]);
        assert!(events.is_empty());
    }
}
