// SPDX-License-Identifier: MIT
#pragma once

// The part picker's groups as a tree (mitcad#527; app/COMMANDS.md, "Part
// picker"; the manifests' side in docs/libraries.md): the libraries'
// categories with their parents and path names, as `library_search` gives
// them for each part (`groups`: the names from the top group down, each
// with its place in its library's list). Groups of several libraries with
// the same names are one group; a part in several groups is listed in
// each, and a group lists its subgroups' parts too. Plain Qt Core, so that
// the unit tests (tests/part_picker_data.cpp) build it on its own.

#include <QHash>
#include <QList>
#include <QString>
#include <QStringList>

namespace mitcad {

// A group a part is in: the category and the group's names from the top
// group down, each with its place in its library's list of categories
// (the groups' order).
struct PartGroupPath {
  QString category;
  QStringList names;
  QList<int> order;
};

// A group's key: its names in lower case with single spaces, joined by
// '/' ("wood/dimensioned timber"). Libraries share a group by its names;
// the stars of groups are kept by their keys.
QString groupKey(const QStringList& names);
// A name for people: an id-like name ("linear-parts") with spaces and its
// first letter capital ("Linear parts"); other names as they are.
QString groupLabel(const QString& name);
// Whether a part of these groups is in the group of `key` or in one of its
// subgroups.
bool inGroup(const QList<PartGroupPath>& groups, const QString& key);

// The tree of the groups of a list of parts.
class PartGroupTree {
public:
  struct Node {
    QString key;
    QString name;            // shown: the label of its last name
    QString path;            // the labels from the top, joined by '/' (logs)
    QString parent;          // the parent's key, empty at the top
    QStringList children;    // the subgroups' keys, in order (after finish)
    QStringList categories;  // the ids of the categories that are this group
    PartGroupPath group;     // its names and order from the top (no category)
    int order = 0;           // the least place of the categories naming it
  };

  void clear();
  // A part's group and the groups above it.
  void add(const PartGroupPath& group);
  // Puts every level in order: by the libraries' order, then by name.
  void finish();
  bool isEmpty() const { return m_top.isEmpty(); }
  bool contains(const QString& key) const { return m_nodes.contains(key); }
  const Node* node(const QString& key) const;
  // The groups at the top, in order.
  const QStringList& top() const { return m_top; }
  // The group and every group below it, each before its subgroups.
  QStringList subtree(const QString& key) const;

private:
  void sortKeys(QStringList& keys) const;

  QHash<QString, Node> m_nodes;
  QStringList m_top;
};

} // namespace mitcad
