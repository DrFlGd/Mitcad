// SPDX-License-Identifier: MIT
#include "PropertySetsDialog.hpp"

#include <functional>
#include <utility>

#include <QCheckBox>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QFont>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonValue>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QPalette>
#include <QPushButton>
#include <QSet>
#include <QTimer>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>
#include <QVariantMap>
#include <QtLogging>

#include "../framework/Dialogs.hpp"
#include "../framework/Icons.hpp"
#include "../framework/Theme.hpp"
#include "DocumentHost.hpp"

namespace mitcad {
namespace {

constexpr int kLevelRole = Qt::UserRole;        // body, occurrence or component
constexpr int kOwnerRole = Qt::UserRole + 1;    // the level's owner (F2.b0, O3, C1)
constexpr int kSetRole = Qt::UserRole + 2;      // the property's set
constexpr int kNameRole = Qt::UserRole + 3;     // the property's key
constexpr int kPropertyRole = Qt::UserRole + 4; // the property as the query gave it

QString text(const QJsonObject& object, const char* field) {
  return object.value(QLatin1String(field)).toString();
}

// What the source column says: the source, and whether the value is the
// component's (on an occurrence) or the user changed what its source gave.
QString sourceText(const QJsonObject& property) {
  QString source = text(property, "source");
  QStringList notes;
  if (property.value(QStringLiteral("inherited")).toBool()) {
    notes << QObject::tr("inherited");
  }
  if (property.value(QStringLiteral("overrides")).toBool()) {
    notes << QObject::tr("overrides");
  }
  if (property.value(QStringLiteral("edited")).toBool()) {
    notes << QObject::tr("edited");
  }
  return notes.isEmpty() ? source : QStringLiteral("%1, %2").arg(source, notes.join(QStringLiteral(", ")));
}

// The standard types, as the model names them.
const QStringList& standardTypes() {
  static const QStringList types{QStringLiteral("text"),   QStringLiteral("integer"), QStringLiteral("number"),
                                 QStringLiteral("length"), QStringLiteral("area"),    QStringLiteral("volume"),
                                 QStringLiteral("mass"),   QStringLiteral("angle"),   QStringLiteral("boolean"),
                                 QStringLiteral("date")};
  return types;
}

// F2 edits the current row's value, as double-click does.
class PropertyTable : public QTreeWidget {
public:
  explicit PropertyTable(std::function<void(QTreeWidgetItem*, int)> edit) : m_edit(std::move(edit)) {}

protected:
  void keyPressEvent(QKeyEvent* event) override {
    if (event->key() == Qt::Key_F2 && currentItem() != nullptr && state() != QAbstractItemView::EditingState) {
      m_edit(currentItem(), 1);
      return;
    }
    QTreeWidget::keyPressEvent(event);
  }

private:
  std::function<void(QTreeWidgetItem*, int)> m_edit;
};

// Add Property: the level, set, key, type and value; stays open with the
// model's reason when the model refuses them.
class AddPropertyDialog : public QDialog {
public:
  AddPropertyDialog(DocumentHost& host, const QJsonArray& levels, const QStringList& types, QWidget* parent)
      : QDialog(parent), m_host(host) {
    setWindowTitle(tr("Add Property"));
    auto* form = new QFormLayout(this);
    m_level = new QComboBox;
    for (const QJsonValue& value : levels) {
      const QJsonObject level = value.toObject();
      m_level->addItem(QStringLiteral("%1 %2").arg(text(level, "level"), text(level, "name")), text(level, "owner"));
    }
    m_set = new QLineEdit(QStringLiteral("Properties"));
    m_key = new QLineEdit;
    m_type = new QComboBox;
    m_type->addItems(standardTypes());
    m_type->addItems(types);
    m_value = new QLineEdit;
    m_value->setToolTip(tr("In the design's units unless typed with one (1.2 m); yes/no; dates YYYY-MM-DD"));
    m_error = new QLabel;
    m_error->setWordWrap(true);
    setErrorStyleSheet(m_error);
    form->addRow(tr("Level"), m_level);
    form->addRow(tr("Set"), m_set);
    form->addRow(tr("Key"), m_key);
    form->addRow(tr("Type"), m_type);
    form->addRow(tr("Value"), m_value);
    form->addRow(m_error);
    auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
    form->addRow(buttons);
    connect(buttons, &QDialogButtonBox::accepted, this, &AddPropertyDialog::add);
    connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
    m_key->setFocus();
  }

private:
  void add() {
    const QString key = m_key->text().trimmed();
    QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("set_property")},
                        {QStringLiteral("owner"), m_level->currentData().toString()},
                        {QStringLiteral("set"), m_set->text().trimmed()},
                        {QStringLiteral("key"), key},
                        {QStringLiteral("type"), m_type->currentText()}};
    if (!m_value->text().trimmed().isEmpty()) {
      command.insert(QStringLiteral("value"), m_value->text().trimmed());
    }
    if (m_host.runModelCommand(command)) {
      qDebug().noquote() << QStringLiteral("Added property %1/%2 (%3)")
                                .arg(m_set->text().trimmed(), key, m_type->currentText());
      accept();
      return;
    }
    m_error->setText(m_host.lastError());
    qDebug().noquote() << QStringLiteral("Add property %1 refused: %2").arg(key, m_host.lastError());
  }

  DocumentHost& m_host;
  QComboBox* m_level;
  QLineEdit* m_set;
  QLineEdit* m_key;
  QComboBox* m_type;
  QLineEdit* m_value;
  QLabel* m_error;
};

// Property Types: the design's own types, defined and removed here.
class PropertyTypesDialog : public QDialog {
public:
  PropertyTypesDialog(DocumentHost& host, QWidget* parent) : QDialog(parent), m_host(host) {
    setWindowTitle(tr("Property Types"));
    resize(520, 360);
    auto* layout = new QVBoxLayout(this);
    m_list = new QListWidget;
    layout->addWidget(m_list, 1);
    auto* form = new QFormLayout;
    m_name = new QLineEdit;
    m_base = new QComboBox;
    m_base->addItems(standardTypes());
    m_values = new QLineEdit;
    m_values->setPlaceholderText(tr("Allowed values, separated by commas (any when empty)"));
    m_unit = new QLineEdit;
    m_unit->setPlaceholderText(tr("A quantity's unit (m, deg, kg) or a number's label"));
    m_description = new QLineEdit;
    form->addRow(tr("Name"), m_name);
    form->addRow(tr("Base type"), m_base);
    form->addRow(tr("Values"), m_values);
    form->addRow(tr("Unit"), m_unit);
    form->addRow(tr("Description"), m_description);
    layout->addLayout(form);
    m_error = new QLabel;
    m_error->setWordWrap(true);
    setErrorStyleSheet(m_error);
    layout->addWidget(m_error);
    auto* buttons = new QHBoxLayout;
    auto* define = new QPushButton(tr("Define"));
    auto* remove = new QPushButton(tr("Remove"));
    auto* close = new QPushButton(tr("Close"));
    for (QPushButton* button : {define, remove, close}) {
      button->setAutoDefault(false);
    }
    buttons->addWidget(define);
    buttons->addWidget(remove);
    buttons->addStretch(1);
    buttons->addWidget(close);
    layout->addLayout(buttons);
    connect(define, &QPushButton::clicked, this, &PropertyTypesDialog::define);
    connect(remove, &QPushButton::clicked, this, &PropertyTypesDialog::remove);
    connect(close, &QPushButton::clicked, this, &QDialog::accept);
    load();
  }

private:
  void load() {
    m_list->clear();
    try {
      const QJsonObject answer =
          m_host.model().queryObject({{QStringLiteral("query"), QStringLiteral("property_types")}});
      for (const QJsonValue& value : answer.value(QStringLiteral("types")).toArray()) {
        const QJsonObject type = value.toObject();
        QStringList values;
        for (const QJsonValue& v : type.value(QStringLiteral("values")).toArray()) {
          values << text(v.toObject(), "text");
        }
        auto* item = new QListWidgetItem(
            QStringLiteral("%1 (%2)%3")
                .arg(text(type, "name"), text(type, "base"),
                     values.isEmpty() ? QString() : QStringLiteral(": ") + values.join(QStringLiteral(", "))),
            m_list);
        item->setData(Qt::UserRole, text(type, "name"));
        item->setToolTip(text(type, "description"));
      }
    } catch (const std::exception& e) {
      m_error->setText(QString::fromUtf8(e.what()));
    }
  }

  void define() {
    const QString name = m_name->text().trimmed();
    QJsonArray values;
    for (const QString& value : m_values->text().split(QLatin1Char(','), Qt::SkipEmptyParts)) {
      if (!value.trimmed().isEmpty()) {
        values.append(value.trimmed());
      }
    }
    QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("define_property_type")},
                        {QStringLiteral("name"), name},
                        {QStringLiteral("base"), m_base->currentText()},
                        {QStringLiteral("values"), values},
                        {QStringLiteral("description"), m_description->text().trimmed()}};
    if (!m_unit->text().trimmed().isEmpty()) {
      command.insert(QStringLiteral("unit"), m_unit->text().trimmed());
    }
    if (m_host.runModelCommand(command)) {
      m_error->clear();
      qDebug().noquote() << QStringLiteral("Property type defined: %1 (%2)").arg(name, m_base->currentText());
      m_name->clear();
      m_values->clear();
      m_unit->clear();
      m_description->clear();
    } else {
      m_error->setText(m_host.lastError());
      qDebug().noquote() << QStringLiteral("Property type %1 refused: %2").arg(name, m_host.lastError());
    }
    load();
  }

  void remove() {
    QListWidgetItem* item = m_list->currentItem();
    if (item == nullptr) {
      return;
    }
    const QString name = item->data(Qt::UserRole).toString();
    if (m_host.runModelCommand({{QStringLiteral("cmd"), QStringLiteral("remove_property_type")},
                                {QStringLiteral("name"), name}})) {
      m_error->clear();
      qDebug().noquote() << QStringLiteral("Property type removed: %1").arg(name);
    } else {
      m_error->setText(m_host.lastError());
    }
    load();
  }

  DocumentHost& m_host;
  QListWidget* m_list;
  QLineEdit* m_name;
  QComboBox* m_base;
  QLineEdit* m_values;
  QLineEdit* m_unit;
  QLineEdit* m_description;
  QLabel* m_error;
};

} // namespace

PropertySetsDialog::PropertySetsDialog(DocumentHost& host, QWidget* parent) : QDialog(parent), m_host(host) {
  setWindowTitle(tr("Property Sets"));
  setWindowIcon(themeIcon(QStringLiteral("parameters")));
  resize(720, 480);
  auto* layout = new QVBoxLayout(this);

  auto* top = new QHBoxLayout;
  m_title = new QLabel;
  m_computed = new QCheckBox(tr("Computed values"));
  m_computed->setToolTip(tr("Also the values worked out from the model: volume, area, mass, a member's length"));
  m_computed->setChecked(true);
  top->addWidget(m_title, 1);
  top->addWidget(m_computed);
  layout->addLayout(top);

  m_table = new PropertyTable([this](QTreeWidgetItem* item, int column) { edit(item, column); });
  m_table->setColumnCount(4);
  m_table->setHeaderLabels({tr("Property"), tr("Value"), tr("Type"), tr("Source")});
  m_table->setEditTriggers(QAbstractItemView::NoEditTriggers);
  m_table->setSelectionMode(QAbstractItemView::SingleSelection);
  m_table->setAlternatingRowColors(true);
  m_table->setUniformRowHeights(true);
  m_table->header()->setStretchLastSection(false);
  m_table->header()->resizeSection(kKey, 220);
  m_table->header()->setSectionResizeMode(kValue, QHeaderView::Stretch);
  m_table->header()->resizeSection(kType, 110);
  m_table->header()->resizeSection(kSource, 130);
  layout->addWidget(m_table, 1);

  m_message = new QLabel;
  m_message->setWordWrap(true);
  setErrorStyleSheet(m_message);
  layout->addWidget(m_message);

  auto* buttons = new QHBoxLayout;
  m_add = new QPushButton(themeIcon(QStringLiteral("add")), tr("Add..."));
  m_add->setToolTip(tr("Add a property to one of the levels"));
  m_remove = new QPushButton(themeIcon(QStringLiteral("delete")), tr("Remove"));
  m_remove->setToolTip(tr("Remove the selected property (an occurrence's shows its component's again)"));
  m_types = new QPushButton(tr("Types..."));
  m_types->setToolTip(tr("The design's own property types: allowed values, unit, description"));
  m_close = new QPushButton(tr("Close"));
  for (QPushButton* button : {m_add, m_remove, m_types, m_close}) {
    button->setAutoDefault(false);
  }
  buttons->addWidget(m_add);
  buttons->addWidget(m_remove);
  buttons->addWidget(m_types);
  buttons->addStretch(1);
  buttons->addWidget(m_close);
  layout->addLayout(buttons);

  connect(m_close, &QPushButton::clicked, this, &QDialog::accept);
  connect(m_add, &QPushButton::clicked, this, &PropertySetsDialog::addProperty);
  connect(m_remove, &QPushButton::clicked, this, &PropertySetsDialog::removeProperty);
  connect(m_types, &QPushButton::clicked, this, &PropertySetsDialog::editTypes);
  connect(m_computed, &QCheckBox::toggled, this, &PropertySetsDialog::refresh);
  connect(m_table, &QTreeWidget::itemDoubleClicked, this, &PropertySetsDialog::edit);
  connect(m_table, &QTreeWidget::itemChanged, this, &PropertySetsDialog::cellEdited);
  connect(m_table, &QTreeWidget::itemSelectionChanged, this, &PropertySetsDialog::updateButtons);
  // Typing a key's first letters goes to its row; F2 edits it.
  m_table->setFocus();
}

void PropertySetsDialog::setTarget(const PropertyTarget& target) {
  m_target = target;
  m_message->clear();
  refresh();
}

QString PropertySetsDialog::rowName(const QTreeWidgetItem* item) {
  return QStringLiteral("%1 %2/%3")
      .arg(item->data(kKey, kLevelRole).toString(), item->data(kKey, kSetRole).toString(),
           item->data(kKey, kNameRole).toString());
}

void PropertySetsDialog::refresh() {
  if (m_target.isEmpty()) {
    return;
  }
  QJsonObject query{{QStringLiteral("query"), QStringLiteral("property_sets")},
                    {QStringLiteral("computed"), m_computed->isChecked()}};
  if (!m_target.body.isEmpty()) {
    query.insert(QStringLiteral("body"), m_target.body);
  }
  if (!m_target.occurrence.isEmpty()) {
    query.insert(QStringLiteral("occurrence"), m_target.occurrence);
  }
  if (!m_target.component.isEmpty()) {
    query.insert(QStringLiteral("component"), m_target.component);
  }
  try {
    m_answer = m_host.model().queryObject(query);
  } catch (const std::exception& e) {
    // The item is gone (deleted, rolled back): nothing to show.
    m_message->setText(QString::fromUtf8(e.what()));
    m_answer = QJsonObject();
  }
  // Folded sets stay folded.
  QSet<QString> folded;
  for (QTreeWidgetItemIterator it(m_table); *it != nullptr; ++it) {
    if (!(*it)->isExpanded() && (*it)->childCount() > 0) {
      folded.insert((*it)->data(kKey, kLevelRole).toString() + QLatin1Char('|') +
                    (*it)->data(kKey, kSetRole).toString());
    }
  }
  QString current;
  if (QTreeWidgetItem* item = m_table->currentItem()) {
    current = rowName(item);
  }
  m_building = true;
  m_table->clear();
  QStringList rows;
  QStringList titles;
  const QJsonArray levels = m_answer.value(QStringLiteral("levels")).toArray();
  for (const QJsonValue& levelValue : levels) {
    const QJsonObject level = levelValue.toObject();
    const QString kind = text(level, "level");
    const QString owner = text(level, "owner");
    const QString title = kind == QStringLiteral("body")         ? tr("Body %1").arg(text(level, "name"))
                          : kind == QStringLiteral("occurrence") ? tr("Occurrence %1").arg(text(level, "name"))
                                                                 : tr("Component %1").arg(text(level, "name"));
    titles << title;
    auto* top = new QTreeWidgetItem(m_table, {title});
    top->setData(kKey, kLevelRole, kind);
    top->setFirstColumnSpanned(true);
    QFont bold = top->font(kKey);
    bold.setBold(true);
    top->setFont(kKey, bold);
    const QJsonArray sets = level.value(QStringLiteral("sets")).toArray();
    if (sets.isEmpty()) {
      auto* none = new QTreeWidgetItem(top, {tr("No properties")});
      none->setFirstColumnSpanned(true);
      none->setDisabled(true);
    }
    for (const QJsonValue& setValue : sets) {
      const QJsonObject set = setValue.toObject();
      const QString setName = text(set, "name");
      auto* group = new QTreeWidgetItem(top, {setName});
      group->setData(kKey, kLevelRole, kind);
      group->setData(kKey, kSetRole, setName);
      group->setFirstColumnSpanned(true);
      for (const QJsonValue& propertyValue : set.value(QStringLiteral("properties")).toArray()) {
        const QJsonObject property = propertyValue.toObject();
        auto* row = new QTreeWidgetItem(group);
        row->setText(kKey, text(property, "key"));
        row->setText(kValue, text(property, "text"));
        row->setText(kType, text(property, "type"));
        row->setText(kSource, sourceText(property));
        row->setData(kKey, kLevelRole, kind);
        row->setData(kKey, kOwnerRole, owner);
        row->setData(kKey, kSetRole, setName);
        row->setData(kKey, kNameRole, text(property, "key"));
        row->setData(kKey, kPropertyRole, property.toVariantMap());
        const bool readOnly = property.value(QStringLiteral("read_only")).toBool();
        if (!readOnly) {
          row->setFlags(row->flags() | Qt::ItemIsEditable);
        } else {
          row->setForeground(kValue, palette().color(QPalette::Disabled, QPalette::Text));
        }
        if (property.contains(QStringLiteral("source_value"))) {
          row->setToolTip(kSource, tr("%1 gave %2").arg(text(property, "source"), text(property, "source_value")));
          row->setToolTip(kValue, row->toolTip(kSource));
        }
        if (text(property, "type") != text(property, "base")) {
          row->setToolTip(kType, tr("Own type, based on %1").arg(text(property, "base")));
        }
        if (rowName(row) == current) {
          m_table->setCurrentItem(row);
        }
        rows << QStringLiteral("%1 = %2 (%3)").arg(rowName(row), row->text(kValue), row->text(kSource));
      }
      group->setExpanded(!folded.contains(kind + QLatin1Char('|') + setName));
    }
    top->setExpanded(true);
  }
  m_title->setText(titles.join(QStringLiteral(" · ")));
  m_building = false;
  updateButtons();
  if (isVisible()) {
    const QString logged = rows.join(QLatin1Char('\n'));
    if (logged != m_loggedRows) {
      m_loggedRows = logged;
      qDebug().noquote() << QStringLiteral("Property sets of %1: %2 properties")
                                .arg(titles.join(QStringLiteral(", ")))
                                .arg(rows.size());
      for (const QString& row : std::as_const(rows)) {
        qDebug().noquote() << QStringLiteral("Property sets row: %1").arg(row);
      }
    }
  }
  QTimer::singleShot(0, this, &PropertySetsDialog::logLayout);
}

void PropertySetsDialog::edit(QTreeWidgetItem* item, int /*column*/) {
  if (item == nullptr || item->data(kKey, kNameRole).toString().isEmpty()) {
    return;
  }
  const QVariantMap property = item->data(kKey, kPropertyRole).toMap();
  if (property.value(QStringLiteral("read_only")).toBool()) {
    m_message->setText(tr("%1 is worked out from the model; it cannot be set.").arg(item->text(kKey)));
    return;
  }
  QStringList choices = property.value(QStringLiteral("choices")).toStringList();
  if (choices.isEmpty() && property.value(QStringLiteral("base")).toString() == QStringLiteral("boolean")) {
    choices = {QStringLiteral("yes"), QStringLiteral("no")};
  }
  if (!choices.isEmpty()) {
    editChoice(item, choices);
    return;
  }
  m_table->editItem(item, kValue);
  qDebug().noquote() << QStringLiteral("Editing property %1").arg(rowName(item));
}

void PropertySetsDialog::editChoice(QTreeWidgetItem* item, const QStringList& choices) {
  auto* combo = new QComboBox;
  combo->addItems(choices);
  combo->setCurrentText(item->text(kValue));
  m_table->setItemWidget(item, kValue, combo);
  const QString name = rowName(item);
  const QString owner = item->data(kKey, kOwnerRole).toString();
  const QString set = item->data(kKey, kSetRole).toString();
  const QString key = item->data(kKey, kNameRole).toString();
  // The model command rebuilds the table, this combo box too: not while
  // it is still handling its change.
  connect(combo, &QComboBox::currentIndexChanged, this, [this, combo, name, owner, set, key](int) {
    const QString chosen = combo->currentText();
    QTimer::singleShot(0, this, [this, name, owner, set, key, chosen] {
      QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("set_property")},
                          {QStringLiteral("owner"), owner},
                          {QStringLiteral("set"), set},
                          {QStringLiteral("key"), key},
                          {QStringLiteral("value"), chosen}};
      if (m_host.runModelCommand(command)) {
        m_message->clear();
        qDebug().noquote() << QStringLiteral("Property %1 chosen: %2").arg(name, chosen);
      } else {
        m_message->setText(QStringLiteral("%1: %2").arg(key, m_host.lastError()));
        qDebug().noquote() << QStringLiteral("Property %1: %2 refused: %3").arg(name, chosen, m_host.lastError());
      }
      refresh();
    });
  });
  qDebug().noquote() << QStringLiteral("Editing property %1 as a choice: %2")
                            .arg(name, choices.join(QStringLiteral(" | ")));
  // After the view has placed it.
  QTimer::singleShot(100, combo, [this, combo] {
    if (parentWidget() == nullptr || !combo->isVisible()) {
      return;
    }
    const QPoint origin = parentWidget()->window()->mapToGlobal(QPoint(0, 0));
    const QPoint at = combo->mapToGlobal(combo->rect().center()) - origin;
    qDebug().noquote() << QStringLiteral("Property sets choice at %1,%2").arg(at.x()).arg(at.y());
  });
  combo->setFocus();
}

void PropertySetsDialog::cellEdited(QTreeWidgetItem* item, int column) {
  if (m_building || column != kValue || item->data(kKey, kNameRole).toString().isEmpty()) {
    return;
  }
  const QVariantMap property = item->data(kKey, kPropertyRole).toMap();
  const QString typed = item->text(kValue).trimmed();
  if (typed == property.value(QStringLiteral("text")).toString()) {
    return;
  }
  setValue(item, typed);
}

void PropertySetsDialog::setValue(QTreeWidgetItem* item, const QString& typed) {
  const QString name = rowName(item);
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("set_property")},
                      {QStringLiteral("owner"), item->data(kKey, kOwnerRole).toString()},
                      {QStringLiteral("set"), item->data(kKey, kSetRole).toString()},
                      {QStringLiteral("key"), item->data(kKey, kNameRole).toString()},
                      {QStringLiteral("value"), typed}};
  const QString key = item->data(kKey, kNameRole).toString();
  // The model command refreshes the window, this table too: not while the
  // table is still handling its edit.
  QTimer::singleShot(0, this, [this, name, key, typed, command] {
    if (m_host.runModelCommand(command)) {
      m_message->clear();
      qDebug().noquote() << QStringLiteral("Property %1 set: %2").arg(name, typed);
    } else {
      m_message->setText(QStringLiteral("%1: %2").arg(key, m_host.lastError()));
      qDebug().noquote() << QStringLiteral("Property %1: %2 refused: %3").arg(name, typed, m_host.lastError());
    }
    refresh();
  });
}

void PropertySetsDialog::addProperty() {
  QStringList own;
  try {
    const QJsonObject answer =
        m_host.model().queryObject({{QStringLiteral("query"), QStringLiteral("property_types")}});
    for (const QJsonValue& value : answer.value(QStringLiteral("types")).toArray()) {
      own << text(value.toObject(), "name");
    }
  } catch (const std::exception&) {
    // Standard types only.
  }
  AddPropertyDialog dialog(m_host, m_answer.value(QStringLiteral("levels")).toArray(), own, this);
  qDebug().noquote() << "Add Property dialog opened";
  prepareModal(&dialog);
  execDialog(&dialog);
  refresh();
}

void PropertySetsDialog::removeProperty() {
  QTreeWidgetItem* item = m_table->currentItem();
  if (item == nullptr || item->data(kKey, kNameRole).toString().isEmpty()) {
    return;
  }
  const QString name = rowName(item);
  if (m_host.runModelCommand({{QStringLiteral("cmd"), QStringLiteral("remove_property")},
                              {QStringLiteral("owner"), item->data(kKey, kOwnerRole).toString()},
                              {QStringLiteral("set"), item->data(kKey, kSetRole).toString()},
                              {QStringLiteral("key"), item->data(kKey, kNameRole).toString()}})) {
    m_message->clear();
    qDebug().noquote() << QStringLiteral("Removed property %1").arg(name);
  } else {
    m_message->setText(m_host.lastError());
    qDebug().noquote() << QStringLiteral("Remove property %1 refused: %2").arg(name, m_host.lastError());
  }
  refresh();
}

void PropertySetsDialog::editTypes() {
  PropertyTypesDialog dialog(m_host, this);
  qDebug().noquote() << "Property Types dialog opened";
  prepareModal(&dialog);
  execDialog(&dialog);
  refresh();
}

void PropertySetsDialog::updateButtons() {
  QTreeWidgetItem* item = m_table->currentItem();
  const bool own = item != nullptr && !item->data(kKey, kNameRole).toString().isEmpty() &&
                   !item->data(kKey, kPropertyRole).toMap().value(QStringLiteral("inherited")).toBool() &&
                   !item->data(kKey, kPropertyRole).toMap().value(QStringLiteral("read_only")).toBool() &&
                   item->data(kKey, kPropertyRole).toMap().value(QStringLiteral("source")).toString() !=
                       QStringLiteral("library");
  m_remove->setEnabled(own);
  m_add->setEnabled(!m_answer.value(QStringLiteral("levels")).toArray().isEmpty());
}

void PropertySetsDialog::logLayout() {
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
  for (QTreeWidgetItemIterator it(m_table); *it != nullptr; ++it) {
    QTreeWidgetItem* item = *it;
    if (item->data(kKey, kNameRole).toString().isEmpty()) {
      continue;
    }
    const QRect rect = m_table->visualItemRect(item);
    if (!rect.isValid() || !m_table->viewport()->rect().contains(rect.center())) {
      continue;
    }
    const QRect cell(m_table->header()->sectionViewportPosition(kValue), rect.top(),
                     m_table->header()->sectionSize(kValue), rect.height());
    lines << QStringLiteral("Property sets %1 value at %2").arg(rowName(item), at(m_table->viewport(), cell.center()));
  }
  lines << QStringLiteral("Property sets add at %1").arg(at(m_add, m_add->rect().center()));
  lines << QStringLiteral("Property sets remove at %1").arg(at(m_remove, m_remove->rect().center()));
  lines << QStringLiteral("Property sets types at %1").arg(at(m_types, m_types->rect().center()));
  lines << QStringLiteral("Property sets computed at %1").arg(at(m_computed, QPoint(10, m_computed->height() / 2)));
  lines << QStringLiteral("Property sets close at %1").arg(at(m_close, m_close->rect().center()));
  const QString logged = lines.join(QLatin1Char('\n'));
  if (logged != m_logged) {
    m_logged = logged;
    for (const QString& line : std::as_const(lines)) {
      qDebug().noquote() << line;
    }
  }
}

} // namespace mitcad
