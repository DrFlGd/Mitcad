// SPDX-License-Identifier: MIT
#pragma once

#include <functional>

#include <QDialog>
#include <QJsonArray>
#include <QJsonObject>
#include <QString>
#include <QVector>

class QCheckBox;
class QLabel;
class QPushButton;

namespace mitcad {

// Design Policy (mitcad#220): a component's (or the root's) seven
// settings, each with what it means in words (incoming and outgoing links
// told apart), the Assembly, Part and Defaults presets, and next to each
// blocked setting how many objects break it, a button that lists them.
// Counts follow the settings as edited (the model's `policy` query with
// the proposed policy); nothing changes until OK, which the caller turns
// into one `set_policy` command. Changing a policy changes no content.
class PolicyDialog : public QDialog {
  Q_OBJECT

public:
  using Query = std::function<QJsonObject(const QJsonObject&)>;
  PolicyDialog(Query query, const QString& component, QWidget* parent = nullptr);

  // The settings as edited: the `policy` object of set_policy.
  QJsonObject policy() const;
  // Whether they differ from the component's.
  bool changed() const;

private:
  struct Row {
    QString key;
    QCheckBox* box = nullptr;
    QLabel* explanation = nullptr;
    QPushButton* count = nullptr;
    QJsonArray items;
  };

  void applyPreset(const QString& preset);
  void refresh();
  void showItems(int row);

  Query m_query;
  QString m_component;
  QString m_name;
  QJsonObject m_initial;
  QVector<Row> m_rows;
  QLabel* m_preset = nullptr;
  QLabel* m_message = nullptr;
  bool m_building = false;
  QString m_logged;
};

} // namespace mitcad
