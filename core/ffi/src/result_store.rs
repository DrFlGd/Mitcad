// SPDX-License-Identifier: MIT
//! The result store for C++ (P7d, `mitcad_model::store`): the application
//! and `mitcad-cli` give a document a store folder; recomputes then take
//! results from it, and `persist_results` writes what was costly to
//! evaluate.

use std::path::Path;
use std::time::{Duration, Instant};

use mitcad_model::BodyUid;
use mitcad_model::store::{self, GcReport, PersistReport, ResultStore};
use serde_json::json;

use crate::Document;

fn gc_json(report: &GcReport) -> String {
    json!({"files": report.files, "bytes": report.bytes, "removed": report.removed,
           "removed_bytes": report.removed_bytes})
    .to_string()
}

pub fn result_store_gc(dir: &str, budget_mb: u64) -> String {
    gc_json(&store::gc(
        Path::new(dir),
        budget_mb.saturating_mul(1024 * 1024),
    ))
}

pub fn result_store_clear(dir: &str) -> String {
    gc_json(&store::clear(Path::new(dir)))
}

/// The `cache` query's answer with the process's memory (`process`:
/// `resident`, `peak_resident`, `heap` bytes); other answers as they are.
pub fn with_process_memory(query: &str, answer: String) -> String {
    // Queries come many times a change; only this one is parsed again.
    let is_cache = query.contains("\"cache\"")
        && serde_json::from_str::<serde_json::Value>(query)
            .is_ok_and(|q| q.get("query").and_then(|v| v.as_str()) == Some("cache"));
    if !is_cache {
        return answer;
    }
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&answer) else {
        return answer;
    };
    let memory = crate::kernel::persist::ffi::process_memory();
    value["process"] = json!({"resident": memory.resident, "peak_resident": memory.peak_resident,
                              "heap": memory.heap});
    value.to_string()
}

impl Document {
    pub(crate) fn set_memory_cache_budget(&mut self, megabytes: u64) {
        self.0
            .set_memory_budget((megabytes > 0).then(|| megabytes.saturating_mul(1024 * 1024)));
    }

    pub(crate) fn set_result_store(&mut self, dir: &str, build_id: &str, label: &str) {
        self.0.set_result_store(
            (!dir.is_empty()).then(|| ResultStore::new(dir, build_id).with_label(label)),
        );
    }

    pub(crate) fn persist_results(&mut self, min_ms: f64) -> String {
        let min = Duration::from_secs_f64(min_ms.max(0.0) / 1000.0);
        persist_json(&self.0.persist_results(min))
    }

    pub(crate) fn hand_off_results(&self, dir: &str, build_id: &str, label: &str) -> String {
        let store = ResultStore::new(dir, build_id).with_label(label);
        persist_json(&self.0.hand_off_results(&store))
    }

    // Display meshes kept in the store (mitcad#858).

    pub(crate) fn restore_display_meshes(&self, request: &str) -> String {
        let start = Instant::now();
        let (bodies, options) = mesh_request(request);
        let mut restored = Vec::new();
        let mut faces = 0;
        if let Some(store) = self.0.result_store() {
            let before = store::stats().mesh_read_bytes;
            for uid in bodies {
                let (Some(version), Some(shape)) =
                    (self.0.body_version(uid), self.0.body_shape(uid))
                else {
                    continue;
                };
                if let Some(given) =
                    store.restore_display_mesh(self.0.kernel(), version, &options, shape)
                {
                    restored.push(uid.to_string());
                    faces += given;
                }
            }
            // (The counters are the process's: other documents may read
            // meanwhile, rarely while one opens.)
            let bytes = store::stats().mesh_read_bytes.saturating_sub(before);
            return json!({"restored": restored, "faces": faces, "bytes": bytes,
                          "ms": start.elapsed().as_secs_f64() * 1000.0})
            .to_string();
        }
        json!({"restored": restored, "faces": faces, "bytes": 0, "ms": 0.0}).to_string()
    }

    pub(crate) fn persist_display_meshes(&self, request: &str) -> String {
        let start = Instant::now();
        let (bodies, options) = mesh_request(request);
        let (mut stored, mut bytes, mut errors) = (0, 0, Vec::new());
        if let Some(store) = self.0.result_store() {
            for uid in bodies {
                let (Some(version), Some(shape)) =
                    (self.0.body_version(uid), self.0.body_shape(uid))
                else {
                    continue;
                };
                match store.write_display_mesh(self.0.kernel(), version, &options, shape) {
                    Ok(written) => {
                        stored += 1;
                        bytes += written;
                    }
                    Err(error) => errors.push(format!("{uid}: {error}")),
                }
            }
        }
        json!({"stored": stored, "bytes": bytes, "failed": errors.len(), "errors": errors,
               "ms": start.elapsed().as_secs_f64() * 1000.0})
        .to_string()
    }
}

/// The bodies and the meshing options of a display mesh request; bodies
/// whose uids do not parse are left out.
fn mesh_request(request: &str) -> (Vec<BodyUid>, String) {
    let value: serde_json::Value = serde_json::from_str(request).unwrap_or_default();
    let bodies = value["bodies"]
        .as_array()
        .map(|bodies| {
            bodies
                .iter()
                .filter_map(|uid| uid.as_str()?.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    let options = value["options"].as_str().unwrap_or_default().to_owned();
    (bodies, options)
}

fn persist_json(report: &PersistReport) -> String {
    json!({"results": report.results, "shapes": report.shapes, "bytes": report.bytes,
           "failed": report.failed, "errors": report.errors,
           "ms": report.time.as_secs_f64() * 1000.0})
    .to_string()
}
