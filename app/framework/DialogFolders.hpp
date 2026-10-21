// SPDX-License-Identifier: MIT
#pragma once

#include <QString>

class QSettings;

namespace mitcad {

// The folders the file dialogs start in (mitcad#498). Each group of dialogs
// remembers the folder of the file last accepted in one of them, across
// restarts (settings `fileDialogs/<group>`), and starts there while that
// folder exists; otherwise in the folder the caller gives (the design's
// folder, the project, Documents). Save Copy As of a version starts in the
// design's own folder: it neither reads nor writes a remembered folder.
enum class DialogGroup {
  Open,     // Open, Open Read-Only, Save As (also the first save of an untitled design)
  Export,   // Export, Export Drawing, Export Flat Pattern, Save As DXF, the build lists' PDF, reports
  Import,   // Import, Insert SVG, Mesh, DXF and Component
  Texture,  // texture images (Edit Appearances)
  Program,  // programs (the slicer, git)
  SaveCopy, // Save Copy As of a version: never remembered
};

// The group's name in the settings and the log ("open", "export", ...;
// empty for SaveCopy).
QString dialogGroupName(DialogGroup group);

// The folder a dialog of the group starts in: the remembered one while it
// is a folder, else `fallback`.
QString dialogStartFolder(QSettings& settings, DialogGroup group, const QString& fallback);
// Remembers the folder a file accepted in a dialog of the group is in (an
// application bundle chosen as a program too); nothing for an empty path or
// SaveCopy.
void rememberDialogFolder(QSettings& settings, DialogGroup group, const QString& chosen);

// The same with the application's settings, logged ("Dialog folder export:
// <folder>", "Dialog folder export remembered: <folder>"), the settings
// written at once (a killed app keeps them).
QString dialogStartFolder(DialogGroup group, const QString& fallback);
void rememberDialogFolder(DialogGroup group, const QString& chosen);

} // namespace mitcad
