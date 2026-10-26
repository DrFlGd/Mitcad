// SPDX-License-Identifier: MIT
#include "MyLibraryDialog.hpp"

#include <exception>
#include <iterator>
#include <utility>

#include <QComboBox>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QLabel>
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
#include "../framework/DialogFolders.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Icons.hpp"
#include "../framework/TestSync.hpp"
#include "../framework/Theme.hpp"
#include "../platform/MacDialogs.hpp"
#include "ImportDetailsDialog.hpp"
#include "MaterialsDialog.hpp"
#include "PrintKeys.hpp"
#include "PrintLibrary.hpp"
#include "ProfilesDialog.hpp"

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

  // The place (in a project only) and Import… with its line.
  auto* top = new QHBoxLayout;
  m_place = new QComboBox;
  m_place->addItem(placeName(QStringLiteral("user")), QStringLiteral("user"));
  m_place->addItem(placeName(QStringLiteral("project")), QStringLiteral("project"));
  m_place->setAccessibleName(tr("Library"));
  m_place->hide();
  m_importLine = new QLabel;
  m_importLine->setStyleSheet(QStringLiteral("color: gray;"));
  m_importLine->hide();
  m_details = new QPushButton(tr("Details…"));
  m_details->hide();
  m_import = new QPushButton(tr("Import from Slicer…"));
  top->addWidget(m_place);
  top->addStretch(1);
  top->addWidget(m_importLine);
  top->addWidget(m_details);
  top->addWidget(m_import);
  layout->addLayout(top);

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
  for (QPushButton* button : {m_details, m_import}) {
    mac::auxiliaryButton(button);
  }
  mac::styleDialog(this);

  m_materials->setLibrary(QStringLiteral("user"));
  m_printers->setLibrary(QStringLiteral("user"));
  m_processes->setLibrary(QStringLiteral("user"));

  const auto changed = [this] { m_changed = true; };
  connect(m_materials, &MaterialsPanel::changed, this, changed);
  connect(m_printers, &ProfilesPanel::changed, this, changed);
  connect(m_processes, &ProfilesPanel::changed, this, changed);
  connect(m_place, &QComboBox::activated, this, [this] { placeChosen(); });
  connect(m_import, &QPushButton::clicked, this, &MyLibraryDialog::importFromSlicer);
  connect(m_details, &QPushButton::clicked, this, [this] { ImportDetailsDialog::showReport(m_summary, this); });
  connect(m_tabs, &QTabWidget::currentChanged, this, [this](int index) {
    if (index >= 0 && index < static_cast<int>(std::size(kTabs))) {
      qDebug().noquote() << QStringLiteral("My Library tab: %1").arg(QLatin1String(kTabs[index]));
    }
    scheduleLogPlaces();
  });
}

MyLibraryDialog::~MyLibraryDialog() {
  // The panels before the model they work through.
  delete m_tabs;
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

void MyLibraryDialog::placeChosen() {
  const QString library = place();
  qDebug().noquote() << QStringLiteral("My Library place: %1").arg(library);
  m_importLine->hide();
  m_details->hide();
  m_materials->setLibrary(library);
  m_printers->setLibrary(library);
  m_processes->setLibrary(library);
  scheduleLogPlaces();
}

void MyLibraryDialog::importFromSlicer() {
  // As 3D Print's Import from slicer…, into the library shown (mitcad#704).
  const QString path = dialogOpenFileName(this, tr("Import from Slicer"),
                                          dialogStartFolder(DialogGroup::Import, QString()), slicerFileFilter());
  if (path.isEmpty()) {
    return;
  }
  rememberDialogFolder(DialogGroup::Import, path);
  const QString library = place();
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("import_print_profile")},
                      {QStringLiteral("path"), path},
                      {QStringLiteral("into"), library}};
  if (library == QStringLiteral("project")) {
    command.insert(QStringLiteral("project"), m_host.projectFolder());
  }
  PrintKeyTable keys;
  try {
    keys = PrintKeyTable(m_host.model().queryObject({{QStringLiteral("query"), QStringLiteral("print_keys")}}));
  } catch (const std::exception&) {
    // Without the names: as reported.
  }
  QJsonObject result;
  if (!m_host.runModelCommand(command, &result)) {
    m_importLine->setText(plainModelError(m_host.lastError(), keys, command));
    setErrorStyleSheet(m_importLine);
    m_importLine->show();
    m_details->hide();
    qDebug().noquote() << QStringLiteral("My Library import refused: %1").arg(m_host.lastError());
    scheduleLogPlaces();
    return;
  }
  m_changed = true;
  m_summary = printImportSummary(QFileInfo(path).fileName(), library, result, &keys);
  m_importLine->setText(m_summary.line);
  m_importLine->setStyleSheet(QStringLiteral("color: gray;"));
  m_importLine->show();
  m_details->setVisible(!m_summary.details.isEmpty());
  QStringList items;
  for (const QJsonValue& value : result.value(QStringLiteral("items")).toArray()) {
    const QJsonObject item = value.toObject();
    items << QStringLiteral("%1 %2").arg(item.value(QStringLiteral("kind")).toString(),
                                         item.value(QStringLiteral("name")).toString());
  }
  qDebug().noquote() << QStringLiteral("My Library imported into %1: %2")
                            .arg(library, items.isEmpty() ? QStringLiteral("nothing") : items.join(QStringLiteral(", ")));
  qDebug().noquote() << QStringLiteral("My Library import: %1").arg(m_summary.line);
  refresh();
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
  const std::pair<const char*, QWidget*> widgets[] = {
      {"place", m_place}, {"import", m_import}, {"details", m_details}};
  for (const auto& [name, widget] : widgets) {
    if (widget->isVisibleTo(this)) {
      lines << QStringLiteral("My Library %1 at %2").arg(QLatin1String(name), at(widget, widget->rect().center()));
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
