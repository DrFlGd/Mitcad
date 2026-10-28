// SPDX-License-Identifier: MIT
#pragma once

// A provisional drag presentation (mitcad#636): while a value is dragged
// and the exact preview of the new value is still computed on the model's
// worker, the view shows the exact preview's tool body changed to the
// dragged value directly, from an owned copy of its display mesh.
//
// The mesh (ProxyMesh) is copied from the tool's triangulations on the UI
// thread once the job that meshed them has ended, so nothing the worker
// owns is read during a drag and no model query is made; each move only
// moves the copy's nodes (applyDragProxy). Which change a dragged value
// makes is the command's (CommandDef::dragProxy, DragProxyRule): Extrude
// stretches its tool along the dragged arrow, or moves it for its start
// offset. The exact preview stays what is committed and replaces the
// proxy when its value catches up (CommandSession).

#include <array>
#include <cstddef>
#include <limits>
#include <memory>
#include <optional>
#include <vector>

#include <TopoDS_Shape.hxx>
#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>
#include <gp_Trsf.hxx>

namespace mitcad {

// A triangle mesh with its edges' polylines, owned and never changed once
// made (shared as std::shared_ptr<const ProxyMesh>), in the coordinates
// of the shape it was copied from.
struct ProxyMesh {
  std::vector<gp_Pnt> nodes;
  std::vector<std::array<int, 3>> triangles; // node indices, counter-clockwise outwards
  std::vector<std::vector<int>> outlines;    // node indices along each edge
};

// What a dragged value does to the exact preview's tool.
struct DragProxyRule {
  enum class Kind {
    Stretch,     // along the arrow about a fixed plane, both sides of it
    StretchSide, // the same, only the part beyond the fixed plane
    Translate,   // along the arrow by to - from
  };
  Kind kind = Kind::Stretch;
  double from = 0.0; // the value of the exact preview shown (mm)
  double to = 0.0;   // the value dragged to (mm)
  // The tool's length along the arrow in the exact preview: which axis of
  // the tool's coordinates is the arrow's (applyDragProxy), and a guard
  // against a tool of another shape. NaN: not checked.
  double length = std::numeric_limits<double>::quiet_NaN();
  // Stretches: how far the tool reaches beyond the fixed plane along the
  // arrow; the fixed plane is that far back from its farthest point.
  double reach = 0.0;
};

// Copies the triangulations of a meshed shape's faces (a face without one
// is left out) and its edges' polygons on them, in the shape's own
// coordinates with each face's location applied. Reads only; null when no
// face has a triangulation.
std::shared_ptr<const ProxyMesh> proxyMeshOf(const TopoDS_Shape& shape);

struct DragProxy {
  std::shared_ptr<const ProxyMesh> mesh; // the changed copy
  gp_Dir axis;                           // the arrow's direction in the mesh's coordinates
  double fixed = 0.0;                    // stretches: the fixed plane's height along it
};

// The mesh changed by the rule along the first of `axes` (the arrow's
// direction, in the mesh's coordinates as each placement of it would
// give it) along which the mesh has the rule's length. None when no axis
// fits, when the dragged value makes the tool flat, or for a side stretch
// across its fixed plane: the proxy is then not shown.
std::optional<DragProxy> applyDragProxy(const ProxyMesh& mesh, const DragProxyRule& rule,
                                        const std::vector<gp_Dir>& axes);

// A provisional motion (mitcad#838, CommandDef::dragMotion): what a drag
// does to each body an exact preview draws.
enum class MotionSide {
  None,       // stays where the exact preview draws it
  Shape,      // its shape moved (or made) by the command, in its component's
              // coordinates: drawn at its placement times the change
  Occurrence, // in an occurrence the command places elsewhere: the change,
              // taken into the design, before its placement
};

// Where a body the exact preview draws at `placement` (into the design) is
// drawn while a drag takes the command's motion from `from` (the exact
// preview's) to `to`, both in the coordinates `frame` puts into the design.
gp_Trsf draggedPlacement(const gp_Trsf& placement, MotionSide side, const gp_Trsf& from, const gp_Trsf& to,
                         const gp_Trsf& frame);

// Whether two motions are the same: each entry of their matrices within
// `tolerance` (mm for the translations).
bool sameMotion(const gp_Trsf& a, const gp_Trsf& b, double tolerance = 1e-9);

} // namespace mitcad
