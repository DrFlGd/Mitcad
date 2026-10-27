// SPDX-License-Identifier: MIT
//! Prints what the reader finds in a drawing: its version and sections,
//! header variables, classes, object counts, objects that could not be
//! read, the account of its 3D content and the plan of components, and
//! each B-rep record's parse.
//!
//!     mitcad-dwg-inspect <file.dwg> [--write-test <dir>]

use mitcad_dwg::scene::{ItemKind, Scene};
use mitcad_dwg::{Version, solid, testfile};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--write-test" {
        let dir = std::path::Path::new(&args[1]);
        for v in Version::ALL {
            let path = dir.join(format!("test-{}.dwg", v.name()));
            if let Err(e) = std::fs::write(&path, testfile::write(&testfile::test_drawing(), v)) {
                eprintln!("{}: {e}", path.display());
                std::process::exit(1);
            }
        }
        return;
    }
    // An object's stored bytes in hexadecimal and as bits.
    if args.len() == 3 && args[0] == "--dump" {
        let bytes = std::fs::read(&args[1]).unwrap_or_default();
        let handle = u64::from_str_radix(&args[2], 16).unwrap_or(0);
        match mitcad_dwg::object_bytes(&bytes, handle) {
            Ok((v, b)) => {
                println!("{v}, {} bytes", b.len());
                for (i, chunk) in b.chunks(8).enumerate() {
                    let hex: Vec<String> = chunk.iter().map(|x| format!("{x:02X}")).collect();
                    let bits: Vec<String> = chunk.iter().map(|x| format!("{x:08b}")).collect();
                    println!("{:5} {:24} {}", i * 64, hex.join(" "), bits.join(" "));
                }
            }
            Err(e) => eprintln!("{e}"),
        }
        return;
    }
    // A B-rep record (as MITCAD_DWG_DUMP_DIR writes them) record by record.
    // (An optional third argument: how long a record's text may get.)
    if (args.len() == 2 || args.len() == 3) && args[0] == "--asm" {
        let max = args.get(2).and_then(|m| m.parse().ok()).unwrap_or(400);
        let bytes = std::fs::read(&args[1]).unwrap_or_default();
        let form = if args[1].ends_with(".txt") {
            solid::Form::Text
        } else {
            solid::Form::Binary
        };
        match solid::parse(form, &bytes) {
            Ok(asm) => {
                println!("{:?}", asm.header);
                for i in 0..asm.records.len() {
                    println!("{}", asm.record_text(i, max));
                }
            }
            Err(e) => eprintln!("{e}"),
        }
        return;
    }
    // The data storage section's bytes.
    if args.len() == 3 && args[0] == "--data-storage" {
        let bytes = std::fs::read(&args[1]).unwrap_or_default();
        if let Ok(Some(d)) = mitcad_dwg::data_storage_bytes(&bytes) {
            let _ = std::fs::write(&args[2], d);
        }
        return;
    }
    let Some(path) = args.first() else {
        eprintln!("usage: mitcad-dwg-inspect <file.dwg> | --write-test <dir>");
        std::process::exit(2);
    };
    let file = match mitcad_dwg::DwgFile::open(std::path::Path::new(path)) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "version {} maintenance {} codepage {}",
        file.version, file.maintenance, file.codepage
    );
    for (name, size) in &file.sections {
        println!("section {name}: {size} bytes");
    }
    println!("header {:?}", file.header);
    for c in &file.classes {
        println!(
            "class {} {} ({}, {})",
            c.number,
            c.dxf_name,
            c.cpp_name,
            if c.entity { "entity" } else { "object" }
        );
    }
    for (name, n) in file.type_counts() {
        println!("objects {name}: {n}");
    }
    for (handle, kind, why) in file.failed.iter().take(50) {
        println!("failed {handle:X} {kind}: {why}");
    }
    if file.failed.len() > 50 {
        println!("failed: {} more", file.failed.len() - 50);
    }
    println!("data storage records: {}", file.stored_breps.len());
    let scene = Scene::of(&file);
    println!(
        "units {:?} ({} mm per unit{}), extents {:?}",
        scene.units,
        scene.unit_mm,
        if scene.units_assumed { ", assumed" } else { "" },
        scene.extents
    );
    let a = &scene.account;
    println!("entities {:?}", a.entities);
    println!("taken {:?}", a.taken);
    println!("left out {:?}", a.left_out);
    for w in &a.warnings {
        println!("warning: {w}");
    }
    let plan = scene.plan();
    let occurrences = plan.occurrences();
    for (i, n) in plan.nodes.iter().enumerate() {
        println!(
            "component {i} {:?}: {} bodies, {} occurrences placed in it, placed {} times",
            n.name,
            n.bodies.len(),
            n.children.len(),
            occurrences[i]
        );
    }
    for w in &plan.warnings {
        println!("plan: {w}");
    }
    let options = mitcad_asm::convert::Options {
        scale: scene.unit_mm,
        ..Default::default()
    };
    for b in scene.blocks.values() {
        for item in &b.items {
            if let ItemKind::Brep {
                form,
                bytes,
                entity,
            } = &item.kind
            {
                // The records as files, to read (MITCAD_DWG_DUMP_DIR).
                if let Ok(dir) = std::env::var("MITCAD_DWG_DUMP_DIR") {
                    let ext = if *form == solid::Form::Text {
                        "txt"
                    } else {
                        "bin"
                    };
                    let path = std::path::Path::new(&dir).join(format!("{:X}.{ext}", item.handle));
                    let _ = std::fs::write(path, bytes);
                }
                match solid::parse(*form, bytes) {
                    Ok(asm) => {
                        let bodies = mitcad_asm::convert::convert_file(&asm, &options);
                        println!(
                            "{entity} {:X} in {}: {} form, {} ({}), {} records, {} bodies, {} faces, {} issues",
                            item.handle,
                            b.name,
                            form.name(),
                            asm.header.asm_version,
                            asm.header.version,
                            asm.records.len(),
                            bodies.len(),
                            bodies.iter().map(|b| b.body.faces.len()).sum::<usize>(),
                            bodies.iter().map(|b| b.issues.len()).sum::<usize>()
                        );
                        for issue in bodies.iter().flat_map(|b| &b.issues).take(5) {
                            println!("  issue: {issue}");
                        }
                    }
                    Err(e) => println!(
                        "{entity} {:X} in {}: {} form: {e}",
                        item.handle,
                        b.name,
                        form.name()
                    ),
                }
            }
        }
    }
}
