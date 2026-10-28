// SPDX-License-Identifier: MIT
#include "bridge/display_mesh.hpp"

#include <stdexcept>
#include <string_view>

#include <Standard_Failure.hxx>

#include "mitcad/io/mesh.hpp"
#include "mitcad/io/mesh_store.hpp"
#include "mitcad_bridge/kernel/display_mesh.h"

namespace mitcad::bridge {

std::unique_ptr<std::string> display_mesh_bytes(const geometry::Shape& shape) {
  // Only the final tier's meshes, made to the end: a stand-in's are the
  // interactive tier's, and a shape still being meshed is not whole.
  if (!shape.display_meshed() || shape.display_interactive()) {
    throw std::runtime_error("storing display meshes: the shape is not meshed for display");
  }
  try {
    return std::make_unique<std::string>(io::triangulation_bytes(shape.occt()));
  } catch (const Standard_Failure& failure) {
    throw std::runtime_error(std::string("storing display meshes: ") + failure.what());
  }
}

std::uint64_t restore_display_mesh(const geometry::Shape& shape, rust::Slice<const std::uint8_t> data) {
  const TopoDS_Shape& occt = shape.occt();
  // A shape meshed already keeps what it has (a body the view may draw).
  if (shape.display_meshed() || shape.display_interactive() || io::is_mesh(occt) || io::has_triangulation(occt)) {
    return 0;
  }
  std::size_t given = 0;
  try {
    given = io::restore_triangulations(occt,
                                       std::string_view(reinterpret_cast<const char*>(data.data()), data.size()));
  } catch (const Standard_Failure& failure) {
    throw std::runtime_error(std::string("restoring display meshes: ") + failure.what());
  }
  // Meshed to the end, as the meshing that stored them left it: a face
  // without a triangulation is one OCCT's mesher failed on, which the view
  // must not mesh again itself (DisplayMeshes, mitcad#589).
  shape.set_display_meshed();
  return given;
}

} // namespace mitcad::bridge
