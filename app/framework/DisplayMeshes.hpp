// SPDX-License-Identifier: MIT
#pragma once

// The 3D view's meshes, made on the model's worker thread (mitcad#398).
// OCCT meshes a shape for display when it is first drawn, on the UI thread;
// a design of large curved faces then held the window (and every MCP call
// with it) for as long as that took. The jobs that change what the view
// shows (opening, model commands, undo and redo, previews) therefore end
// with the meshes of the bodies the view will draw, made as the view makes
// them (viewMeshOptions, view/ViewTessellation.hpp), while the job still
// owns the document: the window waits as for any job (progress dialog after
// 400 ms, MCP queries answered busy with a time to retry), and the view
// finds the triangulations and keeps them. Cancel stops the meshing only
// (what the job computed stays); the view meshes what is left.
//
// Two tiers (mitcad#635). The final tier is the view's meshes as above,
// written into the bodies' own faces. A preview's bodies are meshed in the
// interactive tier: each is drawn through a stand-in, a copy of it that
// shares no face, edge or vertex (geometry::independent_copy) meshed with
// previewMeshOptions, and the body's own faces stay without triangulations
// until a job meshes it in the final tier (the command that commits the
// preview, or refineDisplay once a drag ends). So the two tiers never meet
// in one face, a body the view draws is never meshed again on another
// thread, and a committed body never keeps a preview's meshes. Both tiers
// first take the triangulations of the faces a preview or command left as
// they were from the bodies as the view shows them (io::reuse_
// triangulations): only the faces it changed are meshed.

#include <cstddef>
#include <memory>
#include <vector>

#include <QJsonObject>
#include <QString>
#include <QtGlobal>

#include "ModelWorker.hpp"
#include "mitcad/geometry/shape.hpp"
#include "mitcad/io/mesh.hpp"

namespace mitcad {

using DisplayShapes = std::vector<std::shared_ptr<geometry::Shape>>;

enum class DisplayTier {
  Final,       // viewMeshOptions, in the bodies' own faces
  Interactive, // previewMeshOptions, in stand-ins
};

// The shown bodies' shapes (the `instances` query's visible bodies, each
// once). Called on the thread that owns the document.
DisplayShapes shownBodyShapes(const Document& document);

// A preview's changed and made bodies (its report's `bodies` with
// `changed`, and `instances`) and its tool body.
DisplayShapes previewShapes(const Document& document, const QJsonObject& report);

// The bodies a preview changes as the document has them before it (its
// report's `bodies` with `changed`): the donors of the faces it left as
// they were (meshForDisplay).
DisplayShapes previewDonors(const Document& document, const QJsonObject& report);

// What meshForDisplay did.
struct DisplayMeshing {
  std::size_t bodies = 0; // shapes meshed
  std::size_t incomplete = 0; // of them, shapes with faces OCCT's mesher failed on
  std::size_t ready = 0; // shapes that had a triangulation on every face, or a stand-in (not meshed)
  std::size_t reused = 0; // faces that took a donor's triangulation
  DisplayTier tier = DisplayTier::Final;
  qint64 milliseconds = 0;
  // The stand-ins made or drawn again (the interactive tier): the caller
  // keeps them for the view, whose bodies hold them only weakly.
  DisplayShapes standIns;
  io::MeshReport report; // the meshing's faces and phases (MITCAD_MESH_TRACE, mitcad#635)
};

// Meshes the shapes that have faces without a triangulation as the view
// does, in the job (its stage says so; its cancel stops the meshing), all
// of them together, so that their faces are meshed in parallel on every
// core (io::mesh_all_in_place, mitcad#438). A shape meshed to the end is
// marked so (geometry::Shape::display_meshed): a face OCCT's mesher failed
// on is not drawn, and neither this nor the view meshes it again
// (mitcad#589: meshing such a face again took without end, on the UI
// thread). A shape left when the meshing stopped or threw is left to the
// view, as before.
//
// In the interactive tier each such shape gets a stand-in meshed instead
// (see above), unless it has one; a shape that has a triangulation on
// every face (a body shown already) is drawn as it is. In the final tier a
// shape meshed to the end drops its stand-in. `donors` are the bodies as
// the view shows them, meshed in the final tier, whose untouched faces the
// shapes may take (only bodies meshed to the end by a job are read).
DisplayMeshing meshForDisplay(const DisplayShapes& shapes, ModelJob& job, DisplayTier tier = DisplayTier::Final,
                              const DisplayShapes& donors = {});

// What the view draws for a shape: its stand-in while it has one (the
// interactive tier), else the shape. Null stays null.
std::shared_ptr<geometry::Shape> displayShape(const std::shared_ptr<geometry::Shape>& shape);

// The bodies behind the stand-ins among `shown` that are still without the
// final tier's meshes: what refining a preview's display meshes (a job's
// meshForDisplay in the final tier) works on. Their faces are not drawn
// (the stand-ins are), so a job may mesh them while the view draws.
DisplayShapes refinableShapes(const DisplayShapes& shown);

// On the UI thread after the job: "Display meshes of <label>: 2 bodies in
// 85 ms on the model's thread", when it meshed any, and "Display meshes of
// <label>: 1 bodies with faces that could not be meshed (not drawn)" when
// some faces failed. With MITCAD_MESH_TRACE (io::mesh_tracing, mitcad#635)
// also "Mesh trace <label> <tier>: <n> bodies ready, <m> meshed, <r> faces
// reused; <the report's line>": which faces kept the triangulation they
// had, which were meshed, the phases' times and the slowest faces.
void logDisplayMeshing(const DisplayMeshing& meshing, const QString& label);

} // namespace mitcad
