// SPDX-License-Identifier: MIT
#pragma once

// Change Parameters while a sketch or a feature is being created or edited
// (mitcad#564): what is open while the parameter table changes the
// parameters, what the table leaves to an open panel, and what a change
// means for that panel. Plain Qt Core: the rules without the widgets.

#include <QJsonObject>
#include <QString>

namespace mitcad {

// What is being edited while the parameter table is open: nothing (empty),
// a sketch in sketch mode, a command's panel, or a panel of a command that
// runs in a sketch (Sketch Fillet).
struct ParameterEditScope {
  QString sketch;  // the name of the sketch edited ("Sketch1"), else empty
  QString command; // the name of the command whose panel is open ("Extrude")
  // The feature that panel edits (its uid, "F2") and its name
  // ("Extrude1"); empty for a new feature. Its own parameters (the
  // extrusion's distance d3) are the panel's until OK or Cancel.
  QString editedFeature;
  QString editedName;

  bool isEmpty() const { return sketch.isEmpty() && command.isEmpty(); }
  bool operator==(const ParameterEditScope& other) const;
  bool operator!=(const ParameterEditScope& other) const { return !(*this == other); }

  // For the log: "no edit", "sketch Sketch1", "command Extrude", "command
  // Extrude editing Extrude1", "command Sketch Fillet in sketch Sketch1".
  QString describe() const;
  // The line the parameter table shows above its rows: what stays open
  // and how to go back to it; empty outside edits.
  QString note() const;
  // Whether the open panel holds a parameter (an entry of the `parameters`
  // query: its `owner` is the feature edited): its name and expression are
  // the panel's, which OK writes.
  bool holds(const QJsonObject& parameter) const;
  // Why the table does not change such a parameter's name or expression.
  QString holdReason(const QString& name) const;
  // Whether a model command of the table must not run now: it renames,
  // deletes or sets the expression, unit or value of a parameter that the
  // open panel holds (`owner`: the owner of the parameter the command
  // names, empty for a user parameter). `reason` (when given) says why.
  bool refuses(const QJsonObject& command, const QString& owner, QString* reason = nullptr) const;
};

// Whether a command of the parameter table can change values that an open
// panel's expressions use: a new or deleted parameter, a rename, an
// expression, a unit or a value; not a comment or the favourite's star.
bool parameterCommandChangesValues(const QJsonObject& command);

// `text` with each use of the parameter `from` as a whole name replaced by
// `to` ("from * 2 + from_2" with from -> to: "to * 2 + from_2"): an open
// panel's expressions after the table renamed a parameter they use.
QString renameParameterIn(const QString& text, const QString& from, const QString& to);

} // namespace mitcad
