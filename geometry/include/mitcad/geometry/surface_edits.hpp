// SPDX-License-Identifier: MIT
#pragma once

// Surface modelling (mitcad#455): Trim, Untrim, Unstitch and Reverse
// Normal. None changes its inputs. Lengths are millimetres.

#include <optional>
#include <string>
#include <utility>
#include <vector>

#include <gp_Dir.hxx>
#include <gp_Pnt.hxx>

#include "mitcad/geometry/path_sweep.hpp"
#include "mitcad/geometry/shape.hpp"
#include "mitcad/geometry/tool.hpp"

namespace mitcad::geometry {

// What a trim cuts with: a face operation's tool (a face extended along
// its surface with `extend`), or chains of curves swept both ways along
// `direction` through the body.
struct TrimInput {
  std::optional<Tool> tool;
  std::vector<Path> chains;
  gp_Dir direction{0.0, 0.0, 1.0};
  bool extend = true;
};

// The surface body cut by the tool, only the pieces kept that the points
// pick (the piece nearest each; a piece is what hangs together once cut).
// The faces keep their names ("#k" for pieces). Throws when the tool does
// not cut the body.
ShapePtr trim_surface(const Shape& body, const TrimInput& input, const std::vector<gp_Pnt>& keep);

// A face restored towards its surface: without its holes (`holes`), its
// outer boundary at the surface's bounds (`outer`; a surface without
// bounds: the face's span). With `in_place` the body with the face
// replaced (the face's names kept, the faces sewn again), else a new
// surface body of one face "<feature>:untrim(<face>)".
ShapePtr untrim_surface(const std::string& feature, const Shape& body, const std::string& face, bool holes,
                        bool outer, bool in_place);

// The body taken apart: the faces left (null when none is), then each
// named face (references; every face when none is named) as a surface
// body of its own. The faces keep their names.
std::pair<ShapePtr, std::vector<ShapePtr>> unstitch(const Shape& body, const std::vector<std::string>& faces);

// The body with the faces (every face when none is named) turned over;
// the faces keep their names.
ShapePtr reverse_faces(const Shape& body, const std::vector<std::string>& faces);

} // namespace mitcad::geometry
