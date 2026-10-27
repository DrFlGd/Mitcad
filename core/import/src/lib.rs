// SPDX-License-Identifier: MIT
//! The .f3d design import (T1) with all of the replay's families: a design
//! dump IR ([`mitcad_dump`], from an `.f3d` file's streams or an external
//! dump) replayed as Mitcad document commands, and `.solidcraft` designs
//! ([`solidcraft`]).
//!
//! The replay is split into crates (mitcad#785) so that an edit in one part
//! does not rebuild the others:
//!
//! - `mitcad-import-replay`: the core, `Importer`'s state, the timeline
//!   walk, references, fallbacks and the report (its crate docs describe
//!   the replay). This crate re-exports its public paths, so that
//!   `mitcad_import::history`, `mitcad_import::Options` and the others keep
//!   working.
//! - The families above the core, reached from its timeline walk through
//!   its [`Families`] trait, which this crate fills ([`import_design`]):
//!   `mitcad-import-joints` (joints, joint origins, ground items, assembly
//!   relationships), `mitcad-import-sheet` (sheet metal),
//!   `mitcad-import-primitives` (boxes, cylinders, spheres, tori, coils)
//!   and `mitcad-import-appearances` (appearances and materials,
//!   re-exported as [`appearances`], mitcad#791).
//! - `mitcad-import-solidcraft`, re-exported as [`solidcraft`].
//!
//! The replay's tests are here, where the whole replay is.

pub use mitcad_import_replay::*;
// .solidcraft designs with their history (mitcad#450), a crate of their
// own (mitcad#785).
pub use mitcad_import_solidcraft as solidcraft;
// The appearances family (mitcad#228), a crate of its own above the core
// (mitcad#791), at the module's old path.
pub use mitcad_import_appearances as appearances;

use std::collections::HashMap;

use mitcad_dump::{CoilDetail, Dump, PrimitiveDetail, TimelineItem};
use mitcad_import_appearances::Appearances;
use mitcad_import_joints::Joints;
use mitcad_import_primitives::Primitives;
use mitcad_import_sheet::Sheets;
use mitcad_model::Document;
use serde_json::{Map, Value};

/// The replay's families above its core, as its timeline walk reaches them
/// ([`Families`], mitcad#785).
struct ReplayFamilies;

impl<K: ImportKernel> Families<K> for ReplayFamilies {
    fn assembly_item(
        &self,
        importer: &mut Importer<'_, K>,
        at: usize,
        index: i64,
        item: &TimelineItem,
    ) {
        importer.assembly_item(at, index, item);
    }

    fn sheet_item(
        &self,
        importer: &mut Importer<'_, K>,
        index: i64,
        item: &TimelineItem,
        sheet: &Map<String, Value>,
    ) -> Result<Vec<Candidate>, String> {
        importer.sheet_item(index, item, sheet)
    }

    fn dressups_on_sheets(
        &self,
        importer: &Importer<'_, K>,
        candidates: Vec<Candidate>,
        round: bool,
    ) -> Vec<Candidate> {
        importer.dressups_on_sheets(candidates, round)
    }

    fn primitive(
        &self,
        importer: &mut Importer<'_, K>,
        index: i64,
        object_type: &str,
        d: &PrimitiveDetail,
    ) -> Result<Vec<Candidate>, String> {
        importer.primitive(index, object_type, d)
    }

    fn coil_primitive(
        &self,
        importer: &mut Importer<'_, K>,
        index: i64,
        d: &CoilDetail,
    ) -> Result<Vec<Candidate>, String> {
        importer.coil_primitive(index, d)
    }

    fn appearances(&self, importer: &mut Importer<'_, K>) {
        importer.appearances();
    }
}

/// Imports a design into the document (normally a new one). The report
/// tells how each timeline item came in.
pub fn import_design<K: ImportKernel>(
    doc: &mut Document<K>,
    dump: &Dump,
    geometry: &mut dyn StoredGeometry<K::Shape>,
    options: &Options,
) -> DesignReport {
    import_design_with(doc, dump, geometry, options, None)
}

/// [`import_design`] with the documents the design's components of other
/// documents come from, imported already (by their URNs): the components
/// are filled from them before the items are replayed (`prefill`,
/// mitcad#196).
pub fn import_design_with<K: ImportKernel>(
    doc: &mut Document<K>,
    dump: &Dump,
    geometry: &mut dyn StoredGeometry<K::Shape>,
    options: &Options,
    sources: Option<&HashMap<String, references::Referenced<'_, K>>>,
) -> DesignReport {
    #[cfg(test)]
    let options = &with_api_commands::<K>(options);
    replay_design(doc, dump, geometry, options, sources, &ReplayFamilies)
}

/// The options with the api's commands where they give none for the
/// kernel: the unit tests, which have the api as a dev-dependency, run the
/// api's commands when they give none (a cut and paste moved by Move to
/// Component, `pasting` of the core; mitcad#778, mitcad#785).
#[cfg(test)]
fn with_api_commands<K: ImportKernel>(options: &Options) -> Options {
    let mut options = options.clone();
    if options
        .commands
        .as_ref()
        .and_then(Commands::get::<K>)
        .is_none()
    {
        options.commands = Some(Commands::new::<K>(api_command::<K>));
    }
    options
}

/// The api's `command` (the unit tests' commands).
#[cfg(test)]
fn api_command<K: mitcad_model::Kernel>(
    doc: &mut Document<K>,
    json: &str,
) -> Result<String, String> {
    use mitcad_model_api::DocumentApi;
    doc.command(json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
