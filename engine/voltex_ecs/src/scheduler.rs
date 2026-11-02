//! Simple in-order system scheduler. See [`DependencyScheduler`](crate::DependencyScheduler)
//! for the parallel variant.

use crate::World;

/// A system that can be run on the world.
pub trait System {
    /// Executes one tick of the system against `world`.
    fn run(&mut self, world: &mut World);
}

/// Blanket impl: any FnMut(&mut World) is a System.
impl<F: FnMut(&mut World)> System for F {
    fn run(&mut self, world: &mut World) {
        (self)(world);
    }
}

/// Runs registered systems in order.
pub struct Scheduler {
    systems: Vec<Box<dyn System>>,
}

impl Scheduler {
    /// Creates an empty scheduler.
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    /// Add a system. Systems run in the order they are added.
    pub fn add<S: System + 'static>(&mut self, system: S) -> &mut Self {
        self.systems.push(Box::new(system));
        self
    }

    /// Run all systems in registration order.
    pub fn run_all(&mut self, world: &mut World) {
        for system in &mut self.systems {
            system.run(world);
        }
    }

    /// Number of registered systems.
    pub fn len(&self) -> usize {
        self.systems.len()
    }

    /// `true` if no systems are registered.
    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::World;

    #[derive(Debug, PartialEq)]
    struct Counter(u32);

    #[test]
    fn test_scheduler_runs_in_order() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Counter(0));

        let mut scheduler = Scheduler::new();
        scheduler.add(|world: &mut World| {
            let e = world.query::<Counter>().next().unwrap().0;
            let c = world.get_mut::<Counter>(e).unwrap();
            c.0 += 1; // 0 -> 1
        });
        scheduler.add(|world: &mut World| {
            let e = world.query::<Counter>().next().unwrap().0;
            let c = world.get_mut::<Counter>(e).unwrap();
            c.0 *= 10; // 1 -> 10
        });

        scheduler.run_all(&mut world);

        let c = world.get::<Counter>(e).unwrap();
        assert_eq!(c.0, 10); // proves order: add first, then multiply
    }

    #[test]
    fn test_scheduler_empty() {
        let mut world = World::new();
        let mut scheduler = Scheduler::new();
        scheduler.run_all(&mut world); // should not panic
    }

    #[test]
    fn test_scheduler_multiple_runs() {
        let mut world = World::new();
        let e = world.spawn();
        world.add(e, Counter(0));

        let mut scheduler = Scheduler::new();
        scheduler.add(|world: &mut World| {
            let e = world.query::<Counter>().next().unwrap().0;
            let c = world.get_mut::<Counter>(e).unwrap();
            c.0 += 1;
        });

        scheduler.run_all(&mut world);
        scheduler.run_all(&mut world);
        scheduler.run_all(&mut world);

        assert_eq!(world.get::<Counter>(e).unwrap().0, 3);
    }

    #[test]
    fn test_scheduler_add_chaining() {
        let mut scheduler = Scheduler::new();
        scheduler.add(|_: &mut World| {}).add(|_: &mut World| {});
        assert_eq!(scheduler.len(), 2);
    }
}
