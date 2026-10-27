// SPDX-License-Identifier: MIT
//! A drawing written as a `.dwg` file (mitcad#728) on the mock kernel: the
//! file reads back with Mitcad's DWG reader and holds what the DXF export
//! of the same sheet holds, layer by layer and entity by entity.

use serde_json::{Value, json};

use mitcad_dxf::{Geometry, Point2, Text};

use mitcad_model::Document;
use mitcad_model::testing::MockKernel;
use mitcad_model_api::DocumentApi;

const R: &str = "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}";

fn command(doc: &mut Document<MockKernel>, command: Value) -> Value {
    let result = doc
        .command(&command.to_string())
        .unwrap_or_else(|e| panic!("{command}: {e}"));
    serde_json::from_str(&result).unwrap()
}

/// A fresh folder of the test's own beside the test program.
fn own_folder(name: &str) -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("the test program's path");
    let dir = exe
        .parent()
        .expect("the test program's folder")
        .join("test-folders")
        .join(format!("{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A 40 x 30 x 20 block; a drawing with the ISO template (title block,
/// revision table), a front view at 1:2 and a revision.
fn bracket() -> Document<MockKernel> {
    let mut doc = Document::new(MockKernel::default());
    doc.kernel().measures.set(true);
    command(
        &mut doc,
        json!([
            {"cmd": "sketch.create"},
            {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [0, 0], "width": 40, "height": 30},
            {"cmd": "add_feature", "def": {"type": "extrude", "profiles": [{"sketch": "F1", "region": R}],
              "extent": {"type": "distance", "distance": 20}, "operation": "new_body"}},
        ]),
    );
    command(&mut doc, json!({"cmd": "add_drawing", "template": "iso"}));
    command(
        &mut doc,
        json!({"cmd": "add_drawing_view", "drawing": "D1", "type": "base", "direction": "front",
               "at": [150, 150], "scale": "1:2"}),
    );
    command(
        &mut doc,
        json!({"cmd": "add_drawing_revision", "drawing": "D1", "description": "First release"}),
    );
    doc
}

/// The DXF drawing's entities with each text of several lines as a text
/// per line, as the DWG writer stores them.
fn per_line(entities: &[mitcad_dxf::Entity]) -> Vec<mitcad_dxf::Entity> {
    let mut out = Vec::new();
    for e in entities {
        let Geometry::Text(t) = &e.geometry else {
            out.push(e.clone());
            continue;
        };
        let down = Point2::polar(t.rotation).perp() * (-t.height * 5.0 / 3.0);
        for (i, line) in t.content.split('\n').enumerate() {
            let mut piece = e.clone();
            piece.geometry = Geometry::Text(Text {
                position: t.position + down * i as f64,
                content: line.to_owned(),
                ..t.clone()
            });
            out.push(piece);
        }
    }
    out
}

/// The same kind of geometry at the same places (1e-6 mm), the same text.
fn same(a: &Geometry, b: &Geometry) -> bool {
    if std::mem::discriminant(a) != std::mem::discriminant(b) {
        return false;
    }
    if let (Geometry::Text(x), Geometry::Text(y)) = (a, b)
        && (x.content != y.content || (x.height - y.height).abs() > 1e-6)
    {
        return false;
    }
    let (p, q) = (a.extent_points(), b.extent_points());
    p.len() == q.len() && p.iter().zip(&q).all(|(u, v)| u.distance(*v) < 1e-6)
}

#[test]
fn a_drawing_is_written_as_dwg_with_what_its_dxf_has() {
    let mut doc = bracket();
    let dir = own_folder("dwg-export");
    let dwg = dir.join("sheet.dwg").to_string_lossy().into_owned();
    let dxf = dir.join("sheet.dxf").to_string_lossy().into_owned();
    let result = command(
        &mut doc,
        json!({"cmd": "export_drawing", "drawing": "D1", "path": dwg, "title": "Bracket"}),
    );
    assert_eq!(result["format"], "dwg", "{result}");
    assert_eq!(result["files"], json!([dwg]));
    assert_eq!(result["sheets"], 1);
    command(
        &mut doc,
        json!({"cmd": "export_drawing", "drawing": "D1", "path": dxf, "title": "Bracket"}),
    );

    let bytes = std::fs::read(&dwg).unwrap();
    assert_eq!(&bytes[..6], b"AC1015");
    let file = mitcad_dwg::read(&bytes).unwrap();
    assert!(file.failed.is_empty(), "{:?}", file.failed);
    assert!(file.warnings.is_empty(), "{:?}", file.warnings);
    let back = mitcad_dwg::flat::drawing(&file);
    let expected = mitcad_dxf::read_file(&dxf).unwrap();
    assert_eq!(back.units, mitcad_dxf::Units::Millimeters);
    for layer in ["VISIBLE", "THIN", "FRAME", "TEXT"] {
        assert!(back.layer(layer).is_some(), "{layer}");
    }
    for layer in &expected.layers {
        assert!(back.layer(&layer.name).is_some(), "{}", layer.name);
    }
    // Entity by entity, in the same order on the same layers.
    let expected = per_line(&expected.entities);
    assert_eq!(back.entities.len(), expected.len());
    for (i, (b, e)) in back.entities.iter().zip(&expected).enumerate() {
        assert_eq!(b.layer, e.layer, "entity {i}");
        assert!(same(&b.geometry, &e.geometry), "entity {i}: {b:?} {e:?}");
    }
    let texts: Vec<&str> = back
        .entities
        .iter()
        .filter_map(|e| match &e.geometry {
            Geometry::Text(t) => Some(t.content.as_str()),
            _ => None,
        })
        .collect();
    for text in ["MATERIAL", "First release", "Bracket", "DESCRIPTION"] {
        assert!(texts.iter().any(|t| t.contains(text)), "{text}: {texts:?}");
    }
    // The sheet is the layout's paper and the limits: an A3 sheet.
    let h = file.header.as_ref().unwrap();
    let (lo, hi) = (h.extmin.unwrap(), h.extmax.unwrap());
    assert!(lo[0] >= 0.0 && lo[1] >= 0.0 && hi[0] <= 420.0 && hi[1] <= 297.0);

    // "format" names it too; an unknown one is refused with the list.
    let other = dir.join("sheet.out").to_string_lossy().into_owned();
    let result = command(
        &mut doc,
        json!({"cmd": "export_drawing", "drawing": "D1", "path": other, "format": "DWG"}),
    );
    assert_eq!(result["format"], "dwg");
    assert_eq!(&std::fs::read(&other).unwrap()[..6], b"AC1015");
    let error = doc
        .command(
            &json!({"cmd": "export_drawing", "drawing": "D1", "path": other, "format": "dwf"})
                .to_string(),
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("pdf, svg, dxf or dwg"), "{error}");
    std::fs::remove_dir_all(&dir).ok();
}
