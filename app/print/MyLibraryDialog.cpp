// SPDX-License-Identifier: MIT
#include "MyLibraryDialog.hpp"

#include <exception>
#include <iterator>
#include <utility>

#include <QComboBox>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonObject>
#include <QPoint>
#include <QPushButton>
#include <QSignalBlocker>
#include <QStringList>
#include <QTabBar>
#include <QTabWidget>
#include <QVBoxLayout>
#include <QVector>
#include <QtLogging>

#include "../framework/Command.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Icons.hpp"
#include "../framework/TestSync.hpp"
#include "../framework/Theme.hpp"
#include "../platform/MacDialogs.hpp"
#include "MaterialsDialog.hpp"
#include "PrintLibrary.hpp"
#include "ProfilesDialog.hpp"
#include "SlicerImport.hpp"

namespace mitcad {
namespace {

// The tabs' ids in the log, in their order.
const char* const kTabs[] = {"materials", "printers", "processes"};

} // namespace

MyLibraryDialog::MyLibraryDialog(QWidget* parent) : QDialog(parent) {
  setWindowTitle(tr("My Library"));
  setWindowIcon(themeIcon(QStringLiteral("material")));
  resize(960, 640);
  auto* layout = new QVBoxLayout(this);

  // The place (in a project only) and Import from Slicer… with its line
  // and Details… (SlicerImportBar, mitcad#829: one import with 3D Print's
  // and Printers and Processes'), into the library shown.
  m_place = new QComboBox;
  m_place->addItem(placeName(QStringLiteral("user")), QStringLiteral("user"));
  m_place->addItem(placeName(QStringLiteral("project")), QStringLiteral("project"));
  m_place->setAccessibleName(tr("Library"));
  m_place->hide();
  m_importBar = new SlicerImportBar(m_host, QStringLiteral("My Library"));
  m_importBar->target = [this] { return place(); };
  m_importBar->row()->insertWidget(0, m_place);
  layout->addWidget(m_importBar);

  // The panels of the design's windows, on the library.
  m_tabs = new QTabWidget;
  m_tabs->setDocumentMode(true);
  m_materials = new MaterialsPanel(m_host, QStringLiteral("My Library materials"));
  m_printers = new ProfilesPanel(m_host, QStringLiteral("My Library printers"));
  m_printers->setKinds({QStringLiteral("printer")});
  m_processes = new ProfilesPanel(m_host, QStringLiteral("My Library processes"));
  m_processes->setKinds({QStringLiteral("process")});
  m_tabs->addTab(m_materials, tr("Materials"));
  m_tabs->addTab(m_printers, tr("Printers"));
  m_tabs->addTab(m_processes, tr("Processes"));
  layout->addWidget(m_tabs, 1);
  // Close at the end of each tab's row, where the other windows have it.
  const auto makeClose = [this] {
    auto* button = new QPushButton(tr("Close"));
    connect(button, &QPushButton::clicked, this, &QDialog::reject);
    return button;
  };
  m_materials->addButton(makeClose(), QStringLiteral("close"));
  m_printers->addButton(makeClose(), QStringLiteral("close"));
  m_processes->addButton(makeClose(), QStringLiteral("close"));
  mac::styleDialog(this);

  m_materials->setLibrary(QStringLiteral("user"));
  m_printers->setLibrary(QStringLiteral("user"));
  m_processes->setLibrary(QStringLiteral("user"));

  const auto changed = [this] { m_changed = true; };
  connect(m_materials, &MaterialsPanel::changed, this, changed);
  connect(m_printers, &ProfilesPanel::changed, this, changed);
  connect(m_processes, &ProfilesPanel::changed, this, changed);
  connect(m_place, &QComboBox::activated, this, [this] { placeChosen(); });
  connect(m_importBar, &SlicerImportBar::imported, this, [this](const SlicerImportResult& result) {
    m_changed = true;
    refresh();
    showImported(result);
  });
  connect(m_tabs, &QTabWidget::currentChanged, this, [this](int index) {
    if (index >= 0 && index < static_cast<int>(std::size(kTabs))) {
      qDebug().noquote() << QStringLiteral("My Library tab: %1").arg(QLatin1String(kTabs[index]));
    }
    scheduleLogPlaces();
  });
}

MyLibraryDialog::~MyLibraryDialog() {
  // The panels and the import before the model they work through.
  delete m_tabs;
  delete m_importBar;
}

QString MyLibraryDialog::place() const {
  return m_host.projectFolder().isEmpty() ? QStringLiteral("user") : m_place->currentData().toString();
}

void MyLibraryDialog::setProjectFolder(const QString& folder) {
  m_host.setProjectFolder(folder);
  // The project's library only in a project; My library else.
  m_place->setVisible(!folder.isEmpty());
  if (folder.isEmpty() && m_place->currentIndex() != 0) {
    const QSignalBlocker block(m_place);
    m_place->setCurrentIndex(0);
    placeChosen();
  }
}

void MyLibraryDialog::refresh() {
  m_materials->refresh();
  m_printers->refresh();
  m_processes->refresh();
  scheduleLogPlaces();
}

void MyLibraryDialog::showImported(const SlicerImportResult& result) {
  // The first item that came in, on its tab and selected (mitcad#830
  // review): the library's answer lists the new ones, then the overridden.
  const QJsonArray items = result.answer.value(QStringLiteral("items")).toArray();
  if (items.isEmpty()) {
    return;
  }
  const QJsonObject first = items.first().toObject();
  const QString kind = first.value(QStringLiteral("kind")).toString();
  const QString key = result.into + QLatin1Char('/') + first.value(QStringLiteral("id")).toString();
  if (kind == QStringLiteral("material")) {
    m_tabs->setCurrentWidget(m_materials);
    m_materials->showItem(key);
  } else if (kind == QStringLiteral("printer")) {
    m_tabs->setCurrentWidget(m_printers);
    m_printers->showItem(kind, key);
  } else if (kind == QStringLiteral("process")) {
    m_tabs->setCurrentWidget(m_processes);
    m_processes->showItem(kind, key);
  }
}

void MyLibraryDialog::placeChosen() {
  const QString library = place();
  qDebug().noquote() << QStringLiteral("My Library place: %1").arg(library);
  m_importBar->clear();
  m_materials->setLibrary(library);
  m_printers->setLibrary(library);
  m_processes->setLibrary(library);
  scheduleLogPlaces();
}

void MyLibraryDialog::showEvent(QShowEvent* event) {
  QDialog::showEvent(event);
  m_loggedPlaces.clear();
  scheduleLogPlaces();
}

void MyLibraryDialog::hideEvent(QHideEvent* event) {
  QDialog::hideEvent(event);
  qDebug().noquote() << QStringLiteral("My Library closed%1").arg(m_changed ? QStringLiteral(" after changes")
                                                                             : QString());
  if (m_changed) {
    m_changed = false;
    emit libraryChanged();
  }
}

void MyLibraryDialog::resizeEvent(QResizeEvent* event) {
  QDialog::resizeEvent(event);
  scheduleLogPlaces();
}

void MyLibraryDialog::scheduleLogPlaces() {
  TestSync::singleShot(0, this, [this] { logPlaces(); });
}

void MyLibraryDialog::logPlaces() {
  if (!isVisible() || parentWidget() == nullptr) {
    return;
  }
  // In the main window's coordinates, as the other logged places.
  const QPoint origin = parentWidget()->window()->mapToGlobal(QPoint(0, 0));
  const auto at = [&origin](QWidget* widget, const QPoint& point) {
    const QPoint global = widget->mapToGlobal(point) - origin;
    return QStringLiteral("%1,%2").arg(global.x()).arg(global.y());
  };
  QStringList lines;
  for (int i = 0; i < m_tabs->count() && i < static_cast<int>(std::size(kTabs)); ++i) {
    lines << QStringLiteral("My Library tab %1 at %2")
                 .arg(QLatin1String(kTabs[i]), at(m_tabs->tabBar(), m_tabs->tabBar()->tabRect(i).center()));
  }
  // The import's buttons are logged by their row.
  if (m_place->isVisibleTo(this)) {
    lines << QStringLiteral("My Library place at %1").arg(at(m_place, m_place->rect().center()));
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
