// SPDX-License-Identifier: MIT
//! A table as CSV (mitcad#804) in the model, on the mock kernel: a round
//! trip with every unit kind, a `;`-separated file with decimal commas,
//! units in cells, inferred columns and renamed column names, refused
//! cells, and an import refused because a key in use disappears. They use
//! the tables of mitcad#803 and the lookups of mitcad#802 (`XLOOKUP`,
//! `INDEX`, `MATCH`); the CSV alone is tested in `tables_csv::tests`.

use crate::Document;
use crate::document::TableCsvChange;
use crate::expr::Unit;
use crate::test_support::block;
use crate::testing::MockKernel;

const SCREWS: &str = "Size [Text],d [mm],dk [mm],k [mm],Pitch [mm]\r\n\
                      M3,3,5.5,3,0.5\r\n\
                      M4,4,7,4,0.7\r\n\
                      M5,5,8.5,5,0.8\r\n";

fn new_doc() -> Document<MockKernel> {
    Document::new(MockKernel::default())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

/// A parameter's value in millimetres or radians.
fn value(doc: &Document<MockKernel>, name: &str) -> f64 {
    let params = doc.parameters();
    let id = params
        .find(name)
        .unwrap_or_else(|| panic!("no parameter {name}"));
    params.value(id).unwrap()
}

#[test]
fn a_round_trip_with_every_unit_kind() {
    let csv = "Key [Text],n [],d [mm],w [in],a [deg],r [rad],A [mm^2],L [m]\r\n\
               A1,3,5.5,0.25,30,0.5,12.5,1.2\r\n\
               \"B, 2\",,0.1,1,-45,,1e-12,0\r\n";
    let mut first = new_doc();
    let done = first.import_table_csv("Sizes", csv).unwrap();
    assert!(done.created && !done.unchanged);
    assert_eq!(done.summary(), "new table Sizes: 8 columns, 2 rows");
    assert_eq!(first.undo_label(), Some("Import Table Sizes"));
    let (exported, rows) = first.table_csv("Sizes").unwrap();
    assert_eq!(rows, 2);
    assert_eq!(exported, csv);

    // Into another design: the same table.
    let mut second = new_doc();
    second.import_table_csv("Sizes", &exported).unwrap();
    assert_eq!(second.table_csv("Sizes").unwrap().0, csv);

    // Into the same design again: nothing changes, no undo step.
    let depth = first.undo_depth();
    let again = first.import_table_csv("Sizes", &exported).unwrap();
    assert!(again.unchanged && !again.created);
    assert_eq!(again.summary(), "Sizes unchanged: 8 columns, 2 rows");
    assert_eq!(first.undo_depth(), depth);

    // Undo takes the import back.
    first.undo();
    assert!(first.table_csv("Sizes").is_err());
}

#[test]
fn a_semicolon_separated_file_with_decimal_commas() {
    let csv = "Size;d [mm];dk [mm];Pitch [mm]\r\n\
               M3;3;5,5;0,5\r\n\
               M4;4;7;0,7\r\n";
    let mut doc = new_doc();
    let preview = doc.preview_table_csv("Screws", csv).unwrap();
    assert_eq!(preview.columns[0].unit, Unit::TEXT);
    assert!(preview.columns[0].inferred);
    assert!(preview.refused.is_empty(), "{:?}", preview.refused);
    doc.import_table_csv("Screws", csv).unwrap();
    assert_eq!(
        doc.table_csv("Screws").unwrap().0,
        "Size [Text],d [mm],dk [mm],Pitch [mm]\r\nM3,3,5.5,0.5\r\nM4,4,7,0.7\r\n"
    );
}

#[test]
fn units_in_cells_inferred_columns_and_renamed_column_names() {
    let csv = "Size,Head diameter [mm],Length,Count,d [mm],d [mm]\n\
               M3,0.25 in,10 mm,2,3,3.1\n\
               M4,7,0.5 in,3,4,4.1\n";
    let mut doc = new_doc();
    let preview = doc.preview_table_csv("Parts", csv).unwrap();
    let columns: Vec<(&str, Option<&str>, String, bool)> = preview
        .columns
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.renamed.as_deref(),
                c.unit.to_string(),
                c.inferred,
            )
        })
        .collect();
    assert_eq!(
        columns,
        [
            ("Size", None, "Text".to_owned(), true),
            (
                "Head diameter",
                Some("Head_diameter"),
                "mm".to_owned(),
                false
            ),
            ("Length", None, "mm".to_owned(), true),
            ("Count", None, String::new(), true),
            ("d", None, "mm".to_owned(), false),
            ("d", Some("d_1"), "mm".to_owned(), false),
        ]
    );
    let number = |row: usize, column: usize| match &preview.rows[row][column] {
        crate::tables_csv::CsvCell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    };
    // `0.25 in` in an mm column is 6.35; `0.5 in` in an inferred one 12.7.
    assert!(close(number(0, 1), 6.35));
    assert!(close(number(0, 2), 10.0));
    assert!(close(number(1, 2), 12.7));
    assert!(close(number(1, 3), 3.0));
    assert_eq!(
        preview.summary(),
        "new table Parts: 6 columns, 2 rows, 2 renamed, 3 inferred"
    );
    doc.import_table_csv("Parts", csv).unwrap();
    let (exported, _) = doc.table_csv("Parts").unwrap();
    assert!(
        exported.starts_with(
            "Size [Text],Head_diameter [mm],Length [mm],Count [],d [mm],d_1 [mm]\r\nM3,"
        ),
        "{exported}"
    );
}

#[test]
fn refused_cells_refuse_the_import() {
    let csv = "Size [Text],d [mm]\nM3,3\nM4,four\nM5,5 deg\n";
    let mut doc = new_doc();
    let preview = doc.preview_table_csv("Sizes", csv).unwrap();
    let refused: Vec<(usize, &str, &str)> = preview
        .refused
        .iter()
        .map(|r| (r.line, r.column.as_str(), r.text.as_str()))
        .collect();
    assert_eq!(refused, [(3, "d", "four"), (4, "d", "5 deg")]);
    assert_eq!(
        preview.summary(),
        "new table Sizes: 2 columns, 3 rows, 2 cells refused"
    );
    let depth = doc.undo_depth();
    let error = doc.import_table_csv("Sizes", csv).unwrap_err().to_string();
    assert!(
        error.starts_with("2 cells of the file refused, the first on line 3, column d: "),
        "{error}"
    );
    assert!(error.ends_with("; nothing is imported"), "{error}");
    assert_eq!(doc.undo_depth(), depth);
    assert!(doc.table_csv("Sizes").is_err());
    // A file that is no table is refused with the reason.
    let error = doc.preview_table_csv("Sizes", "").unwrap_err().to_string();
    assert_eq!(error, "the CSV file is empty");
}

#[test]
fn an_import_is_refused_when_a_key_in_use_disappears() {
    let mut b = block();
    let doc = &mut b.doc;
    doc.import_table_csv("Screws", SCREWS).unwrap();
    doc.add_parameter_expression("Size", "'M5'", Some(Unit::TEXT), "")
        .unwrap();
    doc.add_parameter_expression(
        "HeadDia",
        "XLOOKUP(Size; Screws.Size; Screws.dk)",
        Some(Unit::MM),
        "",
    )
    .unwrap();
    doc.add_parameter_expression(
        "HeadH",
        "INDEX(Screws.k; MATCH(Size; Screws.Size))",
        Some(Unit::MM),
        "",
    )
    .unwrap();
    assert!(close(value(doc, "HeadDia"), 8.5));
    assert!(close(value(doc, "HeadH"), 5.0));

    // A changed cell: the parameter that reads it follows, in one undo step.
    let changed = SCREWS.replace("M5,5,8.5,5,0.8", "M5,5,9,5,0.8");
    let preview = doc.preview_table_csv("Screws", &changed).unwrap();
    assert_eq!(
        preview.changed,
        [TableCsvChange {
            name: "HeadDia".to_owned(),
            from: "8.5 mm".to_owned(),
            to: "9 mm".to_owned(),
        }]
    );
    assert!(preview.failing.is_empty());
    assert_eq!(
        preview.summary(),
        "Screws replaced: 5 columns, 3 rows, 1 parameter changes"
    );
    let depth = doc.undo_depth();
    doc.import_table_csv("Screws", &changed).unwrap();
    assert!(close(value(doc, "HeadDia"), 9.0));
    assert_eq!(doc.undo_label(), Some("Import Table Screws"));
    assert_eq!(doc.undo_depth(), depth + 1);
    doc.undo();
    assert!(close(value(doc, "HeadDia"), 8.5));

    // M5 gone: both lookups would fail, the import is refused and nothing
    // changes.
    let without = "Size [Text],d [mm],dk [mm],k [mm],Pitch [mm]\r\n\
                   M3,3,5.5,3,0.5\r\n\
                   M4,4,7,4,0.7\r\n";
    let preview = doc.preview_table_csv("Screws", without).unwrap();
    let mut failing: Vec<&str> = preview.failing.iter().map(|f| f.name.as_str()).collect();
    failing.sort_unstable();
    assert_eq!(failing, ["HeadDia", "HeadH"]);
    assert!(
        preview.failing.iter().all(|f| f.error.contains("M5")),
        "{:?}",
        preview.failing
    );
    assert!(preview.changed.is_empty());
    assert!(
        preview.summary().ends_with(", 2 parameters fail"),
        "{}",
        preview.summary()
    );
    let depth = doc.undo_depth();
    let error = doc
        .import_table_csv("Screws", without)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("the table would make "), "{error}");
    assert!(error.contains("HeadDia"), "{error}");
    assert!(error.ends_with("; nothing is imported"), "{error}");
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.table_csv("Screws").unwrap().1, 3);
    assert!(close(value(doc, "HeadDia"), 8.5));

    // A used column dropped fails the same way.
    let preview = doc
        .preview_table_csv("Screws", "Size [Text],d [mm]\nM5,5\n")
        .unwrap();
    assert!(preview.failing.iter().any(|f| f.name == "HeadDia"));

    // A table cannot take a parameter's name, nor a name no table can have.
    let error = doc
        .preview_table_csv("Size", SCREWS)
        .unwrap_err()
        .to_string();
    assert!(error.contains("is the name of a parameter"), "{error}");
    let error = doc.import_table_csv("2nd", SCREWS).unwrap_err().to_string();
    assert!(error.contains("not a valid table name"), "{error}");
}
