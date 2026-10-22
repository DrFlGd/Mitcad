// SPDX-License-Identifier: MIT
#pragma once

// The library parts a design uses (mitcad#418): the part picker's "In this
// design" group. The model answers with the `library_usage` query
// (core/model/src/api/commands.md, "The library parts a design uses").

#include <QHash>
#include <QList>
#include <QString>

namespace mitcad {

class CommandContext;

// The library parts a design uses (mitcad#418): BUILD sources and inserted
// library components, each with how many times it is used.
struct UsedPart {
  // The uses placed in one configuration row of the component (mitcad#518).
  struct Row {
    QString config;  // the row's name
    int count = 0;
  };

  QString library;    // the library's id as library_search gives it
  QString url;        // its URL, empty when the id is enough
  QString component;  // the component's id in the library
  int count = 0;      // occurrences in the design, of every row
  // The same per row, in the order they first appear; the uses placed
  // without a row (the component's default row) are in `count` only.
  QList<Row> rows;
};

// The design's uses of library parts by component and by configuration row
// (mitcad#518): what the picker's tiles count (xN), so that a tile of one
// size shows that size's uses and not its component's total.
class UsedCounts {
public:
  void set(const QList<UsedPart>& used);
  // The uses of a part: with no `config`, the component's (every row);
  // else the row's, and for the component's default row (`defaultRow`) also
  // those placed without a row, which are placed in it.
  int of(const QString& library, const QString& component, const QString& config = QString(),
         bool defaultRow = false) const;

private:
  QHash<QString, int> m_components;  // "<library>|<component>" -> uses
  QHash<QString, int> m_rows;        // "<library>|<component>|<row>" -> uses
  QHash<QString, int> m_unrowed;     // "<library>|<component>" -> uses placed without a row
};

// In the order of the design's parts list; empty when the model cannot
// answer (the query refused is logged).
QList<UsedPart> usedLibraryParts(const CommandContext& context);

} // namespace mitcad
