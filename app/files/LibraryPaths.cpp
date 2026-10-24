// SPDX-License-Identifier: MIT
#include "Libraries.hpp"

#include "mitcad_bridge/lib.h"

namespace mitcad {

QString librariesDirectory() {
  const rust::String root = default_libraries_directory();
  return QString::fromUtf8(root.data(), static_cast<qsizetype>(root.size()));
}

} // namespace mitcad
