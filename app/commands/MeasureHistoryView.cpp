// SPDX-License-Identifier: MIT
#include "MeasureHistoryView.hpp"

#include <algorithm>
#include <utility>

#include <QClipboard>
#include <QCoreApplication>
#include <QEvent>
#include <QGuiApplication>
#include <QHBoxLayout>
#include <QLabel>
#include <QListWidget>
#include <QListWidgetItem>
#include <QPalette>
#include <QPushButton>
#include <QRect>
#include <QScrollBar>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/GlassCard.hpp"
#include "../framework/TestSync.hpp"
#include "MeasureHistory.hpp"

namespace mitcad {
namespace {

// Rows shown before the list scrolls: the panel does not grow past the
// screen however many measurements there are.
constexpr int kShownRows = 4;

// The row's number is kept in the item.
constexpr int kNumberRole = Qt::UserRole + 1;

QString rowText(const MeasureHistory::Row& row) {
  QString text = QStringLiteral("%1. %2").arg(row.number).arg(row.picked);
  if (!row.values.isEmpty()) {
    text += QLatin1Char('\n') + row.values.join(QStringLiteral(", "));
  }
  if (row.gone) {
    text += QLatin1Char('\n') + QObject::tr("The geometry is not there any more.");
  }
  return text;
}

// Tabs and line breaks of copied text, readable on one log line.
QString loggable(QString text) {
  return text.replace(QLatin1Char('\t'), QStringLiteral(" | ")).replace(QLatin1Char('\n'), QStringLiteral(" / "));
}

// Whether a row can be kept as a measurement object (mitcad#649): one
// clicked whose geometry is there.
bool rowKeepable(const MeasureHistory& history, const QListWidgetItem* item) {
  if (item == nullptr) {
    return false;
  }
  const MeasureHistory::Row* row = history.row(item->data(kNumberRole).toInt());
  return row != nullptr && !row->gone;
}

} // namespace

MeasureHistoryView::MeasureHistoryView(const MeasureHistory& history, QWidget* parent)
    : QWidget(parent), m_history(history) {
  setObjectName(QStringLiteral("measureHistory"));
  auto* layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(4);
  // Keep (mitcad#649): the measurement shown becomes a measurement object
  // of the design.
  auto* keepRow = new QHBoxLayout;
  m_keep = new QPushButton(tr("Keep"), this);
  m_keepTip = tr("Keep this measurement in the design as a dimension, drawn in the view");
  m_keep->setToolTip(m_keepTip);
  m_keep->setEnabled(false);
  // Keep Row: the history row clicked last (beside Keep, so that the row
  // of Copy, Copy All and Clear keeps the panel's width).
  m_keepRow = new QPushButton(tr("Keep Row"), this);
  m_keepRow->setToolTip(tr("Keep the history row clicked last in the design as a dimension"));
  m_keepRow->setEnabled(false);
  // A leader note on the one pick shown, and the Annotations window
  // (mitcad#702).
  m_note = new QPushButton(tr("Note…"), this);
  m_note->setToolTip(tr("A note with an arrow to the face, edge or vertex picked"));
  m_note->setEnabled(false);
  // A datum label and a feature control frame on it (phase 2).
  m_datum = new QPushButton(tr("Datum Label…"), this);
  m_datum->setToolTip(tr("A datum label on the face, edge or datum picked"));
  m_datum->setEnabled(false);
  m_frame = new QPushButton(tr("Frame…"), this);
  m_frame->setToolTip(tr("A feature control frame with an arrow to the face or edge picked"));
  m_frame->setEnabled(false);
  m_annotations = new QPushButton(tr("Annotations…"), this);
  m_annotations->setToolTip(tr("The dimensions, notes, datum labels and frames kept in the design"));
  // Note…, Datum Label… and Frame… on a row of their own, so that the panel
  // keeps its width; Annotations… beside Keep.
  auto* annotationRow = new QHBoxLayout;
  for (QPushButton* button : {m_keep, m_keepRow, m_annotations, m_note, m_datum, m_frame}) {
    button->setFocusPolicy(Qt::NoFocus);
    button->setAutoDefault(false);
    (button == m_note || button == m_datum || button == m_frame ? annotationRow : keepRow)->addWidget(button);
  }
  keepRow->addStretch();
  layout->addLayout(keepRow);
  annotationRow->addStretch();
  layout->addLayout(annotationRow);
  m_title = new QLabel(this);
  layout->addWidget(m_title);
  m_list = new QListWidget(this);
  m_list->setObjectName(QStringLiteral("measureHistoryRows"));
  m_list->setSelectionMode(QAbstractItemView::SingleSelection);
  // Two lines a row (the picks, the values), cut at the panel's width; the
  // tooltip has all of it.
  m_list->setWordWrap(false);
  m_list->setTextElideMode(Qt::ElideRight);
  m_list->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
  m_list->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel);
  // The keyboard stays with the panel's fields and the view.
  m_list->setFocusPolicy(Qt::NoFocus);
  layout->addWidget(m_list);
  auto* buttons = new QHBoxLayout;
  m_copy = new QPushButton(tr("Copy"), this);
  m_copy->setToolTip(tr("Copy the row clicked last as tab-separated text"));
  m_copyAll = new QPushButton(tr("Copy All"), this);
  m_copyAll->setToolTip(tr("Copy every row as tab-separated text"));
  m_clear = new QPushButton(tr("Clear"), this);
  m_clear->setToolTip(tr("Empty the history"));
  for (QPushButton* button : {m_copy, m_copyAll, m_clear}) {
    button->setFocusPolicy(Qt::NoFocus);
    button->setAutoDefault(false);
    buttons->addWidget(button);
  }
  buttons->addStretch();
  layout->addLayout(buttons);

  connect(m_list, &QListWidget::itemClicked, this, [this](QListWidgetItem* item) {
    m_copy->setEnabled(item != nullptr);
    m_keepRow->setEnabled(rowKeepable(m_history, item));
    // The buttons' places again, Copy and Keep Row enabled.
    TestSync::singleShot(0, this, [this] { logPlaces(); });
    if (item != nullptr && m_actions.show) {
      m_actions.show(item->data(kNumberRole).toInt());
    }
  });
  connect(m_keep, &QPushButton::clicked, this, [this] {
    qDebug().noquote() << QStringLiteral("Measure Keep clicked");
    if (m_actions.keep) {
      m_actions.keep();
    }
  });
  connect(m_keepRow, &QPushButton::clicked, this, [this] {
    const QListWidgetItem* item = m_list->currentItem();
    if (item == nullptr) {
      return;
    }
    const int number = item->data(kNumberRole).toInt();
    qDebug().noquote() << QStringLiteral("Measure history Keep clicked: row %1").arg(number);
    if (m_actions.keepRow) {
      m_actions.keepRow(number);
    }
  });
  connect(m_note, &QPushButton::clicked, this, [this] {
    qDebug().noquote() << QStringLiteral("Measure Note clicked");
    if (m_actions.note) {
      m_actions.note();
    }
  });
  connect(m_datum, &QPushButton::clicked, this, [this] {
    qDebug().noquote() << QStringLiteral("Measure Datum Label clicked");
    if (m_actions.datum) {
      m_actions.datum();
    }
  });
  connect(m_frame, &QPushButton::clicked, this, [this] {
    qDebug().noquote() << QStringLiteral("Measure Frame clicked");
    if (m_actions.frame) {
      m_actions.frame();
    }
  });
  connect(m_annotations, &QPushButton::clicked, this, [this] {
    qDebug().noquote() << QStringLiteral("Measure Annotations clicked");
    if (m_actions.annotations) {
      m_actions.annotations();
    }
  });
  connect(m_copy, &QPushButton::clicked, this, [this] { copy(false); });
  connect(m_copyAll, &QPushButton::clicked, this, [this] { copy(true); });
  connect(m_clear, &QPushButton::clicked, this, [this] {
    qDebug().noquote() << QStringLiteral("Measure history Clear clicked");
    if (m_actions.clear) {
      m_actions.clear();
    }
  });
  // Its places again when the list scrolls.
  connect(m_list->verticalScrollBar(), &QScrollBar::valueChanged, this,
          [this] { TestSync::singleShot(0, this, [this] { logPlaces(); }); });
  refresh();
}

void MeasureHistoryView::refresh() {
  const int current = m_list->currentItem() != nullptr ? m_list->currentItem()->data(kNumberRole).toInt() : 0;
  m_list->clear();
  const QColor grey = palette().color(QPalette::Disabled, QPalette::Text);
  const auto& rows = m_history.rows();
  qDebug().noquote() << QStringLiteral("Measure history: %1 rows").arg(static_cast<int>(rows.size()));
  for (const MeasureHistory::Row& row : rows) {
    auto* item = new QListWidgetItem(rowText(row), m_list);
    item->setData(kNumberRole, row.number);
    item->setToolTip(MeasureHistory::tabSeparated(row).replace(QLatin1Char('\t'), QLatin1Char('\n')));
    if (row.gone) {
      item->setForeground(grey);
    }
    if (row.number == current) {
      m_list->setCurrentItem(item);
    }
    qDebug().noquote() << QStringLiteral("Measure history row %1: %2: %3%4")
                              .arg(row.number)
                              .arg(row.picked, row.values.join(QStringLiteral(", ")),
                                   row.gone ? QStringLiteral(" (gone)") : QString());
  }
  m_title->setText(rows.empty() ? tr("<b>History</b>: none yet")
                                : tr("<b>History</b>: %n measurement(s)", nullptr, static_cast<int>(rows.size())));
  m_copy->setEnabled(m_list->currentItem() != nullptr);
  m_keepRow->setEnabled(rowKeepable(m_history, m_list->currentItem()));
  m_copyAll->setEnabled(!rows.empty());
  m_clear->setEnabled(!rows.empty());
  fitHeight();
  m_list->setVisible(m_list->count() > 0);
  TestSync::singleShot(0, this, [this] { logPlaces(); });
}

void MeasureHistoryView::setKeepable(bool keepable, const QString& why) {
  // A greyed-out Keep says why (mitcad#869).
  const QString reason = keepable ? QString() : why;
  const QString tip = reason.isEmpty() ? m_keepTip : reason;
  if (m_keep->isEnabled() == keepable && m_keep->toolTip() == tip) {
    return;
  }
  m_keep->setEnabled(keepable);
  m_keep->setToolTip(tip);
  qDebug().noquote() << QStringLiteral("Measure Keep %1%2")
                            .arg(keepable ? QStringLiteral("enabled") : QStringLiteral("disabled"),
                                 reason.isEmpty() ? QString() : QStringLiteral(": ") + reason);
  TestSync::singleShot(0, this, [this] { logPlaces(); });
}

void MeasureHistoryView::setNoteable(bool noteable, bool feature) {
  const bool featured = noteable && feature;
  if (m_note->isEnabled() == noteable && m_datum->isEnabled() == featured) {
    return;
  }
  m_datum->setEnabled(featured);
  m_frame->setEnabled(featured);
  // The places again either way (Datum Label… and Frame… with them).
  TestSync::singleShot(0, this, [this] { logPlaces(); });
  if (m_note->isEnabled() == noteable) {
    return;
  }
  m_note->setEnabled(noteable);
  qDebug().noquote() << QStringLiteral("Measure Note %1")
                            .arg(noteable ? QStringLiteral("enabled") : QStringLiteral("disabled"));
}

void MeasureHistoryView::fitHeight() {
  // As high as its rows up to kShownRows of them, then it scrolls.
  const int line = m_list->fontMetrics().height() + 4;
  int listHeight = 2 * m_list->frameWidth();
  for (int i = 0; i < m_list->count() && i < kShownRows; ++i) {
    listHeight += std::max(m_list->sizeHintForRow(i), line);
  }
  if (m_list->count() == 0) {
    listHeight += line;
  }
  if (m_list->height() != listHeight) {
    m_list->setFixedHeight(listHeight);
  }
}

QStringList MeasureHistoryView::places() const {
  QStringList lines;
  if (!isVisible()) {
    return lines;
  }
  const auto at = [](const QWidget* widget, const QPoint& point) {
    const QPoint host = GlassCard::mapToHost(widget, point);
    return QStringLiteral("%1,%2").arg(host.x()).arg(host.y());
  };
  const QRect shown = m_list->viewport()->rect();
  for (int i = 0; i < m_list->count(); ++i) {
    const QListWidgetItem* item = m_list->item(i);
    const QRect rect = m_list->visualItemRect(item);
    const QString name = QStringLiteral("Measure history row %1").arg(item->data(kNumberRole).toInt());
    // In sight by its height: a long row is wider than the list (its end
    // elided), so its middle can lie right of the view while the row
    // shows. Clicked in the view's middle across.
    const QPoint middle(shown.center().x(), rect.center().y());
    if (m_list->isVisible() && shown.contains(middle) && rect.left() <= middle.x()) {
      lines << QStringLiteral("%1 at %2").arg(name, at(m_list->viewport(), middle));
    } else {
      lines << QStringLiteral("%1 out of sight").arg(name);
    }
  }
  const std::pair<const char*, const QPushButton*> buttons[] = {
      {"Copy", m_copy}, {"Copy All", m_copyAll}, {"Clear", m_clear}, {"Keep", m_keepRow}};
  for (const auto& [name, button] : buttons) {
    lines << QStringLiteral("Measure history %1 at %2%3")
                 .arg(QString::fromLatin1(name), at(button, button->rect().center()),
                      button->isEnabled() ? QString() : QStringLiteral(" (disabled)"));
  }
  lines << QStringLiteral("Measure Keep at %1%2")
               .arg(at(m_keep, m_keep->rect().center()), m_keep->isEnabled() ? QString() : QStringLiteral(" (disabled)"));
  lines << QStringLiteral("Measure Note at %1%2")
               .arg(at(m_note, m_note->rect().center()), m_note->isEnabled() ? QString() : QStringLiteral(" (disabled)"));
  lines << QStringLiteral("Measure Datum Label at %1%2")
               .arg(at(m_datum, m_datum->rect().center()), m_datum->isEnabled() ? QString() : QStringLiteral(" (disabled)"));
  lines << QStringLiteral("Measure Frame at %1%2")
               .arg(at(m_frame, m_frame->rect().center()), m_frame->isEnabled() ? QString() : QStringLiteral(" (disabled)"));
  lines << QStringLiteral("Measure Annotations at %1").arg(at(m_annotations, m_annotations->rect().center()));
  return lines;
}

void MeasureHistoryView::logPlaces() {
  // The rows laid out now: the list lays them out late, when it paints,
  // and a window that is never painted (Windows session 0) had only the
  // current row in place.
  m_list->doItemsLayout();
  fitHeight();
  QCoreApplication::sendPostedEvents(nullptr, QEvent::LayoutRequest);
  const QStringList lines = places();
  if (lines.isEmpty() || lines == m_logged) {
    return;
  }
  m_logged = lines;
  for (const QString& line : lines) {
    qDebug().noquote() << line;
  }
}

void MeasureHistoryView::copy(bool all) {
  QString text;
  QString what;
  if (all) {
    text = m_history.tabSeparated();
    what = QStringLiteral("%1 rows").arg(static_cast<int>(m_history.rows().size()));
  } else {
    const QListWidgetItem* item = m_list->currentItem();
    const MeasureHistory::Row* row = item != nullptr ? m_history.row(item->data(kNumberRole).toInt()) : nullptr;
    if (row == nullptr) {
      return;
    }
    text = MeasureHistory::tabSeparated(*row);
    what = QStringLiteral("row %1").arg(row->number);
  }
  if (QClipboard* clipboard = QGuiApplication::clipboard()) {
    clipboard->setText(text);
  }
  qDebug().noquote() << QStringLiteral("Measure history copied %1: %2").arg(what, loggable(text));
}

} // namespace mitcad
