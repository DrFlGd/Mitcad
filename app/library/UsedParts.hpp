// SPDX-License-Identifier: MIT
#pragma once

// The library parts a design uses (mitcad#418): the part picker's "In this
// design" group. The model answers with the `library_usage` query
// (core/model/src/api/commands.md, "The library parts a design uses").

#include <QList>
#include <QString>

namespace mitcad {

class CommandContext;

// The library parts a design uses (mitcad#418): BUILD sources and inserted
// library components, each with how many times it is used.
struct UsedPart {
  QString library;    // the library's id as library_search gives it
  QString url;        // its URL, empty when the id is enough
  QString component;  // the component's id in the library
  int count = 0;      // occurrences in the design
};

// In the order of the design's parts list; empty when the model cannot
// answer (the query refused is logged).
QList<UsedPart> usedLibraryParts(const CommandContext& context);

} // namespace mitcad
