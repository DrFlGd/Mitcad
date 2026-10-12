// SPDX-License-Identifier: MIT
#pragma once

#include "mitcad/geometry/sheet_domain.hpp"

namespace mitcad::geometry {

// Exact planar Boolean in a common operation chart. Returns every retained
// region in its own domain chart, with complete source-edge history. Union
// requires one domain. Difference/Intersection keep domain ownership separate.
// Named straight seams are split and paired over all surviving intervals.
SheetTrimResult sheet_trim_domain(const SheetTrimSpec& spec);

} // namespace mitcad::geometry
