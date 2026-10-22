// SPDX-License-Identifier: MIT
#pragma once

// Place Part (mitcad#525; app/COMMANDS.md, "Component libraries"): a
// library part with mount frames, chosen in Insert from Library, placed by
// a frame on a face, a hole or a grid with the joint the frame asks for;
// Enter without a pick drops it in as Insert from Library always did.

#include <memory>

#include <QJsonArray>
#include <QJsonObject>
#include <QString>

namespace mitcad {

class CommandRegistry;
class DocumentHost;

// The part Place Part places: Insert from Library's choice.
struct MountPart {
  // insert_component's `library`: {"id", "url", "rev", "component", "config"}.
  QJsonObject library;
  QString name;
  QString version; // "v1.0.0 (3f9a2c1)"
  QString license;
  bool link = true;
  // The manifest's `mounts` of the component.
  QJsonArray frames;
};

// The command's id, started by Insert from Library for a part with frames.
inline const char* const kMountPartCommand = "insert.mount_part";

// Registers Place Part; `part` is what Insert from Library chose last.
void registerMountCommands(CommandRegistry& registry, DocumentHost& host, std::shared_ptr<MountPart> part);

} // namespace mitcad
