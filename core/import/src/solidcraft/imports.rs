// SPDX-License-Identifier: MIT
//! Bodies the file keeps: an Import feature's STEP text and a mesh
//! import's triangles, as base features (bodies without history, as
//! SolidCraft keeps them). And body transfers between components.

use mitcad_model::features::Operation;
use mitcad_model::{BaseInput, ComponentUid, ImportBody, Kernel};
use mitcad_solidcraft::Feature;
use mitcad_solidcraft::kinds as k;

use super::{Importer, MapResult, Mapped, NotMapped};

impl<K: Kernel> Importer<'_, '_, K> {
    /// The STEP text of an Import feature, read by the kernel from a
    /// temporary file.
    pub(super) fn import_step(&mut self, f: &Feature, component: ComponentUid) -> MapResult {
        let i: k::Import = f.typed()?;
        if i.step.trim().is_empty() {
            return Err(NotMapped::Failed(
                "the Import feature keeps no STEP text".to_owned(),
            ));
        }
        let dir = std::env::temp_dir().join(format!("mitcad-solidcraft-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("import-{}.step", f.id));
        std::fs::write(&path, i.step.as_bytes()).map_err(|e| e.to_string())?;
        let read = self.doc.kernel().read_file(
            &path.to_string_lossy(),
            &mitcad_model::exchange::ReadOptions { unit_mm: 1.0 },
        );
        let _ = std::fs::remove_file(&path);
        let bodies = read.map_err(|e| format!("{}: {e}", i.file))?;
        let mut notes = Vec::new();
        let bodies: Vec<ImportBody<K::Shape>> = bodies
            .into_iter()
            .enumerate()
            .filter(|(n, _)| !i.skip.contains(&(*n as u64)))
            .map(|(_, b)| b)
            .collect();
        if !i.skip.is_empty() {
            notes.push(format!(
                "{} instances shown by other occurrences are left out",
                i.skip.len()
            ));
        }
        self.add_bodies(f, component, Some(i.file.clone()), bodies)?;
        Ok(Mapped::partial(notes))
    }

    /// Mesh bodies of a mesh import.
    pub(super) fn import_meshes(&mut self, f: &Feature, component: ComponentUid) -> MapResult {
        let m: k::MeshImport = f.typed()?;
        let mut bodies = Vec::new();
        for mesh in &m.meshes {
            let points: Vec<[f64; 3]> = mesh.positions.as_chunks::<3>().0.to_vec();
            let triangles: Vec<[u32; 3]> = mesh.triangles.as_chunks::<3>().0.to_vec();
            let shape = self
                .doc
                .kernel()
                .triangle_body(&points, &triangles)
                .map_err(|e| format!("mesh {}: {e}", mesh.name))?;
            bodies.push(ImportBody {
                name: Some(mesh.name.clone()),
                color: mesh.color.map(|c| [c[0] as f64, c[1] as f64, c[2] as f64]),
                shape,
            });
        }
        self.add_bodies(f, component, Some(m.file.clone()), bodies)?;
        Ok(Mapped::parametric())
    }

    fn add_bodies(
        &mut self,
        f: &Feature,
        component: ComponentUid,
        source: Option<String>,
        bodies: Vec<ImportBody<K::Shape>>,
    ) -> Result<(), String> {
        let before = self.body_versions();
        let imported = self
            .doc
            .add_base_feature(BaseInput {
                name: Some(f.name.clone()),
                source,
                bodies,
                operation: Operation::NewBody,
                participants: Vec::new(),
                replaces: Vec::new(),
                component: Some(component),
            })
            .map_err(|e| e.to_string())?;
        self.track_bodies(&before, imported.feature.uid);
        Ok(())
    }

    /// Bodies moved into another component on the timeline.
    pub(super) fn body_transfer(&mut self, _f: &Feature, _component: ComponentUid) -> MapResult {
        Err(NotMapped::Unmapped(
            "moving bodies into another component at a point of the timeline".to_owned(),
        ))
    }

    /// A mesh body rebuilt as planes and cylinders (mitcad#462): Mitcad's
    /// Convert Mesh of the body by its name, the tolerance the input's
    /// parameter. The body keeps its name and id.
    pub(super) fn convert_mesh(&mut self, f: &Feature, component: ComponentUid) -> MapResult {
        let m: k::MeshConvert = f.typed()?;
        let body = self.body(&m.body)?;
        let tolerance = self.slot(f, "/tolerance")?;
        let def = serde_json::json!({"type": "mesh_convert", "body": body.to_string(),
            "tolerance": tolerance});
        self.add(&def, Some(&f.name), component)?;
        Ok(Mapped::parametric())
    }
}
