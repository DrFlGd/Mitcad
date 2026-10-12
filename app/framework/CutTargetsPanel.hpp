// SPDX-License-Identifier: MIT
#pragma once

#include <QJsonObject>
#include <QWidget>

#include "Selection.hpp"

class QLabel;
class QModelIndex;
class QPushButton;
class QTreeWidget;
class QTreeWidgetItem;

namespace mitcad {

// Pure scope edits: neither a changed preview nor an empty candidate list
// changes these persistent identities.
QJsonObject setCutTargetChecked(const QJsonObject& scope, const QJsonObject& target,
                                const QString& body, bool checked);
QJsonObject selectCutTargets(const QJsonObject& scope, bool all);

class CutTargetsPanel final : public QWidget {
  Q_OBJECT
public:
  explicit CutTargetsPanel(QWidget* parent = nullptr);
  void setScope(const QJsonObject& scope);
  void setPreview(const QJsonObject& report);

signals:
  void scopeChanged(const QJsonObject& scope);
  void highlightChanged(const mitcad::Selection& items);

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void hideEvent(QHideEvent* event) override;

private:
  void rebuild();
  void highlight(const QModelIndex& index);
  QJsonObject m_scope;
  QJsonObject m_report;
  bool m_current = false;
  bool m_refreshing = false;
  QTreeWidget* m_tree = nullptr;
  QLabel* m_status = nullptr;
  QPushButton* m_all = nullptr;
  QPushButton* m_none = nullptr;
};

} // namespace mitcad
