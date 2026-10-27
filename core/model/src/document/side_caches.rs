// SPDX-License-Identifier: MIT
//! Side caches of a document: runtime caches kept beside the modelling
//! results, one per type, such as the drawing views projected before
//! (mitcad#707) and the annotations' measured picks (mitcad#716). They change
//! no state, so they hold nothing a command or undo has to take back.
//!
//! The caches are keyed by their type, so that a layer above the model (the
//! JSON api) can keep a cache of its own type on a document without the
//! document naming that type. A fork of a document starts with none.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The caches of one document, one per type, each made on first use.
#[derive(Default)]
pub(crate) struct SideCaches {
    caches: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl SideCaches {
    /// The cache of type `T`, made empty (its `Default`) on first use.
    /// The caches synchronise themselves; they are shared through `&self`.
    pub(crate) fn get<T: Any + Send + Sync + Default>(&self) -> Arc<T> {
        let mut caches = match self.caches.lock() {
            Ok(caches) => caches,
            Err(poisoned) => poisoned.into_inner(),
        };
        let cache = caches
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Arc::new(T::default()) as Arc<dyn Any + Send + Sync>);
        match Arc::clone(cache).downcast::<T>() {
            Ok(cache) => cache,
            // The map is keyed by the type, so the entry is always a `T`.
            Err(_) => unreachable!("a side cache under another type's key"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Counter(AtomicUsize);

    #[derive(Default)]
    struct Other(AtomicUsize);

    #[test]
    fn a_side_cache_is_made_once_per_type_and_shared() {
        let caches = SideCaches::default();
        caches.get::<Counter>().0.fetch_add(2, Ordering::Relaxed);
        caches.get::<Counter>().0.fetch_add(1, Ordering::Relaxed);
        assert_eq!(caches.get::<Counter>().0.load(Ordering::Relaxed), 3);
        assert_eq!(caches.get::<Other>().0.load(Ordering::Relaxed), 0);
    }
}
