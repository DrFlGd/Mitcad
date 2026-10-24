// SPDX-License-Identifier: MIT
//! Bodies that belong to one history replay. A receiving document's old
//! bodies are neither source references nor candidates for verification or
//! fallback replacement. Scope follows the document, including nested imports
//! and the separate document on each candidate worker.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use mitcad_model::{BodyUid, BodyView, ComponentUid, Document, Kernel};

#[derive(Clone, Default)]
pub(crate) struct Scope(Arc<BTreeMap<BodyUid, Option<u128>>>);

struct Entry {
    document: usize,
    scope: Scope,
}

thread_local! {
    static ACTIVE: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
}

fn key<K: Kernel>(doc: &Document<K>) -> usize {
    // Importer borrows this document in place for the guard's lifetime.
    // The address is only an identity key; it is never dereferenced later.
    std::ptr::from_ref(doc) as usize
}

impl Scope {
    pub(crate) fn capture<K: Kernel>(doc: &Document<K>) -> Self {
        Self(Arc::new(
            doc.bodies()
                .iter()
                .map(|b| (b.uid, doc.body_version(b.uid)))
                .collect(),
        ))
    }

    pub(crate) fn enter<K: Kernel>(&self, doc: &Document<K>) -> Guard {
        let document = key(doc);
        ACTIVE.with_borrow_mut(|active| {
            active.push(Entry {
                document,
                scope: self.clone(),
            })
        });
        Guard {
            document,
            scope: self.clone(),
            _thread: PhantomData,
        }
    }
}

pub(crate) struct Guard {
    document: usize,
    scope: Scope,
    _thread: PhantomData<Rc<()>>,
}

impl Guard {
    pub(crate) fn scope(&self) -> Scope {
        self.scope.clone()
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with_borrow_mut(|active| {
            if let Some(index) = active.iter().rposition(|e| {
                e.document == self.document && Arc::ptr_eq(&e.scope.0, &self.scope.0)
            }) {
                active.remove(index);
            }
        });
    }
}

fn with_scope<K: Kernel, T>(doc: &Document<K>, f: impl FnOnce(Option<&Scope>) -> T) -> T {
    ACTIVE.with_borrow(|active| {
        f(active
            .iter()
            .rev()
            .find(|e| e.document == key(doc))
            .map(|e| &e.scope))
    })
}

pub(crate) fn protected<K: Kernel>(doc: &Document<K>, uid: BodyUid) -> bool {
    with_scope(doc, |scope| scope.is_some_and(|s| s.0.contains_key(&uid)))
}

pub(crate) fn protected_in<K: Kernel>(doc: &Document<K>, component: ComponentUid) -> bool {
    with_scope(doc, |scope| {
        scope.is_some_and(|s| {
            s.0.keys()
                .any(|&uid| doc.body_component(uid).unwrap_or(ComponentUid::ROOT) == component)
        })
    })
}

pub(crate) fn filter<'d, K: Kernel>(
    doc: &Document<K>,
    mut bodies: Vec<BodyView<'d, K::Shape>>,
) -> Vec<BodyView<'d, K::Shape>> {
    with_scope(doc, |scope| {
        if let Some(scope) = scope.filter(|s| !s.0.is_empty()) {
            bodies.retain(|b| !scope.0.contains_key(&b.uid));
        }
    });
    bodies
}

pub(crate) fn bodies<K: Kernel>(doc: &Document<K>) -> Vec<BodyView<'_, K::Shape>> {
    filter(doc, doc.bodies())
}

pub(crate) fn shape<K: Kernel>(doc: &Document<K>, uid: BodyUid) -> Option<&K::Shape> {
    (!protected(doc, uid))
        .then(|| doc.body_shape(uid))
        .flatten()
}

/// Reject an unexpected explicit or implicit reference to an old body before
/// adopting a candidate. The kernel's immutable inputs keep its shape safe;
/// undoing the candidate restores the receiving document's result.
pub(crate) fn unchanged<K: Kernel>(doc: &Document<K>) -> Result<(), String> {
    with_scope(doc, |scope| {
        if let Some((uid, _)) = scope
            .into_iter()
            .flat_map(|s| s.0.iter())
            .find(|(uid, version)| doc.body_version(**uid) != **version)
        {
            return Err(format!(
                "the definition would change pre-existing body {uid} outside this import"
            ));
        }
        Ok(())
    })
}
