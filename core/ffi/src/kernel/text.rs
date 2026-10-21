// SPDX-License-Identifier: MIT
//! Glyph outlines for sketch text (`geometry/include/mitcad/geometry/text.hpp`).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// One character: `contours` pieces per contour, `points` control
    /// points per piece, then x, y of every point (ems).
    struct TextGlyph {
        advance: f64,
        contours: Vec<u32>,
        points: Vec<u32>,
        coordinates: Vec<f64>,
    }

    struct TextOutlines {
        family: String,
        fallback: bool,
        ascender: f64,
        descender: f64,
        line_spacing: f64,
        glyphs: Vec<TextGlyph>,
    }

    /// An installed font's file (mitcad#473).
    struct FontFileInfo {
        found: bool,
        family: String,
        path: String,
        index: u32,
        synthetic_italic: bool,
    }

    unsafe extern "C++" {
        include!("bridge/text.hpp");

        /// The glyph outlines of a text in a font family (empty for the
        /// bundled font).
        fn text_outlines(
            family: &str,
            bold: bool,
            italic: bool,
            text: &str,
        ) -> Result<TextOutlines>;

        /// The file of an installed font family and style.
        fn font_file(family: &str, bold: bool, italic: bool) -> FontFileInfo;
    }
}

use mitcad_model::{FontGlyphs, FontRequest, FontSource, Glyph, KernelError};

/// The kernel's `font_source` (mitcad#473).
pub fn font_source(request: &FontRequest<'_>) -> Result<FontSource, KernelError> {
    let file = ffi::font_file(request.family, request.bold, request.italic);
    if !file.found {
        return Err(KernelError::failed(format!(
            "the font {:?} is not installed",
            request.family
        )));
    }
    Ok(FontSource {
        family: file.family,
        path: file.path,
        index: file.index,
        synthetic_italic: file.synthetic_italic,
    })
}

/// The kernel's `font_glyphs`.
pub fn font_glyphs(request: &FontRequest<'_>) -> Result<FontGlyphs, KernelError> {
    let outlines = ffi::text_outlines(request.family, request.bold, request.italic, request.text)
        .map_err(|e| KernelError::failed(e.what()))?;
    let glyphs = outlines
        .glyphs
        .iter()
        .map(|g| {
            let mut points = g.points.iter();
            let mut coordinates = g.coordinates.chunks(2).map(|c| [c[0], c[1]]);
            let contours = g
                .contours
                .iter()
                .map(|pieces| {
                    (0..*pieces)
                        .map(|_| {
                            let n = points.next().copied().unwrap_or(0);
                            (0..n).filter_map(|_| coordinates.next()).collect()
                        })
                        .collect()
                })
                .collect();
            Glyph {
                advance: g.advance,
                contours,
            }
        })
        .collect();
    Ok(FontGlyphs {
        family: outlines.family,
        fallback: outlines.fallback,
        ascender: outlines.ascender,
        descender: outlines.descender,
        cap_height: 0.0,
        line_spacing: outlines.line_spacing,
        glyphs,
    })
}
