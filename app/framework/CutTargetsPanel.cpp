// SPDX-License-Identifier: MIT
#include "CutTargetsPanel.hpp"

#include <QEvent>
#include <QHeaderView>
#include <QHideEvent>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QPushButton>
#include <QScrollBar>
#include <QSet>
#include <QSignalBlocker>
#include <QTreeWidget>
#include <QVBoxLayout>
#include <QtLogging>

#include "../platform/NativeDialogs.hpp"

namespace mitcad {
namespace {
constexpr int kTargetRole = Qt::UserRole;
constexpr int kBodyRole = Qt::UserRole + 1;
constexpr int kBodiesRole = Qt::UserRole + 2;

QJsonObject identity(const QJsonObject& row) {
  return {{QStringLiteral("component"), row.value(QStringLiteral("component"))},
          {QStringLiteral("occurrence"), row.value(QStringLiteral("occurrence"))}};
}

QJsonArray membership(QJsonArray values, const QJsonObject& item, bool contains) {
  QJsonArray result;
  for (const QJsonValue& value : values) {
    if (value.toObject() != item) { result.append(value); }
  }
  if (contains) { result.append(item); }
  return result;
}
} // namespace

QJsonObject setCutTargetChecked(const QJsonObject& scope, const QJsonObject& target,
                                const QString& body, bool checked) {
  QJsonObject result = scope;
  QJsonObject targets = result.value(QStringLiteral("targets")).toObject();
  const QJsonObject component = identity(target);
  if (body.isEmpty()) {
    const bool automatic = targets.value(QStringLiteral("type")).toString() == QStringLiteral("automatic");
    const QString key = automatic ? QStringLiteral("excluded") : QStringLiteral("included");
    targets.insert(key, membership(targets.value(key).toArray(), component, automatic ? !checked : checked));
    if (checked) {
      QJsonArray exceptions;
      for (const QJsonValue& value : targets.value(QStringLiteral("excluded_bodies")).toArray()) {
        if (identity(value.toObject()) != component) { exceptions.append(value); }
      }
      targets.insert(QStringLiteral("excluded_bodies"), exceptions);
    }
  } else {
    QJsonObject item = component;
    item.insert(QStringLiteral("body"), body);
    targets.insert(QStringLiteral("excluded_bodies"),
                   membership(targets.value(QStringLiteral("excluded_bodies")).toArray(), item, !checked));
  }
  result.insert(QStringLiteral("targets"), targets);
  return result;
}

QJsonObject selectCutTargets(const QJsonObject& scope, bool all) {
  QJsonObject result = scope;
  result.insert(QStringLiteral("targets"),
                QJsonObject{{QStringLiteral("type"), all ? QStringLiteral("automatic") : QStringLiteral("explicit")},
                            {all ? QStringLiteral("excluded") : QStringLiteral("included"), QJsonArray()},
                            {QStringLiteral("excluded_bodies"), QJsonArray()}});
  return result;
}

CutTargetsPanel::CutTargetsPanel(QWidget* parent) : QWidget(parent) {
  auto* layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  m_status = new QLabel(tr("Select profiles to find affected components."));
  m_status->setObjectName(QStringLiteral("cut_targets_status"));
  m_status->setWordWrap(true);
  layout->addWidget(m_status);
  m_tree = new QTreeWidget;
  m_tree->setObjectName(QStringLiteral("cut_targets_list"));
  m_tree->setProperty("nativeModelHoverEvents", true);
  m_tree->setHeaderHidden(true);
  m_tree->setColumnCount(1);
  m_tree->setMinimumHeight(120);
  m_tree->setMaximumHeight(180);
  m_tree->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Fixed);
  m_tree->setMouseTracking(true);
  m_tree->viewport()->installEventFilter(this);
  m_tree->installEventFilter(this);
  m_tree->setTextElideMode(Qt::ElideMiddle);
  m_tree->setSelectionMode(QAbstractItemView::SingleSelection);
  m_tree->setEditTriggers(QAbstractItemView::NoEditTriggers);
  m_tree->header()->setStretchLastSection(true);
  layout->addWidget(m_tree);
  auto* buttons = new QHBoxLayout;
  m_all = new QPushButton(tr("Select all"));
  m_all->setObjectName(QStringLiteral("cut_targets_all"));
  m_none = new QPushButton(tr("Select none"));
  m_none->setObjectName(QStringLiteral("cut_targets_none"));
  buttons->addWidget(m_all);
  buttons->addWidget(m_none);
  layout->addLayout(buttons);
  connect(m_all, &QPushButton::clicked, this, [this] { emit scopeChanged(selectCutTargets(m_scope, true)); });
  connect(m_none, &QPushButton::clicked, this, [this] { emit scopeChanged(selectCutTargets(m_scope, false)); });
  connect(m_tree, &QTreeWidget::itemChanged, this, [this](QTreeWidgetItem* item, int) {
    if (m_refreshing || !m_current) { return; }
    emit scopeChanged(setCutTargetChecked(m_scope, item->data(0, kTargetRole).toJsonObject(),
                                          item->data(0, kBodyRole).toString(), item->checkState(0) != Qt::Unchecked));
  });
  connect(m_tree, &QAbstractItemView::entered, this, &CutTargetsPanel::highlight);
  connect(m_tree->selectionModel(), &QItemSelectionModel::currentChanged, this,
          [this](const QModelIndex& index, const QModelIndex&) { highlight(index); });
  rebuild();
}

void CutTargetsPanel::setScope(const QJsonObject& scope) { m_scope = scope; }

void CutTargetsPanel::setPreview(const QJsonObject& report) {
  m_current = !report.isEmpty();
  emit highlightChanged({});
  m_tree->setEnabled(m_current);
  if (!m_current) {
    // A checkbox callback may still be on the model's stack. Retain its
    // items until the next asynchronous result; stale rows cannot be used.
    m_all->setEnabled(false);
    m_none->setEnabled(false);
    m_status->setText(tr("Updating affected components…"));
    return;
  }
  m_report = report;
  rebuild();
}

void CutTargetsPanel::rebuild() {
  m_refreshing = true;
  const QSignalBlocker blocker(m_tree);
  const int scroll = m_tree->verticalScrollBar()->value();
  QSet<QString> expanded;
  for (int i = 0; i < m_tree->topLevelItemCount(); ++i) {
    auto* item = m_tree->topLevelItem(i);
    if (item->isExpanded()) { expanded.insert(item->data(0, NativeControlIdentifierRole).toString()); }
  }
  m_tree->clear();
  const QJsonArray rows = m_report.value(QStringLiteral("rows")).toArray();
  for (const QJsonValue& value : rows) {
    const QJsonObject row = value.toObject();
    const QString path = row.value(QStringLiteral("occurrence")).toString();
    const QString component = row.value(QStringLiteral("component")).toString();
    const QString name = row.value(QStringLiteral("path_name")).toString(row.value(QStringLiteral("name")).toString(component));
    const QString reason = row.value(QStringLiteral("reason")).toString();
    const bool eligible = row.value(QStringLiteral("eligible")).toBool();
    const bool intersects = row.value(QStringLiteral("intersects")).toBool();
    QString text = name;
    if (!intersects) { text += tr(" — outside current cut"); }
    if (!reason.isEmpty()) { text += QStringLiteral(" — ") + reason; }
    auto* item = new QTreeWidgetItem(m_tree, {text});
    item->setToolTip(0, text + QStringLiteral("\n") + component + QStringLiteral(" @ ") + (path.isEmpty() ? tr("Root") : path));
    item->setData(0, kTargetRole, identity(row));
    item->setData(0, kBodiesRole, row.value(QStringLiteral("bodies")).toArray());
    item->setData(0, NativeControlIdentifierRole, QStringLiteral("cut_target_") + component + QLatin1Char('@') + path);
    item->setFlags(Qt::ItemIsSelectable | Qt::ItemIsUserCheckable | (m_current && eligible ? Qt::ItemIsEnabled : Qt::NoItemFlags));
    item->setCheckState(0, row.value(QStringLiteral("partial")).toBool() ? Qt::PartiallyChecked
                         : row.value(QStringLiteral("checked")).toBool() ? Qt::Checked : Qt::Unchecked);
    for (const QJsonValue& bodyValue : row.value(QStringLiteral("bodies")).toArray()) {
      const QJsonObject body = bodyValue.toObject();
      const QString uid = body.value(QStringLiteral("body")).toString();
      auto* child = new QTreeWidgetItem(item, {body.value(QStringLiteral("name")).toString(uid)});
      child->setData(0, kTargetRole, identity(row));
      child->setData(0, kBodyRole, uid);
      child->setData(0, kBodiesRole, QJsonArray{body});
      child->setData(0, NativeControlIdentifierRole, QStringLiteral("cut_body_") + uid + QLatin1Char('@') + path);
      const QJsonObject scope = m_scope.value(QStringLiteral("targets")).toObject();
      const bool automatic = scope.value(QStringLiteral("type")) == QStringLiteral("automatic");
      const QJsonArray membership = scope.value(automatic ? QStringLiteral("excluded") : QStringLiteral("included")).toArray();
      const bool included = automatic ? !membership.contains(identity(row)) : membership.contains(identity(row));
      child->setFlags(Qt::ItemIsSelectable | Qt::ItemIsUserCheckable |
                      (m_current && eligible && included ? Qt::ItemIsEnabled : Qt::NoItemFlags));
      child->setCheckState(0, body.value(QStringLiteral("checked")).toBool() ? Qt::Checked : Qt::Unchecked);
      child->setToolTip(0, item->toolTip(0) + QStringLiteral("\n") + uid);
    }
    item->setExpanded(expanded.contains(item->data(0, NativeControlIdentifierRole).toString()));
    qDebug().noquote() << QStringLiteral("Cut target %1@%2: %3, %4, %5")
        .arg(component, path, eligible ? QStringLiteral("eligible") : QStringLiteral("unavailable"),
             item->checkState(0) == Qt::Checked ? QStringLiteral("checked") : item->checkState(0) == Qt::PartiallyChecked
                 ? QStringLiteral("partial") : QStringLiteral("unchecked"), intersects ? QStringLiteral("intersects") : QStringLiteral("outside"));
  }
  m_tree->verticalScrollBar()->setValue(scroll);
  m_all->setEnabled(m_current);
  m_none->setEnabled(m_current);
  const int selected = m_report.value(QStringLiteral("selected_components")).toInt();
  m_status->setText(!m_current ? tr("Update the inputs to preview affected components.")
      : selected > 0 ? tr("%1 components selected (%2 bodies)").arg(selected).arg(m_report.value(QStringLiteral("selected_bodies")).toInt())
      : rows.isEmpty() ? tr("No eligible components intersect the cut.") : tr("No components selected."));
  m_refreshing = false;
}

void CutTargetsPanel::highlight(const QModelIndex& index) {
  Selection items;
  if (m_current && !m_refreshing && index.isValid()) {
    const QString occurrence = index.data(kTargetRole).toJsonObject().value(QStringLiteral("occurrence")).toString();
    for (const QJsonValue& value : index.data(kBodiesRole).toJsonArray()) {
      items.append({SelectKind::Body, value.toObject().value(QStringLiteral("body")).toString(), QString(), QString(), occurrence});
    }
    if (items.isEmpty() && index.data(kBodyRole).toString().isEmpty()) {
      items.append({SelectKind::Component, index.data(kTargetRole).toJsonObject().value(QStringLiteral("component")).toString(),
                    QString(), QString(), occurrence});
    }
  }
  qDebug().noquote() << QStringLiteral("Cut target highlight: %1 item(s)").arg(items.size());
  emit highlightChanged(items);
}

bool CutTargetsPanel::eventFilter(QObject* watched, QEvent* event) {
  if (event->type() == QEvent::Leave || event->type() == QEvent::FocusOut) { emit highlightChanged({}); }
  return QWidget::eventFilter(watched, event);
}

void CutTargetsPanel::hideEvent(QHideEvent* event) {
  emit highlightChanged({});
  QWidget::hideEvent(event);
}
} // namespace mitcad
