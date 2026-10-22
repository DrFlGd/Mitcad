// SPDX-License-Identifier: MIT
#pragma once

#include <optional>

#include <QRegularExpression>
#include <QSet>
#include <QString>
#include <QVector>

#include "DocumentSnapshot.hpp"

namespace mitcad {

// What Find in Design lists (mitcad#350): a component placed in the design
// (an occurrence) or a body, once per place it is seen in: a body of a
// component that is placed twice is two results with one uid.
struct FindResult {
  enum class Kind { Component, Body };

  Kind kind = Kind::Body;
  QString name;
  // The body's uid (F2.b0), or the component's (C1).
  QString uid;
  // The occurrence that places a body or a component ("O1/O4"); empty for
  // the root component's bodies.
  QString occurrence;
  // The component a body is in, or an occurrence places.
  QString component;
  // The names of the parent occurrences from the root, "Root/Frame:1": the
  // place the result is in.
  QString path;
  // Shown, as the browser's row shows it: the body's own light bulb and
  // every occurrence above it.
  bool visible = true;

  // A key that is the same for the same result in another snapshot.
  QString key() const;
};

// What the search window asks for.
struct FindOptions {
  // Substrings, case-insensitive; `*` and `?` are wildcards. Empty: every name.
  QString text;
  bool components = true;
  bool bodies = true;
  enum class Visibility { Any, Visible, Hidden };
  Visibility visibility = Visibility::Any;
  // A property search (mitcad#496, `FireRating = EI60`): only the owners
  // the model's `property_search` found (components, occurrences and bodies
  // by uid) match, whatever their names; unset, the names do.
  std::optional<QSet<QString>> propertyOwners;
};

// Whether a search text asks for property values (`FireRating = EI60`,
// `Mass > 10 kg`, `Name ~ wall`: it has a comparison) rather than names
// (mitcad#496).
bool isPropertySearch(const QString& text);

// A name filter: the text as a case-insensitive substring, with `*` (any
// characters) and `?` (one character) as wildcards.
class NameMatcher {
public:
  explicit NameMatcher(const QString& text);
  bool matches(const QString& name) const;

private:
  QString m_text;
  bool m_wildcards = false;
  QRegularExpression m_pattern;
};

// The components (occurrences) and bodies whose names match, from the
// model's snapshot of names (not from the browser's rows, so branches that
// were never expanded are searched too), in the browser's order: the root's
// bodies, then each occurrence followed by its bodies and its children.
QVector<FindResult> findInDesign(const DocumentSnapshot& snapshot, const FindOptions& options);

} // namespace mitcad
