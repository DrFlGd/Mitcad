// SPDX-License-Identifier: MIT
//! The design policy in the document (mitcad#220, `crate::policy`): the new
//! design's arrangement, policy changes as undo steps, reports of what
//! breaks a policy, and the check that keeps commands from adding to it.

use std::collections::BTreeMap;

use super::{DocState, Document, ModelError, invalid};
use crate::features::FeatureDef;
use crate::ids::ComponentUid;
use crate::kernel::Kernel;
use crate::policy::{self, Policy, PolicyObject, Setting, Violation};
use crate::transform::Transform;

/// The component a new design starts with, active.
pub const NEW_DESIGN_PART: &str = "Part1";

/// An object that breaks one setting of an owner: a sketch, a feature or
/// an occurrence placed in it; for attachments and references the
/// consumer, with the owners on the other side of its links.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyItem {
    /// `sketch`, `feature` or `occurrence`.
    pub kind: &'static str,
    /// `F3` or `O2`.
    pub uid: String,
    pub name: String,
    /// The component the object is in.
    pub component: ComponentUid,
    /// The other owners of its links (attachments and references).
    pub others: Vec<ComponentUid>,
}

/// An owner's policy and the objects that break each setting.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyReport {
    pub component: ComponentUid,
    pub policy: Policy,
    /// Each setting in [`Setting::ALL`] order with its offending objects.
    pub settings: Vec<(Setting, Vec<PolicyItem>)>,
}

impl DocState {
    /// The state of a new, empty design: the root with the Assembly policy
    /// and an empty component Part1 with the Part policy, active, so that
    /// the first sketch or feature goes into Part1.
    pub(crate) fn new_design() -> Self {
        let mut state = Self::default();
        state.assembly.root_policy = Policy::ASSEMBLY;
        let part = state.assembly.create(Some(NEW_DESIGN_PART), None);
        state.assembly.set_policy(part, Policy::PART);
        state
            .assembly
            .place(part, ComponentUid::ROOT, Transform::IDENTITY);
        state.assembly.active = part;
        state
    }

    fn object_name(&self, object: PolicyObject) -> String {
        match object {
            PolicyObject::Feature(uid) => self
                .entry(uid)
                .map_or_else(|| uid.to_string(), |f| f.name.clone()),
            PolicyObject::Occurrence(uid) => self.assembly.occurrence_name(uid),
        }
    }

    /// Why a new violation is refused, in words.
    fn refusal(&self, v: &Violation) -> String {
        let a = &self.assembly;
        let owner = a.name(v.owner);
        let object = self.object_name(v.object);
        let other = v.other.map(|o| a.name(o)).unwrap_or_default();
        let is_sketch = matches!(v.object, PolicyObject::Feature(f)
            if self.entry(f).is_some_and(|e| matches!(e.def, FeatureDef::Sketch(_))));
        let unlinked = if is_sketch {
            " A projection without Projection Link copies the geometry without a reference."
        } else {
            ""
        };
        match v.setting {
            Setting::Sketches | Setting::Features | Setting::Components => {
                format!("{object}: {}", creation_refusal(&owner, v.setting))
            }
            Setting::AttachToOthers => format!(
                "{object} in {owner} cannot be attached to a face or plane of {other}: \
                 {owner}'s policy does not allow its sketches to attach to other components"
            ),
            Setting::AttachmentsFromOthers => format!(
                "{object} in {other} cannot be attached to a face or plane of {owner}: \
                 {owner}'s policy does not allow sketches of other components to attach to it"
            ),
            Setting::ReferenceOthers => format!(
                "{object} in {owner} cannot reference geometry of {other}: {owner}'s policy \
                 does not allow references to other components.{unlinked}"
            ),
            Setting::ReferencesFromOthers => format!(
                "{object} in {other} cannot reference geometry of {owner}: {owner}'s policy \
                 does not allow references from other components.{unlinked}"
            ),
        }
    }
}

/// Why `owner` refuses what a containment setting governs.
fn creation_refusal(owner: &str, setting: Setting) -> String {
    let what = match setting {
        Setting::Sketches => "sketches",
        Setting::Features => "modelling features",
        _ => "components in it",
    };
    format!(
        "{owner} does not allow {what} (its policy). Activate a component that allows them, \
         or change {owner}'s policy."
    )
}

impl<K: Kernel> Document<K> {
    /// A new, empty design ([`DocState::new_design`]); it is not modified.
    pub fn new_design(kernel: K) -> Self {
        let mut document = Self::from_state(kernel, DocState::new_design());
        document.mark_saved();
        document
    }

    /// Runs `f` with changes checked against the design policy (what the
    /// JSON commands and previews do), unless checks are suspended.
    pub fn with_policy_checks<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let before = self.policy_checks;
        self.policy_checks = !self.policy_suspended;
        let out = f(self);
        self.policy_checks = before;
        out
    }

    /// Runs `f` without policy checks, even in the commands it runs: an
    /// importer replays a file's design as it is.
    pub fn without_policy_checks<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let before = (self.policy_checks, self.policy_suspended);
        self.policy_checks = false;
        self.policy_suspended = true;
        let out = f(self);
        (self.policy_checks, self.policy_suspended) = before;
        out
    }

    /// A component's policy.
    pub fn policy(&self, component: ComponentUid) -> Policy {
        self.state.assembly.policy(component)
    }

    /// Sets a component's policy: one undo step that recomputes nothing
    /// and changes no content (what breaks the new policy is reported as
    /// violations, [`Document::policy_report`]).
    pub fn set_policy(
        &mut self,
        component: ComponentUid,
        policy: Policy,
    ) -> Result<(), ModelError> {
        if !self.state.assembly.exists(component) {
            return Err(invalid(format!("component {component} does not exist")));
        }
        if self.state.assembly.policy(component) == policy {
            return Ok(());
        }
        self.apply_with(|state| {
            state.assembly.set_policy(component, policy);
            let label = format!("Policy of {}", state.assembly.name(component));
            Ok((label, (), false))
        })
    }

    /// What breaks each setting of a component's policy, or of `policy` in
    /// its place (what a change would show). Objects are listed once per
    /// setting: for containment the objects directly in the component, for
    /// attachments and references the consumers.
    pub fn policy_report(
        &self,
        component: ComponentUid,
        policy: Option<Policy>,
    ) -> Result<PolicyReport, ModelError> {
        let a = &self.state.assembly;
        if !a.exists(component) {
            return Err(invalid(format!("component {component} does not exist")));
        }
        let policy = policy.unwrap_or_else(|| a.policy(component));
        let policy_of = |c: ComponentUid| {
            if c == component { policy } else { a.policy(c) }
        };
        let all = policy::violations(a, &self.state.features, &policy_of);
        let mut found: BTreeMap<(Setting, PolicyObject), Vec<ComponentUid>> = BTreeMap::new();
        for v in all.iter().filter(|v| v.owner == component) {
            let others = found.entry((v.setting, v.object)).or_default();
            if let Some(other) = v.other
                && !others.contains(&other)
            {
                others.push(other);
            }
        }
        let settings = Setting::ALL
            .into_iter()
            .map(|setting| {
                let items = found
                    .iter()
                    .filter(|((s, _), _)| *s == setting)
                    .map(|((_, object), others)| self.policy_item(*object, others.clone()))
                    .collect();
                (setting, items)
            })
            .collect();
        Ok(PolicyReport {
            component,
            policy,
            settings,
        })
    }

    fn policy_item(&self, object: PolicyObject, others: Vec<ComponentUid>) -> PolicyItem {
        let name = self.state.object_name(object);
        match object {
            PolicyObject::Feature(uid) => {
                let entry = self.state.entry(uid);
                let sketch = entry.is_some_and(|e| matches!(e.def, FeatureDef::Sketch(_)));
                PolicyItem {
                    kind: if sketch { "sketch" } else { "feature" },
                    uid: uid.to_string(),
                    name,
                    component: entry.map_or(ComponentUid::ROOT, |e| e.component),
                    others,
                }
            }
            PolicyObject::Occurrence(uid) => PolicyItem {
                kind: "occurrence",
                uid: uid.to_string(),
                name,
                component: self
                    .state
                    .assembly
                    .occurrence(uid)
                    .map_or(ComponentUid::ROOT, |o| o.component),
                others,
            },
        }
    }

    /// Why `component` refuses new objects of a containment setting
    /// (sketches, features, components), or None when it allows them.
    pub fn creation_refusal(&self, component: ComponentUid, setting: Setting) -> Option<String> {
        (!self.policy(component).allows(setting))
            .then(|| creation_refusal(&self.state.assembly.name(component), setting))
    }

    /// The owners whose faces and planes sketches of `component` cannot
    /// be attached to, each with the reason (sketch-support picking leaves
    /// their geometry out).
    pub fn attachment_denials(&self, component: ComponentUid) -> Vec<(ComponentUid, String)> {
        let a = &self.state.assembly;
        let own = a.policy(component);
        let name = a.name(component);
        std::iter::once(ComponentUid::ROOT)
            .chain(a.components.iter().map(|c| c.uid))
            .filter(|c| *c != component)
            .filter_map(|other| {
                let reason = if !own.attach_to_others {
                    format!(
                        "{name}'s policy does not allow its sketches to attach to other components"
                    )
                } else if !a.policy(other).attachments_from_others {
                    format!(
                        "{}'s policy does not allow sketches of other components to attach to it",
                        a.name(other)
                    )
                } else {
                    return None;
                };
                Some((other, reason))
            })
            .collect()
    }

    /// Refuses a next state that adds a violation of the design policy
    /// (counted per owner, setting, object and other owner: a consumer that
    /// already broke a setting may not add another link that breaks it).
    /// Violations the state already had, and fewer of them, pass, so
    /// existing objects stay editable; a policy change itself adds none, as
    /// both states are measured with the next state's policies. Only while
    /// checks are on ([`Document::with_policy_checks`]). Every change goes
    /// through here before it is computed or committed (`apply_with`,
    /// `apply_move`, previews), so a move or reparenting is refused before
    /// anything changes.
    pub(crate) fn check_policy(&self, next: &DocState) -> Result<(), ModelError> {
        if !self.policy_checks {
            return Ok(());
        }
        match self.policy_refusal(next) {
            None => Ok(()),
            Some(message) => Err(invalid(message)),
        }
    }

    /// Why the design policy refuses `next` (see [`Document::check_policy`]),
    /// whether checks are on or not: what a plan shows before its command
    /// runs (a reparenting, mitcad#219). None when it adds no violation.
    pub(crate) fn policy_refusal(&self, next: &DocState) -> Option<String> {
        let policy_of = |c: ComponentUid| {
            if next.assembly.exists(c) {
                next.assembly.policy(c)
            } else {
                self.state.assembly.policy(c)
            }
        };
        let before = policy::violations(&self.state.assembly, &self.state.features, &policy_of);
        let after = policy::violations(&next.assembly, &next.features, &policy_of);
        let added = policy::new_violations(&before, &after);
        added.first().map(|v| {
            let mut message = next.refusal(v);
            if added.len() > 1 {
                message.push_str(&format!(
                    " ({} more {} refused)",
                    added.len() - 1,
                    if added.len() == 2 {
                        "item is"
                    } else {
                        "items are"
                    }
                ));
            }
            message
        })
    }
}
