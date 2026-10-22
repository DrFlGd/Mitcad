// SPDX-License-Identifier: MIT
#pragma once

// What File > Open from Project lists (mitcad#530): the projects of the
// Projects panel (readProjects, `project_tree`) as places, folders inside
// them, and designs, never the file system's raw folders: `.mitcad/`
// internals and files that are no designs are not in `project_tree`'s
// answer and are not listed. A place's contents are its sub-folders first,
// then its designs, each sorted by name; a search finds designs by part of
// their name in every project or in one. Plain Qt Core, for app.unit; the
// window is projects/ProjectBrowser.
//
// Keys, as the logs have them: a project's is its folder's name (its whole
// path when another listed project's folder has the same name, as the
// Projects panel's), a folder's "<project>/<path>/", a design's
// "<project>/<path>".

#include <QHash>
#include <QString>
#include <QStringList>
#include <QVector>

#include "ProjectsTree.hpp"

namespace mitcad {

// A place: a project's folder ("" its top).
struct BrowserPlace {
  QString root;
  QString folder; // relative, '/'

  bool operator==(const BrowserPlace& other) const;
  bool operator!=(const BrowserPlace& other) const { return !(*this == other); }
};

// A row of a place's contents or of a search's results.
struct BrowserEntry {
  enum class Kind { Folder, Design };
  Kind kind = Kind::Design;
  QString root;        // the project's folder
  QString projectKey;  // the project's key in the logs
  QString projectName; // the project's name as the panel shows it
  QString path;        // relative to the project, '/': the folder's, or the design's file
  QString name;        // shown: the folder's name, the design's without .mitcad
  ProjectDesignRow design; // a design's version, changes and edit lock

  // The folder the entry is in (relative; "" the project's top).
  QString folder() const;
  // "<project>/<path>" ("/" after a folder's).
  QString key() const;
};

// The projects offered: those with versions whose trees were read (Local
// and Cloud), sorted by name (case-insensitive, then by folder).
QVector<ProjectRow> browsableProjects(const QVector<ProjectRow>& rows);

// The projects' keys by root: their folders' names, a whole path when two
// have folders of the same name.
QHash<QString, QString> projectKeys(const QVector<ProjectRow>& projects);

// The project of `root` among `projects`; null when it is not listed.
const ProjectRow* findProject(const QVector<ProjectRow>& projects, const QString& root);

// Every folder of a project: those `project_tree` lists and those its
// designs are in, sorted.
QStringList allFolders(const ProjectRow& project);

// The folders right inside `folder` ("" the project's top), by name.
QStringList subFolders(const ProjectRow& project, const QString& folder);

// Whether a place is listed (a remembered one may have gone).
bool placeExists(const QVector<ProjectRow>& projects, const BrowserPlace& place);

// A place's contents: its sub-folders first, then its designs, each by name.
QVector<BrowserEntry> folderContents(const ProjectRow& project, const QString& projectKey, const QString& folder);

// The designs whose name (the file's without .mitcad) holds `text`, case
// ignored, in every project (`root` "") or only in the one at `root`: by
// project name, then path. Nothing for a text that is only spaces.
QVector<BrowserEntry> searchDesigns(const QVector<ProjectRow>& projects, const QHash<QString, QString>& keys,
                                    const QString& text, const QString& root);

// The path bar's parts of a place: the project's name, then each folder
// down to it, each with the place it goes to.
struct BrowserPathPart {
  QString text;
  BrowserPlace place;
};
QVector<BrowserPathPart> pathParts(const ProjectRow& project, const QString& folder);

// A place's key: "<project>" or "<project>/<folder>/".
QString placeKey(const QString& projectKey, const QString& folder);

// The place a step up from `place`: its parent folder, or the project's
// top; the same place at the top.
BrowserPlace placeUp(const BrowserPlace& place);

// An entry's state in the log's words: "sub/", "gear.mitcad v2, changed,
// editing elsewhere" (designMarks' words).
QString entryWords(const BrowserEntry& entry);

} // namespace mitcad
