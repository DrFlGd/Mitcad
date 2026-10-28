// SPDX-License-Identifier: MIT
#include "SlicerPresetsDialog.hpp"

#include <utility>

#include <QComboBox>
#include <QDialogButtonBox>
#include <QFont>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonValue>
#include <QLabel>
#include <QPoint>
#include <QPushButton>
#include <QRect>
#include <QStyle>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>
#include <QVector>
#include <QtLogging>

#include "../framework/Icons.hpp"
#include "../framework/TestSync.hpp"
#include "../framework/Theme.hpp"
#include "../platform/MacDialogs.hpp"
#include "PrintLibrary.hpp"

namespace mitcad {
namespace {

constexpr int kPathRole = Qt::UserRole;       // a preset row's file; empty on a group
constexpr int kKindRole = Qt::UserRole + 1;   // printer, process, material
constexpr int kChoiceRole = Qt::UserRole + 2; // a same-name row's copy or override; empty else

QString text(const QJsonObject& object, const char* field) {
  return object.value(QLatin1String(field)).toString();
}

// The preset rows of the list, in their order.
QVector<QTreeWidgetItem*> presetRows(QTreeWidget* list) {
  QVector<QTreeWidgetItem*> rows;
  for (QTreeWidgetItemIterator it(list); *it != nullptr; ++it) {
    if (!(*it)->data(0, kPathRole).toString().isEmpty()) {
      rows.append(*it);
    }
  }
  return rows;
}

// The choices of a same-name row and of Same name, in their order.
void addChoices(QComboBox* box) {
  box->addItem(QObject::tr("Copy"), QStringLiteral("copy"));
  box->setItemData(0, QObject::tr("Imports it as a new item under the next free name"), Qt::ToolTipRole);
  box->addItem(QObject::tr("Override"), QStringLiteral("override"));
  box->setItemData(1, QObject::tr("Updates the existing item with its values"), Qt::ToolTipRole);
}

} // namespace

SlicerPresetsDialog::SlicerPresetsDialog(const QJsonArray& slicers, int index, const QString& into, QWidget* parent)
    : QDialog(parent), m_slicers(slicers), m_index(index), m_into(into) {
  setWindowTitle(tr("Import Presets"));
  setWindowIcon(themeIcon(QStringLiteral("print-3d")));
  resize(560, 560);
  auto* layout = new QVBoxLayout(this);

  // The slicer: its name, a box when several were found.
  auto* top = new QHBoxLayout;
  m_slicerLabel = new QLabel;
  m_slicer = new QComboBox;
  m_slicer->setAccessibleName(tr("Slicer"));
  for (const QJsonValue& value : slicers) {
    m_slicer->addItem(slicerTitle(value.toObject()));
  }
  const bool several = slicers.size() > 1;
  m_slicer->setVisible(several);
  if (several) {
    m_slicerLabel->setText(tr("Slicer:"));
  } else {
    QFont bold = m_slicerLabel->font();
    bold.setBold(true);
    m_slicerLabel->setFont(bold);
  }
  top->addWidget(m_slicerLabel);
  top->addWidget(m_slicer);
  top->addStretch(1);
  // Same name: every same-name row's choice at once.
  m_sameLabel = new QLabel(tr("Same name:"));
  m_sameAll = new QComboBox;
  m_sameAll->setAccessibleName(tr("Same name"));
  addChoices(m_sameAll);
  top->addWidget(m_sameLabel);
  top->addWidget(m_sameAll);
  layout->addLayout(top);

  // The presets by kind, a box each.
  m_list = new QTreeWidget;
  m_list->setColumnCount(2);
  m_list->setHeaderHidden(true);
  m_list->setUniformRowHeights(true);
  m_list->setSelectionMode(QAbstractItemView::NoSelection);
  m_list->header()->setStretchLastSection(false);
  m_list->header()->setSectionResizeMode(0, QHeaderView::Stretch);
  m_list->header()->setSectionResizeMode(1, QHeaderView::ResizeToContents);
  mac::styleListTree(m_list);
  layout->addWidget(m_list, 1);

  auto* buttons = new QDialogButtonBox;
  m_import = new QPushButton(tr("&Import"));
  m_cancel = new QPushButton(tr("Cancel"));
  buttons->addButton(m_import, QDialogButtonBox::AcceptRole);
  buttons->addButton(m_cancel, QDialogButtonBox::RejectRole);
  m_import->setDefault(true);
  layout->addWidget(buttons);
  mac::styleDialogButtons(buttons, m_import);
  mac::styleDialog(this);

  connect(m_import, &QPushButton::clicked, this, &QDialog::accept);
  connect(m_cancel, &QPushButton::clicked, this, &QDialog::reject);
  connect(m_slicer, &QComboBox::activated, this, [this](int chosen) { showSlicer(chosen); });
  connect(m_sameAll, &QComboBox::activated, this, [this] {
    const QString choice = m_sameAll->currentData().toString();
    for (QTreeWidgetItem* row : presetRows(m_list)) {
      if (!row->data(0, kChoiceRole).toString().isEmpty()) {
        setRowChoice(row, choice);
      }
    }
    qDebug().noquote() << QStringLiteral("Slicer presets same name: %1").arg(choice);
  });
  connect(m_list, &QTreeWidget::itemChanged, this, [this] {
    if (!m_building) {
      ticksChanged();
    }
  });
  if (!slicers.isEmpty()) {
    showSlicer(index >= 0 && index < slicers.size() ? index : 0);
  }
}

QJsonObject SlicerPresetsDialog::shownSlicer() const {
  return m_index >= 0 && m_index < m_slicers.size() ? m_slicers.at(m_index).toObject() : QJsonObject();
}

QString SlicerPresetsDialog::slicerTitle(const QJsonObject& slicer) {
  const QString name = text(slicer, "name");
  return slicer.value(QStringLiteral("flatpak")).toBool() ? tr("%1 (Flatpak)").arg(name) : name;
}

QString SlicerPresetsDialog::slicerName() const { return text(shownSlicer(), "name"); }

QString SlicerPresetsDialog::sameTooltip(const QString& kind, const QJsonObject& same) const {
  const QString what = kind == QStringLiteral("printer")   ? tr("printer")
                       : kind == QStringLiteral("process") ? tr("process")
                                                           : tr("material");
  return tr("%1 has a %2 of this name").arg(placeName(text(same, "place")), what);
}

void SlicerPresetsDialog::showSlicer(int index) {
  m_index = index;
  const QJsonObject slicer = m_slicers.at(index).toObject();
  if (m_slicers.size() <= 1) {
    m_slicerLabel->setText(slicerTitle(slicer));
  }
  m_building = true;
  m_list->clear();
  const QJsonArray presets = slicer.value(QStringLiteral("presets")).toArray();
  const std::pair<QString, QString> groups[] = {{QStringLiteral("printer"), tr("Printers")},
                                                {QStringLiteral("process"), tr("Processes")},
                                                {QStringLiteral("material"), tr("Filaments")}};
  bool anySame = false;
  for (const auto& [kind, title] : groups) {
    QTreeWidgetItem* group = nullptr;
    for (const QJsonValue& value : presets) {
      const QJsonObject preset = value.toObject();
      if (text(preset, "kind") != kind) {
        continue;
      }
      if (group == nullptr) {
        // The group's box ticks all or none of it.
        group = new QTreeWidgetItem(m_list, {title});
        group->setFlags(Qt::ItemIsEnabled | Qt::ItemIsUserCheckable | Qt::ItemIsAutoTristate);
        group->setCheckState(0, Qt::Unchecked);
        group->setFirstColumnSpanned(true);
        QFont bold = group->font(0);
        bold.setBold(true);
        group->setFont(0, bold);
      }
      auto* row = new QTreeWidgetItem(group, {text(preset, "name")});
      row->setFlags(Qt::ItemIsEnabled | Qt::ItemIsUserCheckable);
      row->setData(0, kPathRole, text(preset, "path"));
      row->setData(0, kKindRole, kind);
      const QJsonObject same = preset.value(QStringLiteral("same")).toObject();
      if (same.isEmpty()) {
        // New ones are ticked at first.
        row->setCheckState(0, Qt::Checked);
        continue;
      }
      // A name the target has: the warning colour, what it meets as the
      // tooltip, unticked, Copy at first (it loses nothing).
      anySame = true;
      const QString tooltip = sameTooltip(kind, same);
      row->setForeground(0, warningColor(palette()));
      row->setToolTip(0, tooltip);
      row->setData(0, kChoiceRole, QStringLiteral("copy"));
      row->setCheckState(0, Qt::Unchecked);
      auto* choice = new QComboBox;
      choice->setAccessibleName(tr("Same name"));
      choice->setToolTip(tooltip);
      addChoices(choice);
      m_list->setItemWidget(row, 1, choice);
      connect(choice, &QComboBox::activated, this, [this, row, choice] {
        setRowChoice(row, choice->currentData().toString());
        qDebug().noquote() << QStringLiteral("Slicer presets choice: %1 %2")
                                  .arg(row->text(0), row->data(0, kChoiceRole).toString());
      });
    }
  }
  m_list->expandAll();
  m_sameLabel->setVisible(anySame);
  m_sameAll->setVisible(anySame);
  m_sameAll->setCurrentIndex(0);
  m_building = false;
  qDebug().noquote() << QStringLiteral("Slicer presets opened: %1, %2 presets")
                            .arg(slicerTitle(slicer))
                            .arg(presets.size());
  m_loggedTicked = -1;
  ticksChanged();
  logRows();
  scheduleLogPlaces();
}

void SlicerPresetsDialog::setRowChoice(QTreeWidgetItem* row, const QString& choice) {
  row->setData(0, kChoiceRole, choice);
  if (auto* box = qobject_cast<QComboBox*>(m_list->itemWidget(row, 1))) {
    const int at = box->findData(choice);
    if (at >= 0 && at != box->currentIndex()) {
      box->setCurrentIndex(at);
    }
  }
}

int SlicerPresetsDialog::tickedCount() const {
  int ticked = 0;
  for (QTreeWidgetItem* row : presetRows(m_list)) {
    if (row->checkState(0) == Qt::Checked) {
      ++ticked;
    }
  }
  return ticked;
}

void SlicerPresetsDialog::ticksChanged() {
  const int ticked = tickedCount();
  m_import->setEnabled(ticked > 0);
  if (ticked != m_loggedTicked) {
    m_loggedTicked = ticked;
    qDebug().noquote() << QStringLiteral("Slicer presets ticked: %1").arg(ticked);
  }
}

QJsonObject SlicerPresetsDialog::command() const {
  const QJsonObject slicer = shownSlicer();
  QJsonArray presets;
  for (QTreeWidgetItem* row : presetRows(m_list)) {
    if (row->checkState(0) != Qt::Checked) {
      continue;
    }
    QJsonObject preset{{QStringLiteral("path"), row->data(0, kPathRole).toString()}};
    const QString choice = row->data(0, kChoiceRole).toString();
    if (!choice.isEmpty()) {
      preset.insert(QStringLiteral("same_name"), choice);
    }
    presets.append(preset);
  }
  return {{QStringLiteral("cmd"), QStringLiteral("import_slicer_presets")},
          {QStringLiteral("dialect"), slicer.value(QStringLiteral("dialect"))},
          {QStringLiteral("folder"), slicer.value(QStringLiteral("folder"))},
          {QStringLiteral("presets"), presets}};
}

void SlicerPresetsDialog::logRows() {
  for (QTreeWidgetItem* row : presetRows(m_list)) {
    const QString choice = row->data(0, kChoiceRole).toString();
    qDebug().noquote() << QStringLiteral("Slicer presets row: %1 %2, %3%4")
                              .arg(row->data(0, kKindRole).toString(), row->text(0),
                                   row->checkState(0) == Qt::Checked ? QStringLiteral("ticked")
                                                                     : QStringLiteral("unticked"),
                                   choice.isEmpty() ? QString()
                                                    : QStringLiteral(", same name: %1, %2").arg(row->toolTip(0), choice));
  }
}

void SlicerPresetsDialog::showEvent(QShowEvent* event) {
  QDialog::showEvent(event);
  m_loggedPlaces.clear();
  scheduleLogPlaces();
}

void SlicerPresetsDialog::scheduleLogPlaces() {
  TestSync::singleShot(0, this, [this] { logPlaces(); });
}

void SlicerPresetsDialog::logPlaces() {
  if (!isVisible()) {
    return;
  }
  // In the main window's coordinates, as the other logged places: the
  // window this one's chain of parents starts from.
  QWidget* top = this;
  while (top->window()->parentWidget() != nullptr) {
    top = top->window()->parentWidget();
  }
  const QPoint origin = top->window()->mapToGlobal(QPoint(0, 0));
  const auto at = [&origin](QWidget* widget, const QPoint& point) {
    const QPoint global = widget->mapToGlobal(point) - origin;
    return QStringLiteral("%1,%2").arg(global.x()).arg(global.y());
  };
  QStringList lines;
  const std::pair<const char*, QWidget*> widgets[] = {
      {"slicer", m_slicer}, {"same_all", m_sameAll}, {"import", m_import}, {"cancel", m_cancel}};
  for (const auto& [name, widget] : widgets) {
    if (widget->isVisibleTo(this)) {
      lines << QStringLiteral("Slicer presets %1 at %2").arg(QLatin1String(name), at(widget, widget->rect().center()));
    }
  }
  // A row's box: at the start of its rect.
  const int box = m_list->style()->pixelMetric(QStyle::PM_IndicatorWidth, nullptr, m_list) / 2 + 3;
  for (QTreeWidgetItem* row : presetRows(m_list)) {
    const QRect rect = m_list->visualItemRect(row);
    if (rect.isValid() && m_list->viewport()->rect().contains(rect.center())) {
      lines << QStringLiteral("Slicer presets item %1 at %2")
                   .arg(row->text(0), at(m_list->viewport(), QPoint(rect.left() + box, rect.center().y())));
    }
  }
  const QString logged = lines.join(QLatin1Char('\n'));
  if (logged != m_loggedPlaces) {
    m_loggedPlaces = logged;
    for (const QString& line : std::as_const(lines)) {
      qDebug().noquote() << line;
    }
  }
}

} // namespace mitcad
