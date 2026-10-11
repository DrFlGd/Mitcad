// SPDX-License-Identifier: MIT
#pragma once

// The project picker (mitcad#174, mitcad#176): a place, a design or a
// project chosen among the projects the Projects panel lists (the known
// ones and those of the projects folder, readProjects on a thread of its
// own), each by the name the panel shows, with its Local or Cloud icon and
// its folder and remote's address in the tooltip, instead of a file or
// folder dialog. A secondary button (Another Folder..., Another File...)
// leaves the choice to the caller's file or folder dialog, for what is
// outside the listed projects.
//
//   Place:   a project and a folder in it, and a design's name typed in
//            the dialog (a clash with a file there shown inline): Move to
//            a Project, Save As, Version History's Save Copy As.
//   Design:  a design of a project (Insert Component).
//   Project: a project (Open Project).
//
// Only projects with versions are offered for a place or a design (no
// project without versions, none whose folder is missing); Open Project
// also lists projects without versions (their history starts there).
//
// For the UI tests (app/COMMANDS.md, "Logs that UI tests read"): "Project
// picker <title>: <n> projects: <name> (local) | <name> (cloud) | ..." once
// read, after a line "Project picker project <key>: <kind>, <root>" per
// project (the key: its name, or its folder when another listed project
// has the same name); "Project picker row <key> at x,y" for the rows in
// sight (keys: "<key>", "<key>/<folder>/", "<key>/<design path>"),
// "Project picker name at x,y", "Project picker button <text> at x,y"
// (main window coordinates); "Project picker selected <key>"; "Project
// picker target <key>/<path>: free" ("exists", "invalid"); "Project
// picker <title>: chose <path>" ("another folder", "another file",
// "cancelled").

#include <optional>

#include <QString>

class QWidget;

namespace mitcad {

struct ProjectPickerOptions {
  enum class Mode { Place, Design, Project };
  Mode mode = Mode::Place;
  QString title;      // the window's title
  QString text;       // what the dialog asks, above the list
  QString acceptText; // the accept button's text ("&Move", "&Save", "&Insert", "&Open")
  QString otherText;  // the secondary button ("&Another Folder...", "&Another File..."; "" none)
  // The project selected at first ("" the first listed), listed first; and
  // (Place) the folder in it, relative to it ("" its top). Design: the
  // project is listed first and expanded.
  QString root;
  QString folder;
  // Place: the name offered (without .mitcad).
  QString name;
  // Place: a file that is no clash (the design's own file for Save As);
  // Design: a design not offered (the open design). Absolute; "" none.
  QString ownFile;
};

struct ProjectPickerResult {
  bool other = false; // the secondary button: the caller asks with its file or folder dialog
  QString root;       // the project chosen
  QString folder;     // Place: the folder in it (relative, "" its top)
  // Place: the new design's file; Design: the design chosen; Project: the
  // project's folder. Absolute, '/'.
  QString path;
};

// The picker, modal to `parent`; none when cancelled.
std::optional<ProjectPickerResult> pickInProjects(QWidget* parent, const ProjectPickerOptions& options);

// A design's name as typed (trimmed, without a trailing .mitcad); "" when
// it is no name of a file (empty, a path, "." or ".."). For the picker's
// name field.
QString designNameOf(const QString& typed);

// A folder's path relative to the project at `root` ('/'), "" for the
// project's top or a folder outside it: the picker's `folder`.
QString folderInProject(const QString& root, const QString& folder);

} // namespace mitcad
