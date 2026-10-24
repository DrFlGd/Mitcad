// SPDX-License-Identifier: MIT
#pragma once

// The selection card's Create row (mitcad#619; app/COMMANDS.md, "What the
// framework does"): up to four CREATE commands that fit the selection, as
// buttons under the selection's text. The rule is here, in one place: the
// lists per kind of selection, Extrude first, and whether a command can
// use the whole selection.

#include <array>
#include <functional>

#include <QGroupBox>
#include <QString>
#include <QStringList>
#include <QVector>

#include "Selection.hpp"

class QToolButton;

namespace mitcad {

struct CommandDef;

// How many buttons the row has at most.
constexpr int kSelectionCreateButtons = 4;

// The kinds of selection the row has a list for. Every item of the
// selection must be of the kind; a mixed selection is None.
enum class SelectionCreateKind {
  None,
  Profiles,     // sketch profiles, one or several
  PlanarFaces,  // planar faces of bodies
  Round,        // cylindrical faces and circular edges
  SketchPoints, // sketch points
  Path,         // sketch curves and edges (not all of them round)
  Bodies,       // bodies
  Planes,       // construction and origin planes
};

SelectionCreateKind selectionCreateKind(const Selection& selection);

// The command ids the row tries for a selection, in the row's order:
// Extrude first, then the kind's list (Loft only with two or more
// profiles). Empty for an empty or mixed selection.
QStringList selectionCreateCandidates(const Selection& selection);

// The command ids the row shows: the candidates that `fits` accepts (the
// command is enabled and can use the whole selection), the first
// kSelectionCreateButtons of them.
QStringList selectionCreateCommands(const Selection& selection, const std::function<bool(const QString& id)>& fits);

// Whether a command can use the whole selection, as it is selected before
// the command: a feature command whose first selection input is shown at
// the start and takes every item, as many as it takes, and which the
// definition offers for the selection (offeredFor; surface-only commands
// are never offered here); Create Sketch for one plane or planar face.
// Whether the command is enabled is the caller's.
bool takesWholeSelection(const CommandDef& def, const Selection& selection);

// The row's log line text: "Extrude | Revolve | Sweep", or "none".
QString selectionCreateLogText(const QStringList& names);

// The row: a "Create" group of up to four buttons with the command's icon
// and name. A click asks for the command (commandRequested); the window
// starts it as the ribbon does, with the selection as its input. The
// buttons are made once and changed in place, so the row does not flicker
// when the selection changes; it is hidden when no command fits.
class SelectionCreatePanel : public QGroupBox {
  Q_OBJECT

public:
  struct Entry {
    QString id;
    QString name;
    QString icon;
    QString tooltip;
    bool operator==(const Entry& other) const {
      return id == other.id && name == other.name && icon == other.icon && tooltip == other.tooltip;
    }
    bool operator!=(const Entry& other) const { return !(*this == other); }
  };

  explicit SelectionCreatePanel(QWidget* parent = nullptr);
  // Shows the entries (the first kSelectionCreateButtons); true when they
  // changed. Hidden when there are none.
  bool setEntries(const QVector<Entry>& entries);
  const QVector<Entry>& entries() const { return m_entries; }
  QToolButton* button(int index) const;
  // The Docked chrome: `title` (a section title) over the buttons instead
  // of the group box's frame and title, as AutoselectPanel's. The panel
  // takes it.
  void setSectionTitle(QWidget* title);
  // A name on one or two lines, as the button shows it: a name of several
  // words is broken at the space nearest its middle.
  static QString buttonText(const QString& name);

signals:
  void commandRequested(const QString& id);

protected:
  void paintEvent(QPaintEvent* event) override;

private:
  QVector<Entry> m_entries;
  std::array<QToolButton*, kSelectionCreateButtons> m_buttons{};
  bool m_section = false; // setSectionTitle: no frame of its own
};

} // namespace mitcad
