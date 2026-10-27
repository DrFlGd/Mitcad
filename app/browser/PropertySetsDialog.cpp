// SPDX-License-Identifier: MIT
#include "PropertySetsDialog.hpp"

#include <algorithm>
#include <exception>
#include <functional>
#include <utility>

#include <QCheckBox>
#include <QComboBox>
#include <QCompleter>
#include <QDialogButtonBox>
#include <QFont>
#include <QColor>
#include <QFontMetrics>
#include <QIcon>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QHash>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonValue>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMouseEvent>
#include <QPainter>
#include <QPixmap>
#include <QRect>
#include <QPalette>
#include <QPushButton>
#include <QScrollBar>
#include <QStyledItemDelegate>
#include <QSet>
#include <QTimer>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QVBoxLayout>
#include <QVariantMap>
#include <QtLogging>

#include "../framework/Appearances.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Icons.hpp"
#include "../framework/Theme.hpp"
#include "../platform/MacDialogs.hpp"
#include "../print/PrintLibrary.hpp"
#include "../print/SettingField.hpp"
#include "DocumentHost.hpp"

namespace mitcad {
namespace {

constexpr int kLevelRole = Qt::UserRole;        // body, occurrence or component
constexpr int kOwnerRole = Qt::UserRole + 1;    // the level's owner (F2.b0, O3, C1)
constexpr int kSetRole = Qt::UserRole + 2;      // the property's set
constexpr int kNameRole = Qt::UserRole + 3;     // the property's key
constexpr int kPropertyRole = Qt::UserRole + 4; // the property as the query gave it
constexpr int kPrintKeyRole = Qt::UserRole + 5; // a print value's key (mitcad#655)
constexpr int kGlyphsRole = Qt::UserRole + 6;   // a print row's Edit and ↺ (mitcad#655)
constexpr int kProfileRole = Qt::UserRole + 7;  // a profile row's kind (printer, process)

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

// The small buttons after a print row's value (mitcad#655): Edit and ↺,
// by their names in the row's kGlyphsRole, where the delegate paints them
// and takes their clicks.
QVector<QPair<QString, QRect>> glyphPlaces(const QRect& cell, const QFontMetrics& metrics, const QString& shown,
                                          bool icon, const QStringList& glyphs) {
  QVector<QPair<QString, QRect>> places;
  int x = cell.left() + 4 + (icon ? 20 : 0) + metrics.horizontalAdvance(shown) + 10;
  for (const QString& glyph : glyphs) {
    places.append({glyph, QRect(x, cell.center().y() - 8, 16, 16)});
    x += 22;
  }
  return places;
}

// A print value's editor is the settings form's field, a profile's a
// drop-down of the choosable items (the factory's); else the standard text
// editor. The field commits itself; the cell's text is the model's after
// the refresh.
class PrintValueDelegate : public QStyledItemDelegate {
public:
  using Factory = std::function<QWidget*(QWidget*, const QModelIndex&)>;
  using Clicked = std::function<void(const QModelIndex&, const QString&)>;
  PrintValueDelegate(QObject* parent, Factory factory, Clicked clicked)
      : QStyledItemDelegate(parent), m_factory(std::move(factory)), m_clicked(std::move(clicked)) {}

  QWidget* createEditor(QWidget* parent, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
    if (QWidget* editor = m_factory(parent, index)) {
      return editor;
    }
    return QStyledItemDelegate::createEditor(parent, option, index);
  }
  void setEditorData(QWidget* editor, const QModelIndex& index) const override {
    if (!editor->property("printField").toBool()) {
      QStyledItemDelegate::setEditorData(editor, index);
    }
  }
  void setModelData(QWidget* editor, QAbstractItemModel* model, const QModelIndex& index) const override {
    if (!editor->property("printField").toBool()) {
      QStyledItemDelegate::setModelData(editor, model, index);
    }
  }
  void paint(QPainter* painter, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
    QStyledItemDelegate::paint(painter, option, index);
    const QStringList glyphs = index.sibling(index.row(), 0).data(kGlyphsRole).toStringList();
    if (glyphs.isEmpty()) {
      return;
    }
    QFont font = option.font;
    if (const QVariant own = index.data(Qt::FontRole); own.isValid()) {
      font = own.value<QFont>();
    }
    const bool icon = !index.data(Qt::DecorationRole).isNull();
    for (const auto& [glyph, rect] : glyphPlaces(option.rect, QFontMetrics(font), index.data().toString(), icon, glyphs)) {
      if (glyph == QStringLiteral("clear")) {
        painter->save();
        painter->setPen(option.palette.color(QPalette::Text));
        painter->drawText(rect, Qt::AlignCenter, QStringLiteral("↺"));
        painter->restore();
      } else {
        themeIcon(QStringLiteral("edit")).paint(painter, rect);
      }
    }
  }
  bool editorEvent(QEvent* event, QAbstractItemModel* model, const QStyleOptionViewItem& option,
                   const QModelIndex& index) override {
    // The left button's only: the view's itemClicked, which takes back the
    // click flag a glyph sets, is the left button's (mitcad#761).
    if (event->type() == QEvent::MouseButtonRelease &&
        static_cast<QMouseEvent*>(event)->button() == Qt::LeftButton) {
      const QStringList glyphs = index.sibling(index.row(), 0).data(kGlyphsRole).toStringList();
      if (!glyphs.isEmpty()) {
        QFont font = option.font;
        if (const QVariant own = index.data(Qt::FontRole); own.isValid()) {
          font = own.value<QFont>();
        }
        const bool icon = !index.data(Qt::DecorationRole).isNull();
        const QPoint at = static_cast<QMouseEvent*>(event)->position().toPoint();
        for (const auto& [glyph, rect] :
             glyphPlaces(option.rect, QFontMetrics(font), index.data().toString(), icon, glyphs)) {
          if (rect.contains(at)) {
            m_clicked(index, glyph);
            return true;
          }
        }
      }
    }
    return QStyledItemDelegate::editorEvent(event, model, option, index);
  }

private:
  Factory m_factory;
  Clicked m_clicked;
};

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
    mac::styleDialogButtons(buttons, nullptr, true);
    mac::styleDialog(this);
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
    auto* buttons = new QDialogButtonBox;
    auto* define = new QPushButton(tr("Define"));
    auto* remove = new QPushButton(tr("Remove"));
    auto* close = new QPushButton(tr("Close"));
    for (QPushButton* button : {define, remove, close}) {
      button->setAutoDefault(false);
    }
#ifdef Q_OS_MACOS
    auto* controls = new QHBoxLayout;
    controls->setSpacing(0);
    controls->addWidget(define);
    controls->addWidget(remove);
    controls->addStretch(1);
    layout->insertLayout(1, controls);
#else
    buttons->addButton(define, QDialogButtonBox::ResetRole);
    buttons->addButton(remove, QDialogButtonBox::ResetRole);
#endif
    mac::styleListButtons(define, remove);
    mac::stylePreview(m_list);
    buttons->addButton(close, QDialogButtonBox::RejectRole);
    layout->addWidget(buttons);
    mac::styleDialogButtons(buttons, close);
    mac::styleDialog(this);
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
  m_computed->setToolTip(tr("Also the values worked out from the design: volume, area, mass, a member's length"));
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
  // The names whole (mitcad#655): the column as wide as they are.
  m_table->header()->setSectionResizeMode(kKey, QHeaderView::ResizeToContents);
  m_table->header()->setSectionResizeMode(kValue, QHeaderView::Stretch);
  m_table->header()->resizeSection(kType, 110);
  m_table->header()->resizeSection(kSource, 130);
  layout->addWidget(m_table, 1);

  m_message = new QLabel;
  m_message->setWordWrap(true);
  setErrorStyleSheet(m_message);
  layout->addWidget(m_message);

  auto* buttons = new QDialogButtonBox;
  m_add = new QPushButton(themeIcon(QStringLiteral("add")), tr("Add…"));
  m_add->setToolTip(tr("Add a property to one of the levels"));
  m_remove = new QPushButton(themeIcon(QStringLiteral("delete")), tr("Remove"));
  m_types = new QPushButton(tr("Types…"));
  m_types->setToolTip(tr("The design's own property types: allowed values, unit, description"));
  m_close = new QPushButton(tr("Close"));
  for (QPushButton* button : {m_add, m_remove, m_types, m_close}) {
    button->setAutoDefault(false);
  }
#ifdef Q_OS_MACOS
  auto* controls = new QHBoxLayout;
  controls->setSpacing(0);
  controls->addWidget(m_add);
  controls->addWidget(m_remove);
  controls->addStretch(1);
  layout->insertLayout(2, controls);
#else
  buttons->addButton(m_add, QDialogButtonBox::ResetRole);
  buttons->addButton(m_remove, QDialogButtonBox::ResetRole);
#endif
  mac::styleListButtons(m_add, m_remove);
  mac::styleListTree(m_table);
  buttons->addButton(m_types, QDialogButtonBox::ActionRole);
  buttons->addButton(m_close, QDialogButtonBox::RejectRole);
  layout->addWidget(buttons);
  mac::styleDialogButtons(buttons, m_close);
  mac::styleDialog(this);

  connect(m_close, &QPushButton::clicked, this, &QDialog::accept);
  connect(m_add, &QPushButton::clicked, this, &PropertySetsDialog::addProperty);
  connect(m_remove, &QPushButton::clicked, this, &PropertySetsDialog::removeProperty);
  connect(m_types, &QPushButton::clicked, this, &PropertySetsDialog::editTypes);
  connect(m_computed, &QCheckBox::toggled, this, &PropertySetsDialog::refresh);
  connect(m_table, &QTreeWidget::itemDoubleClicked, this, &PropertySetsDialog::edit);
  connect(m_table, &QTreeWidget::itemChanged, this, &PropertySetsDialog::cellEdited);
  connect(m_table, &QTreeWidget::itemSelectionChanged, this, &PropertySetsDialog::updateButtons);
  // A print value opens the settings form's field, a profile a drop-down of
  // the choosable items (mitcad#655): a click on it, or F2. Edit and ↺
  // after a value are the delegate's.
  m_table->setItemDelegateForColumn(
      kValue,
      new PrintValueDelegate(
          m_table,
          [this](QWidget* parent, const QModelIndex& index) -> QWidget* {
            const QModelIndex first = index.sibling(index.row(), kKey);
            const QString set = first.data(kSetRole).toString();
            const QString kind = first.data(kProfileRole).toString();
            if (!kind.isEmpty()) {
              const QString project = m_host.projectFolder();
              const QVector<LibraryItem> items = queryLibraryItems(m_host.model(), kind, project);
              const QString chosen = first.data(kPropertyRole).toMap().value(QStringLiteral("key")).toString();
              auto* combo = new QComboBox(parent);
              combo->setProperty("printField", true);
              if (chosen.isEmpty()) {
                combo->addItem(tr("Slicer default"), QString());
              }
              // By name, no places, as in 3D Print (mitcad#655).
              for (const LibraryItem& item : items) {
                combo->addItem(item.name, item.key());
              }
              combo->setCurrentIndex(std::max(0, combo->findData(chosen)));
              connect(combo, &QComboBox::activated, this, [this, combo, items, set, project, chosen](int at) {
                const QString key = combo->itemData(at).toString();
                const LibraryItem* item = findItem(items, key);
                if (key == chosen || item == nullptr) {
                  return;
                }
                QJsonObject fields;
                nameItem(fields, *item, project, QStringLiteral("value"));
                setPrint(set, QStringLiteral("profile"), fields, item->name);
              });
              QTimer::singleShot(0, combo, [combo] { combo->showPopup(); });
              return combo;
            }
            const PrintKey* key = m_keys.find(first.data(kPrintKeyRole).toString());
            if (key == nullptr) {
              return nullptr;
            }
            const QVariantMap cellProperty = first.data(kPropertyRole).toMap();
            // [unit | %] only where the design's slicer takes it (mitcad#655).
            const QString slicer = designDialectOf(m_host.model());
            const bool percent = key->kind != PrintKey::Kind::NumberOrPercent || key->takesPercent(slicer);
            auto* field = new SettingField(*key, parent, percent);
            field->setDialect(slicer); // its range (mitcad#746)
            field->setProperty("printField", true);
            field->setAutoFillBackground(true);
            field->setClearable(false); // ↺ is the row's
            // The value the row shows: the inherited one opens the field.
            field->setValue(QJsonValue::fromVariant(cellProperty.value(QStringLiteral("value"))));
            connect(field, &SettingField::committed, this,
                    [this, set, key](const QString& name, const QJsonValue& value) {
                      const QJsonObject fields =
                          value.isNull() ? QJsonObject() : QJsonObject{{QStringLiteral("value"), value}};
                      setPrint(set, name, fields, value.isNull() ? QString() : printValueText(*key, value));
                    });
            QTimer::singleShot(0, field, [field] { field->startEditing(); });
            return field;
          },
          [this](const QModelIndex& index, const QString& glyph) {
            const QModelIndex first = index.sibling(index.row(), kKey);
            m_glyphClicked = true;
            const QString set = first.data(kSetRole).toString();
            const QString name = first.data(kNameRole).toString();
            qDebug().noquote() << QStringLiteral("Property sets print %1/%2 %3").arg(set, name, glyph);
            if (glyph == QStringLiteral("clear")) {
              setPrint(set, name, QJsonObject(), QString());
            } else if (name == QStringLiteral("own_settings")) {
              // Own settings: this level's own process values in the
              // settings form (mitcad#655); Process's pencil opens the
              // process.
              openLevelOwnSettings(first.data(kOwnerRole).toString());
            } else if (!first.data(kProfileRole).toString().isEmpty()) {
              if (openProfile) {
                openProfile(first.data(kProfileRole).toString(),
                            first.data(kPropertyRole).toMap().value(QStringLiteral("key")).toString());
              }
            } else {
              openPrintMaterial();
            }
          }));
  connect(m_table, &QTreeWidget::itemClicked, this, [this](QTreeWidgetItem* item, int column) {
    if (std::exchange(m_glyphClicked, false) || item == nullptr ||
        item->data(kKey, kLevelRole).toString() != QStringLiteral("print") || column != kValue) {
      return;
    }
    if (item->data(kKey, kNameRole).toString() == QStringLiteral("material")) {
      openPrintMaterial();
    } else if (item->data(kKey, kNameRole).toString() == QStringLiteral("own_settings")) {
      openLevelOwnSettings(item->data(kKey, kOwnerRole).toString());
    } else {
      editPrint(item);
    }
  });
  // The cells' places again when the table scrolls (the print settings
  // below the levels, mitcad#640).
  connect(m_table->verticalScrollBar(), &QScrollBar::valueChanged, this,
          [this] { QTimer::singleShot(0, this, &PropertySetsDialog::logLayout); });
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
    // A body by its name alone, not "Body Body1" (mitcad#655).
    const QString title = kind == QStringLiteral("body")         ? text(level, "name")
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
      // The `Print` sets are what Print settings edits: not
      // listed twice (mitcad#655).
      if (setName == QStringLiteral("Print Printer") || setName == QStringLiteral("Print Process") ||
          setName == QStringLiteral("Print Filament") || setName.startsWith(QStringLiteral("Print Slicer"))) {
        continue;
      }
      auto* group = new QTreeWidgetItem(top, {setName});
      group->setData(kKey, kLevelRole, kind);
      group->setData(kKey, kSetRole, setName);
      group->setFirstColumnSpanned(true);
      for (const QJsonValue& propertyValue : set.value(QStringLiteral("properties")).toArray()) {
        const QJsonObject property = propertyValue.toObject();
        // The material is shown once, in Print settings (mitcad#655): not
        // again among the computed values.
        if (property.value(QStringLiteral("read_only")).toBool() && text(property, "key") == QStringLiteral("Material")) {
          continue;
        }
        auto* row = new QTreeWidgetItem(group);
        // A print setting shows its plain name, its key in the tooltip
        // (mitcad#640).
        const QString plain = text(property, "name");
        row->setText(kKey, plain.isEmpty() ? text(property, "key") : plain);
        if (!plain.isEmpty()) {
          row->setToolTip(kKey, text(property, "key"));
        }
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
  addPrintSettings(levels, folded, current, rows);
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

void PropertySetsDialog::addPrintSettings(const QJsonArray& levels, const QSet<QString>& folded,
                                          const QString& current, QStringList& rows) {
  m_printWidgets.clear();
  if (levels.isEmpty()) {
    return;
  }
  QJsonObject query{{QStringLiteral("query"), QStringLiteral("print_settings")}};
  if (!m_target.body.isEmpty()) {
    query.insert(QStringLiteral("body"), m_target.body);
    if (!m_target.occurrence.isEmpty()) {
      query.insert(QStringLiteral("occurrence"), m_target.occurrence);
    }
  } else if (!m_target.occurrence.isEmpty()) {
    query.insert(QStringLiteral("occurrence"), m_target.occurrence);
  } else {
    query.insert(QStringLiteral("component"), m_target.component);
  }
  const QString project = m_host.projectFolder();
  QJsonObject print;
  try {
    print = m_host.model().queryObject(query);
    if (m_keys.isEmpty()) {
      m_keys = PrintKeyTable(m_host.model().queryObject({{QStringLiteral("query"), QStringLiteral("print_keys")}}));
    }
  } catch (const std::exception&) {
    // Shown without what could not be asked.
  }
  if (print.isEmpty()) {
    return;
  }
  // Edits set the most specific level's own values.
  const QJsonObject level = levels.first().toObject();
  const QString owner = text(level, "owner");
  // A plate has one printer (mitcad#655): it is chosen and set at the root
  // only; below it the printer is shown read only, the process can still
  // be overridden.
  const bool rootLevel = owner == QStringLiteral("C0");
  auto* top = new QTreeWidgetItem(m_table, {tr("Print settings")});
  top->setData(kKey, kLevelRole, QStringLiteral("print"));
  top->setFirstColumnSpanned(true);
  QFont bold = top->font(kKey);
  bold.setBold(true);
  top->setFont(kKey, bold);
  // Where a value comes from (mitcad#655): the profile's, the material's or
  // the owner's name in words, the icon of its kind beside it.
  const auto sourceOf = [](const QJsonObject& value) {
    const QString from = text(value, "from");
    if (from == QStringLiteral("profile")) {
      return std::make_pair(QStringLiteral("settings"), text(value, "profile_name"));
    }
    if (from == QStringLiteral("material")) {
      return std::make_pair(QStringLiteral("material"), text(value, "material_name"));
    }
    const QString level = text(value, "level");
    return std::make_pair(level == QStringLiteral("body")         ? QStringLiteral("body")
                          : level == QStringLiteral("occurrence") ? QStringLiteral("insert-component")
                                                                  : QStringLiteral("component"),
                          text(value, "owner_name"));
  };
  // The log's source text, as before.
  const auto fromText = [](const QJsonObject& value) {
    const QString from = text(value, "from");
    if (from == QStringLiteral("profile")) {
      return tr("profile %1 (%2)").arg(text(value, "profile_name"), text(value, "owner_name"));
    }
    if (from == QStringLiteral("material")) {
      return tr("material %1").arg(text(value, "material_name"));
    }
    return tr("%1 %2").arg(text(value, "level"), text(value, "owner_name"));
  };
  // A value of the level shown: bold on a tinted row (mitcad#655: more
  // than the weight of the font tells it), with ↺ after it.
  QColor ownTint = palette().color(QPalette::Highlight);
  ownTint.setAlpha(40);
  const auto own = [this, ownTint](QTreeWidgetItem* row) {
    QFont font = row->font(kKey);
    font.setBold(true);
    row->setFont(kKey, font);
    row->setFont(kValue, font);
    for (int column = 0; column < m_table->columnCount(); ++column) {
      row->setBackground(column, ownTint);
    }
  };
  const std::pair<const char*, QString> sets[] = {{"printer", QStringLiteral("Print Printer")},
                                                  {"process", QStringLiteral("Print Process")},
                                                  {"filament", QStringLiteral("Print Filament")}};
  // A material's appearance: what its unset filament colour shows.
  QString lookName;
  QColor lookColor;
  for (const auto& entry : sets) {
    const QString kind = QString::fromLatin1(entry.first);
    const QString& setName = entry.second;
    const QJsonObject block = print.value(kind).toObject();
    const bool filament = kind == QStringLiteral("filament");
    const bool readOnly = filament || (kind == QStringLiteral("printer") && !rootLevel);
    // One word for the material everywhere (mitcad#655).
    auto* group = new QTreeWidgetItem(top, {kind == QStringLiteral("printer")   ? tr("Printer")
                                            : kind == QStringLiteral("process") ? tr("Process")
                                                                                : tr("Material")});
    group->setData(kKey, kLevelRole, QStringLiteral("print"));
    group->setData(kKey, kSetRole, setName);
    group->setData(kKey, kOwnerRole, owner);
    group->setData(kKey, kPropertyRole, QVariantMap{{QStringLiteral("inherited"), true},
                                                    {QStringLiteral("read_only"), true}});
    if (filament) {
      // The material, plain; a click on it or Edit opens Materials.
      const QJsonObject material = block.value(QStringLiteral("material")).toObject();
      group->setText(kValue, text(material, "name"));
      // Its name once, in the value (mitcad#655): the Source column does
      // not repeat it. Its appearance names an unset filament colour.
      try {
        for (const QJsonValue& value : m_host.model().queryArray(QStringLiteral("materials"))) {
          if (text(value.toObject(), "id") == text(material, "id")) {
            const QVector<Appearance> looks = appearancesOf(m_host.model().queryArray(QStringLiteral("appearances")));
            if (const Appearance* look = findAppearance(looks, text(value.toObject(), "appearance"))) {
              lookName = look->name;
              lookColor = look->displayColor;
            }
          }
        }
      } catch (const std::exception&) {
        // Without the appearance's name.
      }
      group->setData(kKey, kNameRole, QStringLiteral("material"));
      group->setData(kKey, kGlyphsRole, QStringList{QStringLiteral("edit")});
      rows << QStringLiteral("print %1/material = %2").arg(setName, text(material, "name"));
    } else {
      // The profile chosen, plain; a click opens the drop-down of the
      // choosable ones, Edit opens Printers and Processes, ↺ takes away a
      // choice of this level.
      const QJsonObject profile = block.value(QStringLiteral("profile")).toObject();
      const QVector<LibraryItem> items = queryLibraryItems(m_host.model(), kind, project);
      const LibraryItem* chosen = findDesignCopy(items, text(profile, "id"));
      group->setText(kValue, chosen != nullptr ? chosen->name : tr("Slicer default"));
      group->setData(kKey, kNameRole, QStringLiteral("profile"));
      group->setData(kKey, kProfileRole, kind);
      group->setData(kKey, kPropertyRole,
                     QVariantMap{{QStringLiteral("inherited"), true},
                                 {QStringLiteral("read_only"), true},
                                 {QStringLiteral("key"), chosen != nullptr ? chosen->key() : QString()}});
      QStringList glyphs;
      if (!readOnly) {
        group->setFlags(group->flags() | Qt::ItemIsEditable);
        glyphs << QStringLiteral("edit");
      }
      if (!readOnly && text(profile, "owner") == owner && !profile.isEmpty()) {
        own(group);
        glyphs << QStringLiteral("clear");
      }
      group->setData(kKey, kGlyphsRole, glyphs);
      rows << QStringLiteral("print %1/profile = %2 (%3)")
                  .arg(setName, text(profile, "name"),
                       profile.isEmpty() ? tr("none chosen") : tr("chosen on %1").arg(text(profile, "owner_name")));
      if (readOnly) {
        // Below the root the design's printer in one line, no values
        // (mitcad#655).
        group->setExpanded(false);
        continue;
      }
    }
    // The values under category › sub-group headings, in the key table's
    // order: a summary of what the level prints with (mitcad#655).
    QHash<QString, QJsonObject> byKey;
    for (const QJsonValue& value : block.value(QStringLiteral("values")).toArray()) {
      byKey.insert(text(value.toObject(), "key"), value.toObject());
    }
    const QVector<const PrintKey*> keys = m_keys.keys(kind);
    QString shownGroup;
    QStringList ownNames; // the own process values below the root, by plain name
    // Below the root (mitcad#655): the process lists its own values only;
    // what a body or a component sets of its own goes under a row Own
    // settings of its own, the owner as Source. The root has none: its
    // values are the process.
    QTreeWidgetItem* ownRow =
        kind == QStringLiteral("process") && !rootLevel ? new QTreeWidgetItem(top, {tr("Own settings")}) : nullptr;
    for (const PrintCategory& category : m_keys.layout(kind)) {
      for (const PrintGroup& sub : category.groups) {
        for (const PrintKey* key : keys) {
          // A value per slicer (a preset's name) is not shown, as in the
          // form, nor the filament's density: the material's (mitcad#655).
          const bool hidden = key->perDialect || key->key == QStringLiteral("filament_density");
          if (hidden || m_keys.categoryOf(*key) != category.id || m_keys.groupOf(*key) != sub.id ||
              (!byKey.contains(key->key) && key->key != m_addedKey)) {
            continue;
          }
          const QJsonObject value = byKey.value(key->key);
          const bool ownValue = ownRow != nullptr && text(value, "from") == QStringLiteral("property") &&
                                text(value, "owner") != QStringLiteral("C0");
          if (!ownValue && shownGroup != category.id + QLatin1Char('/') + sub.id) {
            // One heading line per category › group (mitcad#655), not a
            // branch: the values follow it.
            shownGroup = category.id + QLatin1Char('/') + sub.id;
            const QString title = category.title == sub.title ? category.title
                                                              : QStringLiteral("%1 › %2").arg(category.title, sub.title);
            auto* heading = new QTreeWidgetItem(group, {title});
            heading->setFirstColumnSpanned(true);
            heading->setFlags(Qt::ItemIsEnabled);
            // A heading, not a value (mitcad#655): small and bold on a band
            // of the row's whole width.
            QFont headingFont = heading->font(kKey);
            headingFont.setBold(true);
            if (headingFont.pointSizeF() > 0.0) {
              headingFont.setPointSizeF(headingFont.pointSizeF() * 0.9);
            }
            heading->setFont(kKey, headingFont);
            for (int column = 0; column < m_table->columnCount(); ++column) {
              heading->setBackground(column, palette().color(QPalette::AlternateBase));
            }
          }
          const QJsonValue raw = value.value(QStringLiteral("value"));
          const QString shown = raw.isUndefined() ? tr("Slicer default") : printValueText(*key, raw);
          auto* row = new QTreeWidgetItem(ownValue ? ownRow : group);
          row->setText(kKey, key->name);
          if (key->kind == PrintKey::Kind::Color) {
            // A colour as a swatch with a border and its name, never its
            // code (mitcad#655): unset, the appearance's, which is exported.
            const QColor color = raw.isUndefined() ? lookColor : QColor(raw.toString());
            QPixmap swatch(18, 12);
            swatch.fill(Qt::transparent);
            {
              QPainter painter(&swatch);
              painter.setBrush(color.isValid() ? QBrush(color) : QBrush(Qt::NoBrush));
              painter.setPen(QColor(140, 140, 140));
              painter.drawRect(0, 0, 17, 11);
            }
            row->setIcon(kValue, QIcon(swatch));
            const bool look = !lookName.isEmpty() && (raw.isUndefined() || color == lookColor);
            row->setText(kValue, look ? lookName : (raw.isUndefined() ? shown : QString()));
          } else {
            row->setText(kValue, shown);
          }
          // A source only where it differs from the group's (mitcad#655): an
          // owner that sets it, this level too ("Body1", on the tinted row);
          // the profile's and the material's are the group's.
          const QString from = text(value, "from");
          if (!value.isEmpty() && from == QStringLiteral("property")) {
            const auto [icon, name] = sourceOf(value);
            row->setText(kSource, name);
            row->setIcon(kSource, themeIcon(icon));
            if (ownValue && !ownNames.contains(key->name)) {
              ownNames << key->name;
            }
          }
          if (value.isEmpty()) {
            // Unset: the placeholder's grey and italic, as the editors' Slicer
            // default; a computed value elsewhere is grey upright.
            row->setForeground(kValue, palette().color(QPalette::PlaceholderText));
            QFont unset = row->font(kValue);
            unset.setItalic(true);
            row->setFont(kValue, unset);
          }
          row->setData(kKey, kLevelRole, QStringLiteral("print"));
          row->setData(kKey, kOwnerRole, owner);
          row->setData(kKey, kSetRole, setName);
          row->setData(kKey, kNameRole, key->key);
          row->setData(kKey, kPrintKeyRole, key->key);
          QVariantMap cellProperty{{QStringLiteral("read_only"), true},
                                   {QStringLiteral("inherited"), true},
                                   {QStringLiteral("text"), shown},
                                   {QStringLiteral("value"), raw.toVariant()}};
          row->setData(kKey, kPropertyRole, cellProperty);
          // A summary (mitcad#655): the values are edited in the settings
          // form, Edit… on Process (the level's own) or Materials; this
          // level's own values stand out.
          if (!filament && text(value, "from") == QStringLiteral("property") && text(value, "owner") == owner) {
            own(row);
          }
          if (rowName(row) == current) {
            m_table->setCurrentItem(row);
          }
          if (!value.isEmpty()) {
            rows << QStringLiteral("%1 = %2 (%3)").arg(rowName(row), shown, fromText(value));
          }
        }
      }
    }
    group->setExpanded(!folded.contains(QStringLiteral("print|") + setName));
    if (ownRow != nullptr) {
      // "Own settings: Walls", whose pencil opens the level's own values
      // in the settings form (mitcad#655); the values under it.
      ownRow->setText(kValue, ownNames.isEmpty()     ? tr("None")
                              : ownNames.size() > 2 ? tr("%1 settings").arg(ownNames.size())
                                                    : ownNames.join(QStringLiteral(", ")));
      if (ownNames.isEmpty()) {
        ownRow->setForeground(kValue, palette().color(QPalette::PlaceholderText));
      }
      ownRow->setData(kKey, kLevelRole, QStringLiteral("print"));
      ownRow->setData(kKey, kSetRole, setName);
      ownRow->setData(kKey, kOwnerRole, owner);
      ownRow->setData(kKey, kNameRole, QStringLiteral("own_settings"));
      ownRow->setData(kKey, kPropertyRole, QVariantMap{{QStringLiteral("inherited"), true},
                                                       {QStringLiteral("read_only"), true}});
      ownRow->setData(kKey, kGlyphsRole, QStringList{QStringLiteral("edit")});
      rows << QStringLiteral("print %1/own_settings = %2").arg(setName, ownRow->text(kValue));
      ownRow->setExpanded(true);
    }
  }
  top->setExpanded(true);
}

void PropertySetsDialog::openLevelOwnSettings(const QString& owner) {
  if (!openOwnSettings) {
    return;
  }
  const QJsonObject level = m_answer.value(QStringLiteral("levels")).toArray().first().toObject();
  openOwnSettings(owner, text(level, "name"));
}

void PropertySetsDialog::openPrintMaterial() {
  if (!openMaterial) {
    return;
  }
  if (!m_target.body.isEmpty()) {
    openMaterial({m_target.body}, QString());
    return;
  }
  // A component, or the component of the occurrence shown.
  for (const QJsonValue& value : m_answer.value(QStringLiteral("levels")).toArray()) {
    if (text(value.toObject(), "level") == QStringLiteral("component")) {
      openMaterial({}, text(value.toObject(), "owner"));
      return;
    }
  }
}

void PropertySetsDialog::editPrint(QTreeWidgetItem* item) {
  if (item == nullptr ||
      (item->data(kKey, kPrintKeyRole).toString().isEmpty() && item->data(kKey, kProfileRole).toString().isEmpty()) ||
      !(item->flags() & Qt::ItemIsEditable)) {
    return;
  }
  m_table->editItem(item, kValue);
  qDebug().noquote() << QStringLiteral("Editing property %1").arg(rowName(item));
}

void PropertySetsDialog::setPrint(const QString& set, const QString& key, const QJsonObject& fields,
                                  const QString& shown) {
  const QString owner =
      text(m_answer.value(QStringLiteral("levels")).toArray().first().toObject(), "owner");
  const QString name = QStringLiteral("print %1/%2").arg(set, key);
  // The model command rebuilds the table and its editors: later.
  QTimer::singleShot(0, this, [this, set, key, fields, shown, owner, name] {
    QJsonObject command{{QStringLiteral("owner"), owner},
                        {QStringLiteral("set"), set},
                        {QStringLiteral("key"), key}};
    const QJsonValue given = fields.value(QStringLiteral("value"));
    const bool clear = given.isUndefined() || given.isNull();
    command.insert(QStringLiteral("cmd"), clear ? QStringLiteral("remove_property") : QStringLiteral("set_property"));
    for (auto it = fields.begin(); it != fields.end() && !clear; ++it) {
      command.insert(it.key(), it.value());
    }
    if (m_host.runModelCommand(command)) {
      m_message->clear();
      if (key == m_addedKey && !clear) {
        m_addedKey.clear();
      }
      qDebug().noquote() << (clear ? QStringLiteral("Property %1 cleared").arg(name)
                                   : QStringLiteral("Property %1 set: %2").arg(name, shown));
    } else {
      // By the plain name, never the raw key (mitcad#655).
      const QString plain = m_keys.plainName(key);
      m_message->setText(QStringLiteral("%1: %2").arg(plain.isEmpty() ? tr("Profile") : plain,
                                                      m_keys.plainText(m_host.lastError())));
      qDebug().noquote() << QStringLiteral("Property %1: %2 refused: %3").arg(name, shown, m_host.lastError());
    }
    refresh();
  });
}

void PropertySetsDialog::edit(QTreeWidgetItem* item, int /*column*/) {
  if (item != nullptr && item->data(kKey, kLevelRole).toString() == QStringLiteral("print")) {
    editPrint(item); // the settings form's field (mitcad#655)
    return;
  }
  if (item == nullptr || item->data(kKey, kNameRole).toString().isEmpty()) {
    return;
  }
  const QVariantMap property = item->data(kKey, kPropertyRole).toMap();
  if (property.value(QStringLiteral("read_only")).toBool()) {
    m_message->setText(tr("%1 is worked out from the design; it cannot be set.").arg(item->text(kKey)));
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
  if (m_building || column != kValue || item->data(kKey, kNameRole).toString().isEmpty() ||
      !item->data(kKey, kPrintKeyRole).toString().isEmpty()) {
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
    // Edit and ↺ after a print row's value (mitcad#655), where the delegate
    // paints them.
    const QStringList glyphs = item->data(kKey, kGlyphsRole).toStringList();
    if (!glyphs.isEmpty()) {
      const QFont font = item->font(kValue);
      for (const auto& [glyph, place] :
           glyphPlaces(cell, QFontMetrics(font), item->text(kValue), !item->icon(kValue).isNull(), glyphs)) {
        lines << QStringLiteral("Property sets %1 %2 at %3")
                     .arg(rowName(item), glyph, at(m_table->viewport(), place.center()));
      }
    }
  }
  // The print block's boxes and buttons (mitcad#655).
  for (const auto& [name, widget] : std::as_const(m_printWidgets)) {
    if (widget && widget->isVisible() && m_table->viewport()->rect().contains(
                                             widget->mapTo(m_table->viewport(), widget->rect().center()))) {
      lines << QStringLiteral("Property sets %1 at %2").arg(name, at(widget, widget->rect().center()));
    }
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
