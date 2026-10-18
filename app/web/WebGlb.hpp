// SPDX-License-Identifier: MIT
#pragma once

// A design's bodies as the web viewer's binary glTF file and component tree
// (mitcad#423, app/web/WebPage.hpp). Qt Core and Gui and the renderer's
// plain types (render/Renderer.hpp), no OCCT: the unit tests build it.

#include <array>
#include <memory>
#include <vector>

#include <QByteArray>
#include <QJsonArray>
#include <QString>
#include <QStringList>

#include "render/Renderer.hpp"

namespace mitcad::web {

// A body as placed by an occurrence (or the root's own), with its look.
struct PageBody {
  QString uid;
  QString name;
  // The occurrence that places it: its path of occurrence uids ("O1/O4"),
  // empty for a body of the root.
  QString occurrence;
  // Its triangles in the body's own coordinates (mm); bodies of the same
  // body share it.
  std::shared_ptr<const render::MeshData> mesh;
  // Where it is placed: a row-major 3 x 4 matrix (rotation, translation in
  // mm).
  std::array<double, 12> transform{1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0};
  render::MaterialData material;
  // Faces with materials of their own (indices into mesh->faceTriangles).
  std::vector<render::FaceMaterial> faces;
};

struct PageGlb {
  QByteArray glb;
  // The node of each body, in the order of the bodies ("n1", "n2", ...;
  // empty for a body without triangles, which has none).
  QStringList nodes;
  // Texture images that could not be read (their materials keep their
  // colours).
  QStringList warnings;
};

// The bodies as a binary glTF 2.0 file (.glb) in model units: positions
// and placements in mm times `scale` (model units per mm). One node per
// body, named as `nodes` says, with the body's placement as its matrix and
// {"body": uid, "body_name": name, "occurrence": path} as its extras (not
// "name": a glTF loader keeps the node's name there); bodies of
// the same mesh and look share a glTF mesh. A body's faces with materials
// of their own are primitives of their own. Materials are glTF's metallic
// roughness ones (linear colours; see-through as opacity times 1 - 0.9 x
// transmission, double sided); a texture's images go into the file (PNG or
// JPEG, at most 2048 pixels a side) with texture coordinates made by the
// projection the rendered view uses (planar along Z, else box), and its
// roughness and metalness maps are combined into glTF's one image.
PageGlb pageGlb(const std::vector<PageBody>& bodies, double scale);

// The viewer's component tree (the page's data, "tree"): one root item
// {"id": "root", "name": rootName, "nodes": [the root's own bodies' nodes],
// "children": [...]}, its children the occurrences of the `components`
// query's tree (`occurrences`: each `uid`, `name`, `children`) as
// {"id": <path of uids>, "name", "nodes", "children"}, each with the nodes
// of the bodies placed directly by it; occurrences without a node in them
// or below are left out (hidden ones).
QJsonArray pageTree(const QString& rootName, const QJsonArray& occurrences, const std::vector<PageBody>& bodies,
                    const QStringList& nodes);

} // namespace mitcad::web
