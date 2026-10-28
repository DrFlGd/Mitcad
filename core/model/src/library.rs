// SPDX-License-Identifier: MIT
//! Components from libraries (mitcad#64, mitcad#63): a library is a git
//! repository of Mitcad designs with a manifest (`mitcad-vcs`, `library`);
//! the model knows none of that. It asks a [`LinkResolver`] for a
//! component's design at a version (a commit) and records where the
//! component came from on the component ([`LibraryRef`]): the library,
//! the commit, the configuration row, the licence and the designation a
//! bill of materials gives it.
//!
//! The version is the one the user chose when inserting the part, kept
//! per component: opening the design reads the part at that commit even
//! when the library has moved on, and only an explicit update
//! (`update_library_parts`) changes it. A part whose library is not on
//! this computer keeps the bodies saved with the design.

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::file::MemoryStore;

/// Where a component came from when it came from a library: the
/// component's `library` in the design file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryRef {
    /// The library's id (`mitcad-parts`).
    pub library: String,
    /// Where the library was fetched from (never with credentials).
    pub url: String,
    /// The commit the part is read from: the version chosen for it.
    pub rev: String,
    /// The version's name for people (a tag such as `v1.2.0`), if any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// The component's id in the library's manifest.
    pub component: String,
    /// The design's path in the library at that commit.
    pub path: String,
    /// The configuration row it was inserted in (`M5x16`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    /// The part's licence (an SPDX expression), as the library gives it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    /// The library's authors, for the attribution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// The name a bill of materials gives the part (`ISO 4762 M5x16`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub designation: String,
    /// Who sells the part, the supplier's article number (the row's own
    /// first) and its product page, as the library gives them (mitcad#375).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub supplier: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub part_number: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub supplier_url: String,
}

impl LibraryRef {
    /// The commit shortened for people, with the label: `v1.2.0
    /// (3f9a2c1)`.
    pub fn version_text(&self) -> String {
        let short: String = self.rev.chars().take(7).collect();
        if self.label.is_empty() {
            short
        } else {
            format!("{} ({short})", self.label)
        }
    }
}

/// What a resolver is asked for: a component of a library at a version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRequest {
    pub library: String,
    pub url: String,
    /// A commit id, a tag or another revision the library's repository
    /// knows; the answer gives the commit.
    pub rev: String,
    pub component: String,
}

/// A library component's design at a version, as a resolver reads it.
#[derive(Debug)]
pub struct LibrarySource {
    /// The design file's text.
    pub text: String,
    /// The B-rep data it refers to (a library may be a project with a
    /// store).
    pub store: MemoryStore,
    /// The commit (40 hex digits).
    pub rev: String,
    /// The tag of that commit, if any.
    pub label: String,
    /// The library's id, URL, name, authors.
    pub library: String,
    pub url: String,
    pub library_name: String,
    /// The component's id in the manifest.
    pub component: String,
    pub authors: Vec<String>,
    /// The component's path, name, standard, licence and designation
    /// pattern (`{standard} {config}`; `{name}`, `{config}` and the
    /// options' names in braces are replaced).
    pub path: String,
    pub name: String,
    pub standard: String,
    pub license: String,
    pub designation: String,
    /// The manifest's `supplier`, `part_number` and `supplier_url` of the
    /// component (empty when it has none), and its `build` block as it is
    /// written there (None for a part the BUILD tab does not place;
    /// mitcad#375, `docs/libraries.md`, "BUILD parts").
    pub supplier: Option<String>,
    pub part_number: Option<String>,
    pub supplier_url: Option<String>,
    pub build: Option<serde_json::Value>,
}

/// Reads library components for the model (the application and
/// `mitcad-cli` install one that reads git repositories, `mitcad-vcs`).
pub trait LinkResolver: Send + Sync {
    fn library_source(&self, request: &LibraryRequest) -> Result<LibrarySource, String>;

    /// Reads a texture relative to the component's design at the exact
    /// commit returned by `library_source`. Editable copies embed these
    /// bytes so their appearances do not depend on the library's cache.
    /// Resolvers without external images need only `library_source`.
    fn library_texture(&self, request: &LibraryRequest, path: &str) -> Result<Vec<u8>, String> {
        Err(format!(
            "the library resolver cannot read texture '{path}' of {}",
            request.component
        ))
    }

    /// The base library of materials and print profiles (mitcad#640,
    /// `crate::print_library`): the item files Mitcad's parts library
    /// lists in its newest version (`materials/...`, `print/printers/...`,
    /// `print/processes/...`), each with its path in the library and its
    /// bytes. None when there is none on this computer; resolvers without
    /// libraries have none.
    fn print_library(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        Ok(Vec::new())
    }
}

static RESOLVER: RwLock<Option<Arc<dyn LinkResolver>>> = RwLock::new(None);

/// Sets the resolver documents use when they have none of their own
/// ([`crate::Document::set_link_resolver`]).
pub fn set_link_resolver(resolver: Option<Arc<dyn LinkResolver>>) {
    *RESOLVER.write().unwrap_or_else(|e| e.into_inner()) = resolver;
}

/// The resolver set for the process, if any.
pub fn link_resolver() -> Option<Arc<dyn LinkResolver>> {
    RESOLVER.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The designation of a part made from a design in the state `state`
/// (its row applied, mitcad#798): the row of the combination its options
/// have (`row` when the parameters have its values, else the first that
/// matches) gives its own designation, else the pattern with
/// `{standard}`, `{name}`, `{config}` (the row's name, empty when the
/// combination is no row) and `{<option>}` (the option's value as text)
/// replaced; without a pattern `{standard} {config}` (or the name for
/// `{standard}` when there is no standard). Spaces are folded.
pub(crate) fn designation(
    source: &LibrarySource,
    state: &crate::DocState,
    row: Option<&crate::configurations::ConfigurationRow>,
) -> String {
    let table = &state.configurations;
    let row = table.combination_row(&state.parameters, row.map(|r| r.name.as_str()));
    if let Some(own) = row.and_then(|r| r.designation.as_ref()) {
        return own.clone();
    }
    let pattern = if source.designation.trim().is_empty() {
        if source.standard.trim().is_empty() {
            "{name} {config}"
        } else {
            "{standard} {config}"
        }
    } else {
        source.designation.as_str()
    };
    let mut text = pattern
        .replace("{standard}", &source.standard)
        .replace("{name}", &source.name)
        .replace("{config}", row.map_or("", |r| r.name.as_str()));
    for (option, value) in table.designation_values(&state.parameters) {
        text = text.replace(&format!("{{{option}}}"), &value);
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A URL as it may be shown: user information (`user:token@`) left out.
pub fn shown_url(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
            match authority.rsplit_once('@') {
                Some((_, host)) => format!("{scheme}://{host}{path}"),
                None => url.to_owned(),
            }
        }
        None => url.to_owned(),
    }
}

/// Standard base64 of binary data (preview images in JSON answers).
pub fn encode_base64(data: &[u8]) -> String {
    crate::base64::encode(data)
}

/// Reads standard base64.
pub fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    crate::base64::decode(text)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::Document;
    use crate::configurations::{ConfigurationRow, Configurations, OptionValue};
    use crate::expr::Unit;
    use crate::testing::MockKernel;

    fn source(pattern: &str) -> LibrarySource {
        LibrarySource {
            text: String::new(),
            store: MemoryStore::new(),
            rev: String::new(),
            label: String::new(),
            library: "test".to_owned(),
            url: String::new(),
            library_name: String::new(),
            component: "screw".to_owned(),
            authors: Vec::new(),
            path: "screw.mitcad".to_owned(),
            name: "Screw".to_owned(),
            standard: "ISO 4762".to_owned(),
            license: String::new(),
            designation: pattern.to_owned(),
            supplier: None,
            part_number: None,
            supplier_url: None,
            build: None,
        }
    }

    fn row(name: &str, size: &str, length: &str, own: Option<&str>) -> ConfigurationRow {
        ConfigurationRow {
            name: name.to_owned(),
            values: BTreeMap::from([
                ("Size".to_owned(), format!("'{size}'")),
                ("Length".to_owned(), length.to_owned()),
            ]),
            designation: own.map(str::to_owned),
            ..ConfigurationRow::default()
        }
    }

    /// mitcad#798: an open table's combination that is no row takes the
    /// pattern with the option values in braces.
    #[test]
    fn a_combination_that_is_no_row_takes_the_pattern() {
        let mut doc = Document::new(MockKernel::default());
        doc.add_parameter_expression("Size", "'M5'", None, "")
            .unwrap();
        doc.add_parameter_expression("Length", "16 mm", Some(Unit::MM), "")
            .unwrap();
        doc.set_configurations(Configurations {
            options: vec!["Size".to_owned(), "Length".to_owned()],
            rows: vec![
                row("M5x16", "M5", "16 mm", None),
                row("M5x20", "M5", "20 mm", Some("Screw M5x20 special")),
            ],
            ..Configurations::default()
        })
        .unwrap();
        let pattern = source("{standard} {Size}x{Length} ({config})");
        let named = doc.configurations().row("M5x16").cloned();
        assert_eq!(
            designation(&pattern, doc.state(), named.as_ref()),
            "ISO 4762 M5x16 (M5x16)"
        );
        // 25 mm: no row; the pattern with the options and no row name.
        let values = BTreeMap::from([("Length".to_owned(), OptionValue::Number(25.0))]);
        let outcome = doc.configure(&values).unwrap();
        assert!(outcome.refused.is_none() && outcome.row.is_none());
        assert_eq!(
            designation(&pattern, doc.state(), named.as_ref()),
            "ISO 4762 M5x25 ()"
        );
        // A row's own designation wins.
        let values = BTreeMap::from([("Length".to_owned(), OptionValue::Number(20.0))]);
        doc.configure(&values).unwrap();
        assert_eq!(
            designation(&pattern, doc.state(), None),
            "Screw M5x20 special"
        );
    }
}
