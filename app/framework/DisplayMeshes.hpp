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

#include <cstddef>
#include <memory>
#include <vector>

#include <QJsonObject>
#include <QString>
#include <QtGlobal>

#include "ModelWorker.hpp"
#include "mitcad/geometry/shape.hpp"

namespace mitcad {

using DisplayShapes = std::vector<std::shared_ptr<geometry::Shape>>;

// The shown bodies' shapes (the `instances` query's visible bodies, each
// once). Called on the thread that owns the document.
DisplayShapes shownBodyShapes(const Document& document);

// A preview's changed and made bodies (its report's `bodies` with
// `changed`, and `instances`) and its tool body.
DisplayShapes previewShapes(const Document& document, const QJsonObject& report);

// What meshForDisplay did.
struct DisplayMeshing {
  std::size_t bodies = 0; // shapes meshed
  std::size_t incomplete = 0; // of them, shapes with faces OCCT's mesher failed on
  qint64 milliseconds = 0;
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
DisplayMeshing meshForDisplay(const DisplayShapes& shapes, ModelJob& job);

// On the UI thread after the job: "Display meshes of <label>: 2 bodies in
// 85 ms on the model's thread", when it meshed any, and "Display meshes of
// <label>: 1 bodies with faces that could not be meshed (not drawn)" when
// some faces failed.
void logDisplayMeshing(const DisplayMeshing& meshing, const QString& label);

} // namespace mitcad
