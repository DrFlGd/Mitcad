// SPDX-License-Identifier: MIT
#pragma once

// A project's web viewer folder (mitcad#423): `web/` in the project's
// folder holds a page per design (web/WebPage.hpp writes them), each
// design's thumbnail, `designs.json` and a plain `index.html` (HTML and
// CSS, no scripts) with a card per design linking its page:
//
//   web/index.html        the project's name, a card per design
//   web/<slug>.html       the page of the design's newest version
//   web/<slug>.png        its thumbnail (the version's preview, 256 px)
//   web/designs.json      [{"name", "file", "page", "thumbnail",
//                           "saved_by", "saved_at"}]
//
// Plain Qt Core: the names, the index and designs.json, written into a
// folder (app/tests/project_web.cpp tests them). The commands and the Save
// that write the pages are web/MainWindowWeb.cpp.

#include <vector>

#include <QByteArray>
#include <QDateTime>
#include <QHash>
#include <QString>
#include <QStringList>

namespace mitcad::web {

// The folder of the viewer in a project's folder.
inline constexpr const char* kWebFolder = "web";

// A design's card in the index and its entry in designs.json.
struct DesignCard {
  QString name;           // the design's name ("bracket")
  QString file;           // its path in the project, '/' ("parts/bracket.mitcad")
  QString slug;           // its pages' base name ("parts--bracket")
  bool thumbnail = false; // web/<slug>.png is there
  QString savedBy;        // who saved its newest version ("" unknown)
  QDateTime savedAt;      // when (invalid: unknown)

  QString page() const { return slug + QStringLiteral(".html"); }
  QString thumbnailFile() const { return slug + QStringLiteral(".png"); }
};

// The base name of a design's pages: its path in the project with '/' as
// "--", any other character outside [A-Za-z0-9._-] as '-', without the
// ".mitcad" suffix ("parts/My bracket.mitcad" -> "parts--My-bracket"); a
// leading '.' is '-' (a hidden file is not served), nothing is "design".
QString designSlug(const QString& path);

// The slugs of a project's designs (their paths in the project): each
// unique ignoring case (file systems that ignore it) and none "index" (the
// index's own page); a clash gets "-2", "-3", ... in the order of the paths
// sorted ignoring case. Path -> slug.
QHash<QString, QString> designSlugs(QStringList paths);

// Whether `name` is a plain file name the folder may hold with that suffix
// (".html", ".png"): no folder, not hidden, not the index. What a
// designs.json someone else wrote names is removed only then.
bool isPageFileName(const QString& name, const QString& suffix);

// designs.json of the cards (in their order), and read back: the entries
// whose page is a plain file name (isPageFileName), the slug from the page.
QByteArray designsJson(const std::vector<DesignCard>& cards);
std::vector<DesignCard> readDesignsJson(const QByteArray& json);

// The index page: the project's name and a card per design (thumbnail, or
// a placeholder; name; path; last saved by and when, in UTC) linking its
// page; light and dark (prefers-color-scheme), no scripts.
QByteArray indexHtml(const QString& projectName, const std::vector<DesignCard>& cards);

// What writeIndex did: the files written and removed (absolute), which go
// into the version, and what failed ("" none).
struct FolderUpdate {
  QStringList written;
  QStringList removed;
  QString error;
};

// Writes index.html and designs.json into `folder` (made when missing; the
// cards sorted by their files ignoring case) and removes the pages and
// thumbnails the folder's previous designs.json listed for designs no card
// has (a design removed or renamed). Each file whole or not at all.
FolderUpdate writeIndex(const QString& folder, const QString& projectName, std::vector<DesignCard> cards);

} // namespace mitcad::web
