// SPDX-License-Identifier: MIT
//! The `.rvt` import on the corpus's projects under `MITCAD_RVT_CORPUS`
//! (each test skipped without its file), as one test binary: each module
//! was a binary of its own, and each linked the import and the model, which
//! a change in the model rebuilt three times (mitcad#831).

mod rvt_corpus;
mod rvt_mullion_trims;
mod rvt_rails;
