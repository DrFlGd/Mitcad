// SPDX-License-Identifier: MIT
#pragma once

// Sheet templates of drawings (mitcad#707 phase 4, app/COMMANDS.md
// "Drawings"): the choices the model's `drawing_templates` query lists (the
// built-in ISO and ANSI, the user's and the project's) and the simple title
// block, each with what the model's `template` field takes for it. Plain
// Qt Core, so that the tests check it without a window.

#include <QJsonObject>
#include <QJsonValue>
#include <QList>
#include <QString>

namespace mitcad::drawings {

struct TemplateChoice {
  // The model's choice: "iso", "user:company-a3", "" for the simple block.
  QString choice;
  // As the dialogs show it: "ISO", "Company A3 (User)", "Simple Title Block".
  QString label;
  // The sheet size it starts with ("A3"), empty when none.
  QString size;
  // The projection method it starts with ("first_angle", mitcad#725),
  // empty when none.
  QString projection;
  // What `add_drawing`'s and `edit_drawing_sheet`'s `template` takes: a
  // built-in one's id, a user's or a project's data, "" for none.
  QJsonValue value;
};

// The choices of a `drawing_templates` answer, in its order, then the
// simple title block.
QList<TemplateChoice> templateChoices(const QJsonObject& answer);

// The index of a choice, or -1.
int templateIndex(const QList<TemplateChoice>& choices, const QString& choice);

// The template a new drawing starts with on a sheet of this size: ANSI on
// the ANSI sizes, else ISO.
QString defaultTemplate(const QString& size);

} // namespace mitcad::drawings
