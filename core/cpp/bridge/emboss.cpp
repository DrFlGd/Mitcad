// SPDX-License-Identifier: MIT
#include "bridge/emboss.hpp"

#include <string>

#include "bridge/profile.hpp"
#include "mitcad/geometry/emboss.hpp"
#include "mitcad_bridge/kernel/emboss.h"

namespace mitcad::bridge {

std::shared_ptr<geometry::Shape> wrapped_emboss(rust::Str feature, const Frame& frame,
                                                rust::Slice<const Region> regions, const EmbossWrap& wrap) {
  geometry::EmbossWrapSpec spec;
  spec.feature = std::string(feature);
  spec.frame = to_geometry(frame);
  spec.regions = to_geometry(regions);
  spec.axis = gp_Ax1(gp_Pnt(wrap.axis_origin[0], wrap.axis_origin[1], wrap.axis_origin[2]),
                     gp_Dir(wrap.axis_direction[0], wrap.axis_direction[1], wrap.axis_direction[2]));
  spec.radius = wrap.radius;
  spec.to_radius = wrap.to_radius;
  return geometry::wrapped_emboss(spec);
}

} // namespace mitcad::bridge
