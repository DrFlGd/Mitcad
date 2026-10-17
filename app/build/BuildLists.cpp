// SPDX-License-Identifier: MIT
#include "BuildLists.hpp"

#include <algorithm>
#include <cmath>
#include <exception>
#include <limits>
#include <utility>

#include <QApplication>
#include <QClipboard>
#include <QDate>
#include <QDialogButtonBox>
#include <QDir>
#include <QDoubleSpinBox>
#include <QFileInfo>
#include <QFont>
#include <QFontMetrics>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonArray>
#include <QLabel>
#include <QPageLayout>
#include <QPageSize>
#include <QPainter>
#include <QPdfWriter>
#include <QPushButton>
#include <QTabWidget>
#include <QTableWidget>
#include <QVBoxLayout>
#include <QtLogging>

#include "../framework/Command.hpp"
#include "../framework/Dialogs.hpp"
#include "../framework/Json.hpp"
#include "BuildModel.hpp"

namespace mitcad::build {
namespace {

constexpr double kPi = 3.14159265358979323846;
constexpr double kNaN = std::numeric_limits<double>::quiet_NaN();

QString text(const QJsonObject& object, const char* key) { return object.value(QLatin1String(key)).toString(); }

QString millimetresText(double value) {
  if (!std::isfinite(value)) {
    return QString();
  }
  return QString::number(std::round(value * 10.0) / 10.0, 'f', 1);
}

// A cut's angle (radians, as the replies give angles) in degrees: "45 deg",
// or "0".
QString angleText(const QJsonValue& angle) {
  const double degrees = angle.toDouble() * 180.0 / kPi;
  if (!angle.isDouble() || !std::isfinite(degrees) || std::abs(degrees) < 1e-6) {
    return QStringLiteral("0");
  }
  return QStringLiteral("%1 deg").arg(QString::number(std::round(degrees * 10.0) / 10.0, 'g', 6));
}

// An end of a cut part: "square", or "45 deg (x)".
QString endText(const QJsonValue& value) {
  const QJsonObject end = value.toObject();
  const QString shown = angleText(end.value(QStringLiteral("angle")));
  if (shown == QStringLiteral("0")) {
    return QObject::tr("square");
  }
  const QString tilt = text(end, "tilt");
  return tilt.isEmpty() ? shown : QStringLiteral("%1 (%2)").arg(shown, tilt);
}

// The holes machined into a part in millimetres: "D6.8 x 15 M8 at end_a".
// The cut list's are flat, {at, diameter, depth, thread, connector};
// build_edit's {end, hole: {...}}.
QString machiningText(const QJsonValue& value) {
  QStringList holes;
  for (const QJsonValue& entry : value.toArray()) {
    const QJsonObject item = entry.toObject();
    const QJsonObject hole = item.value(QStringLiteral("hole")).isObject() ? item.value(QStringLiteral("hole")).toObject() : item;
    QString shown = QStringLiteral("D%1").arg(millimetresText(hole.value(QStringLiteral("diameter")).toDouble(kNaN)));
    const double depth = hole.value(QStringLiteral("depth")).toDouble(kNaN);
    if (std::isfinite(depth)) {
      shown += QStringLiteral(" x %1").arg(millimetresText(depth));
    }
    if (!text(hole, "thread").isEmpty()) {
      shown += QLatin1Char(' ') + text(hole, "thread");
    }
    if (!text(item, "end").isEmpty()) {
      shown += QObject::tr(" at end %1").arg(text(item, "end"));
    } else if (!text(item, "at").isEmpty()) {
      shown += QObject::tr(" at %1").arg(text(item, "at"));
    }
    holes << shown;
  }
  return holes.join(QStringLiteral("; "));
}

// A mass in kilograms.
QString massText(const QJsonValue& mass) {
  return mass.isDouble() ? QString::number(mass.toDouble(), 'f', 3) : QString();
}

QString countText(const QJsonValue& value) {
  if (value.isDouble()) {
    return QString::number(value.toInt());
  }
  if (value.isArray()) {
    return QString::number(value.toArray().size());
  }
  return value.toString();
}

QString csvField(const QString& cell) {
  if (!cell.contains(QLatin1Char(',')) && !cell.contains(QLatin1Char('"')) && !cell.contains(QLatin1Char('\n'))) {
    return cell;
  }
  return QStringLiteral("\"%1\"").arg(QString(cell).replace(QLatin1Char('"'), QStringLiteral("\"\"")));
}

void fill(QTableWidget* table, const ListTable& list) {
  table->clear();
  table->setColumnCount(static_cast<int>(list.columns.size()));
  table->setHorizontalHeaderLabels(list.columns);
  table->setRowCount(static_cast<int>(list.rows.size()));
  for (int r = 0; r < list.rows.size(); ++r) {
    const QStringList& cells = list.rows[r];
    for (int c = 0; c < cells.size() && c < list.columns.size(); ++c) {
      table->setItem(r, c, new QTableWidgetItem(cells[c]));
    }
  }
  table->resizeColumnsToContents();
}

QTableWidget* newTable(const QString& name) {
  auto* table = new QTableWidget;
  table->setObjectName(name);
  table->setEditTriggers(QAbstractItemView::NoEditTriggers);
  table->setSelectionBehavior(QAbstractItemView::SelectRows);
  table->horizontalHeader()->setStretchLastSection(true);
  table->verticalHeader()->setVisible(false);
  return table;
}

} // namespace

QString csvOf(const QVector<ListTable>& tables) {
  QStringList lines;
  for (const ListTable& table : tables) {
    if (!lines.isEmpty()) {
      lines << QString();
    }
    if (tables.size() > 1 && !table.heading.isEmpty()) {
      lines << csvField(table.heading);
    }
    QStringList header;
    for (const QString& column : table.columns) {
      header << csvField(column);
    }
    lines << header.join(QLatin1Char(','));
    for (const QStringList& row : table.rows) {
      QStringList cells;
      for (const QString& cell : row) {
        cells << csvField(cell);
      }
      lines << cells.join(QLatin1Char(','));
    }
  }
  return lines.join(QLatin1Char('\n')) + QLatin1Char('\n');
}

QVector<ListTable> cutListTables(const QJsonObject& answer) {
  ListTable rows{QObject::tr("Cut list"),
                 {QObject::tr("Quantity"), QObject::tr("Part"), QObject::tr("Length (mm)"), QObject::tr("End A"),
                  QObject::tr("End B"), QObject::tr("Machining"), QObject::tr("Own features"), QObject::tr("Supplier"),
                  QObject::tr("Part number"), QObject::tr("Mass each (kg)")},
                 {}};
  for (const QJsonValue& value : answer.value(QStringLiteral("rows")).toArray()) {
    const QJsonObject row = value.toObject();
    const QJsonObject ends = row.value(QStringLiteral("ends")).toObject();
    rows.rows.append({countText(row.value(QStringLiteral("quantity"))), text(row, "designation"),
                      millimetresText(lengthOf(row.value(QStringLiteral("length")))),
                      endText(ends.value(QStringLiteral("a"))), endText(ends.value(QStringLiteral("b"))),
                      machiningText(row.value(QStringLiteral("machining"))),
                      countText(row.value(QStringLiteral("features"))), text(row, "supplier"),
                      text(row, "part_number"), massText(row.value(QStringLiteral("mass")))});
  }
  ListTable stock{QObject::tr("Stock"),
                  {QObject::tr("Part"), QObject::tr("Stock length (mm)"),
                   QObject::tr("Pieces (mm, in cutting order; | between stock pieces)"), QObject::tr("Stock pieces"),
                   QObject::tr("Offcut (mm)"), QObject::tr("Too long for the stock (mm)")},
                  {}};
  for (const QJsonValue& value : answer.value(QStringLiteral("stock")).toArray()) {
    // {source, designation, stock_length, pieces, bars, needed, offcut, too_long}
    const QJsonObject entry = value.toObject();
    const QJsonArray pieces = entry.value(QStringLiteral("pieces")).toArray();
    // `bars`: where each stock piece's cuts end in `pieces`.
    QList<int> ends;
    for (const QJsonValue& end : entry.value(QStringLiteral("bars")).toArray()) {
      ends << end.toInt();
    }
    QStringList bars;
    QStringList bar;
    for (int i = 0; i < pieces.size(); ++i) {
      bar << millimetresText(lengthOf(pieces[i]));
      if (ends.contains(i + 1)) {
        bars << bar.join(QStringLiteral(", "));
        bar.clear();
      }
    }
    if (!bar.isEmpty()) {
      bars << bar.join(QStringLiteral(", "));
    }
    const QJsonValue tooLong = entry.value(QStringLiteral("too_long"));
    QStringList longer;
    for (const QJsonValue& piece : tooLong.toArray()) {
      longer << millimetresText(lengthOf(piece));
    }
    if (tooLong.isDouble() && tooLong.toDouble() > 0.0) {
      longer << QString::number(tooLong.toInt());
    } else if (tooLong.isBool() && tooLong.toBool()) {
      longer << QObject::tr("yes");
    }
    const QString part = text(entry, "designation").isEmpty() ? text(entry, "source") : text(entry, "designation");
    stock.rows.append({part, millimetresText(lengthOf(entry.value(QStringLiteral("stock_length")))),
                       bars.join(QStringLiteral(" | ")), countText(entry.value(QStringLiteral("needed"))),
                       millimetresText(lengthOf(entry.value(QStringLiteral("offcut")))), longer.join(QStringLiteral(", "))});
  }
  ListTable other{QObject::tr("Other parts"),
                  {QObject::tr("Quantity"), QObject::tr("Part"), QObject::tr("Supplier"), QObject::tr("Part number")},
                  {}};
  for (const QJsonValue& value : answer.value(QStringLiteral("other")).toArray()) {
    const QJsonObject row = value.toObject();
    other.rows.append({countText(row.value(QStringLiteral("quantity"))),
                       text(row, "designation").isEmpty() ? text(row, "name") : text(row, "designation"),
                       text(row, "supplier"), text(row, "part_number")});
  }
  return {rows, stock, other};
}

QVector<ListTable> partsListTables(const QJsonObject& answer) {
  ListTable parts{QObject::tr("Parts list"),
                  {QObject::tr("Quantity"), QObject::tr("Part"), QObject::tr("Supplier"), QObject::tr("Part number"),
                   QObject::tr("Library"), QObject::tr("Version"), QObject::tr("Licence")},
                  {}};
  for (const QJsonValue& value : answer.value(QStringLiteral("rows")).toArray()) {
    const QJsonObject row = value.toObject();
    parts.rows.append({countText(row.value(QStringLiteral("quantity"))),
                       text(row, "designation").isEmpty() ? text(row, "name") : text(row, "designation"),
                       text(row, "supplier"), text(row, "part_number"), text(row, "library"), text(row, "version"),
                       text(row, "license")});
  }
  return {parts};
}

bool writeListPdf(const QString& path, const QString& title, const QString& subtitle,
                  const QVector<ListTable>& tables, QString& error) {
  QPdfWriter writer(path);
  writer.setPageSize(QPageSize(QPageSize::A4));
  writer.setPageOrientation(QPageLayout::Landscape);
  writer.setPageMargins(QMarginsF(12, 12, 12, 12), QPageLayout::Millimeter);
  writer.setResolution(300);
  writer.setTitle(title);
  writer.setCreator(QStringLiteral("Mitcad"));
  QPainter painter;
  if (!painter.begin(&writer)) {
    error = QObject::tr("%1 cannot be written").arg(QDir::toNativeSeparators(path));
    return false;
  }
  const int width = writer.width();
  const int height = writer.height();
  QFont heading(QStringLiteral("Helvetica"));
  heading.setPointSizeF(14);
  heading.setBold(true);
  QFont body(QStringLiteral("Helvetica"));
  body.setPointSizeF(8.5);
  QFont bold = body;
  bold.setBold(true);
  int y = 0;
  const auto lineOf = [&painter](const QFont& font) {
    painter.setFont(font);
    return painter.fontMetrics().height();
  };
  const auto room = [&](int needed) {
    if (y + needed > height) {
      writer.newPage();
      y = 0;
    }
  };
  // The title and the design's name with the date.
  int line = lineOf(heading);
  painter.drawText(QRect(0, y, width, line), Qt::AlignLeft | Qt::AlignVCenter, title);
  y += line;
  line = lineOf(body);
  painter.drawText(QRect(0, y, width, line), Qt::AlignLeft | Qt::AlignVCenter, subtitle);
  y += line * 2;
  for (const ListTable& table : tables) {
    if (table.rows.isEmpty()) {
      continue;
    }
    // Columns as wide as their texts need, in the page's width.
    QVector<double> widths(table.columns.size(), 0.0);
    painter.setFont(bold);
    for (int c = 0; c < table.columns.size(); ++c) {
      widths[c] = painter.fontMetrics().horizontalAdvance(table.columns[c]);
    }
    painter.setFont(body);
    for (const QStringList& row : table.rows) {
      for (int c = 0; c < row.size() && c < widths.size(); ++c) {
        widths[c] = std::max(widths[c], static_cast<double>(painter.fontMetrics().horizontalAdvance(row[c])));
      }
    }
    double total = 0.0;
    for (double& w : widths) {
      w += painter.fontMetrics().averageCharWidth() * 2.0;
      total += w;
    }
    const double scale = total > width ? width / total : 1.0;
    const auto drawRow = [&](const QStringList& cells, const QFont& font) {
      const int h = lineOf(font);
      room(h);
      double x = 0.0;
      for (int c = 0; c < cells.size() && c < widths.size(); ++c) {
        const int w = static_cast<int>(widths[c] * scale);
        const QString shown = painter.fontMetrics().elidedText(cells[c], Qt::ElideRight, w - 4);
        painter.drawText(QRect(static_cast<int>(x), y, w, h), Qt::AlignLeft | Qt::AlignVCenter, shown);
        x += widths[c] * scale;
      }
      y += h;
    };
    line = lineOf(bold);
    room(line * 3);
    painter.drawText(QRect(0, y, width, line), Qt::AlignLeft | Qt::AlignVCenter, table.heading);
    y += line;
    drawRow(table.columns, bold);
    painter.drawLine(0, y, width, y);
    for (const QStringList& row : table.rows) {
      drawRow(row, body);
    }
    y += line;
  }
  if (!painter.end()) {
    error = QObject::tr("%1 was not written").arg(QDir::toNativeSeparators(path));
    return false;
  }
  return true;
}

BuildListsDialog::BuildListsDialog(const CommandContext& model, const QString& design, Page page, QWidget* parent)
    : QDialog(parent), m_model(model), m_design(design) {
  setWindowTitle(page == Page::CutList ? tr("Cut List") : tr("Parts List"));
  resize(980, 600);
  auto* layout = new QVBoxLayout(this);
  m_tabs = new QTabWidget;
  layout->addWidget(m_tabs, 1);

  auto* cutPage = new QWidget;
  auto* cut = new QVBoxLayout(cutPage);
  auto* options = new QFormLayout;
  m_kerf = new QDoubleSpinBox;
  m_kerf->setObjectName(QStringLiteral("cutListKerf"));
  m_kerf->setRange(0.0, 20.0);
  m_kerf->setDecimals(1);
  m_kerf->setSingleStep(0.5);
  m_kerf->setSuffix(QStringLiteral(" mm"));
  m_kerf->setValue(3.0);
  m_kerf->setToolTip(tr("What each saw cut takes from the stock, for the stock pieces needed"));
  options->addRow(tr("&Kerf:"), m_kerf);
  cut->addLayout(options);
  const QStringList names{QStringLiteral("cutListRows"), QStringLiteral("cutListStock"), QStringLiteral("cutListOther")};
  const QStringList headings{tr("Cut parts"), tr("Stock"), tr("Other parts")};
  for (int i = 0; i < names.size(); ++i) {
    cut->addWidget(new QLabel(QStringLiteral("<b>%1</b>").arg(headings[i])));
    QTableWidget* table = newTable(names[i]);
    m_cutTables.append(table);
    cut->addWidget(table, i == 0 ? 3 : 1);
  }
  m_tabs->addTab(cutPage, tr("Cut List"));

  auto* partsPage = new QWidget;
  auto* parts = new QVBoxLayout(partsPage);
  m_partsTable = newTable(QStringLiteral("buildPartsList"));
  parts->addWidget(m_partsTable, 1);
  m_tabs->addTab(partsPage, tr("Parts List"));
  m_tabs->setCurrentIndex(page == Page::CutList ? 0 : 1);

  auto* buttons = new QHBoxLayout;
  auto* copy = new QPushButton(tr("Copy as &CSV"));
  copy->setObjectName(QStringLiteral("listCopyCsv"));
  auto* pdf = new QPushButton(tr("Save as &PDF..."));
  pdf->setObjectName(QStringLiteral("listSavePdf"));
  auto* close = new QDialogButtonBox(QDialogButtonBox::Close);
  buttons->addWidget(copy);
  buttons->addWidget(pdf);
  buttons->addStretch(1);
  buttons->addWidget(close);
  layout->addLayout(buttons);
  connect(copy, &QPushButton::clicked, this, &BuildListsDialog::copyCsv);
  connect(pdf, &QPushButton::clicked, this, &BuildListsDialog::savePdf);
  connect(close, &QDialogButtonBox::rejected, this, &QDialog::reject);
  connect(m_kerf, &QDoubleSpinBox::valueChanged, this, [this] { load(); });
  connect(m_tabs, &QTabWidget::currentChanged, this, [this](int index) {
    qInfo().noquote() << QStringLiteral("Build lists tab: %1").arg(m_tabs->tabText(index).remove(QLatin1Char('&')));
  });
  load();
}

void BuildListsDialog::load() {
  QJsonObject cut;
  QJsonObject parts;
  try {
    cut = m_model.queryObject({{QStringLiteral("query"), QStringLiteral("build_cut_list")},
                               {QStringLiteral("kerf"), m_kerf->value()}});
  } catch (const std::exception& error) {
    qWarning().noquote() << QStringLiteral("Cut list not read: %1").arg(errorText(error));
  }
  try {
    parts = m_model.queryObject({{QStringLiteral("query"), QStringLiteral("parts_list")}});
  } catch (const std::exception& error) {
    qWarning().noquote() << QStringLiteral("Parts list not read: %1").arg(errorText(error));
  }
  m_cut = cutListTables(cut);
  m_parts = partsListTables(parts);
  for (int i = 0; i < m_cut.size() && i < m_cutTables.size(); ++i) {
    fill(m_cutTables[i], m_cut[i]);
  }
  fill(m_partsTable, m_parts.first());
  // For UI tests: what the lists hold.
  for (const QStringList& row : std::as_const(m_cut[0].rows)) {
    qInfo().noquote() << QStringLiteral("Cut list row: %1 x %2, %3 mm, A %4, B %5%6")
                             .arg(row[0], row[1], row[2], row[3], row[4],
                                  row[5].isEmpty() ? QString() : QStringLiteral(", machining ") + row[5]);
  }
  for (const QStringList& row : std::as_const(m_cut[1].rows)) {
    qInfo().noquote() << QStringLiteral("Cut list stock: %1, %2 mm stock x %3, pieces %4, offcut %5")
                             .arg(row[0], row[1], row[3], row[2], row[4]);
  }
  qInfo().noquote() << QStringLiteral("Cut list: %1 row(s), %2 stock, %3 other part(s), kerf %4 mm")
                           .arg(m_cut[0].rows.size())
                           .arg(m_cut[1].rows.size())
                           .arg(m_cut[2].rows.size())
                           .arg(m_kerf->value());
  qInfo().noquote() << QStringLiteral("Build parts list: %1 row(s)").arg(m_parts.first().rows.size());
}

QVector<ListTable> BuildListsDialog::shownTables() const { return m_tabs->currentIndex() == 0 ? m_cut : m_parts; }

void BuildListsDialog::copyCsv() {
  const QString csv = csvOf(shownTables());
  QApplication::clipboard()->setText(csv);
  const QStringList lines = csv.split(QLatin1Char('\n'), Qt::SkipEmptyParts);
  for (const QString& line : lines) {
    qInfo().noquote() << QStringLiteral("Build list CSV: %1").arg(line);
  }
  qInfo().noquote() << QStringLiteral("Build list copied as CSV: %1 line(s)").arg(lines.size());
}

void BuildListsDialog::savePdf() {
  const bool cutList = m_tabs->currentIndex() == 0;
  const QString what = cutList ? tr("cut list") : tr("parts list");
  const QString name = m_design.isEmpty() ? tr("Untitled") : m_design;
  const QString path = dialogSaveFileName(this, cutList ? tr("Save Cut List as PDF") : tr("Save Parts List as PDF"),
                                          QStringLiteral("%1 %2.pdf").arg(QFileInfo(name).completeBaseName(), what),
                                          tr("PDF documents (*.pdf)"));
  if (path.isEmpty()) {
    return;
  }
  const QString title = cutList ? tr("Cut list") : tr("Parts list");
  const QString subtitle = tr("%1, %2").arg(name, QDate::currentDate().toString(Qt::ISODate));
  QString error;
  if (!writeListPdf(path, title, subtitle, shownTables(), error)) {
    qWarning().noquote() << QStringLiteral("Build list PDF not written: %1").arg(error);
    sheetWarning(this, tr("Save as PDF"), error);
    return;
  }
  qInfo().noquote() << QStringLiteral("Build list PDF written: %1 (%2 bytes; %3 of %4, %5)")
                           .arg(path)
                           .arg(QFileInfo(path).size())
                           .arg(title, name, QDate::currentDate().toString(Qt::ISODate));
}

} // namespace mitcad::build
