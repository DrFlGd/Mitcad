// SPDX-License-Identifier: MIT
#include "MaterialsDialog.hpp"

#include <algorithm>
#include <cmath>
#include <exception>
#include <optional>
#include <utility>

#include <QAbstractItemView>
#include <QAction>
#include <QColor>
#include <QComboBox>
#include <QCompleter>
#include <QEvent>
#include <QFont>
#include <QFormLayout>
#include <QFrame>
#include <QHash>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QIcon>
#include <QJsonValue>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QMessageBox>
#include <QPainter>
#include <QPen>
#include <QPixmap>
#include <QPoint>
#include <QPushButton>
#include <QRect>
#include <QRectF>
#include <QSettings>
#include <QSize>
#include <QSignalBlocker>
#include <QSplitter>
#include <QToolButton>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>
#include <QtLogging>

#include "../browser/AssignmentPanel.hpp"
#include "../browser/Bindings.hpp"
#include "../browser/DocumentHost.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Icons.hpp"
#include "../framework/Numbers.hpp"
#include "../framework/TestSync.hpp"
#include "../framework/Theme.hpp"
#include "../platform/MacDialogs.hpp"
#include "../platform/NativeDialogs.hpp"
#include "SettingsForm.hpp"

namespace mitcad {
namespace {

constexpr int kKeyRole = Qt::UserRole; // a row's key; empty on a group's heading
const char* kDefaultKey = "default";
const char* kEditAppearances = "__edit_appearances";

QString text(const QJsonObject& object, const char* field) {
  return object.value(QLatin1String(field)).toString();
}

// A square of a colour (none: the default look's grey).
QIcon swatch(const QColor& color) {
  QPixmap pixmap(18, 18);
  pixmap.fill(Qt::transparent);
  QPainter painter(&pixmap);
  painter.setRenderHint(QPainter::Antialiasing);
  const QColor fill = color.isValid() ? color : QColor(204, 204, 204);
  painter.setBrush(fill);
  painter.setPen(QPen(fill.darker(140), 1));
  painter.drawRoundedRect(QRectF(1.5, 1.5, 15.0, 15.0), 3.0, 3.0);
  return QIcon(pixmap);
}

const QStringList& groupOrder() {
  // The classes' groups (mitcad#655): plastics hold the filaments.
  static const QStringList order{QStringLiteral("metals"), QStringLiteral("plastics"), QStringLiteral("other")};
  return order;
}

} // namespace

MaterialsDialog::MaterialsDialog(DocumentHost& host, QWidget* parent) : QDialog(parent), m_designHost(host) {
  setWindowTitle(tr("Materials"));
  setWindowIcon(themeIcon(QStringLiteral("material")));
  // Taller by the assignment panel's header line (mitcad#861): the list
  // keeps its height with targets.
  resize(960, 680);
  auto* layout = new QVBoxLayout(this);
  m_panel = new MaterialsPanel(m_designHost, QStringLiteral("Materials"));
  layout->addWidget(m_panel);
  m_panel->editAppearances = [this] {
    if (editAppearances) {
      editAppearances();
    }
  };
  m_panel->openConfigurations = [this] {
    if (openConfigurations) {
      openConfigurations();
    }
  };
  // The user's library without a design (mitcad#704), then Close.
  auto* myLibrary = new QPushButton(tr("My Library…"));
  auto* close = new QPushButton(tr("Close"));
  m_panel->addButton(myLibrary, QStringLiteral("my_library"));
  m_panel->addButton(close, QStringLiteral("close"));
  mac::styleDialog(this);
  connect(myLibrary, &QPushButton::clicked, this, [this] {
    qDebug().noquote() << QStringLiteral("Materials: My Library…");
    if (openMyLibrary) {
      openMyLibrary();
    }
  });
  connect(close, &QPushButton::clicked, this, &QDialog::reject);
  connect(m_panel, &MaterialsPanel::assigned, this, &QDialog::accept);
}

MaterialsDialog::~MaterialsDialog() {
  // The panel before the host it works through.
  delete m_panel;
}

void MaterialsDialog::setTargets(const QStringList& bodies, const QString& component, const QStringList& names) {
  m_panel->setTargets(bodies, component, names);
}

void MaterialsDialog::setKeepOpen(bool keepOpen) { m_panel->setKeepOpen(keepOpen); }

void MaterialsDialog::refresh() { m_panel->refresh(); }

MaterialsPanel::MaterialsPanel(PrintItemsHost& host, const QString& logName, QWidget* parent)
    : QWidget(parent), m_host(host), m_log(logName) {
  auto* layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  auto* splitter = new QSplitter(Qt::Horizontal);

  // The list, grouped by class (mitcad#655): one search in the window, the
  // settings', so the list has none and no group filter.
  m_list = new QTreeWidget;
  m_list->setObjectName(QStringLiteral("materialsList"));
  m_list->setColumnCount(2);
  m_list->setHeaderHidden(true);
  m_list->setIconSize(QSize(18, 18));
  m_list->setSelectionMode(QAbstractItemView::SingleSelection);
  m_list->header()->setStretchLastSection(false);
  // The place's icon right after the names (mitcad#655).
  m_list->header()->setSectionResizeMode(0, QHeaderView::ResizeToContents);
  m_list->header()->setSectionResizeMode(1, QHeaderView::Stretch);
  styleItemList(m_list);
  m_list->installEventFilter(this);
  mac::styleListTree(m_list);
  splitter->addWidget(m_list);

  // The editor of the selected one: the header both windows share (its
  // name to edit, Built-in, Save to), then the tabs: General (Class,
  // Density, Appearance) and the print settings' (mitcad#655).
  m_editor = new QWidget;
  auto* editorBox = new QVBoxLayout(m_editor);
  editorBox->setContentsMargins(8, 0, 0, 0);
  editorBox->addLayout(itemHeader(&m_name, &m_usedState, &m_saved, &m_builtIn, &m_headerDuplicate, &m_saveTo));
  // Default's swatch beside its name (mitcad#655).
  m_nameSwatch = m_name->addAction(QIcon(), QLineEdit::LeadingPosition);
  m_nameSwatch->setVisible(false);
  // Nothing selected: a quiet line, not a blank side.
  m_emptyState = new QLabel(tr("No material selected"));
  m_emptyState->setAlignment(Qt::AlignCenter);
  m_emptyState->setStyleSheet(QStringLiteral("color: gray;"));
  m_emptyState->hide();
  editorBox->addWidget(m_emptyState, 1);

  auto* general = new QWidget;
  auto* form = new QFormLayout(general);
  form->setContentsMargins(4, 8, 4, 4);
  // The class is the group the list shows it in (mitcad#655).
  m_class = new QComboBox;
  for (const QString& materialClass : materialClasses()) {
    m_class->addItem(materialClassTitle(materialClass), materialClass);
  }
  m_class->setFixedWidth(160);
  form->addRow(tr("Class"), m_class);
  auto* densityRow = new QHBoxLayout;
  m_density = new QLineEdit;
  // The same widths as the settings form's fields (mitcad#655).
  m_density->setFixedWidth(150);
  densityRow->addWidget(m_density);
  auto* densityUnit = new QLabel(QStringLiteral("g/cm³"));
  densityUnit->setFixedWidth(80);
  densityRow->addWidget(densityUnit);
  densityRow->addStretch(1);
  form->addRow(tr("Density"), densityRow);
  // A refused density under its field (mitcad#655).
  m_densityError = new QLabel;
  m_densityError->setWordWrap(true);
  setErrorStyleSheet(m_densityError);
  m_densityError->hide();
  form->addRow(QString(), m_densityError);
  auto* appearanceRow = new QHBoxLayout;
  m_appearance = new QComboBox;
  m_appearance->setEditable(true);
  m_appearance->setInsertPolicy(QComboBox::NoInsert);
  m_appearance->completer()->setFilterMode(Qt::MatchContains);
  m_appearance->completer()->setCompletionMode(QCompleter::PopupCompletion);
  m_appearance->setFixedWidth(240);
  appearanceRow->addWidget(m_appearance);
  appearanceRow->addStretch(1);
  form->addRow(tr("Appearance"), appearanceRow);
  m_form = new SettingsForm;
  m_form->setGeneralPage(general, tr("General"));
  m_form->setGeneralNames({tr("Class"), tr("Density"), tr("Appearance")});
  editorBox->addWidget(m_form, 1);
  // Default alone (its name, no editor): at the top, not in the middle of
  // the pane.
  editorBox->addStretch(0);
  splitter->addWidget(m_editor);
  // A narrow list, the editor wide.
  splitter->setStretchFactor(0, 0);
  splitter->setStretchFactor(1, 1);
  splitter->setSizes({240, 720});
  layout->addWidget(splitter, 1);

  // The assignment panel (mitcad#861): the targets' material, and whether
  // it varies with an option; Assign and a double-click fill its rows then.
  AssignmentPanel::Host panelHost;
  panelHost.model = [this]() -> const CommandContext& { return m_host.model(); };
  panelHost.runCommand = [this](const QJsonObject& command) { return run(command); };
  panelHost.runCommands = [this](const QJsonArray& commands, const QString& label) {
    if (!m_host.runModelCommands(commands, label)) {
      return false;
    }
    setMessage(QString());
    m_host.commandInputsChanged();
    emit changed();
    return true;
  };
  panelHost.lastError = [this] { return m_host.lastError(); };
  panelHost.canChange = [this] { return canChange(); };
  panelHost.describe = [this](const QString& named) {
    AssignmentPanel::Result result;
    const auto take = [&result](const QString& id, const QString& name, const QColor& color) {
      if (result.id.isEmpty()) {
        result.id = id;
        result.name = name;
        result.icon = swatch(color);
      }
    };
    // By id, the design's materials before the built-in ones; else by name.
    for (int pass = 0; pass < 2; ++pass) {
      const bool byName = pass == 1;
      for (const QJsonValue& value : std::as_const(m_designMaterials)) {
        const QJsonObject material = value.toObject();
        if (text(material, byName ? "name" : "id") == named) {
          take(text(material, "id"), text(material, "name"), colorOf(material.value(QStringLiteral("display_color"))));
        }
      }
      for (const LibraryItem& item : std::as_const(m_items)) {
        if (item.readOnly && item.library.isEmpty() && (byName ? item.name : item.id) == named) {
          take(item.id, item.name, item.displayColor);
        }
      }
    }
    return result;
  };
  panelHost.openConfigurations = [this] {
    if (openConfigurations) {
      openConfigurations();
    }
  };
  m_assignment = new AssignmentPanel(AssignmentPanel::Kind::Material, m_log, panelHost);
  layout->addWidget(m_assignment);

  m_message = new QLabel;
  m_message->setWordWrap(true);
  layout->addWidget(m_message);
  m_buttons = new QHBoxLayout;
  m_new = new QPushButton(tr("New ▾"));
  m_duplicate = new QPushButton(tr("Duplicate"));
  m_delete = new QPushButton(tr("Delete"));
  m_assign = new QPushButton(tr("Assign"));
  m_assign->hide(); // shown for targets (setTargets)
  // The targets' mixed state, "2 materials" (mitcad#655).
  m_mixed = new QLabel;
  m_mixed->setStyleSheet(QStringLiteral("color: gray;"));
  m_mixed->hide();
  m_buttons->addWidget(m_new);
  m_buttons->addWidget(m_duplicate);
  m_buttons->addWidget(m_delete);
  m_buttons->addStretch(1);
  m_buttons->addWidget(m_mixed);
  m_buttons->addWidget(m_assign);
  layout->addLayout(m_buttons);
  // Enter in a field saves it: no button takes Enter.
  for (QPushButton* button : {m_headerDuplicate, m_saveTo, m_new, m_duplicate, m_delete, m_assign}) {
    mac::auxiliaryButton(button);
  }

  connect(m_class, &QComboBox::activated, this, [this](int index) {
    const LibraryItem* item = current();
    const QString chosen = m_class->itemData(index).toString();
    if (item != nullptr && !item->readOnly && chosen != item->materialClass) {
      edit({{QStringLiteral("class"), chosen}}, QStringLiteral("class = %1").arg(chosen));
    }
  });
  connect(m_list, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* item) {
    if (m_building || item == nullptr || item->data(0, kKeyRole).toString().isEmpty()) {
      return;
    }
    m_currentKey = item->data(0, kKeyRole).toString();
    const LibraryItem* chosen = current();
    m_currentDesignId = chosen != nullptr ? chosen->designId() : QString();
    setMessage(QString());
    showCurrent();
    qDebug().noquote() << QStringLiteral("Material selected: %1")
                              .arg(chosen != nullptr
                                       ? QStringLiteral("%1 (%2)").arg(chosen->name, chosen->place)
                                       : QStringLiteral("default"));
    scheduleLogPlaces();
  });
  // A double-click assigns as Assign does (mitcad#861): the targets' material,
  // or the assignment panel's rows while it varies with an option.
  connect(m_list, &QTreeWidget::itemDoubleClicked, this, [this](QTreeWidgetItem* row) {
    const QString key = row != nullptr ? row->data(0, kKeyRole).toString() : QString();
    if (key.isEmpty() || inLibrary() || (m_targetBodies.isEmpty() && m_targetComponent.isEmpty())) {
      return;
    }
    if (key != m_currentKey) {
      selectKey(key);
    }
    qDebug().noquote() << QStringLiteral("%1 double-click: %2").arg(m_log, key);
    assign();
  });
  connect(m_name, &QLineEdit::editingFinished, this, [this] {
    const LibraryItem* item = current();
    const QString name = m_name->text().trimmed();
    if (item != nullptr && !item->readOnly && !name.isEmpty() && name != item->name) {
      edit({{QStringLiteral("name"), name}}, QStringLiteral("name = %1").arg(name));
    }
  });
  connect(m_density, &QLineEdit::editingFinished, this, [this] {
    const LibraryItem* item = current();
    if (item == nullptr || item->readOnly) {
      return;
    }
    const std::optional<double> density = parsePlainNumber(m_density->text());
    const double now = designFields(*item, m_designMaterials, m_library).value(QStringLiteral("density")).toDouble();
    if (!density || *density <= 0.0) {
      m_densityError->setText(tr("Use a value above 0 g/cm³."));
      m_densityError->show();
      m_density->setText(printNumberText(now));
      return;
    }
    m_densityError->hide();
    if (std::abs(*density - now) > 1e-12) {
      edit({{QStringLiteral("density"), *density}}, QStringLiteral("density = %1").arg(printNumberText(*density)));
    }
  });
  connect(m_appearance, &QComboBox::activated, this, &MaterialsPanel::appearanceChosen);
  connect(m_form, &SettingsForm::valueChanged, this, [this](const QString& key, const QJsonValue& value) {
    const PrintKey* printKey = m_keys.find(key);
    const QString shown = value.isNull() ? QStringLiteral("cleared")
                                         : (printKey != nullptr ? printValueText(*printKey, value)
                                                                : value.toVariant().toString());
    edit({{QStringLiteral("filament"), QJsonObject{{key, value}}}}, QStringLiteral("%1 = %2").arg(key, shown), key);
  });
  connect(m_form, &SettingsForm::layoutChanged, this, &MaterialsPanel::scheduleLogPlaces);
  connect(m_saveTo, &QPushButton::clicked, this, &MaterialsPanel::saveTo);
  connect(m_new, &QPushButton::clicked, this, &MaterialsPanel::newMaterial);
  connect(m_duplicate, &QPushButton::clicked, this, &MaterialsPanel::duplicate);
  connect(m_headerDuplicate, &QPushButton::clicked, this, &MaterialsPanel::duplicate);
  connect(m_delete, &QPushButton::clicked, this, &MaterialsPanel::remove);
  connect(m_assign, &QPushButton::clicked, this, &MaterialsPanel::assign);
  connect(m_assignment, &AssignmentPanel::assignTextChanged, this, &MaterialsPanel::updateAssign);
}

MaterialsPanel::~MaterialsPanel() = default;

void MaterialsPanel::setKeepOpen(bool keepOpen) { m_keepOpen = keepOpen; }

void MaterialsPanel::setLibrary(const QString& library) {
  if (library == m_libraryPlace) {
    return;
  }
  m_libraryPlace = library;
  m_currentKey.clear();
  m_currentDesignId.clear();
  m_loggedRows.clear();
  refresh();
}

QString MaterialsPanel::itemsProject() const {
  // My library is the same wherever the design is: no project's items.
  return m_libraryPlace == QStringLiteral("user") ? QString() : m_host.projectFolder();
}

void MaterialsPanel::addButton(QPushButton* button, const QString& name) {
  mac::auxiliaryButton(button);
  m_buttons->addWidget(button);
  m_windowButtons.append({name, button});
}

void MaterialsPanel::setTargets(const QStringList& bodies, const QString& component, const QStringList& names) {
  m_targetBodies = bodies;
  m_targetComponent = component;
  m_targetNames = names;
  // The targets' material chosen at first: the bodies' shared own, the
  // component's default; none: Default (None).
  QString material;
  bool shared = true;
  int materials = 1;
  try {
    if (!component.isEmpty()) {
      for (const QJsonValue& value : m_host.model().queryArray(QStringLiteral("materials"))) {
        if (value.toObject().value(QStringLiteral("components")).toArray().contains(component)) {
          material = text(value.toObject(), "id");
        }
      }
    } else {
      QStringList own;
      for (const QJsonValue& value : m_host.model().queryArray(QStringLiteral("bodies"))) {
        if (bodies.contains(text(value.toObject(), "uid"))) {
          own << text(value.toObject(), "material");
        }
      }
      own.removeDuplicates();
      materials = static_cast<int>(own.size());
      shared = own.size() == 1;
      material = shared ? own.value(0) : QString();
    }
  } catch (const std::exception&) {
    // Default chosen.
  }
  // Bodies of different materials (mitcad#655): nothing selected, the
  // mixed state beside Assign, which waits for a material to be chosen;
  // never Default, which one click would give them all.
  const bool mixed = !shared && !bodies.isEmpty();
  m_currentKey = bodies.isEmpty() && component.isEmpty() ? m_currentKey
                 : mixed                                ? QString()
                                                        : QString::fromLatin1(kDefaultKey);
  m_mixed->setText(mixed ? tr("%1 materials").arg(materials) : QString());
  m_mixed->setVisible(mixed);
  m_currentDesignId = material;
  refresh();
  if (!material.isEmpty()) {
    if (const LibraryItem* item = findDesignCopy(m_items, material)) {
      selectKey(item->key());
    }
  }
  const bool targets = !bodies.isEmpty() || !component.isEmpty();
  m_assign->setVisible(targets);
  if (!component.isEmpty()) {
    m_assignText = tr("Assign to %1").arg(names.value(0));
  } else if (bodies.size() == 1) {
    m_assignText = tr("Assign to %1").arg(names.value(0));
  } else {
    m_assignText = tr("Assign to %1 bodies").arg(bodies.size());
  }
  m_assign->setText(m_assignText);
  // The bodies' material and whether it varies with an option (mitcad#861);
  // a component's default material has no panel.
  m_assignment->setTargets(component.isEmpty() && !inLibrary() ? bodies : QStringList(), {});
  updateAssign();
  qDebug().noquote() << QStringLiteral("%1 for %2: %3").arg(m_log)
                            .arg(!component.isEmpty() ? QStringLiteral("component %1").arg(component)
                                 : targets           ? QStringLiteral("bodies %1").arg(bodies.join(QStringLiteral(", ")))
                                                     : QStringLiteral("no targets"),
                                 mixed                ? QStringLiteral("mixed, %1 materials").arg(materials)
                                 : material.isEmpty() ? QStringLiteral("default")
                                                      : material);
  qDebug().noquote() << QStringLiteral("%1 assign label: %2").arg(m_log).arg(targets ? m_assign->text() : QString());
  scheduleLogPlaces();
}

const LibraryItem* MaterialsPanel::current() const {
  if (m_currentKey.isEmpty() || m_currentKey == QLatin1String(kDefaultKey)) {
    return nullptr;
  }
  return findItem(m_items, m_currentKey);
}

void MaterialsPanel::showItem(const QString& key) {
  m_currentKey = key;
  m_currentDesignId.clear();
  refresh();
  if (const LibraryItem* item = current()) {
    m_currentDesignId = item->designId();
    if (m_list->currentItem() != nullptr) {
      m_list->scrollToItem(m_list->currentItem());
    }
    qDebug().noquote() << QStringLiteral("Material selected: %1 (%2)").arg(item->name, item->place);
  }
}

void MaterialsPanel::refresh() {
  const QString project = itemsProject();
  QString error;
  m_items = queryLibraryItems(m_host.model(), QStringLiteral("material"), project, &error);
  if (inLibrary()) {
    // A library's items and the built-in ones to duplicate (mitcad#704).
    QVector<LibraryItem> kept;
    for (const LibraryItem& item : std::as_const(m_items)) {
      if (item.place == m_libraryPlace || item.readOnly) {
        kept.append(item);
      }
    }
    m_items = kept;
  }
  try {
    m_designMaterials = m_host.model().queryArray(QStringLiteral("materials"));
    QJsonObject libraryQuery{{QStringLiteral("query"), QStringLiteral("print_library")}};
    if (!project.isEmpty()) {
      libraryQuery.insert(QStringLiteral("project"), project);
    }
    m_library = m_host.model().queryObject(libraryQuery);
    m_appearances = appearancesOf(m_host.model().queryArray(QStringLiteral("appearances")));
    m_bodies = m_host.model().queryArray(QStringLiteral("bodies"));
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
  if (current() == nullptr && m_currentKey != QLatin1String(kDefaultKey)) {
    if (const LibraryItem* copy = findDesignCopy(m_items, m_currentDesignId)) {
      m_currentKey = copy->key();
    }
  }
  // The appearances, the design's and the library's, with Edit
  // Appearances… last (none in My Library, which has no design).
  {
    const QSignalBlocker block(m_appearance);
    m_appearance->clear();
    m_carriedAppearance.clear();
    m_appearance->addItem(tr("None"), QString());
    for (const Appearance& appearance : std::as_const(m_appearances)) {
      m_appearance->addItem(swatch(appearance.displayColor), appearance.name, appearance.id);
    }
    if (editAppearances) {
      m_appearance->addItem(tr("Edit Appearances…"), QString::fromLatin1(kEditAppearances));
    }
  }
  rebuildList();
  showCurrent();
  logRows();
  scheduleLogPlaces();
  if (!m_pendingFocus.isEmpty() && m_pendingFocus == m_currentKey) {
    m_pendingFocus.clear();
    // A new material of another class: its density to be set, not a silent
    // 1 g/cm³ (mitcad#655); else its name.
    QLineEdit* field = m_pendingDensity ? m_density : m_name;
    m_pendingDensity = false;
    if (field == m_density) {
      m_form->showGeneral();
    }
    field->setFocus(Qt::OtherFocusReason);
    field->selectAll();
  }
  // The targets' state again (mitcad#861): undo, a parameter, a fill.
  m_assignment->reload();
}

void MaterialsPanel::updateAssign() {
  const bool varies = m_assignment->varies();
  const bool targets = !m_targetBodies.isEmpty() || !m_targetComponent.isEmpty();
  const bool chosen = current() != nullptr || m_currentKey == QLatin1String(kDefaultKey);
  if (varies) {
    // Assign fills the panel's selected rows: "Assign to row Red"; none
    // after the last row was filled, until a row is clicked.
    const QString label = m_assignment->assignText();
    if (label != m_assign->text()) {
      m_assign->setText(label);
      qDebug().noquote() << QStringLiteral("%1 assign label: %2").arg(m_log, label);
    }
    m_assign->setToolTip(m_assignment->assignTip());
    m_assign->setEnabled(targets && chosen && m_assignment->canFill());
  } else {
    if (!m_assignText.isEmpty()) {
      m_assign->setText(m_assignText);
    }
    m_assign->setToolTip(QString());
    m_assign->setEnabled(targets && chosen);
  }
  // The mixed count is None's; the panel says it in words.
  m_mixed->setVisible(!varies && !m_mixed->text().isEmpty());
  scheduleLogPlaces();
}

void MaterialsPanel::rebuildList() {
  m_building = true;
  m_list->clear();
  QTreeWidgetItem* selected = nullptr;
  if (!m_targetBodies.isEmpty() || !m_targetComponent.isEmpty()) {
    // No material of its own: the same word for bodies and components. It
    // says what it is, Default (Steel), with that material's swatch and a
    // line under it (mitcad#655).
    QString inherited;
    QColor inheritedColor;
    for (const QJsonValue& value : std::as_const(m_designMaterials)) {
      const QJsonObject material = value.toObject();
      const bool rootDefault = material.value(QStringLiteral("components")).toArray().contains(QStringLiteral("C0")) &&
                               m_targetComponent != QStringLiteral("C0");
      if (rootDefault || (inherited.isEmpty() && text(material, "id") == QStringLiteral("steel"))) {
        inherited = text(material, "name");
        inheritedColor = colorOf(material.value(QStringLiteral("display_color")));
      }
    }
    auto* none = new QTreeWidgetItem(m_list, {inherited.isEmpty() ? tr("Default") : tr("Default (%1)").arg(inherited)});
    none->setIcon(0, swatch(inheritedColor));
    m_defaultText = none->text(0);
    m_defaultColor = inheritedColor;
    none->setData(0, kKeyRole, QString::fromLatin1(kDefaultKey));
    if (m_currentKey == QLatin1String(kDefaultKey)) {
      selected = none;
    }
    auto* separator = new QTreeWidgetItem(m_list);
    separator->setFlags(Qt::NoItemFlags);
    separator->setFirstColumnSpanned(true);
    separator->setSizeHint(0, QSize(0, 9));
    auto* line = new QFrame;
    line->setFrameShape(QFrame::HLine);
    line->setFrameShadow(QFrame::Sunken);
    m_list->setItemWidget(separator, 0, line);
  }
  // What the design's bodies use (mitcad#655): a material its bodies or
  // components name, and the default of a body without one.
  QStringList used;
  bool bodyWithout = false;
  for (const QJsonValue& value : std::as_const(m_bodies)) {
    bodyWithout = bodyWithout || text(value.toObject(), "material").isEmpty();
  }
  QString rootDefault = QStringLiteral("steel");
  for (const QJsonValue& value : std::as_const(m_designMaterials)) {
    const QJsonObject material = value.toObject();
    if (!material.value(QStringLiteral("bodies")).toArray().isEmpty() ||
        !material.value(QStringLiteral("components")).toArray().isEmpty()) {
      used << text(material, "id");
    }
    if (material.value(QStringLiteral("components")).toArray().contains(QStringLiteral("C0"))) {
      rootDefault = text(material, "id");
    }
  }
  if (bodyWithout) {
    used << rootDefault;
  }
  m_usedIds = used;
  const auto heading = [this](const QString& title, const QString& group) {
    auto* made = new QTreeWidgetItem(m_list, {title});
    made->setFlags(Qt::ItemIsEnabled);
    made->setFirstColumnSpanned(true);
    made->setData(1, kKeyRole, group);
    QFont bold = made->font(0);
    bold.setBold(true);
    made->setFont(0, bold);
    return made;
  };
  const auto addRow = [&](QTreeWidgetItem* under, const LibraryItem& item) {
    auto* row = new QTreeWidgetItem(under, {item.name, QString()});
    row->setIcon(0, swatch(item.displayColor));
    row->setData(0, kKeyRole, item.key());
    row->setData(0, NativeControlIdentifierRole, QStringLiteral("material.") + item.key());
    markInDesign(row, !item.designId().isEmpty() && used.contains(item.designId()), tr("Used in this design"));
    if (item.key() == m_currentKey) {
      selected = row;
    }
  };
  // By class, in the query's order (the built-in ones first, then by
  // name); no places (mitcad#655). In a library its own items by class,
  // then the built-in ones under Built-in (mitcad#704).
  for (const QString& group : groupOrder()) {
    QTreeWidgetItem* groupRow = nullptr;
    for (const LibraryItem& item : std::as_const(m_items)) {
      const QString itemGroup = groupOrder().contains(item.group) ? item.group : QStringLiteral("other");
      if (itemGroup != group || (inLibrary() && item.readOnly)) {
        continue;
      }
      if (groupRow == nullptr) {
        groupRow = heading(materialGroupTitle(group), group);
      }
      addRow(groupRow, item);
    }
  }
  if (inLibrary()) {
    QTreeWidgetItem* builtIn = nullptr;
    for (const LibraryItem& item : std::as_const(m_items)) {
      if (item.readOnly) {
        if (builtIn == nullptr) {
          builtIn = heading(placeName(QStringLiteral("mitcad")), QStringLiteral("built_in"));
        }
        addRow(builtIn, item);
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

void MaterialsPanel::selectKey(const QString& key) {
  for (QTreeWidgetItemIterator it(m_list); *it != nullptr; ++it) {
    if ((*it)->data(0, kKeyRole).toString() == key) {
      m_list->setCurrentItem(*it);
      m_list->scrollToItem(*it);
      return;
    }
  }
}

void MaterialsPanel::showCurrent() {
  const LibraryItem* item = current();
  const bool targets = !m_targetBodies.isEmpty() || !m_targetComponent.isEmpty();
  // While the material varies with an option, a row is to be selected too
  // (mitcad#861).
  m_assign->setEnabled(targets && (item != nullptr || m_currentKey == QLatin1String(kDefaultKey)) &&
                       (!m_assignment->varies() || m_assignment->canFill()));
  m_duplicate->setEnabled(item != nullptr);
  m_delete->setEnabled(item != nullptr && !item->readOnly);
  // Never a blank side (mitcad#655): Default shows its name and swatch,
  // read only; nothing selected a quiet line; an item its editor.
  const bool isDefault = item == nullptr && m_currentKey == QLatin1String(kDefaultKey);
  m_emptyState->setVisible(item == nullptr && !isDefault);
  m_name->setVisible(item != nullptr || isDefault);
  m_nameSwatch->setVisible(isDefault);
  m_form->setVisible(item != nullptr);
  m_form->setKeys(m_keys, QStringLiteral("filament"), m_dialect);
  // The material's density is shown once, in General.
  m_form->setHiddenKeys({QStringLiteral("filament_density")});
  if (item == nullptr) {
    m_name->setText(isDefault ? m_defaultText : QString());
    m_name->setReadOnly(true);
    if (isDefault) {
      m_nameSwatch->setIcon(swatch(m_defaultColor));
    }
    m_builtIn->hide();
    m_saved->hide();
    m_usedState->hide();
    m_saveTo->hide();
    m_density->clear();
    m_appearance->setCurrentIndex(-1);
    m_class->setCurrentIndex(-1);
    m_form->setValues({});
    return;
  }
  // What the design prints with (mitcad#655): its copy when it has one.
  const QJsonObject fields = designFields(*item, m_designMaterials, m_library);
  const bool editable = !item->readOnly;
  if (!m_name->hasFocus() || !m_name->isModified()) {
    m_name->setText(item->name);
    m_name->setModified(false);
  }
  m_name->setReadOnly(!editable);
  // Built-in or the user's own, no place (mitcad#655); Save to only for
  // the user's own that is in no library yet.
  m_builtIn->setVisible(item->readOnly);
  // In a library every item is the library's: no saved state (mitcad#704).
  showSavedState(m_saved, inLibrary() ? nullptr : item);
  m_usedState->setVisible(!item->designId().isEmpty() && m_usedIds.contains(item->designId()));
  m_saveTo->setVisible(item->unsaved() && m_host.hasDesign());
  m_saveTo->setText(saveToText(!m_host.projectFolder().isEmpty()));
  const double density = fields.value(QStringLiteral("density")).toDouble(1.0);
  m_density->setText(printNumberText(density));
  m_density->setReadOnly(!editable);
  const QString appearanceId = text(fields, "appearance");
  {
    const QSignalBlocker block(m_appearance);
    // An appearance a library's material carries, which is neither
    // Mitcad's nor the design's (mitcad#704): its own entry with the
    // item's colour, before Edit Appearances….
    if (!m_carriedAppearance.isEmpty() && m_carriedAppearance != appearanceId) {
      const int old = m_appearance->findData(m_carriedAppearance);
      if (old >= 0) {
        m_appearance->removeItem(old);
      }
      m_carriedAppearance.clear();
    }
    if (!appearanceId.isEmpty() && m_appearance->findData(appearanceId) < 0) {
      const int at = m_appearance->count() - (editAppearances ? 1 : 0);
      m_appearance->insertItem(at, swatch(item->displayColor), appearanceId, appearanceId);
      m_carriedAppearance = appearanceId;
    }
    m_appearance->setCurrentIndex(std::max(0, m_appearance->findData(appearanceId)));
  }
  m_appearance->setEnabled(editable);
  {
    const QSignalBlocker block(m_class);
    m_class->setCurrentIndex(m_class->findData(item->materialClass));
  }
  m_class->setEnabled(editable);
  // Print settings only for a plastic (mitcad#655): metals and others have
  // none to give a slicer; a plastic shows them, empty when unset.
  m_form->setPrintShown(item->materialClass == QStringLiteral("plastic"));
  m_form->setReadOnly(!editable);
  const QJsonObject filament = fields.value(QStringLiteral("filament")).toObject();
  m_form->setValues(filament);
  // An unset filament colour is the appearance's colour, what is exported:
  // its swatch and the appearance's name (mitcad#655).
  QJsonObject placeholders;
  QHash<QString, QString> words;
  for (const Appearance& appearance : std::as_const(m_appearances)) {
    if (appearance.id == appearanceId && appearance.displayColor.isValid()) {
      placeholders.insert(QStringLiteral("filament_colour"), appearance.displayColor.name().toUpper());
      words.insert(QStringLiteral("filament_colour"), appearance.name);
    }
  }
  if (appearanceId == m_carriedAppearance && !appearanceId.isEmpty() && item->displayColor.isValid()) {
    placeholders.insert(QStringLiteral("filament_colour"), item->displayColor.name().toUpper());
    words.insert(QStringLiteral("filament_colour"), appearanceId);
  }
  // Unset purge volumes are those of the colour it prints with (mitcad#705),
  // as the model works them out: what the export uses.
  QString colour = filament.value(QStringLiteral("filament_colour")).toString();
  if (colour.isEmpty()) {
    colour = placeholders.value(QStringLiteral("filament_colour")).toString();
  }
  try {
    QJsonObject query{{QStringLiteral("query"), QStringLiteral("purge_defaults")}};
    if (!colour.isEmpty()) {
      query.insert(QStringLiteral("color"), colour);
    }
    const QJsonObject defaults = m_host.model().queryObject(query);
    for (const QString& key : {QStringLiteral("purge_into"), QStringLiteral("purge_out")}) {
      placeholders.insert(key, defaults.value(key));
    }
  } catch (const std::exception&) {
    // Without them: Slicer default.
  }
  m_form->setPlaceholderValues(placeholders, words);
  // Another design changed it: asked once, when the design uses it or it
  // is edited (mitcad#655), not on a mere selection.
  if (item->changed && !changeAsked(*item) && usedByDesign(*item)) {
    TestSync::singleShot(0, this, [this] { askChanges(); });
  }
}

bool MaterialsPanel::canChange() {
  if (m_host.canChangePrintSettings()) {
    return true;
  }
  setMessage(m_host.changeBlockedReason(), true);
  return false;
}

bool MaterialsPanel::run(const QJsonObject& command, QJsonObject* result) {
  if (m_host.runModelCommand(command, result)) {
    setMessage(QString());
    m_host.commandInputsChanged();
    emit changed();
    return true;
  }
  if (!m_host.lastCancelled()) {
    const LibraryItem* item = current();
    setMessage(plainModelError(m_host.lastError(), m_keys, command, item != nullptr ? item->name : QString()),
               true); // plain words (mitcad#655)
  }
  qDebug().noquote() << QStringLiteral("%1: %2 refused: %3").arg(m_log)
                            .arg(command.value(QStringLiteral("cmd")).toString(), m_host.lastError());
  return false;
}

void MaterialsPanel::setMessage(const QString& message, bool error) {
  m_message->setText(message);
  if (error) {
    setErrorStyleSheet(m_message);
  } else {
    m_message->setStyleSheet(QString());
  }
}

void MaterialsPanel::edit(const QJsonObject& change, const QString& what, const QString& field) {
  const LibraryItem* item = current();
  if (item == nullptr || item->readOnly) {
    return;
  }
  const LibraryItem target = *item;
  // Not while the field that changed still handles its signal: the model
  // command shows the window again.
  TestSync::singleShot(0, this, [this, target, change, what, field] {
    if (!canChange()) {
      refresh();
      return;
    }
    // Edited while another design's change waits: asked first
    // (mitcad#655).
    if (target.changed && !changeAsked(target)) {
      run(askChangedElsewhere(this, target, m_log, itemsProject()));
    }
    QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("edit_material")}};
    nameItem(command, target, itemsProject());
    for (auto it = change.begin(); it != change.end(); ++it) {
      command.insert(it.key(), it.value());
    }
    const bool done = run(command);
    if (done) {
      qDebug().noquote() << QStringLiteral("Material %1 (%2) %3").arg(target.name, target.place, what);
    }
    refresh();
    if (!done && !field.isEmpty() && !m_host.lastCancelled()) {
      // A refusal of a print setting under its field (mitcad#655).
      setMessage(QString());
      m_form->setError(field, m_keys.plainText(m_host.lastError()));
    }
  });
}

void MaterialsPanel::appearanceChosen(int index) {
  const QString id = m_appearance->itemData(index).toString();
  if (id == QLatin1String(kEditAppearances)) {
    showCurrent(); // the box shows the material's again
    if (editAppearances) {
      editAppearances();
    }
    return;
  }
  const LibraryItem* item = current();
  if (item == nullptr || id == text(designFields(*item, m_designMaterials, m_library), "appearance")) {
    return;
  }
  edit({{QStringLiteral("appearance"), id.isEmpty() ? QJsonValue() : QJsonValue(id)}},
       QStringLiteral("appearance = %1").arg(id.isEmpty() ? QStringLiteral("none") : id));
}

void MaterialsPanel::newMaterial() {
  if (!canChange()) {
    return;
  }
  // Its class (mitcad#655); made in this design and selected, put into a
  // library only by Save to; in My Library made in its library
  // (mitcad#704).
  QMenu menu(this);
  QStringList titles;
  for (const QString& materialClass : materialClasses()) {
    menu.addAction(materialClassTitle(materialClass))->setData(materialClass);
    titles << materialClassTitle(materialClass);
  }
  qDebug().noquote() << QStringLiteral("%1 new menu: %2").arg(m_log).arg(titles.join(QStringLiteral(" | ")));
  QAction* chosen = execMenu(&menu, m_new->mapToGlobal(QPoint(0, m_new->height())));
  if (chosen == nullptr) {
    return;
  }
  // The model names it (Material 1). Its density is its class's typical
  // one (mitcad#655): steel's for a metal, PLA's for a plastic; another
  // material has none typical, so Density takes the keyboard to be set.
  const QString materialClass = chosen->data().toString();
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("create_material")},
                      {QStringLiteral("class"), materialClass}};
  if (materialClass == QStringLiteral("metal")) {
    command.insert(QStringLiteral("density"), 7.85);
  } else if (materialClass == QStringLiteral("plastic")) {
    command.insert(QStringLiteral("density"), 1.24);
  }
  inPlace(command);
  QJsonObject result;
  const bool created = run(command, &result);
  if (created) {
    const QString place = inLibrary() ? m_libraryPlace : QStringLiteral("design");
    m_currentKey = place + QLatin1Char('/') + text(result, "id");
    m_currentDesignId = inLibrary() ? QString() : text(result, "id");
    m_pendingFocus = m_currentKey;
    m_pendingDensity = materialClass == QStringLiteral("other");
    qDebug().noquote() << QStringLiteral("Material created: %1 in %2").arg(text(result, "id"), place);
  }
  refresh();
  if (created && m_assignment->varies()) {
    // Into the active row at once (mitcad#861).
    fillRows();
  }
}

void MaterialsPanel::inPlace(QJsonObject& command) const {
  if (inLibrary()) {
    command.insert(QStringLiteral("library"), m_libraryPlace);
    if (!itemsProject().isEmpty()) {
      command.insert(QStringLiteral("project"), itemsProject());
    }
  }
}

void MaterialsPanel::duplicate() {
  const LibraryItem* item = current();
  if (item == nullptr || !canChange()) {
    return;
  }
  const LibraryItem source = *item;
  const QString project = itemsProject();
  // The user's own copy in this design, selected (mitcad#655): the
  // design's copy of the source when it has one, else the library's
  // item, with the appearance it carries. In My Library the copy is made
  // in its library (mitcad#704), from a built-in one by its id.
  const QJsonValue basedOn = source.library.isEmpty() || !source.inDesign.isEmpty()
                                 ? QJsonValue(source.designId())
                                 : QJsonValue(QJsonObject{{QStringLiteral("library"), source.library},
                                                          {QStringLiteral("id"), source.id}});
  // The model names it after the source ("PLA (copy)").
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("create_material")},
                      {QStringLiteral("based_on"), basedOn}};
  if (!project.isEmpty() && basedOn.isObject()) {
    command.insert(QStringLiteral("project"), project);
  }
  inPlace(command);
  QJsonObject result;
  const bool created = run(command, &result);
  if (created) {
    const QString place = inLibrary() ? m_libraryPlace : QStringLiteral("design");
    m_currentKey = place + QLatin1Char('/') + text(result, "id");
    m_currentDesignId = inLibrary() ? QString() : text(result, "id");
    m_pendingFocus = m_currentKey;
    qDebug().noquote() << QStringLiteral("Material duplicated: %1 as %2 in %3").arg(source.name, text(result, "id"), place);
  }
  refresh();
  if (created && m_assignment->varies()) {
    // Into the active row at once (mitcad#861).
    fillRows();
  }
}

void MaterialsPanel::remove() {
  const LibraryItem* item = current();
  if (item == nullptr || item->readOnly || !canChange()) {
    return;
  }
  const LibraryItem target = *item;
  // Always asked, naming the bodies that use it and what they then use
  // (mitcad#655): Default, the root's default material, else Steel; the
  // bodies without a material of their own when it is the root's default.
  QStringList users;
  for (const QJsonValue& uid : target.row.value(QStringLiteral("bodies")).toArray()) {
    for (const QJsonValue& value : std::as_const(m_bodies)) {
      if (text(value.toObject(), "uid") == uid.toString()) {
        users << text(value.toObject(), "name");
      }
    }
  }
  QString rootDefault;
  bool isRootDefault = false;
  for (const QJsonValue& value : std::as_const(m_designMaterials)) {
    const QJsonObject material = value.toObject();
    if (material.value(QStringLiteral("components")).toArray().contains(QStringLiteral("C0"))) {
      rootDefault = text(material, "name");
      isRootDefault = text(material, "id") == target.designId();
    }
  }
  QString steel = tr("Steel");
  for (const QJsonValue& value : std::as_const(m_designMaterials)) {
    if (text(value.toObject(), "id") == QStringLiteral("steel")) {
      steel = text(value.toObject(), "name");
    }
  }
  if (isRootDefault) {
    for (const QJsonValue& value : std::as_const(m_bodies)) {
      if (text(value.toObject(), "material").isEmpty()) {
        users << text(value.toObject(), "name");
      }
    }
  }
  users.removeDuplicates();
  const QString fallback =
      tr("Default (%1)").arg(rootDefault.isEmpty() || isRootDefault ? steel : rootDefault);
  if (!confirmDelete(this, target, m_log, users, fallback)) {
    return;
  }
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("delete_material")}};
  nameItem(command, target, itemsProject());
  if (run(command)) {
    qDebug().noquote() << QStringLiteral("Material deleted: %1 (%2)").arg(target.name, target.place);
  }
  refresh();
}

void MaterialsPanel::saveTo() {
  const LibraryItem* item = current();
  if (item == nullptr || !item->unsaved() || !canChange()) {
    return;
  }
  const LibraryItem target = *item;
  const QString project = m_host.projectFolder();
  const QString library = choosePlace(m_saveTo, m_log + QStringLiteral(" save_to"), !project.isEmpty());
  if (library.isEmpty()) {
    return;
  }
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("save_to_print_library")},
                      {QStringLiteral("library"), library},
                      {QStringLiteral("kind"), QStringLiteral("material")},
                      {QStringLiteral("id"), target.designId()}};
  if (!project.isEmpty()) {
    command.insert(QStringLiteral("project"), project);
  }
  if (run(command)) {
    m_currentDesignId = target.designId();
    m_currentKey.clear(); // found again by its design copy
    qDebug().noquote() << QStringLiteral("Material saved to %1: %2").arg(library, target.name);
  }
  refresh();
}

void MaterialsPanel::askChanges() {
  const LibraryItem* item = current();
  if (item == nullptr || !item->changed || changeAsked(*item) || !isVisible() ||
      !m_host.canChangePrintSettings()) {
    return;
  }
  const LibraryItem target = *item;
  // Use the changes or Keep these, one undo step either way (mitcad#655).
  const QJsonObject command = askChangedElsewhere(this, target, m_log, itemsProject());
  if (run(command)) {
    qDebug().noquote() << QStringLiteral("Material %1: %2").arg(target.name, command.value(QStringLiteral("cmd")).toString());
  }
  refresh();
}

void MaterialsPanel::assign() {
  const bool forComponent = !m_targetComponent.isEmpty();
  if ((m_targetBodies.isEmpty() && !forComponent) || !canChange()) {
    return;
  }
  if (!forComponent && m_assignment->varies()) {
    // The rows of the option the material varies with (mitcad#861): the
    // window stays for the next row.
    fillRows();
    return;
  }
  const LibraryItem* item = current();
  if (item == nullptr && m_currentKey != QLatin1String(kDefaultKey)) {
    return;
  }
  const QString project = m_host.projectFolder();
  const QString logged = item == nullptr ? QStringLiteral("default")
                                         : (item->designId().isEmpty() ? item->id : item->designId());
  // A material from a library is copied into the design in the same undo
  // step (the command's `library`).
  const auto named = [&](QJsonObject command) {
    if (item == nullptr) {
      command.insert(QStringLiteral("material"), QJsonValue());
    } else {
      nameItem(command, *item, project, QStringLiteral("material"));
    }
    return command;
  };
  if (forComponent) {
    if (run(named({{QStringLiteral("cmd"), QStringLiteral("set_component_material")},
                   {QStringLiteral("component"), m_targetComponent}}))) {
      qDebug().noquote() << QStringLiteral("Default material of %1: %2")
                                .arg(m_targetNames.value(0), item == nullptr ? QStringLiteral("none") : logged);
      emit assigned();
    }
    return;
  }
  // The material's appearance replaces the bodies' unless one was set by
  // hand: then the user says which.
  const QString look =
      item == nullptr ? QString() : text(designFields(*item, m_designMaterials, m_library), "appearance");
  QStringList conflicts;
  for (const QJsonValue& value : std::as_const(m_bodies)) {
    const QJsonObject body = value.toObject();
    if (m_targetBodies.contains(text(body, "uid")) && body.value(QStringLiteral("appearance_set_by_hand")).toBool() &&
        text(body, "appearance") != look && item != nullptr) {
      conflicts << text(body, "name");
    }
  }
  QString appearance = QStringLiteral("replace");
  if (!conflicts.isEmpty()) {
    qDebug().noquote() << QStringLiteral("Material: replace the appearance of %1?")
                              .arg(conflicts.join(QStringLiteral(", ")));
    QMessageBox box(QMessageBox::Question, tr("Replace Appearance"),
                    conflicts.size() == 1 ? tr("%1 has an appearance set by hand.").arg(conflicts.first())
                                          : tr("%1 bodies have appearances set by hand.").arg(conflicts.size()),
                    QMessageBox::Cancel, this);
    box.setInformativeText(tr("Replace it with the appearance of %1, or keep it?").arg(item->name));
    QPushButton* replace = box.addButton(tr("&Replace"), QMessageBox::AcceptRole);
    QPushButton* keep = box.addButton(tr("&Keep"), QMessageBox::RejectRole);
    box.setDefaultButton(replace);
    prepareModal(&box);
    execDialog(&box);
    if (box.clickedButton() == keep) {
      appearance = QStringLiteral("keep");
    } else if (box.clickedButton() != replace) {
      qDebug() << "Material cancelled";
      return;
    }
  }
  QJsonArray commands;
  for (const QString& body : std::as_const(m_targetBodies)) {
    commands.append(named({{QStringLiteral("cmd"), QStringLiteral("set_body_material")},
                           {QStringLiteral("uid"), body},
                           {QStringLiteral("appearance"), appearance}}));
  }
  const bool done = commands.size() == 1 ? run(commands.first().toObject())
                                         : m_host.runModelCommands(commands, tr("Material"));
  if (!done) {
    if (commands.size() > 1) {
      setMessage(plainModelError(m_host.lastError(), m_keys, commands.first().toObject()), true);
    }
    return;
  }
  m_host.commandInputsChanged();
  qDebug().noquote() << QStringLiteral("Material %1 set for %2 body(ies), appearance %3")
                            .arg(logged)
                            .arg(m_targetBodies.size())
                            .arg(appearance);
  if (m_keepOpen) {
    // From 3D Print's Assign materials… (mitcad#655): the window stays for
    // the next material and bodies.
    refresh();
    return;
  }
  emit assigned();
}

void MaterialsPanel::fillRows() {
  const LibraryItem* item = current();
  if (item == nullptr && m_currentKey != QLatin1String(kDefaultKey)) {
    return;
  }
  QString result;
  QJsonArray before;
  if (item == nullptr) {
    // Default: what it gives, the root's default material, else steel.
    QString id = QStringLiteral("steel");
    for (const QJsonValue& value : std::as_const(m_designMaterials)) {
      if (value.toObject().value(QStringLiteral("components")).toArray().contains(QStringLiteral("C0"))) {
        id = text(value.toObject(), "id");
      }
    }
    result = quotedText(id);
  } else if (!item->designId().isEmpty()) {
    result = quotedText(item->designId());
  } else if (!m_targetBodies.isEmpty()) {
    // A library's material not in the design yet: copied in by the same
    // undo step, as Assign does, and named by its name.
    QJsonObject copy{{QStringLiteral("cmd"), QStringLiteral("set_body_material")},
                     {QStringLiteral("uid"), m_targetBodies.first()}};
    nameItem(copy, *item, m_host.projectFolder(), QStringLiteral("material"));
    before.append(copy);
    result = quotedText(item->name);
  }
  // Copies: the fill's refresh replaces the items.
  const QString logged = item == nullptr ? QStringLiteral("default") : item->name;
  if (!result.isEmpty() && m_assignment->fill(result, before)) {
    qDebug().noquote() << QStringLiteral("%1: %2 into the rows").arg(m_log, logged);
  }
}

void MaterialsPanel::logRows() {
  if (!isVisible()) {
    return;
  }
  QStringList rows;
  for (const LibraryItem& item : std::as_const(m_items)) {
    rows << QStringLiteral("%1/%2 %3 (%4)%5")
                .arg(item.group.isEmpty() ? QStringLiteral("other") : item.group, item.id, item.name, item.place,
                     item.changed ? QStringLiteral(" changed") : QString());
  }
  const QString logged = rows.join(QLatin1Char('\n'));
  if (logged == m_loggedRows) {
    return;
  }
  m_loggedRows = logged;
  qDebug().noquote() << QStringLiteral("%1: %2 items").arg(m_log).arg(rows.size());
  for (const QString& row : std::as_const(rows)) {
    qDebug().noquote() << QStringLiteral("%1 row: %2").arg(m_log).arg(row);
  }
  if (const LibraryItem* item = current()) {
    for (const QString& value : m_form->rows()) {
      qDebug().noquote() << QStringLiteral("Material %1 value %2").arg(item->name, value);
    }
  }
}

void MaterialsPanel::showEvent(QShowEvent* event) {
  QWidget::showEvent(event);
  m_loggedRows.clear();
  m_loggedPlaces.clear();
  logRows();
  scheduleLogPlaces();
}

void MaterialsPanel::resizeEvent(QResizeEvent* event) {
  QWidget::resizeEvent(event);
  scheduleLogPlaces();
}

bool MaterialsPanel::eventFilter(QObject* watched, QEvent* event) {
  // F2 on a row: its name.
  if (watched == m_list && event->type() == QEvent::KeyPress &&
      static_cast<QKeyEvent*>(event)->key() == Qt::Key_F2 && current() != nullptr && !current()->readOnly) {
    m_name->setFocus(Qt::OtherFocusReason);
    m_name->selectAll();
    return true;
  }
  return QWidget::eventFilter(watched, event);
}

void MaterialsPanel::scheduleLogPlaces() {
  TestSync::singleShot(0, this, [this] { logPlaces(); });
}

void MaterialsPanel::logPlaces() {
  // In the main window's coordinates (the parent of the panel's window), as
  // the other logged places.
  if (!isVisible() || window()->parentWidget() == nullptr) {
    return;
  }
  const QPoint origin = window()->parentWidget()->window()->mapToGlobal(QPoint(0, 0));
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
    const LibraryItem* item = findItem(m_items, key);
    lines << QStringLiteral("%1 item %2 at %3").arg(m_log)
                 .arg(item != nullptr ? item->name : QString::fromLatin1(kDefaultKey),
                      at(m_list->viewport(), QPoint(60, rect.center().y())));
  }
  QVector<std::pair<QString, QWidget*>> widgets{
      {QStringLiteral("name"), m_name},
      {QStringLiteral("class"), m_class},
      {QStringLiteral("save_to"), m_saveTo},
      {QStringLiteral("header_duplicate"), m_headerDuplicate},
      {QStringLiteral("density"), m_density},
      {QStringLiteral("appearance"), m_appearance},
      {QStringLiteral("new"), m_new},
      {QStringLiteral("duplicate"), m_duplicate},
      {QStringLiteral("delete"), m_delete},
      {QStringLiteral("assign"), m_assign}};
  // The window's buttons (My Library…, Close).
  for (const auto& [name, button] : std::as_const(m_windowButtons)) {
    widgets.append(std::pair<QString, QWidget*>(name, button));
  }
  for (const auto& [name, widget] : std::as_const(widgets)) {
    if (widget->isVisibleTo(this)) {
      lines << QStringLiteral("%1 %2 at %3").arg(m_log).arg(name, at(widget, widget->rect().center()));
    }
  }
  if (m_form->isVisibleTo(this)) {
    lines << m_form->places(m_log + QStringLiteral(" settings"), at);
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
