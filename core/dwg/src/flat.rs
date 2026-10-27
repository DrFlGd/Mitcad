// SPDX-License-Identifier: MIT
//! The 2D content of a drawing's model space as a [`mitcad_dxf::Drawing`]:
//! its layers and its lines, arcs, circles, points, ellipses and texts
//! (what [`crate::write`] writes, mitcad#728), for the tests that read a
//! written drawing back. Other entities are left out.

use std::collections::BTreeMap;

use mitcad_dxf::{Drawing, Geometry, Layer, Point2, Text, Units};

use crate::DwgFile;
use crate::objects::{Color, EntityData, Kind};

/// The model space's 2D entities in handle order, with their layers.
pub fn drawing(file: &DwgFile) -> Drawing {
    let units = file
        .header
        .as_ref()
        .and_then(|h| h.insunits)
        .and_then(|code| Units::from_code(i32::from(code)))
        .unwrap_or_default();
    let mut out = Drawing::new(units);
    out.version = Some(file.version.id().to_owned());
    let mut names = BTreeMap::new();
    for o in file.objects.values() {
        if let Kind::Layer(l) = &o.kind {
            let mut layer = Layer::new(l.name.clone());
            if let Color::Index(i) = l.color {
                layer.color = i16::from(i);
            }
            layer.visible = !l.off;
            layer.frozen = l.frozen;
            names.insert(o.handle, l.name.clone());
            out.layers.push(layer);
        }
    }
    let p = |q: [f64; 3]| Point2::new(q[0], q[1]);
    for o in file.objects.values() {
        let Kind::Entity(common, data) = &o.kind else {
            continue;
        };
        if common.entmode != 2 {
            continue;
        }
        let geometry = match data {
            EntityData::Line { start, end } => Geometry::Line {
                start: p(*start),
                end: p(*end),
            },
            EntityData::Circle {
                center,
                radius,
                angles: None,
            } => Geometry::Circle {
                center: p(*center),
                radius: *radius,
            },
            EntityData::Circle {
                center,
                radius,
                angles: Some((start, end)),
            } => Geometry::arc(p(*center), *radius, *start, *end),
            EntityData::Point(q) => Geometry::Point(p(*q)),
            EntityData::Ellipse {
                center,
                major,
                ratio,
                start,
                end,
            } => Geometry::Ellipse {
                center: p(*center),
                major_axis: p(*major),
                ratio: *ratio,
                start_param: *start,
                end_param: *end,
            },
            EntityData::Text {
                position,
                height,
                rotation,
                value,
            } => Geometry::Text(Text {
                position: p(*position),
                height: *height,
                rotation: *rotation,
                content: if file.version < crate::Version::R2007 {
                    crate::write::decode(value)
                } else {
                    value.clone()
                },
            }),
            _ => continue,
        };
        let layer = common
            .layer
            .and_then(|h| names.get(&h).cloned())
            .unwrap_or_else(|| "0".to_owned());
        out.push(layer, geometry);
    }
    out
}
