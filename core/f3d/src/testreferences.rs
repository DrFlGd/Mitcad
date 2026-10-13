// SPDX-License-Identifier: MIT
//! Synthetic designs that place components of other documents, with their
//! local reference folder (mitcad#196), for tests here and in the import,
//! the command-line tool and the application (no real designs needed).
//!
//! `Assembly v1.f3d` places the part `Part v2` once and the sub-assembly
//! `Holder v1` twice; `Holder v1` places `Part v2` once. The part is the
//! 1 cm cube of [`crate::testdata::cube_blob`], stored without a design
//! (its bodies only); the assemblies have design streams with occurrence
//! trees and no bodies. Next to the assembly, `Assembly v1.refs/` holds
//! the other two files and `references.json`. [`package_file`] puts the
//! three into one `.f3z` package instead (mitcad#229).

use crate::design::classes::{COMPONENT, OCCURRENCE, OCCURRENCE_CONTAINER};
use crate::design::writer::{DesignWriter, WriterExternal};
use crate::testdata::cube_blob;
use crate::zip::{Method, Writer};

/// The root class of the design streams.
const ROOT: &str = "98542EB9-A4F2-4137-A808-DBB5B3CD6159";

/// A document of the test assembly.
#[derive(Clone, Copy, Debug)]
pub struct TestDocument {
    /// The name the reference folder gives it.
    pub name: &'static str,
    pub version: u32,
    pub lineage: &'static str,
    pub urn: &'static str,
    /// The GUID its `Manifest.dat` lists, which designs that refer to it
    /// keep after its URN.
    pub document_guid: &'static str,
    /// The GUID occurrences of its components carry.
    pub occurrence_guid: &'static str,
    /// The key the designs that refer to it give it.
    pub key: u64,
}

pub const ASSEMBLY: TestDocument = TestDocument {
    name: "Assembly",
    version: 1,
    lineage: "urn:mitcad.test:dm.lineage:assembly",
    urn: "urn:mitcad.test:fs.file:vf.assembly?version=1",
    document_guid: "5d7c3a10-0000-4000-8000-000000000001",
    occurrence_guid: "5d7c3a10-0000-4000-8000-0000000000a1",
    key: 0x1111_0000_0000_0001,
};

pub const HOLDER: TestDocument = TestDocument {
    name: "Holder",
    version: 1,
    lineage: "urn:mitcad.test:dm.lineage:holder",
    urn: "urn:mitcad.test:fs.file:vf.holder?version=1",
    document_guid: "5d7c3a10-0000-4000-8000-000000000002",
    occurrence_guid: "5d7c3a10-0000-4000-8000-0000000000a2",
    key: 0x1111_0000_0000_0002,
};

pub const PART: TestDocument = TestDocument {
    name: "Part",
    version: 2,
    lineage: "urn:mitcad.test:dm.lineage:part",
    urn: "urn:mitcad.test:fs.file:vf.part?version=2",
    document_guid: "5d7c3a10-0000-4000-8000-000000000003",
    occurrence_guid: "5d7c3a10-0000-4000-8000-0000000000a3",
    key: 0x1111_0000_0000_0003,
};

/// Where the assembly places its occurrences (translations, cm): the
/// part, then the holder twice.
pub const ASSEMBLY_PLACEMENTS: [(&TestDocument, [f64; 3]); 3] = [
    (&PART, [5.0, 0.0, 0.0]),
    (&HOLDER, [0.0, 5.0, 0.0]),
    (&HOLDER, [0.0, 10.0, 0.0]),
];

/// Where the holder places the part (cm).
pub const HOLDER_PLACEMENTS: [(&TestDocument, [f64; 3]); 1] = [(&PART, [0.0, 0.0, 2.0])];

/// The holder's root component (the occurrences of other documents name
/// the part's root by the same id: the part has no design).
const HOLDER_ROOT: u64 = 2;

/// The file name of a document in the reference folder (`Part v2.f3d`).
pub fn file_name(doc: &TestDocument) -> String {
    format!("{} v{}.f3d", doc.name, doc.version)
}

fn s16(s: &str) -> Vec<u8> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut v = (units.len() as u32).to_le_bytes().to_vec();
    v.extend(units.iter().flat_map(|u| u.to_le_bytes()));
    v
}

fn r(id: u64) -> Vec<u8> {
    let mut v = vec![1];
    v.extend(id.to_le_bytes());
    v.extend([0, 0]);
    v
}

/// `01 id 01 key 00 str8(guid) 00`: a component of another document.
fn context_ref(id: u64, key: u64) -> Vec<u8> {
    let mut v = vec![1];
    v.extend(id.to_le_bytes());
    v.push(1);
    v.extend(key.to_le_bytes());
    v.push(0);
    v.extend((COMPONENT.len() as u32).to_le_bytes());
    v.extend(COMPONENT.as_bytes());
    v.push(0);
    v
}

/// The design streams of an assembly named `name` that places the root
/// components of other documents.
/// `root`: its root component's object id (the assembly's is no object
/// of the documents it refers to, so their ids are none of its own).
fn assembly_streams(
    name: &str,
    root: u64,
    placements: &[(&TestDocument, [f64; 3])],
) -> (Vec<u8>, Vec<u8>) {
    let mut d = DesignWriter::default();
    for g in [ROOT, COMPONENT, OCCURRENCE, OCCURRENCE_CONTAINER] {
        d.class(g, if g == ROOT { "" } else { ROOT }, 1);
    }
    d.obj(root, COMPONENT, [&[0u8, 0][..], &s16(name)].concat());
    // The root occurrence (no container) names the root component.
    d.obj(3, OCCURRENCE, [&[0u8, 0][..], &r(root), &[1]].concat());
    let mut id = 10;
    for (doc, at) in placements {
        let matrix = [
            1.0, 0.0, 0.0, at[0], 0.0, 1.0, 0.0, at[1], 0.0, 0.0, 1.0, at[2], 0.0, 0.0, 0.0, 1.0,
        ];
        let body = [
            &[0u8, 0][..],
            &context_ref(HOLDER_ROOT, doc.key),
            &[0],
            &matrix
                .iter()
                .flat_map(|x: &f64| x.to_le_bytes())
                .collect::<Vec<u8>>(),
            &0u32.to_le_bytes(),
            &s16(doc.occurrence_guid),
            &r(id + 1),
        ]
        .concat();
        d.obj(id, OCCURRENCE, body);
        d.obj(
            id + 1,
            OCCURRENCE_CONTAINER,
            [&[0u8, 0][..], &r(id), &r(root)].concat(),
        );
        id += 2;
    }
    let mut seen = Vec::new();
    for (doc, _) in placements {
        if seen.contains(&doc.key) {
            continue;
        }
        seen.push(doc.key);
        d.externals.push(WriterExternal {
            key: doc.key,
            guids: [doc.occurrence_guid.into(), doc.document_guid.into()],
            urn: doc.urn.into(),
            document_guid: doc.document_guid.into(),
        });
    }
    d.streams()
}

/// A `Manifest.dat` that lists the document's GUID (and `others`: an
/// assembly's manifest lists those of the documents it refers to too).
fn manifest(document_guid: &str, others: &[&str]) -> Vec<u8> {
    let mut v = [
        &3u32.to_le_bytes()[..],
        b"f3d",
        &s16("A Mitcad test document"),
        &s16(document_guid),
    ]
    .concat();
    for g in others {
        v.extend(s16(g));
    }
    v
}

/// An `.f3d` archive: its manifest, design streams and body blobs.
fn archive(
    document_guid: &str,
    others: &[&str],
    streams: Option<(Vec<u8>, Vec<u8>)>,
    blobs: &[(&str, Vec<u8>)],
) -> Vec<u8> {
    let mut w = Writer::new();
    let mut add = |name: &str, data: &[u8]| {
        w.add(name, data, Method::Stored)
            .expect("a small entry is written");
    };
    add("Manifest.dat", &manifest(document_guid, others));
    if let Some((meta, bulk)) = streams {
        add(
            "FusionAssetName[Active]/FusionDesignSegmentType1/MetaStream.dat",
            &meta,
        );
        add(
            "FusionAssetName[Active]/FusionDesignSegmentType1/BulkStream.dat",
            &bulk,
        );
    }
    for (name, data) in blobs {
        add(
            &format!("FusionAssetName[Active]/Breps.BlobParts/BREP.{name}.smb"),
            data,
        );
    }
    w.finish().expect("the archive is written")
}

/// `Part v2.f3d`: the cube, without a design.
pub fn part_file() -> Vec<u8> {
    archive(
        PART.document_guid,
        &[],
        None,
        &[("mitcad-test-part", cube_blob())],
    )
}

/// `Holder v1.f3d`: places the part.
pub fn holder_file() -> Vec<u8> {
    holder_listing(&[])
}

/// The holder whose manifest lists the GUIDs `others` too.
fn holder_listing(others: &[&str]) -> Vec<u8> {
    archive(
        HOLDER.document_guid,
        others,
        Some(assembly_streams(
            HOLDER.name,
            HOLDER_ROOT,
            &HOLDER_PLACEMENTS,
        )),
        &[],
    )
}

/// `Assembly v1.f3d`: places the part and the holder twice.
pub fn assembly_file() -> Vec<u8> {
    assembly_listing(&[])
}

/// The assembly whose manifest lists the GUIDs `others` too.
fn assembly_listing(others: &[&str]) -> Vec<u8> {
    archive(
        ASSEMBLY.document_guid,
        others,
        Some(assembly_streams(ASSEMBLY.name, 4, &ASSEMBLY_PLACEMENTS)),
        &[],
    )
}

/// The `.f3d` entries of the test package (mitcad#229): the assembly, the
/// holder and the part, named by GUIDs so that the assembly's sorts first.
pub const PACKAGE_ENTRIES: [(&TestDocument, &str); 3] = [
    (&ASSEMBLY, "0a5e3b10-0000-4000-8000-0000000000f1.f3d"),
    (&HOLDER, "5b6c7d20-0000-4000-8000-0000000000f2.f3d"),
    (&PART, "9c8d7e30-0000-4000-8000-0000000000f3.f3d"),
];

/// The name the test package's `DesignDescription.json` gives a document.
pub fn package_name(doc: &TestDocument) -> String {
    format!("Packaged {}", doc.name.to_lowercase())
}

/// An `.f3z` package of the test assembly with the documents it refers to
/// (mitcad#229). The assembly's manifest lists the GUIDs of the holder and
/// the part too, and the holder's that of the part (as a package's
/// assemblies do), so a design found by those GUIDs alone could be the
/// assembly. `described`: with `Manifest.json` (the root) and
/// `DesignDescription.json` (each document's URN, entry and name).
pub fn package_file(described: bool) -> Vec<u8> {
    let mut w = Writer::new();
    let mut add = |name: &str, data: &[u8]| {
        w.add(name, data, Method::Stored)
            .expect("a small entry is written");
    };
    for (doc, entry) in PACKAGE_ENTRIES {
        let data = match doc.urn {
            u if u == ASSEMBLY.urn => assembly_listing(&[HOLDER.document_guid, PART.document_guid]),
            u if u == HOLDER.urn => holder_listing(&[PART.document_guid]),
            _ => part_file(),
        };
        add(entry, &data);
    }
    if described {
        add(
            "Manifest.json",
            format!("{{\"root\":\"{}\"}}", PACKAGE_ENTRIES[0].1).as_bytes(),
        );
        let objects: Vec<String> = PACKAGE_ENTRIES
            .iter()
            .enumerate()
            .map(|(i, (doc, entry))| {
                format!(
                    "{{\"id\":{},\"version\":{},\"about\":\"{}\",\"lineage\":\"{}\",\
                     \"relativePath\":\"{entry}\",\"friendlyName\":\"{}\",\"displayName\":\"{}\"}}",
                    i + 1,
                    doc.version,
                    doc.urn,
                    doc.lineage,
                    package_name(doc),
                    package_name(doc)
                )
            })
            .collect();
        add(
            "DesignDescription.json",
            format!(
                "{{\"name\":\"A Mitcad test package\",\"version\":\"0.1\",\"designDescription\":\
                 {{\"id\":\"0\",\"designGraphs\":[{{\"rootIds\":[1],\"designObjects\":[{}]}}]}}}}",
                objects.join(",")
            )
            .as_bytes(),
        );
    }
    w.finish().expect("the package is written")
}

fn reference_entry(doc: &TestDocument, children: &[String]) -> String {
    let file = if doc.urn == ASSEMBLY.urn {
        String::new()
    } else {
        format!(", \"file\": \"{}\"", file_name(doc))
    };
    format!(
        "{{\"name\": \"{}\", \"lineage\": \"{}\", \"versionId\": \"{}\", \"version\": {}{file}, \
         \"children\": [{}]}}",
        doc.name,
        doc.lineage,
        doc.urn,
        doc.version,
        children.join(", ")
    )
}

/// The reference folder's `references.json`.
pub fn references_json() -> String {
    let part = reference_entry(&PART, &[]);
    let holder = reference_entry(&HOLDER, std::slice::from_ref(&part));
    let design = reference_entry(&ASSEMBLY, &[part, holder]);
    format!(
        "{{\"format\": \"mitcad-test-references\", \"format_version\": 1, \"design\": {design}, \
         \"files\": [\"{}\", \"{}\"]}}\n",
        file_name(&HOLDER),
        file_name(&PART)
    )
}

/// The files of the test: the assembly and its reference folder, as
/// (path relative to a folder, contents).
pub fn files() -> Vec<(String, Vec<u8>)> {
    let refs = format!("{} v{}.refs", ASSEMBLY.name, ASSEMBLY.version);
    vec![
        (file_name(&ASSEMBLY), assembly_file()),
        (format!("{refs}/{}", file_name(&HOLDER)), holder_file()),
        (format!("{refs}/{}", file_name(&PART)), part_file()),
        (
            format!("{refs}/references.json"),
            references_json().into_bytes(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::F3dFile;
    use crate::design::{decode_streams, manifest_guids, referenced_documents};

    #[test]
    fn the_assembly_names_its_documents() {
        let doc = F3dFile::from_bytes(assembly_file()).unwrap();
        let listed = referenced_documents(&doc).unwrap();
        let summary: Vec<(String, Option<u32>, String, usize)> = listed
            .iter()
            .map(|r| {
                (
                    r.urn.clone(),
                    r.version,
                    r.document_guid.clone(),
                    r.occurrences,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    PART.urn.to_owned(),
                    Some(2),
                    PART.document_guid.to_owned(),
                    1
                ),
                (
                    HOLDER.urn.to_owned(),
                    Some(1),
                    HOLDER.document_guid.to_owned(),
                    2
                ),
            ]
        );
        let dump = decode_streams(&doc, "Assembly v1.f3d")
            .unwrap()
            .dump
            .unwrap();
        let occurrences = dump.occurrences.unwrap();
        assert_eq!(occurrences.len(), 3);
        assert!(
            occurrences
                .iter()
                .all(|o| o.is_referenced_component == Some(true))
        );
        let f3d = occurrences[1].f3d.as_ref().unwrap();
        assert_eq!(f3d.external_document.as_deref(), Some(HOLDER.urn));
        assert_eq!(
            f3d.guids.as_deref(),
            Some(&[HOLDER.occurrence_guid.to_owned()][..])
        );
        assert_eq!(occurrences[2].transform.unwrap()[1][3], 10.0);
        assert_eq!(
            dump.document.and_then(|d| d.root_component).as_deref(),
            Some("Assembly")
        );

        let part = F3dFile::from_bytes(part_file()).unwrap();
        assert_eq!(manifest_guids(&part), [PART.document_guid]);
        assert!(referenced_documents(&part).unwrap().is_empty());
        assert!(references_json().contains("\"file\": \"Part v2.f3d\""));
    }

    #[test]
    fn the_package_names_its_root_and_documents() {
        let dir = std::env::temp_dir().join(format!("mitcad-package-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Package.f3z");
        std::fs::write(&path, package_file(true)).unwrap();
        let p = crate::design::package(&path).unwrap();
        let labels: Vec<&str> = p.documents.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Package.f3z!0a5e3b10",
                "Package.f3z!5b6c7d20",
                "Package.f3z!9c8d7e30"
            ]
        );
        assert_eq!(p.root.as_deref(), Some("Package.f3z!0a5e3b10"));
        let described: Vec<(&str, &str, Option<&str>)> = p
            .described
            .iter()
            .map(|(l, d)| (l.as_str(), d.urn.as_str(), d.name.as_deref()))
            .collect();
        assert_eq!(
            described[1],
            (labels[1], HOLDER.urn, Some("Packaged holder"))
        );
        assert_eq!(described[2], (labels[2], PART.urn, Some("Packaged part")));
        // The assembly's manifest lists the GUIDs of the others too.
        assert_eq!(
            manifest_guids(&p.documents[0].1),
            [
                ASSEMBLY.document_guid,
                HOLDER.document_guid,
                PART.document_guid
            ]
        );

        std::fs::write(&path, package_file(false)).unwrap();
        let p = crate::design::package(&path).unwrap();
        assert_eq!(p.documents.len(), 3);
        assert!(p.root.is_none() && p.described.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
