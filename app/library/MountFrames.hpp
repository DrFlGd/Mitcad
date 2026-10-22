// SPDX-License-Identifier: MIT
#pragma once

// The mount frames of library parts (mitcad#525; docs/libraries.md, "Mount
// frames"; the model's side in commands.md, "Mount frames"): what Insert
// from Library and Place Part read of a manifest's `mounts` block and of
// the model's `mount_preview` answers. Plain Qt Core, for the unit tests.

#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QString>
#include <QVector>

namespace mitcad {

// A frame as the manifest lists it: its name, kind (face, hole, shaft,
// grid, foot) and joint (rigid, planar, revolute, cylindrical, slider).
struct MountFrameInfo {
  QString name;
  QString kind = QStringLiteral("face");
  QString joint = QStringLiteral("rigid");
  QJsonObject raw;
};

// The frames of a `mounts` block (the manifest's, or the answers'
// `frames`), each name once; an entry without a name is left out.
QVector<MountFrameInfo> mountFramesOf(const QJsonValue& mounts);

// How a placed part may move, for people: "fixed", "free on the face",
// "turns", "turns and slides", "slides".
QString mountJointText(const QString& joint);

// What a frame mounts on, for people: "on a face", "into a hole", "onto a
// shaft", "a grid", "on a grid".
QString mountKindText(const QString& kind);

// "foot (on a grid, fixed)".
QString mountFrameLabel(const MountFrameInfo& frame);

// Whether a frame of the kind goes on an axis (a hole's wall or rim, a
// shaft) rather than on a plane (a face, a grid).
bool mountTakesAxes(const QString& kind);

// Cells as the answers give them ([[1, 0], [2, 0]]): "(1, 0) and (2, 0)".
QString mountCellsText(const QJsonArray& cells);

} // namespace mitcad
