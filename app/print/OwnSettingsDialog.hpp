// SPDX-License-Identifier: MIT
#pragma once

#include <QDialog>
#include <QJsonObject>
#include <QString>

#include "PrintKeys.hpp"

class QLabel;
class QPushButton;

namespace mitcad {

class DocumentHost;
class SettingsForm;

// A body's, an occurrence's or a component's own process settings
// (mitcad#655), edited in the same tabbed settings form as a process, as a
// slicer edits an object's settings: a small non-modal window titled
// "Body1 · Own settings" that follows the model. Every process key can be set
// from its tab; the owner's own values show set, the rest show what the
// owner prints with as their placeholders. A change is `set_property` of
// the owner's `Print Process` (emptying a field or ↺ `remove_property`),
// one undo step each. 3D Print's own-settings links and Property Sets'
// Edit… on Process open it.
//
// Logs `Own settings opened: <title>`, `Own setting <owner> <key> = <text>`
// (`cleared`), refusals `Own settings: <key> refused: <reason>` and the
// form's places as `Own settings field <key> at x,y` (`clear <key>`, `tab
// <category>`, `search`, `advanced`) with `Own settings close at x,y`.
class OwnSettingsDialog : public QDialog {
  Q_OBJECT

public:
  OwnSettingsDialog(DocumentHost& host, QWidget* parent = nullptr);
  ~OwnSettingsDialog() override;

  // Whose settings: the owner's uid (a body, an occurrence, a component)
  // and its name as the design shows it.
  void setOwner(const QString& owner, const QString& name);
  QString owner() const { return m_owner; }
  // Shows the owner's values as the model has them now.
  void refresh();

protected:
  void showEvent(QShowEvent* event) override;

private:
  // The `print_settings` query of the owner.
  QJsonObject query() const;
  void edit(const QString& key, const QJsonValue& value);
  void scheduleLogPlaces();
  void logPlaces();

  DocumentHost& m_host;
  PrintKeyTable m_keys;
  QString m_owner;
  QString m_name;
  SettingsForm* m_form = nullptr;
  QLabel* m_message = nullptr;
  QPushButton* m_close = nullptr;
  QString m_loggedPlaces;
};

} // namespace mitcad
