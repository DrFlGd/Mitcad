// SPDX-License-Identifier: MIT
//! A sheet as a `.dwg` drawing (mitcad#728): the DXF export's drawing
//! ([`super::dxf::drawing`]: a layer per pen, texts on `TEXT`, in
//! millimetres of paper) written by Mitcad's own R2000 writer
//! (`mitcad_dwg::write`), with the sheet as the layout's paper.

use super::sheet::Sheet;

/// The sheet's `.dwg` file.
pub fn write(sheet: &Sheet) -> Result<Vec<u8>, String> {
    let options = mitcad_dwg::write::WriteOptions {
        time: None,
        paper: Some([sheet.width, sheet.height]),
    };
    mitcad_dwg::write::drawing(&super::dxf::drawing(sheet), &options).map_err(|e| e.to_string())
}
