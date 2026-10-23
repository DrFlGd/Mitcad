// SPDX-License-Identifier: MIT
#pragma once

// The sketches a command shows for its time (mitcad#546). A sketch a
// feature uses is hidden while its light bulb is not set (mitcad#7), so an
// edit panel would show the result but not the profile, path or points it
// comes from. While a feature is edited, the sketches it uses (the
// `timeline` query's `sketches` of it) are shown as if their light bulbs
// were on, with their regions, curves and points pickable; a new feature's
// command does the same for the sketches of the profiles, curves and
// points it starts with (a profile of a hidden sketch selected before the
// command). When the command ends (OK, Cancel, Esc, another command) each
// goes back to what the model says: hidden again unless the user turned
// its light bulb on meanwhile. A view state of the window: no model
// command, nothing saved, no undo step.
//
// Plain Qt Core, for app.unit.

#include <functional>
#include <utility>
#include <vector>

#include <QString>
#include <QStringList>

#include "../framework/Selection.hpp"

namespace mitcad {

class CommandSketches {
public:
  // Whether the model shows a sketch now (DocumentSnapshot::isShown).
  using Shown = std::function<bool(const QString& sketch)>;

  // The sketches a command uses: an edited feature's (`edited`, its
  // `sketches`), else the owners of the profiles, sketch curves and sketch
  // points selected before a new command; each once, in that order.
  static QStringList usedBy(const QStringList& edited, const Selection& preselection) {
    QStringList used;
    const auto add = [&used](const QString& sketch) {
      if (!sketch.isEmpty() && !used.contains(sketch)) {
        used << sketch;
      }
    };
    for (const QString& sketch : edited) {
      add(sketch);
    }
    if (used.isEmpty()) {
      for (const SelectionItem& item : preselection) {
        if (item.kind == SelectKind::Profile || item.kind == SelectKind::SketchCurve ||
            item.kind == SelectKind::SketchPoint) {
          add(item.owner);
        }
      }
    }
    return used;
  }

  // A command starts with the sketches it uses; `label` names it for the
  // logs ("the edit of Extrude1", "Extrude"). Returns those the model hides
  // now: shown only for the command (logged).
  QStringList begin(const QStringList& used, const Shown& shown, const QString& label = QString()) {
    m_used = used;
    m_label = label;
    m_temporary.clear();
    for (const QString& sketch : used) {
      if (!shown(sketch)) {
        m_temporary << sketch;
      }
    }
    return m_temporary;
  }

  // Whether the view shows a sketch the model shows or hides
  // (`shownByModel`): also while the running command uses it.
  bool shows(const QString& sketch, bool shownByModel) const {
    return shownByModel || m_used.contains(sketch);
  }
  // Shown only for the command (drawn lighter): its light bulb is off.
  bool temporary(const QString& sketch, bool shownByModel) const {
    return !shownByModel && m_used.contains(sketch);
  }
  // The sketches the running command uses, shown or not by the model.
  const QStringList& used() const { return m_used; }
  // Those the model hid when it started, and the command's label.
  const QStringList& temporaries() const { return m_temporary; }
  const QString& label() const { return m_label; }

  struct Ended {
    QString sketch;
    // Its light bulb was turned on meanwhile: it stays shown.
    bool staysShown = false;
  };
  // The command ended: each sketch it showed that the model hid when it
  // started, and whether the model shows it now. Nothing is shown for the
  // command any more.
  std::vector<Ended> end(const Shown& shown) {
    std::vector<Ended> ended;
    for (const QString& sketch : std::as_const(m_temporary)) {
      ended.push_back({sketch, shown(sketch)});
    }
    m_used.clear();
    m_temporary.clear();
    m_label.clear();
    return ended;
  }

private:
  QStringList m_used;
  QStringList m_temporary; // hidden by the model when the command started
  QString m_label;
};

} // namespace mitcad
