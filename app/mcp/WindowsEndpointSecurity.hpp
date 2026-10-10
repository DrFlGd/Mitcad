// SPDX-License-Identifier: MIT
#pragma once

#include <QString>

namespace mitcad::mcp {

// NTFS ownership and ACL checks for the current user's discovery records.
bool privateWindowsEndpointPath(const QString& path);
// Only for a newly created discovery directory or record. Directories
// pass the restricted ACL to their temporary and published child files.
bool secureWindowsEndpointPath(const QString& path, bool directory);

} // namespace mitcad::mcp
