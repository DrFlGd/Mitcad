// SPDX-License-Identifier: MIT
#include "ImportDetailsDialog.hpp"

#include <algorithm>
#include <utility>

#include <QClipboard>
#include <QDialogButtonBox>
#include <QGuiApplication>
#include <QFrame>
#include <QLabel>
#include <QScrollArea>
#include <QPoint>
#include <QPushButton>
#include <QStringList>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/Dialogs.hpp"
#include "../framework/TestSync.hpp"
#include "../platform/MacDialogs.hpp"

namespace mitcad {

ImportDetailsDialog::ImportDetailsDialog(const PrintImportSummary& summary, QWidget* parent)
    : QDialog(parent), m_report(summary.details) {
  setWindowTitle(tr("Import Details"));
  auto* layout = new QVBoxLayout(this);
  // A read-only view, not a text field (mitcad#655): the title, then each
  // section that has something under its heading.
  QStringList html{summary.title.toHtmlEscaped()};
  for (const auto& [heading, entries] : summary.sections) {
    QStringList escaped;
    for (const QString& entry : entries) {
      escaped << entry.toHtmlEscaped();
    }
    html << (heading.isEmpty() ? escaped.join(QStringLiteral("<br>"))
                               : QStringLiteral("<b>%1</b><br>%2").arg(heading.toHtmlEscaped(), escaped.join(QStringLiteral("<br>"))));
  }
  auto* text = new QLabel(html.join(QStringLiteral("<br><br>")));
  text->setTextFormat(Qt::RichText);
  text->setWordWrap(true);
  text->setTextInteractionFlags(Qt::TextSelectableByMouse);
  text->setAlignment(Qt::AlignTop | Qt::AlignLeft);
  text->setContentsMargins(4, 4, 4, 4);
  auto* scroll = new QScrollArea;
  scroll->setWidgetResizable(true);
  scroll->setFrameShape(QFrame::NoFrame);
  scroll->setWidget(text);
  layout->addWidget(scroll, 1);
  auto* buttons = new QDialogButtonBox;
  m_copy = new QPushButton(tr("&Copy"));
  m_close = new QPushButton(tr("Close"));
  buttons->addButton(m_copy, QDialogButtonBox::ActionRole);
  buttons->addButton(m_close, QDialogButtonBox::RejectRole);
  layout->addWidget(buttons);
  mac::styleDialogButtons(buttons, m_close);
  mac::styleDialog(this);
  connect(m_copy, &QPushButton::clicked, this, [this] {
    QGuiApplication::clipboard()->setText(m_report);
    qDebug().noquote() << QStringLiteral("Import details copied");
  });
  connect(m_close, &QPushButton::clicked, this, &QDialog::reject);
  // As large as the report (mitcad#655), up to a height that scrolls.
  constexpr int kTextWidth = 460;
  const int textHeight = text->heightForWidth(kTextWidth) + 16;
  resize(kTextWidth + 48, std::min(textHeight + buttons->sizeHint().height() + 40, 560));
}

void ImportDetailsDialog::showReport(const PrintImportSummary& summary, QWidget* parent) {
  if (summary.details.isEmpty()) {
    return;
  }
  ImportDetailsDialog dialog(summary, parent);
  // Logged here, not while painting: the tests read the report from it.
  const QStringList lines = summary.details.split(QLatin1Char('\n'));
  qDebug().noquote() << QStringLiteral("Import details opened: %1 lines").arg(lines.size());
  for (const QString& line : lines) {
    qDebug().noquote() << QStringLiteral("Import details: %1").arg(line);
  }
  prepareModal(&dialog);
  execDialog(&dialog);
  qDebug().noquote() << QStringLiteral("Import details closed");
}

void ImportDetailsDialog::showEvent(QShowEvent* event) {
  QDialog::showEvent(event);
  TestSync::singleShot(0, this, [this] { logPlaces(); });
}

void ImportDetailsDialog::logPlaces() {
  if (!isVisible() || parentWidget() == nullptr) {
    return;
  }
  // In the main window's coordinates, as the other logged places.
  const QPoint origin = parentWidget()->window()->mapToGlobal(QPoint(0, 0));
  const std::pair<const char*, QPushButton*> buttons[] = {{"copy", m_copy}, {"close", m_close}};
  for (const auto& [name, button] : buttons) {
    const QPoint at = button->mapToGlobal(button->rect().center()) - origin;
    qDebug().noquote() << QStringLiteral("Import details %1 at %2,%3")
                              .arg(QString::fromLatin1(name))
                              .arg(at.x())
                              .arg(at.y());
  }
}

} // namespace mitcad
