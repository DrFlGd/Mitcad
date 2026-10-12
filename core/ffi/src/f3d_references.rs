// SPDX-License-Identifier: MIT
//! The documents an `.f3d` design refers to, imported from local files
//! and filled into its components of other documents (mitcad#196): where
//! the files are is `mitcad_import::reference_folder`.

use std::path::Path;

use mitcad_import::components::InsertedComponent;
use mitcad_import::reference_folder::{Found, MAX_DEPTH, Resolver, stem_name};
use mitcad_import::references::{Linker, Referenced};
use mitcad_import::{ImportReport, ReferenceReport};

use crate::f3d_import::TimelineOptions;
use crate::kernel::OcctKernel;

impl crate::Document {
    /// Imports the documents the design (just imported, `report`) refers
    /// to, each from its file with its own references, and fills the
    /// components standing for their components
    /// ([`mitcad_import::references::link_references`]); the report says
    /// where each document was found and how it came in. Returns whether
    /// it changed the document.
    pub(crate) fn import_references(
        &mut self,
        path: &str,
        options: &TimelineOptions,
        report: &mut ImportReport,
    ) -> bool {
        let Some(design) = report.designs.first_mut() else {
            return false;
        };
        if design.components.inserted.is_empty() {
            return false;
        }
        let mut resolver = Resolver::new(
            Path::new(path),
            &options.references,
            &options.reference_files,
        );
        design.warnings.extend(resolver.warnings().iter().cloned());
        let here = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_owned());
        let key = |file: &str, label: Option<&str>| format!("{file}!{}", label.unwrap_or(""));
        let mut stack = options.stack.clone();
        stack.push(key(&here, options.design.as_deref()));
        let mut urns: Vec<String> = Vec::new();
        for i in &design.components.inserted {
            if !urns.contains(&i.document) {
                urns.push(i.document.clone());
            }
        }
        let mut entries: Vec<ReferenceReport> = Vec::new();
        let mut linker = Linker::default();
        for urn in urns {
            let inserted: Vec<&InsertedComponent> = design
                .components
                .inserted
                .iter()
                .filter(|i| i.document == urn)
                .collect();
            let guid = inserted.iter().find_map(|i| i.document_guid.clone());
            let found = if urn.is_empty() {
                Found::default()
            } else {
                resolver.resolve(&urn, guid.as_deref())
            };
            let mut entry = ReferenceReport {
                document: urn.clone(),
                name: found.name.clone(),
                version: found.version,
                file: found
                    .file
                    .as_ref()
                    .map(|f| f.to_string_lossy().into_owned()),
                found: found.how.map(str::to_owned),
                status: "missing".to_owned(),
                note: found.note.clone(),
                components: inserted
                    .iter()
                    .map(|i| self.0.assembly().name(i.component))
                    .collect(),
                occurrences: inserted.iter().map(|i| i.occurrences).sum(),
                ..ReferenceReport::default()
            };
            let Some(file) = found.file.clone() else {
                if options.skipped_references.contains(&urn) {
                    entry.status = "skipped".to_owned();
                }
                entries.push(entry);
                continue;
            };
            let file = file.to_string_lossy().into_owned();
            if entry.name.is_none() {
                entry.name = Some(stem_name(&file));
            }
            let canonical = std::fs::canonicalize(&file)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| file.clone());
            let this = key(&canonical, found.design.as_deref());
            if stack.contains(&this) {
                entry.status = "failed".to_owned();
                entry.note = Some("it refers back to a design being imported".to_owned());
                entries.push(entry);
                continue;
            }
            if stack.len() >= MAX_DEPTH {
                entry.status = "failed".to_owned();
                entry.note = Some(format!(
                    "references of references are followed {MAX_DEPTH} levels deep"
                ));
                entries.push(entry);
                continue;
            }
            let sub_options = TimelineOptions {
                design: found.design.clone(),
                dump: None,
                text: false,
                report_path: None,
                list: false,
                list_references: false,
                learn: None,
                references: resolver.folders(),
                reference_files: resolver.files(),
                stack: [stack.clone(), vec![this]].concat(),
                ..options.clone()
            };
            let mut document = crate::Document(mitcad_model::Document::new(OcctKernel));
            // A stop of the import stops these too.
            document.0.set_monitor(self.0.monitor().cloned());
            if std::env::var_os("MITCAD_IMPORT_TRACE").is_some() {
                eprintln!("import: the document {} from {file}", entry.document);
            }
            match document.import_timeline(&file, &sub_options) {
                Ok(mut sub) if !sub.designs.is_empty() => {
                    let d = sub.designs.remove(0);
                    entry.items = d
                        .counts()
                        .into_iter()
                        .map(|(o, n)| (o.as_str().to_owned(), n))
                        .collect();
                    entry.references = d.references.clone();
                    let source = Referenced {
                        document: &document.0,
                        occurrence_guids: &d.components.occurrence_guids,
                        name: entry.name.clone().unwrap_or_else(|| stem_name(&file)),
                    };
                    // Copied at once: the document goes before the next.
                    linker.fill(&mut self.0, design, &urn, &source);
                }
                Ok(_) => {
                    entry.status = "failed".to_owned();
                    entry.note = Some("the file holds no design".to_owned());
                }
                Err(e) => {
                    entry.status = "failed".to_owned();
                    entry.note = Some(e.0);
                }
            }
            entries.push(entry);
        }
        let outcome = linker.finish(&mut self.0, design);
        for e in &mut entries {
            let linked: Vec<String> = outcome
                .linked
                .iter()
                .filter(|l| l.document == e.document)
                .map(|l| l.name.clone())
                .collect();
            if !linked.is_empty() {
                e.status = "linked".to_owned();
                e.components = linked;
            }
            if let Some((_, why)) = outcome.failed.iter().find(|(d, _)| *d == e.document) {
                e.status = "failed".to_owned();
                e.note = Some(why.clone());
            }
        }
        let missing: Vec<String> = entries
            .iter()
            .filter(|e| e.status != "linked")
            .map(|e| format!("{} ({})", e.title(), e.status))
            .collect();
        if !missing.is_empty() {
            design.warnings.push(format!(
                "components of other documents stay empty, their documents not imported: {}; \
                 the documents are looked for in the folder next to the design ({}), in the \
                 folders and files given for them, and by the GUIDs their files list",
                missing.join(", "),
                Path::new(path)
                    .with_extension("refs")
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ));
        }
        if !outcome.positions_left_out.is_empty() {
            let left: Vec<String> = outcome
                .positions_left_out
                .iter()
                .map(|(why, n)| format!("{n} ({why})"))
                .collect();
            design.warnings.push(format!(
                "captured positions inside components of other documents left out: {}",
                left.join(", ")
            ));
        }
        design.references = entries;
        !outcome.linked.is_empty()
    }
}
