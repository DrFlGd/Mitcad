// SPDX-License-Identifier: MIT
#pragma once

// The parts the BUILD tab places (mitcad#375; docs/libraries.md, "BUILD
// parts"): library components with a `build` block, from the fetched
// libraries (the library commands `library_build_candidates` and
// `library_search`) and the sources the design keeps already (the
// `library_parts` query), so that a part placed once can be placed again
// without its library. The app reads them only through those commands and
// queries.

#include <mutex>

#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QPair>
#include <QString>
#include <QStringList>
#include <QVector>

namespace mitcad {

class CommandContext;

namespace build {

using Choices = QVector<QPair<QString, QString>>;

// A part a picker offers: a library component in one configuration row
// (none: its default), or a source of the design.
struct Part {
  QString key;        // the picker's value: "<library>|<component>|<row>" or "source:<S1>|<row>"
  QString library;    // the library's id
  QString url;        // where it was fetched from
  QString rev;        // the commit
  QString version;    // "v1.0.0 (3f9a2c1)", for people
  QString component;  // the component's id
  QString config;     // the configuration row, empty for the default
  QString source;     // the design's source (S1) when it has one
  QString name;       // the component's name
  QString designation;
  QString supplier;
  QString partNumber;
  QString supplierUrl;
  QJsonObject build;  // the manifest's `build`
  bool ownPreview = false; // the row has a picture of its own (`row_previews`, mitcad#484)
  bool defaultRow = false; // `config` is its component's default row (mitcad#491)
  // The component's configuration table as the library gives it; an open
  // one is chosen by its options in the part picker (mitcad#810).
  QJsonObject table;
  // The options chosen ({"Length": "800 mm"}): `build_place`'s
  // `part.parameters`; empty for a row as it is. A part with options has
  // an options key (optionsKey).
  QJsonObject parameters;
  // Whether its table is open: options, not only rows.
  bool openTable() const;

  QString role() const;
  // "B8 30" (system, slot, grid), or the systems a connector fits.
  QString systemText() const;
  // The section, "30x30", or empty.
  QString sizeText() const;
  // "4 slots" (the slot frames), or empty.
  QString slotsText() const;
  // How the picker lists it: "Profile 30x30 B-type slot 8 - B8 30, 4 slots - Motedis P30_N8_B".
  QString label() const;
  // `build_place`'s `part`: the source when the design has it, else the
  // library reference.
  QJsonObject ref() const;
  // Whether every word is in its name, designation, ids, supplier, part
  // number, system, size or library.
  bool matches(const QString& words) const;
  // Cut to length (a `length` block): profiles, rails, cut plates.
  bool cutToLength() const;
  // The length a new one starts with, mm: the length block's `default`,
  // else its shortest stock length, else 500 (mitcad#394).
  double defaultLength() const;
  // The section's width and height in millimetres (0 when unknown).
  double sectionWidth() const;
  double sectionHeight() const;

  // Timber construction (mitcad#478; docs/libraries.md, "Timber parts"):
  // `build` is the component's block with its row's block (`build.rows`)
  // merged over it.
  // A timber member (`kind: timber`).
  bool isTimber() const;
  // A row the picker lists after the standard sizes (`availability: less
  // common`).
  bool lessCommon() const;
  // A wood screw or nail (a `wood` block).
  bool isWoodFastener() const;
  // A connector on the members' outer faces (`connector.placement: outer`).
  bool isOuter() const;
  // A sheet laid on a member's face (`panel.mount: face`).
  bool isFaceSheet() const;

  // Profiled boards (mitcad#477; docs/libraries.md, "Timber parts"): the
  // `board` block's profile ("tongue_and_groove", "shiplap",
  // "feather_edge", "deck"; empty for others), and the width a board that
  // joins the next covers in a row (0 for one laid with a gap).
  QString boardProfile() const;
  double joinedCover() const;
};

// "30 mm" -> 30; a plain number is millimetres; NaN when it is not one.
double millimetres(const QJsonValue& value);

// A part of an open table with options chosen in the part picker
// (mitcad#810) is chosen by an options key: its tile's key with the row the
// combination matches (empty for none) in place of the tile's row, then '?'
// and the options as compact JSON: "lib|shelf-board|?{"Color":"'Black'"}".
// Catalog::find gives the part with those options.
QString optionsKey(const QString& tileKey, const QString& row, const QJsonObject& parameters);
// The key without its options ("lib|shelf-board|"), and the options.
QString baseKey(const QString& key);
QJsonObject keyOptions(const QString& key);
// The key's part without its row: "lib|shelf-board", "source:S1".
QString keyPrefix(const QString& key);

// The parts of the pickers and the connectors of junctions, read when a
// command starts and after it placed something. Shared by the commands'
// definitions (their lambdas), so guarded: an inspection may read it on
// the model's worker.
class Catalog {
public:
  // Reads the parts of a role ("profile", "panel", "linear", ...): the
  // design's sources first (they need no library), then the fetched
  // libraries'. Logs "BUILD parts <role>: <n> (<m> in the design)".
  // Timber (mitcad#478) has pickers of its own: "timber" (the members,
  // `role: profile` with `kind: timber`, the standard sizes first) and
  // "sheet" (`role: panel` with `mount: face`); "profile" lists no timber
  // and "panel" no sheets.
  void refresh(const QString& role, const CommandContext& context);
  QVector<Part> parts(const QString& role) const;
  // A part by its key, from any role read; an options key (optionsKey)
  // gives its component's part in the key's row with the key's options.
  bool find(const QString& key, Part& part) const;
  // Why nothing is listed, when the library commands said so.
  QString problem(const QString& role) const;

  // The connectors that fit a junction (an entry of `build_junctions`),
  // best first, and those that nearly fit with the reason
  // (library_build_candidates); asked once per junction until refresh. A
  // junction of two timber members (its `wood`, mitcad#478) lists each wood
  // fastener once, without a row (the model sizes it from the members), and
  // of an outer connector the rows that fit member a's section.
  QVector<Part> forJunction(const QJsonObject& junction, QStringList* refused = nullptr);
  // The configuration rows of a wood fastener forJunction lists once, by
  // its key (mitcad#478).
  QStringList rowsOf(const QString& key) const;

private:
  mutable std::mutex m_mutex;
  QHash<QString, QVector<Part>> m_byRole;
  QHash<QString, QString> m_problems;
  QHash<QString, QVector<Part>> m_byJunction;
  QHash<QString, QStringList> m_refused;
  QHash<QString, QStringList> m_rows;
  // The design's sources: by "<library>|<component>", their id.
  QHash<QString, QString> m_sources;
};

// The picker's options: the parts whose words match and whose system is
// `system` (empty: any), the chosen one always among them (a part with
// options chosen in place of its tile, by its options key).
Choices partChoices(const QVector<Part>& parts, const QString& words, const QString& system,
                    const QString& current);
// The systems of the parts, for the picker's filter ("" all of them first).
Choices systemChoices(const QVector<Part>& parts);

} // namespace build
} // namespace mitcad
