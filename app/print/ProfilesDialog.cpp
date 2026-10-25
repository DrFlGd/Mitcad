// SPDX-License-Identifier: MIT
#include "ProfilesDialog.hpp"

#include <exception>
#include <utility>

#include <QAbstractItemView>
#include <QAction>
#include <QComboBox>
#include <QEvent>
#include <QFont>
#include <QHBoxLayout>
#include <QHash>
#include <QHeaderView>
#include <QIcon>
#include <QJsonValue>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QPoint>
#include <QPushButton>
#include <QRect>
#include <QSplitter>
#include <QStringList>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>
#include <QtLogging>

#include "../browser/DocumentHost.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Icons.hpp"
#include "../framework/TestSync.hpp"
#include "../framework/Theme.hpp"
#include "../platform/MacDialogs.hpp"
#include "SettingsForm.hpp"

namespace mitcad {
namespace {

constexpr int kKeyRole = Qt::UserRole; // a row's key; empty on a heading

QString text(const QJsonObject& object, const char* field) {
  return object.value(QLatin1String(field)).toString();
}

const QStringList& kinds() {
  static const QStringList all{QStringLiteral("printer"), QStringLiteral("process")};
  return all;
}

} // namespace

ProfilesDialog::ProfilesDialog(DocumentHost& host, QWidget* parent) : QDialog(parent), m_host(host) {
  setWindowTitle(tr("Printers and Processes"));
  setWindowIcon(themeIcon(QStringLiteral("print-3d")));
  resize(960, 620);
  auto* layout = new QVBoxLayout(this);
  auto* splitter = new QSplitter(Qt::Horizontal);

  // The list, as Materials': under Printers and Processes, no filter; the
  // window's one search is the settings' (mitcad#655).
  m_list = new QTreeWidget;
  m_list->setColumnCount(2);
  m_list->setHeaderHidden(true);
  m_list->setUniformRowHeights(true);
  m_list->setSelectionMode(QAbstractItemView::SingleSelection);
  m_list->header()->setStretchLastSection(false);
  // The place's icon right after the names (mitcad#655).
  m_list->header()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
  m_list->header()->setSectionResizeMode(1, QHeaderView::Stretch);
  styleItemList(m_list);
  m_list->installEventFilter(this);
  mac::styleListTree(m_list);
  splitter->addWidget(m_list);

  // The editor of the selected one.
  m_editor = new QWidget;
  auto* editorBox = new QVBoxLayout(m_editor);
  editorBox->setContentsMargins(8, 0, 0, 0);
  // The header both windows share (mitcad#655): its name to edit,
  // Built-in, Save to.
  editorBox->addLayout(itemHeader(&m_name, &m_usedState, &m_saved, &m_builtIn, &m_headerDuplicate, &m_saveTo));
  m_emptyState = new QLabel(tr("No printer or process selected"));
  m_emptyState->setAlignment(Qt::AlignCenter);
  m_emptyState->setStyleSheet(QStringLiteral("color: gray;"));
  m_emptyState->hide();
  editorBox->addWidget(m_emptyState, 1);
  m_form = new SettingsForm;
  editorBox->addWidget(m_form, 1);
  splitter->addWidget(m_editor);
  // A narrow list, the editor wide.
  splitter->setStretchFactor(0, 0);
  splitter->setStretchFactor(1, 1);
  splitter->setSizes({260, 700});
  layout->addWidget(splitter, 1);

  m_message = new QLabel;
  m_message->setWordWrap(true);
  layout->addWidget(m_message);
  // New, Duplicate and Delete under the list, as in Materials.
  auto* buttons = new QHBoxLayout;
  m_new = new QPushButton(tr("New ▾"));
  m_duplicate = new QPushButton(tr("Duplicate"));
  m_delete = new QPushButton(tr("Delete"));
  // The selected one as the design's printer or process (mitcad#655).
  m_use = new QPushButton(tr("Use in this design"));
  m_close = new QPushButton(tr("Close"));
  buttons->addWidget(m_new);
  buttons->addWidget(m_duplicate);
  buttons->addWidget(m_delete);
  buttons->addStretch(1);
  buttons->addWidget(m_use);
  buttons->addWidget(m_close);
  layout->addLayout(buttons);
  // Enter in a field saves it: no button takes Enter.
  for (QPushButton* button : {m_headerDuplicate, m_saveTo, m_new, m_duplicate, m_delete, m_use, m_close}) {
    mac::auxiliaryButton(button);
  }
  mac::styleDialog(this);

  connect(m_list, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* row) {
    if (m_building || row == nullptr || row->data(0, kKeyRole).toString().isEmpty()) {
      return;
    }
    m_currentKey = row->data(0, kKeyRole).toString();
    const LibraryItem* chosen = current();
    m_currentDesignId = chosen != nullptr ? chosen->designId() : QString();
    m_currentKind = chosen != nullptr ? chosen->kind : QString();
    m_loggedValues.clear();
    setMessage(QString());
    showCurrent();
    if (chosen != nullptr) {
      qDebug().noquote() << QStringLiteral("Profile selected: %1 %2 (%3)").arg(chosen->kind, chosen->name, chosen->place);
    }
    scheduleLogPlaces();
  });
  connect(m_name, &QLineEdit::editingFinished, this, [this] {
    const LibraryItem* item = current();
    const QString name = m_name->text().trimmed();
    if (item != nullptr && !item->readOnly && !name.isEmpty() && name != item->name) {
      edit({{QStringLiteral("name"), name}}, QStringLiteral("name = %1").arg(name));
    }
  });
  connect(m_form, &SettingsForm::valueChanged, this, [this](const QString& key, const QJsonValue& value) {
    const PrintKey* printKey = m_keys.find(key);
    const QString shown = value.isNull() ? QStringLiteral("cleared")
                                         : (printKey != nullptr ? printValueText(*printKey, value)
                                                                : value.toVariant().toString());
    edit({{QStringLiteral("values"), QJsonObject{{key, value}}}}, QStringLiteral("%1 = %2").arg(key, shown), key);
  });
  connect(m_form, &SettingsForm::layoutChanged, this, &ProfilesDialog::scheduleLogPlaces);
  connect(m_saveTo, &QPushButton::clicked, this, &ProfilesDialog::saveTo);
  connect(m_new, &QPushButton::clicked, this, &ProfilesDialog::newProfile);
  connect(m_duplicate, &QPushButton::clicked, this, &ProfilesDialog::duplicate);
  connect(m_headerDuplicate, &QPushButton::clicked, this, &ProfilesDialog::duplicate);
  connect(m_use, &QPushButton::clicked, this, &ProfilesDialog::useInDesign);
  connect(m_delete, &QPushButton::clicked, this, &ProfilesDialog::remove);
  connect(m_close, &QPushButton::clicked, this, &QDialog::reject);
}

ProfilesDialog::~ProfilesDialog() = default;

QString ProfilesDialog::rowKey(const LibraryItem& item) { return item.kind + QLatin1Char('|') + item.key(); }

QString ProfilesDialog::kindTitle(const QString& kind) {
  return kind == QStringLiteral("printer") ? tr("Printers") : tr("Processes");
}

void ProfilesDialog::showItem(const QString& kind, const QString& key) {
  m_currentKind = kind;
  m_currentKey = key.isEmpty() ? QString() : kind + QLatin1Char('|') + key;
  m_currentDesignId.clear();
  m_loggedValues.clear();
  refresh();
  if (kind.isEmpty() && key.isEmpty()) {
    // From its own command: the design's process, else its printer.
    for (const QString& wanted : {QStringLiteral("process"), QStringLiteral("printer")}) {
      for (const LibraryItem& item : std::as_const(m_items)) {
        if (m_currentKey.isEmpty() && item.kind == wanted && item.chosen) {
          m_currentKind = item.kind;
          m_currentKey = rowKey(item);
        }
      }
    }
    if (!m_currentKey.isEmpty()) {
      refresh();
    }
  }
  if (const LibraryItem* item = current()) {
    m_currentDesignId = item->designId();
    qDebug().noquote() << QStringLiteral("Profile selected: %1 %2 (%3)").arg(item->kind, item->name, item->place);
  }
  // Slicer default: the kind's heading in sight, nothing selected.
  for (int i = 0; i < m_list->topLevelItemCount(); ++i) {
    if (m_list->topLevelItem(i)->data(1, kKeyRole).toString() == kind && current() == nullptr) {
      m_list->scrollToItem(m_list->topLevelItem(i), QAbstractItemView::PositionAtTop);
    }
  }
}

const LibraryItem* ProfilesDialog::current() const {
  for (const LibraryItem& item : m_items) {
    if (rowKey(item) == m_currentKey) {
      return &item;
    }
  }
  return nullptr;
}

void ProfilesDialog::refresh() {
  const QString project = m_host.projectFolder();
  QString error;
  m_items.clear();
  for (const QString& kind : kinds()) {
    m_items += queryLibraryItems(m_host.model(), kind, project, &error);
  }
  try {
    m_profiles = m_host.model().queryArray(QStringLiteral("print_profiles"));
    QJsonObject libraryQuery{{QStringLiteral("query"), QStringLiteral("print_library")}};
    if (!project.isEmpty()) {
      libraryQuery.insert(QStringLiteral("project"), project);
    }
    m_library = m_host.model().queryObject(libraryQuery);
    // The design's slicer: whose percentages the fields offer (mitcad#655).
    m_dialect = designDialectOf(m_host.model());
    if (m_keys.isEmpty()) {
      m_keys = PrintKeyTable(m_host.model().queryObject({{QStringLiteral("query"), QStringLiteral("print_keys")}}));
    }
  } catch (const std::exception& e) {
    error = QString::fromUtf8(e.what());
  }
  if (!error.isEmpty()) {
    setMessage(error, true);
  }
  // The selected item where it went: Save to puts it into a library, a
  // removed library item the design uses stays as the design's.
  if (current() == nullptr && !m_currentDesignId.isEmpty()) {
    for (const LibraryItem& item : std::as_const(m_items)) {
      if (item.kind == m_currentKind && item.designId() == m_currentDesignId) {
        m_currentKey = rowKey(item);
      }
    }
  }
  rebuildList();
  showCurrent();
  logRows();
  scheduleLogPlaces();
  if (!m_pendingFocus.isEmpty() && m_pendingFocus == m_currentKey) {
    m_pendingFocus.clear();
    m_name->setFocus(Qt::OtherFocusReason);
    m_name->selectAll();
  }
}

void ProfilesDialog::rebuildList() {
  m_building = true;
  m_list->clear();
  QTreeWidgetItem* selected = nullptr;
  QHash<QString, QTreeWidgetItem*> headings;
  for (const QString& kind : kinds()) {
    auto* heading = new QTreeWidgetItem(m_list, {kindTitle(kind)});
    heading->setFlags(Qt::ItemIsEnabled);
    heading->setFirstColumnSpanned(true);
    heading->setData(1, kKeyRole, kind);
    QFont bold = heading->font(0);
    bold.setBold(true);
    heading->setFont(0, bold);
    headings.insert(kind, heading);
  }
  // Each kind's items in the query's order (by name), the design's choice
  // marked; no places (mitcad#655).
  for (const QString& kind : kinds()) {
    for (const LibraryItem& item : std::as_const(m_items)) {
      if (item.kind != kind) {
        continue;
      }
      auto* row = new QTreeWidgetItem(headings.value(kind), {item.name, QString()});
      row->setData(0, kKeyRole, rowKey(item));
      markInDesign(row, usedByDesign(item), tr("Used in this design"));
      if (rowKey(item) == m_currentKey) {
        selected = row;
      }
    }
  }
  m_list->expandAll();
  if (selected != nullptr) {
    m_list->setCurrentItem(selected);
  } else {
    m_currentKey.clear();
  }
  m_building = false;
  scheduleLogPlaces();
}

void ProfilesDialog::selectKey(const QString& key) {
  for (QTreeWidgetItemIterator it(m_list); *it != nullptr; ++it) {
    if ((*it)->data(0, kKeyRole).toString() == key) {
      m_list->setCurrentItem(*it);
      m_list->scrollToItem(*it);
      return;
    }
  }
}

void ProfilesDialog::showCurrent() {
  const LibraryItem* item = current();
  const bool project = !m_host.projectFolder().isEmpty();
  m_duplicate->setEnabled(item != nullptr);
  // Only for an item the design does not use (mitcad#655): its state is
  // the header's "● Used in this design", never a disabled button.
  m_use->setVisible(item != nullptr && !usedByDesign(*item));
  m_delete->setEnabled(item != nullptr && !item->readOnly);
  m_saveTo->setText(saveToText(project));
  // Nothing selected (Slicer default): a quiet line, never a blank side
  // or an empty disabled form (mitcad#655).
  m_emptyState->setVisible(item == nullptr);
  m_name->setVisible(item != nullptr);
  m_form->setVisible(item != nullptr);
  if (item == nullptr) {
    m_name->clear();
    m_builtIn->hide();
    m_saved->hide();
    m_usedState->hide();
    m_saveTo->hide();
    m_form->setKeys(m_keys, m_currentKind.isEmpty() ? QStringLiteral("process") : m_currentKind, m_dialect);
    m_form->setValues({});
    return;
  }
  // What the design prints with (mitcad#655): its copy when it has one.
  const QJsonObject fields = designFields(*item, m_profiles, m_library);
  const bool editable = !item->readOnly;
  if (!m_name->hasFocus() || !m_name->isModified()) {
    m_name->setText(item->name);
    m_name->setModified(false);
  }
  m_name->setReadOnly(!editable);
  // Built-in or the user's own, no place (mitcad#655); Save to only for
  // the user's own that is in no library yet.
  m_builtIn->setVisible(item->readOnly);
  showSavedState(m_saved, item);
  m_usedState->setVisible(usedByDesign(*item));
  m_saveTo->setVisible(item->unsaved());
  m_form->setKeys(m_keys, item->kind, m_dialect);
  m_form->setReadOnly(!editable);
  const QJsonObject values = fields.value(QStringLiteral("values")).toObject();
  m_form->setValues(values);
  // The design's profile: the own values bodies, occurrences or components
  // set show beside the fields, "4 on Body1" (mitcad#655).
  QHash<QString, QVector<QPair<QString, QJsonValue>>> ownValues;
  for (const QJsonValue& value : item->row.value(QStringLiteral("overrides")).toArray()) {
    const QJsonObject owner = value.toObject();
    const QJsonObject own = owner.value(QStringLiteral("values")).toObject();
    for (auto it = own.begin(); it != own.end(); ++it) {
      ownValues[it.key()].append({text(owner, "name"), it.value()});
    }
  }
  m_form->setOwnValues(ownValues);
  // Another design changed it: asked once, when the design uses it or it
  // is edited (mitcad#655), not on a mere selection.
  if (item->changed && !changeAsked(*item) && usedByDesign(*item)) {
    TestSync::singleShot(0, this, [this] { askChanges(); });
  }
  // What the editor shows, for UI tests.
  const QStringList shown = m_form->rows();
  const QString logged = QStringLiteral("%1 %2 (%3)\n").arg(item->kind, item->name, item->place) +
                         shown.join(QLatin1Char('\n'));
  if (isVisible() && logged != m_loggedValues) {
    m_loggedValues = logged;
    qDebug().noquote() << QStringLiteral("Profile shown: %1 %2 (%3)%4")
                              .arg(item->kind, item->name, item->place,
                                   item->readOnly ? QStringLiteral(", read only") : QString());
    for (const QString& value : shown) {
      qDebug().noquote() << QStringLiteral("Profile %1 value %2").arg(item->name, value);
    }
  }
}

bool ProfilesDialog::canChange() {
  if (m_host.canChangePrintSettings()) {
    return true;
  }
  setMessage(m_host.changeBlockedReason(), true);
  return false;
}

bool ProfilesDialog::run(const QJsonObject& command, QJsonObject* result) {
  if (m_host.runModelCommand(command, result)) {
    setMessage(QString());
    m_host.commandInputsChanged();
    return true;
  }
  if (!m_host.lastCancelled()) {
    const LibraryItem* item = current();
    setMessage(plainModelError(m_host.lastError(), m_keys, command, item != nullptr ? item->name : QString()),
               true); // plain words (mitcad#655)
  }
  qDebug().noquote() << QStringLiteral("Profiles: %1 refused: %2")
                            .arg(command.value(QStringLiteral("cmd")).toString(), m_host.lastError());
  return false;
}

void ProfilesDialog::setMessage(const QString& message, bool error) {
  m_message->setText(message);
  if (error) {
    setErrorStyleSheet(m_message);
  } else {
    m_message->setStyleSheet(QString());
  }
}

void ProfilesDialog::edit(const QJsonObject& change, const QString& what, const QString& field) {
  const LibraryItem* item = current();
  if (item == nullptr || item->readOnly) {
    return;
  }
  const LibraryItem target = *item;
  // Not while the field that changed still handles its signal.
  TestSync::singleShot(0, this, [this, target, change, what, field] {
    if (!canChange()) {
      refresh();
      return;
    }
    // Edited while another design's change waits: asked first
    // (mitcad#655).
    if (target.changed && !changeAsked(target)) {
      run(askChangedElsewhere(this, target, QStringLiteral("Profiles"), m_host.projectFolder()));
    }
    QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("edit_print_profile")}};
    nameItem(command, target, m_host.projectFolder());
    if (!target.library.isEmpty()) {
      command.insert(QStringLiteral("kind"), target.kind); // a library may have a printer and a process of the id
    }
    for (auto it = change.begin(); it != change.end(); ++it) {
      command.insert(it.key(), it.value());
    }
    const bool done = run(command);
    if (done) {
      qDebug().noquote() << QStringLiteral("Profile %1 (%2) %3").arg(target.name, target.place, what);
    }
    refresh();
    if (!done && !field.isEmpty() && !m_host.lastCancelled()) {
      // A refusal of a setting under its field (mitcad#655).
      setMessage(QString());
      m_form->setError(field, m_keys.plainText(m_host.lastError()));
    }
  });
}

void ProfilesDialog::newProfile() {
  if (!canChange()) {
    return;
  }
  // A printer or a process (mitcad#655): made in this design as its
  // choice, one undo step; put into a library only by Save to.
  QMenu menu(this);
  QAction* printer = menu.addAction(tr("Printer"));
  QAction* process = menu.addAction(tr("Process"));
  qDebug().noquote() << QStringLiteral("Profiles new menu: %1 | %2").arg(printer->text(), process->text());
  QAction* chosen = execMenu(&menu, m_new->mapToGlobal(QPoint(0, m_new->height())));
  if (chosen == nullptr) {
    return;
  }
  const QString kind = chosen == printer ? QStringLiteral("printer") : QStringLiteral("process");
  const QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("create_print_profile")},
                            {QStringLiteral("kind"), kind},
                            {QStringLiteral("choose"), true}};
  QJsonObject result;
  if (run(command, &result)) {
    m_currentKind = kind;
    m_currentKey = kind + QStringLiteral("|design/") + text(result, "id");
    m_currentDesignId = text(result, "id");
    m_pendingFocus = m_currentKey;
    m_loggedValues.clear();
    qDebug().noquote() << QStringLiteral("Profile created: %1 %2 in design").arg(kind, text(result, "id"));
  }
  refresh();
}

void ProfilesDialog::duplicate() {
  const LibraryItem* item = current();
  if (item == nullptr || !canChange()) {
    return;
  }
  const LibraryItem source = *item;
  const QString project = m_host.projectFolder();
  // The user's own copy in this design, made its choice in the same undo
  // step (mitcad#655): from the design's copy of the source when it has
  // one, else from the library's item.
  const QJsonValue basedOn = source.library.isEmpty() || !source.inDesign.isEmpty()
                                 ? QJsonValue(source.designId())
                                 : QJsonValue(QJsonObject{{QStringLiteral("library"), source.library},
                                                          {QStringLiteral("id"), source.id}});
  // The model names the copy after the source ("Standard 2").
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("create_print_profile")},
                      {QStringLiteral("kind"), source.kind},
                      {QStringLiteral("based_on"), basedOn},
                      {QStringLiteral("choose"), true}};
  if (!project.isEmpty() && basedOn.isObject()) {
    command.insert(QStringLiteral("project"), project);
  }
  QJsonObject result;
  if (!run(command, &result)) {
    refresh();
    return;
  }
  const QString id = text(result, "id");
  qDebug().noquote() << QStringLiteral("Profile duplicated: %1 as %2 in design").arg(source.name, id);
  m_currentKind = source.kind;
  m_currentKey = source.kind + QStringLiteral("|design/") + id;
  m_currentDesignId = id;
  m_pendingFocus = m_currentKey;
  m_loggedValues.clear();
  refresh();
}

void ProfilesDialog::useInDesign() {
  const LibraryItem* item = current();
  if (item == nullptr || item->chosen || !canChange()) {
    return;
  }
  const LibraryItem target = *item;
  // The root's choice, one undo step ("Use Process Standard"); a library's
  // item is copied in with it.
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("set_property")},
                      {QStringLiteral("owner"), QStringLiteral("C0")},
                      {QStringLiteral("set"), target.kind == QStringLiteral("printer") ? QStringLiteral("Print Printer")
                                                                                       : QStringLiteral("Print Process")},
                      {QStringLiteral("key"), QStringLiteral("profile")}};
  nameItem(command, target, m_host.projectFolder(), QStringLiteral("value"));
  if (run(command)) {
    qDebug().noquote() << QStringLiteral("Profile used in this design: %1 %2").arg(target.kind, target.name);
  }
  refresh();
}

void ProfilesDialog::remove() {
  const LibraryItem* item = current();
  if (item == nullptr || item->readOnly || !canChange()) {
    return;
  }
  const LibraryItem target = *item;
  // Always asked, naming what uses it (mitcad#655).
  if (!confirmDelete(this, target, QStringLiteral("Profiles"))) {
    return;
  }
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("delete_print_profile")}};
  nameItem(command, target, m_host.projectFolder());
  if (!target.library.isEmpty()) {
    command.insert(QStringLiteral("kind"), target.kind);
  }
  if (run(command)) {
    qDebug().noquote() << QStringLiteral("Profile deleted: %1 (%2)").arg(target.name, target.place);
  }
  refresh();
}

void ProfilesDialog::saveTo() {
  const LibraryItem* item = current();
  if (item == nullptr || !item->unsaved() || !canChange()) {
    return;
  }
  const LibraryItem target = *item;
  const QString project = m_host.projectFolder();
  const QString library = choosePlace(m_saveTo, QStringLiteral("Profiles save_to"), !project.isEmpty());
  if (library.isEmpty()) {
    return;
  }
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("save_to_print_library")},
                      {QStringLiteral("library"), library},
                      {QStringLiteral("kind"), target.kind},
                      {QStringLiteral("id"), target.designId()}};
  if (!project.isEmpty()) {
    command.insert(QStringLiteral("project"), project);
  }
  if (run(command)) {
    qDebug().noquote() << QStringLiteral("Profile saved to %1: %2").arg(library, target.name);
    m_currentDesignId = target.designId();
    m_currentKind = target.kind;
    m_currentKey.clear(); // found again by its design copy
    m_loggedValues.clear();
  }
  refresh();
}

void ProfilesDialog::askChanges() {
  const LibraryItem* item = current();
  if (item == nullptr || !item->changed || changeAsked(*item) || !isVisible() ||
      !m_host.canChangePrintSettings()) {
    return;
  }
  const LibraryItem target = *item;
  // Use the changes or Keep these, one undo step either way (mitcad#655).
  const QJsonObject command = askChangedElsewhere(this, target, QStringLiteral("Profiles"), m_host.projectFolder());
  if (run(command)) {
    qDebug().noquote() << QStringLiteral("Profile %1: %2").arg(target.name, command.value(QStringLiteral("cmd")).toString());
  }
  m_loggedValues.clear();
  refresh();
}

void ProfilesDialog::logRows() {
  if (!isVisible()) {
    return;
  }
  QStringList rows;
  for (const LibraryItem& item : std::as_const(m_items)) {
    rows << QStringLiteral("%1/%2 %3 (%4)%5")
                .arg(item.kind, item.id, item.name, item.place, item.chosen ? QStringLiteral(", the design's") : QString());
  }
  const QString logged = rows.join(QLatin1Char('\n'));
  if (logged == m_loggedRows) {
    return;
  }
  m_loggedRows = logged;
  qDebug().noquote() << QStringLiteral("Profiles: %1 items").arg(rows.size());
  for (const QString& row : std::as_const(rows)) {
    qDebug().noquote() << QStringLiteral("Profiles row: %1").arg(row);
  }
}

void ProfilesDialog::showEvent(QShowEvent* event) {
  QDialog::showEvent(event);
  m_loggedRows.clear();
  m_loggedPlaces.clear();
  m_loggedValues.clear();
  logRows();
  showCurrent();
  scheduleLogPlaces();
}

void ProfilesDialog::resizeEvent(QResizeEvent* event) {
  QDialog::resizeEvent(event);
  scheduleLogPlaces();
}

bool ProfilesDialog::eventFilter(QObject* watched, QEvent* event) {
  // F2 on a row: its name.
  if (watched == m_list && event->type() == QEvent::KeyPress &&
      static_cast<QKeyEvent*>(event)->key() == Qt::Key_F2 && current() != nullptr && !current()->readOnly) {
    m_name->setFocus(Qt::OtherFocusReason);
    m_name->selectAll();
    return true;
  }
  return QDialog::eventFilter(watched, event);
}

void ProfilesDialog::scheduleLogPlaces() {
  TestSync::singleShot(0, this, [this] { logPlaces(); });
}

void ProfilesDialog::logPlaces() {
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
  for (QTreeWidgetItemIterator it(m_list); *it != nullptr; ++it) {
    QTreeWidgetItem* row = *it;
    const QString key = row->data(0, kKeyRole).toString();
    const QRect rect = m_list->visualItemRect(row);
    if (key.isEmpty() || row->isHidden() || !rect.isValid() || !m_list->viewport()->rect().contains(rect.center())) {
      continue;
    }
    lines << QStringLiteral("Profiles item %1 at %2").arg(row->text(0), at(m_list->viewport(), QPoint(60, rect.center().y())));
  }
  const std::pair<const char*, QWidget*> widgets[] = {
      {"name", m_name},           {"save_to", m_saveTo}, {"header_duplicate", m_headerDuplicate}, {"new", m_new},
      {"duplicate", m_duplicate}, {"delete", m_delete},  {"use", m_use}, {"close", m_close}};
  for (const auto& [name, widget] : widgets) {
    if (widget->isVisibleTo(this)) {
      lines << QStringLiteral("Profiles %1 at %2").arg(QString::fromLatin1(name), at(widget, widget->rect().center()));
    }
  }
  lines << m_form->places(QStringLiteral("Profiles settings"), at);
  const QString logged = lines.join(QLatin1Char('\n'));
  if (logged != m_loggedPlaces) {
    m_loggedPlaces = logged;
    for (const QString& line : std::as_const(lines)) {
      qDebug().noquote() << line;
    }
  }
}

} // namespace mitcad
