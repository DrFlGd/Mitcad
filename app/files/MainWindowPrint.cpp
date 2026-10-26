// SPDX-License-Identifier: MIT
// 3D Print (mitcad#13, mitcad#234, Print3d.hpp): a panel of the bodies to
// send (the selected ones, or every visible one, then picked in the view),
// the format, the refinement and the slicer. OK writes the bodies for a
// slicer into a folder of their own (as one 3MF file or an STL file per
// body, through the model's `export`) and gives them to it in one start;
// without a slicer, or when it does not start, the folder opens. With 3MF
// the panel shows the design's Printer and Process (mitcad#655: chosen at
// once, each an undo step; Edit… opens the profile editor; one another
// design changed asked about once), and Import from slicer… with its line
// and Details….
#include "../MainWindow.hpp"

#include <algorithm>
#include <cmath>
#include <exception>
#include <memory>
#include <utility>

#include <QAction>
#include <QColor>
#include <QCursor>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QMenu>
#include <QMessageBox>
#include <QPoint>
#include <QProcess>
#include <QUrl>
#include <QtLogging>
#include <QtMath>

#include "../browser/BrowserController.hpp"
#include "../framework/Appearances.hpp"
#include "../framework/TestSync.hpp"
#include "../print/ImportDetailsDialog.hpp"
#include "../print/PrintImportSummary.hpp"
#include "../print/PrintLibrary.hpp"
#include "Print3d.hpp"
#include "mitcad/geometry/import.hpp"

#include "../framework/Dialogs.hpp"

namespace mitcad {

// The last import from a slicer while 3D Print's panel is open: its line
// and its whole report (Details…).
struct PrintImport {
  PrintImportSummary summary;
};

namespace {

// The Printer and Process boxes' last entry: a new one (mitcad#655).
const char* kNewProfile = "__new";

// The bodies go on the print bed by their print bed faces (mitcad#731).
QJsonObject exportCommand(const QString& path, const QString& format, const QStringList& bodies,
                          const QJsonValue& refinement) {
  return {{QStringLiteral("cmd"), QStringLiteral("export")},
          {QStringLiteral("path"), path},
          {QStringLiteral("format"), format},
          {QStringLiteral("bodies"), QJsonArray::fromStringList(bodies)},
          {QStringLiteral("refinement"), refinement},
          {QStringLiteral("on_bed"), true}};
}

// A number of the bed's log line: three decimals, no "-0.000".
QString bedNumber(double value) {
  return QString::number(std::abs(value) < 0.0005 ? 0.0 : value, 'f', 3);
}

// How an export put the bodies on the print bed (mitcad#731), for the log:
// the body and face on the bed, the plate's centre and the transform after
// the design's placements (rows "a b c t"); the bodies kept as assembled;
// or as in the design; then each bed face that cannot be used.
QString bedLog(const QJsonObject& bed) {
  const QJsonArray plate = bed.value(QStringLiteral("plate")).toArray();
  const QString centre = QStringLiteral("%1,%2").arg(bedNumber(plate.at(0).toDouble()),
                                                     bedNumber(plate.at(1).toDouble()));
  QString line;
  if (bed.value(QStringLiteral("body")).isString()) {
    QStringList rows;
    for (const QJsonValue& row : bed.value(QStringLiteral("transform")).toArray()) {
      QStringList numbers;
      for (const QJsonValue& number : row.toArray()) {
        numbers << bedNumber(number.toDouble());
      }
      rows << numbers.join(QLatin1Char(' '));
    }
    line = QStringLiteral("%1 on %2, plate %3, transform %4")
               .arg(bed.value(QStringLiteral("name")).toString(), bed.value(QStringLiteral("face")).toString(),
                    centre, rows.join(QStringLiteral(" | ")));
  } else if (!bed.value(QStringLiteral("kept")).toArray().isEmpty()) {
    QStringList kept;
    for (const QJsonValue& uid : bed.value(QStringLiteral("kept")).toArray()) {
      kept << uid.toString();
    }
    line = QStringLiteral("kept as assembled: %1").arg(kept.join(QStringLiteral(", ")));
  } else {
    line = QStringLiteral("as in the design");
  }
  for (const QJsonValue& value : bed.value(QStringLiteral("problems")).toArray()) {
    const QJsonObject problem = value.toObject();
    line += QStringLiteral("; %1: %2").arg(problem.value(QStringLiteral("name")).toString(),
                                           problem.value(QStringLiteral("reason")).toString());
  }
  return line;
}

// An export's warnings, one per line.
QStringList exportWarnings(const QJsonObject& result) {
  QStringList warnings;
  for (const QJsonValue& warning : result.value(QStringLiteral("warnings")).toArray()) {
    warnings << warning.toString();
  }
  return warnings;
}

// What the panel keeps while it is open, besides its inputs.
struct PrintPanel {
  QVector<Slicer> found; // when the command started
  Slicer saved;          // the settings' slicer
  Slicer other;          // one chosen by hand (Other program...)
  QString previous;      // the Slicer choice before Other program...
  bool selected = false; // the bodies came from the selection
  std::shared_ptr<PrintImport> imported = std::make_shared<PrintImport>();
};

// The bodies the panel prints, each once, in their order (uids).
QStringList printedBodies(const CommandState& state) {
  QStringList bodies;
  for (const SelectionItem& item : state.items(QStringLiteral("bodies"))) {
    if (!bodies.contains(item.owner)) {
      bodies << item.owner;
    }
  }
  return bodies;
}

// Solids and mesh bodies can be printed; sheet and empty bodies cannot.
bool printable(const std::shared_ptr<geometry::Shape>& shape) {
  const geometry::BodyKind kind = shape ? geometry::body_kind(shape->occt()) : geometry::BodyKind::Empty;
  return kind == geometry::BodyKind::Solid || kind == geometry::BodyKind::Mesh;
}

} // namespace

QJsonObject MainWindow::appearanceColors(const QStringList& bodies) const {
  QJsonObject colors;
  const QVector<Appearance> appearances = appearancesOf(queryArray(QStringLiteral("appearances")));
  for (const QJsonValue& value : queryArray(QStringLiteral("bodies"))) {
    const QJsonObject body = value.toObject();
    const QString uid = body.value(QStringLiteral("uid")).toString();
    const Appearance* appearance = findAppearance(appearances, body.value(QStringLiteral("appearance")).toString());
    const QColor color = appearance != nullptr ? appearance->displayColor : QColor();
    if ((bodies.isEmpty() || bodies.contains(uid)) && color.isValid()) {
      colors.insert(uid, QJsonArray{color.redF(), color.greenF(), color.blueF()});
    }
  }
  return colors;
}

QString MainWindow::designPrinterName() const {
  // The root's effective printer (mitcad#640, the `print_settings` query).
  try {
    const QJsonObject settings = queryObject({{QStringLiteral("query"), QStringLiteral("print_settings")}});
    const QJsonObject profile =
        settings.value(QStringLiteral("printer")).toObject().value(QStringLiteral("profile")).toObject();
    const QString name = profile.value(QStringLiteral("name")).toString();
    if (!name.isEmpty()) {
      return name;
    }
  } catch (const std::exception&) {
    // As without a printer.
  }
  return tr("Slicer default");
}

QVector<QStringList> MainWindow::processOwnSettings() const {
  // The owners below the root with process values of their own, from the
  // design's process row (`overrides`), each once with what it sets in the
  // one wording (mitcad#655): "Body1: Walls", "Body1: 3 settings".
  PrintKeyTable keys;
  try {
    keys = PrintKeyTable(queryObject({{QStringLiteral("query"), QStringLiteral("print_keys")}}));
  } catch (const std::exception&) {
    // Counted only.
  }
  QVector<QStringList> owners;
  for (const LibraryItem& item : queryLibraryItems(*this, QStringLiteral("process"), projectFolder())) {
    if (!item.chosen) {
      continue;
    }
    for (const QJsonValue& value : item.row.value(QStringLiteral("overrides")).toArray()) {
      const QJsonObject owner = value.toObject();
      const QString uid = owner.value(QStringLiteral("owner")).toString();
      const QString name = owner.value(QStringLiteral("name")).toString();
      QStringList changed;
      for (const QJsonValue& key : owner.value(QStringLiteral("keys")).toArray()) {
        const QString plain = keys.plainName(key.toString());
        changed << (plain.isEmpty() ? tr("Process") : plain);
      }
      const QString sentence = changed.size() > 2 ? tr("%1: %2 settings").arg(name).arg(changed.size())
                                                  : tr("%1: %2").arg(name, changed.join(QStringLiteral(", ")));
      const bool seen = std::any_of(owners.begin(), owners.end(), [&uid](const QStringList& o) { return o.value(0) == uid; });
      if (!seen) {
        owners.append({uid, name, sentence});
      }
    }
  }
  return owners;
}

QString MainWindow::newDesignProfile(const QString& kind, const QString& /*current*/) {
  // In this design, made the design's choice in the same undo step
  // (mitcad#655): the model names it ("New Printer", "New Printer 2", ...);
  // Save to puts it into a library.
  const QJsonObject create{{QStringLiteral("cmd"), QStringLiteral("create_print_profile")},
                           {QStringLiteral("kind"), kind},
                           {QStringLiteral("choose"), true}};
  QJsonObject result;
  if (!runModelCommand(create, &result)) {
    return QString();
  }
  const QString id = result.value(QStringLiteral("id")).toString();
  const QString key = QStringLiteral("design/") + id;
  const QVector<LibraryItem> items = queryLibraryItems(model(), kind, projectFolder());
  const LibraryItem* made = findDesignCopy(items, id);
  qDebug().noquote() << QStringLiteral("3D Print %1 new: %2 (%3)").arg(kind, made != nullptr ? made->name : id, key);
  return key;
}

QString MainWindow::designDialect() const { return designDialectOf(*this); }

void MainWindow::commandInputsChanged() {
  if (m_mode == Mode::Command && m_session) {
    m_session->refreshInputs();
  }
}

bool MainWindow::canChangePrintSettings() const {
  // 3D Print has no preview: the print windows its Edit… opens change the
  // design while its panel is open (mitcad#655).
  return canChangeModel() || (!windowReadOnly() && m_mode == Mode::Command && m_session &&
                              m_session->def().id == QStringLiteral("make.print3d"));
}

void MainWindow::importPrintSettings(const std::shared_ptr<PrintImport>& target) {
  if (!canChangePrintSettings()) {
    showError(changeBlockedReason());
    return;
  }
  const QString path = dialogOpenFileName(this, tr("Import from Slicer"),
                                          dialogStartFolder(DialogGroup::Import, QString()), slicerFileFilter());
  if (path.isEmpty()) {
    return;
  }
  rememberDialogFolder(DialogGroup::Import, path);
  // Into the design, whose root chooses the printer and process read.
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("import_print_profile")},
                      {QStringLiteral("path"), path},
                      {QStringLiteral("into"), QStringLiteral("design")}};
  if (!projectFolder().isEmpty()) {
    command.insert(QStringLiteral("project"), projectFolder());
  }
  QJsonObject result;
  if (!runModelCommand(command, &result)) {
    return;
  }
  const QString file = QFileInfo(path).fileName();
  // The problems by the plain names of their keys (mitcad#655).
  PrintKeyTable keys;
  try {
    keys = PrintKeyTable(queryObject({{QStringLiteral("query"), QStringLiteral("print_keys")}}));
  } catch (const std::exception&) {
    // Without the names: as reported.
  }
  const PrintImportSummary summary = printImportSummary(file, QStringLiteral("design"), result, &keys);
  target->summary = summary;
  const auto names = [&result](const char* field) {
    QStringList list;
    for (const QJsonValue& value : result.value(QLatin1String(field)).toArray()) {
      list << value.toObject().value(QStringLiteral("name")).toString();
    }
    return list.join(QStringLiteral(", "));
  };
  const QJsonObject report = result.value(QStringLiteral("report")).toObject();
  qInfo().noquote() << QStringLiteral("Imported print settings from %1: printers: %2; processes: %3; materials: %4; "
                                      "mapped %5, kept %6, left out: %7%8")
                           .arg(file, names("printers"), names("processes"), names("materials"))
                           .arg(report.value(QStringLiteral("mapped")).toInt())
                           .arg(report.value(QStringLiteral("kept")).toInt())
                           .arg(summary.leftOut.isEmpty() ? QStringLiteral("nothing")
                                                          : summary.leftOut.join(QStringLiteral("; ")),
                                summary.problems.isEmpty()
                                    ? QString()
                                    : QStringLiteral("; not read: %1").arg(summary.problems.join(QStringLiteral("; "))));
  qInfo().noquote() << QStringLiteral("3D Print import: %1").arg(summary.line);
}

CommandDef MainWindow::printersCommand() {
  CommandDef def;
  def.id = QStringLiteral("make.printers");
  def.name = tr("Edit Printers and Processes…");
  def.icon = QStringLiteral("print-3d");
  def.tooltip = tr("Edits printers and processes");
  def.kind = CommandDef::Kind::Action;
  def.mode = CommandDef::Mode::Model;
  def.group = QStringLiteral("MAKE");
  def.keywords = {QStringLiteral("printer"), QStringLiteral("process"), QStringLiteral("print"),
                  QStringLiteral("slicer"),  QStringLiteral("profile"), QStringLiteral("3mf")};
  def.run = [this] { m_controller->openProfiles(QString(), QString()); };
  return def;
}

CommandDef MainWindow::print3dCommand() {
  CommandDef def;
  def.id = QStringLiteral("make.print3d");
  def.name = tr("3D Print");
  def.icon = QStringLiteral("print-3d");
  def.tooltip = tr("Sends bodies to a slicer"); // short, no rules (mitcad#655)
  def.group = QStringLiteral("MAKE");
  def.pinned = true;
  def.keywords = {QStringLiteral("print"),   QStringLiteral("slicer"),   QStringLiteral("3mf"),
                  QStringLiteral("stl"),     QStringLiteral("3d print"), QStringLiteral("make"),
                  QStringLiteral("printer"), QStringLiteral("process"),  QStringLiteral("profile")};
  def.okLabel = tr("Send");
  const auto panel = std::make_shared<PrintPanel>();
  const auto custom = [](const CommandState& state) {
    return state.choice(QStringLiteral("refinement")) == QStringLiteral("custom");
  };
  // The Printer and Process rows, Import from slicer… and its line: only
  // with 3MF, which carries print settings (mitcad#655).
  const auto threeMf = [](const CommandState& state) {
    return state.choice(QStringLiteral("format")) == QStringLiteral("3mf");
  };
  // A printer or process box (mitcad#655): Slicer default always first (its
  // choice takes the design's away), then the items in a steady order by
  // name, so a choice never moves them; no places. Last a new one.
  const auto choices = [this](const QString& kind) {
    QVector<LibraryItem> items = queryLibraryItems(model(), kind, projectFolder());
    std::stable_sort(items.begin(), items.end(), [](const LibraryItem& a, const LibraryItem& b) {
      const int byName = QString::localeAwareCompare(a.name.toLower(), b.name.toLower());
      return byName != 0 ? byName < 0 : a.key() < b.key();
    });
    QVector<QPair<QString, QString>> options{{QString(), tr("Slicer default")}};
    for (const LibraryItem& item : std::as_const(items)) {
      options.append({item.key(), item.name});
    }
    // Last: a new one, made the design's and opened (mitcad#655).
    options.append({QString::fromLatin1(kNewProfile),
                    kind == QStringLiteral("printer") ? tr("New printer…") : tr("New process…")});
    return options;
  };
  // The design's choice now.
  const auto chosenKey = [this](const QString& kind) {
    for (const LibraryItem& item : queryLibraryItems(model(), kind, projectFolder())) {
      if (item.chosen) {
        return item.key();
      }
    }
    return QString();
  };
  // The design's printer or process saved in a library that another
  // design changed: asked once (mitcad#655), when the panel opens or it
  // is chosen. Use the changes or Keep these, one undo step either way.
  const auto askChanges = [this] {
    if (!canChangePrintSettings()) {
      return;
    }
    for (const QString& kind : {QStringLiteral("printer"), QStringLiteral("process")}) {
      for (const LibraryItem& item : queryLibraryItems(model(), kind, projectFolder())) {
        if (!item.chosen || !item.changed || changeAsked(item)) {
          continue;
        }
        const QJsonObject command = askChangedElsewhere(this, item, QStringLiteral("3D Print"), projectFolder());
        if (runModelCommand(command)) {
          qDebug().noquote() << QStringLiteral("3D Print %1 %2: %3")
                                    .arg(kind, item.name, command.value(QStringLiteral("cmd")).toString());
        }
      }
    }
  };
  // A choice changes the design at once, one undo step ("Use Printer …");
  // Cancel does not take it back.
  const auto choose = [this, chosenKey, askChanges](const QString& kind, CommandState& state) {
    const QString value = state.choice(kind);
    const QString before = chosenKey(kind);
    if (value == before) {
      return;
    }
    if (value == QLatin1String(kNewProfile)) {
      const QString made = newDesignProfile(kind, before);
      state.setChoice(kind, chosenKey(kind));
      if (!made.isEmpty()) {
        m_controller->openProfiles(kind, made);
      }
      return;
    }
    const QString project = projectFolder();
    const QString set =
        kind == QStringLiteral("printer") ? QStringLiteral("Print Printer") : QStringLiteral("Print Process");
    QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("set_property")},
                        {QStringLiteral("owner"), QStringLiteral("C0")},
                        {QStringLiteral("set"), set},
                        {QStringLiteral("key"), QStringLiteral("profile")}};
    if (value.isEmpty()) {
      command.insert(QStringLiteral("cmd"), QStringLiteral("remove_property"));
    } else {
      const QVector<LibraryItem> items = queryLibraryItems(model(), kind, project);
      const LibraryItem* item = findItem(items, value);
      if (item == nullptr) {
        state.setChoice(kind, before);
        return;
      }
      nameItem(command, *item, project, QStringLiteral("value"));
    }
    runModelCommand(command);
    state.setChoice(kind, chosenKey(kind));
    qDebug().noquote() << QStringLiteral("3D Print %1: %2")
                              .arg(kind, state.choice(kind).isEmpty() ? QStringLiteral("slicer default")
                                                                      : state.choice(kind));
    TestSync::singleShot(0, this, askChanges);
  };
  const auto profileRow = [&](const QString& kind, const QString& label) {
    return choiceInput(kind, label, {}, QString())
        .withChoices([choices, kind](const CommandState&) { return choices(kind); })
        // No icons: a drop-down that elides a long name in the middle, never
        // wider than the panel (mitcad#655).
        .withChoiceIcon([](const QString&) { return ChoiceIcon{}; })
        // Edit: Printers and Processes on the chosen one; on Slicer default
        // with none selected, as everywhere (mitcad#655).
        .withButton(tr("Edit"), tr("Edit"),
                    [this, kind](const CommandState& state, const CommandContext&) {
                      const QString key = state.choice(kind);
                      m_controller->openProfiles(kind, key.contains(QLatin1Char('/')) ? key : QString());
                    })
        .withButtonIcon(QStringLiteral("edit"))
        // Nothing to edit on Slicer default (mitcad#655): New printer… /
        // New process… makes one.
        .withButtonEnabled([kind](const CommandState& state) { return state.choice(kind).contains(QLatin1Char('/')); })
        .withOnChange([choose, kind](CommandState& state, const CommandContext&) { choose(kind, state); })
        .withEnabled(threeMf); // disabled with STL, not hidden (mitcad#655)
  };
  // The purge table of the bodies' filament slots (mitcad#705, the model's
  // `purge_volumes` query); empty when the model cannot say.
  const auto purgeVolumes = [this](const QStringList& bodies) {
    QJsonObject query{{QStringLiteral("query"), QStringLiteral("purge_volumes")},
                      {QStringLiteral("bodies"), QJsonArray::fromStringList(bodies)}};
    try {
      return queryObject(query);
    } catch (const std::exception&) {
      return QJsonObject();
    }
  };
  def.inputs = {
      // Sheet and empty bodies cannot be picked, and are not sent.
      selectionInput(QStringLiteral("bodies"), tr("Bodies"), SelectKind::Body, 1, 0)
          .withAccepts([this](const SelectionItem& item) { return printable(bodyShape(item.owner)); }),
      // Short values, no rules in tooltips (mitcad#655). The format is
      // remembered as soon as it is chosen; 3MF the first time.
      choiceInput(QStringLiteral("format"), tr("Format"),
                  {{QStringLiteral("stl"), tr("STL")}, {QStringLiteral("3mf"), tr("3MF")}}, QStringLiteral("3mf"))
          .withOnChange([](CommandState& state, const CommandContext&) {
            PrintSettings settings = PrintSettings::load();
            settings.format = state.choice(QStringLiteral("format"));
            settings.save();
          }),
      // Import from slicer… first (mitcad#655): it fills Printer, Process and
      // Materials; with the import's one short line
      // and Details… until the panel closes.
      noteInput(
          QStringLiteral("import"),
          [panel](const CommandState&) {
            // One button; the import's line is the link to Details (mitcad#655).
            const QString line = panel->imported->summary.line;
            if (line.isEmpty()) {
              return InputNote{QString(), {tr("Import from slicer…")}, {}};
            }
            return InputNote{QString(), {tr("Import from slicer…"), line}, {QString(), linkActionIcon()}};
          },
          [this, panel](int action, const CommandState&, const CommandContext&) {
            if (action == 0) {
              importPrintSettings(panel->imported);
            } else {
              ImportDetailsDialog::showReport(panel->imported->summary, this);
            }
          })
          .withFieldColumn() // the first print row, in the field column (mitcad#655)
          .withEnabled(threeMf),
      // The design's printer and process (mitcad#655): chosen here, edited
      // in the profile editor (Edit…).
      profileRow(QStringLiteral("printer"), tr("Printer")),
      profileRow(QStringLiteral("process"), tr("Process")),
      // What sets its own process values below the root, under the label
      // Own settings (mitcad#655): "Body1: Walls", the pencil at the right.
      noteInput(
          QStringLiteral("process_overrides"),
          [this](const CommandState&) {
            QStringList names;
            for (const QStringList& entry : processOwnSettings()) {
              names << entry.value(2);
            }
            // As the other rows (mitcad#655): the value on the label's line,
            // "Body1: Walls" or "None", the pencil at the right edge.
            return InputNote{names.isEmpty() ? tr("None") : names.join(QStringLiteral("; ")), {tr("Edit")},
                             {QStringLiteral("edit")}};
          },
          [this](int, const CommandState& state, const CommandContext& context) {
            // The pencil: the own settings of the body printed, or of the
            // one chosen in a menu of the bodies printed (mitcad#655).
            QStringList uids;
            for (const SelectionItem& item : state.items(QStringLiteral("bodies"))) {
              if (!uids.contains(item.owner)) {
                uids << item.owner;
              }
            }
            if (uids.isEmpty()) {
              return;
            }
            QString uid = uids.first();
            if (uids.size() > 1) {
              QMenu menu(this);
              QStringList shown;
              for (const QString& body : std::as_const(uids)) {
                menu.addAction(context.bodyName(body))->setData(body);
                shown << context.bodyName(body);
              }
              qDebug().noquote() << QStringLiteral("3D Print own settings menu: %1").arg(shown.join(QStringLiteral(" | ")));
              QAction* chosen = execMenu(&menu, QCursor::pos());
              if (chosen == nullptr) {
                return;
              }
              uid = chosen->data().toString();
            }
            m_controller->openOwnSettings(uid, context.bodyName(uid));
          },
          tr("Own settings"))
          .withEnabled(threeMf),
      // The bodies' materials in a line (mitcad#655): "PLA ×2, PETG"; the
      // pencil opens Materials for those bodies.
      noteInput(
          QStringLiteral("materials"),
          [this, panel](const CommandState& state) {
            QStringList order;
            QHash<QString, int> counts;
            QStringList seen;
            for (const SelectionItem& item : state.items(QStringLiteral("bodies"))) {
              if (seen.contains(item.owner)) {
                continue;
              }
              seen << item.owner;
              QString name;
              try {
                const QJsonObject material =
                    queryObject({{QStringLiteral("query"), QStringLiteral("body_material")},
                                 {QStringLiteral("body"), item.owner}});
                name = material.value(QStringLiteral("name")).toString();
                // A body without a material of its own (mitcad#655): the
                // word Materials gives it, "Default (Steel)".
                if (material.value(QStringLiteral("source")).toString() != QStringLiteral("body")) {
                  name = tr("Default (%1)").arg(name);
                }
              } catch (const std::exception&) {
                continue;
              }
              if (!counts.contains(name)) {
                order << name;
              }
              ++counts[name];
            }
            QStringList parts;
            for (const QString& name : std::as_const(order)) {
              // A count only above one (mitcad#655): "Shop PETG, Mount PLA ×2";
              // each name kept whole when the line wraps (no-break spaces).
              const int count = counts.value(name);
              QString part = count > 1 ? QStringLiteral("%1 ×%2").arg(name).arg(count) : name;
              part.replace(QLatin1Char(' '), QChar(0x00A0));
              parts << part;
            }
            if (parts.isEmpty()) {
              return InputNote();
            }
            // The same pencil as the Printer and Process rows (mitcad#655), and
            // after an import that added materials a plain action to give them
            // to the bodies printed: Assign….
            InputNote note{parts.join(QStringLiteral(", ")), {tr("Edit")}, {QStringLiteral("edit")}};
            const int added = panel->imported->summary.materialsAdded;
            if (added > 0) {
              note.actions << tr("Assign…");
              note.icons << QString();
            }
            return note;
          },
          [this](int action, const CommandState& state, const CommandContext&) {
            // The panel's bodies, those being printed (mitcad#655).
            QStringList bodies;
            for (const SelectionItem& item : state.items(QStringLiteral("bodies"))) {
              if (!bodies.contains(item.owner)) {
                bodies << item.owner;
              }
            }
            // Assign materials… keeps the window open after each Assign, so
            // several new materials go to different bodies.
            m_controller->openMaterial(bodies, QString(), action == 1);
          },
          tr("Materials"))
          .withEnabled(threeMf),
      // The purge volumes between the bodies' filaments (mitcad#705): Computed,
      // or how many pairs are set by hand; the pencil opens the table.
      noteInput(
          QStringLiteral("purge"),
          [purgeVolumes](const CommandState& state) {
            const QJsonObject table = purgeVolumes(printedBodies(state));
            const QJsonArray volumes = table.value(QStringLiteral("volumes")).toArray();
            if (volumes.size() < 2) {
              // One filament: no changes to purge for.
              return InputNote{tr("None"), {}, {}};
            }
            int edited = 0;
            for (const QJsonValue& row : volumes) {
              for (const QJsonValue& cell : row.toArray()) {
                edited += cell.toObject().value(QStringLiteral("edited")).toBool() ? 1 : 0;
              }
            }
            return InputNote{edited == 0 ? tr("Computed") : tr("%n set by hand", nullptr, edited), {tr("Edit")},
                             {QStringLiteral("edit")}};
          },
          [this](int, const CommandState& state, const CommandContext&) {
            m_controller->openPurgeVolumes(printedBodies(state));
          },
          tr("Purge volumes"))
          .withEnabled(threeMf),
      choiceInput(QStringLiteral("refinement"), tr("Refinement"),
                  {{QStringLiteral("low"), tr("Low")},
                   {QStringLiteral("medium"), tr("Medium")},
                   {QStringLiteral("high"), tr("High")},
                   {QStringLiteral("custom"), tr("Custom")}},
                  QStringLiteral("high")),
      valueInput(QStringLiteral("deviation"), tr("Deviation"), ValueKind::Length, QStringLiteral("0.01 mm"))
          .withVisible(custom),
      valueInput(QStringLiteral("angle"), tr("Angle"), ValueKind::Angle, QStringLiteral("8 deg"))
          .withVisible(custom),
      // Where the files are opened (mitcad#655): Folder only, a slicer, or
      // another program; "None" would read as the print settings' unset.
      choiceInput(QStringLiteral("slicer"), tr("Open in"), {}, QString())
          .withChoices([panel](const CommandState&) {
            return slicerChoices(panel->found, panel->saved, panel->other);
          })
          .withOnChange([this, panel](CommandState& state, const CommandContext&) {
            const QString choice = state.choice(QStringLiteral("slicer"));
            if (choice != otherSlicerChoice()) {
              panel->previous = choice;
              return;
            }
            // Another program: an AppImage, a slicer installed elsewhere,
            // an application bundle.
            const QString program =
                dialogOpenFileName(this, tr("Slicer Program"), dialogStartFolder(DialogGroup::Program, QString()));
            if (!program.isEmpty()) {
              rememberDialogFolder(DialogGroup::Program, program);
              panel->other = slicerForProgram(QFileInfo(program).completeBaseName(), program);
              panel->previous = slicerChoice(panel->other);
            }
            state.setChoice(QStringLiteral("slicer"), panel->previous);
          }),
  };
  // The Printer and Process boxes show the design's, also after the
  // profile editor, an import or Update changed them.
  def.modelChanged = [chosenKey](CommandState& state, const CommandContext&) {
    for (const QString& kind : {QStringLiteral("printer"), QStringLiteral("process")}) {
      const QString key = chosenKey(kind);
      if (state.choice(kind) != key) {
        state.setChoice(kind, key);
        qDebug().noquote() << QStringLiteral("3D Print %1: %2")
                                  .arg(kind, key.isEmpty() ? QStringLiteral("slicer default") : key);
      }
    }
  };
  // What the selection stands for (faces, edges and vertices their bodies,
  // as Export takes it; an occurrence the bodies shown in it), else every
  // visible body.
  def.preselect = [this, panel](const Selection& before, CommandState& state, const CommandContext&) {
    const Selection start = printStartBodies(before, queryArray(QStringLiteral("instances")), &panel->selected);
    Selection bodies;
    QStringList leftOut;
    QStringList seen;
    for (const SelectionItem& item : start) {
      const auto shape = bodyShape(item.owner);
      if (printable(shape)) {
        bodies.append(item);
      } else if (!seen.contains(item.owner)) {
        const bool sheet = shape && geometry::body_kind(shape->occt()) == geometry::BodyKind::Sheet;
        leftOut << (sheet ? tr("%1 (a surface body)") : tr("%1 (empty)")).arg(bodyName(item.owner));
      }
      seen << item.owner;
    }
    state.setItems(QStringLiteral("bodies"), bodies);
    if (!leftOut.isEmpty()) {
      qWarning().noquote() << QStringLiteral("3D Print: left out %1").arg(leftOut.join(QStringLiteral(", ")));
    }
    if (bodies.isEmpty()) {
      qInfo().noquote() << QStringLiteral("3D Print: no solid or mesh bodies");
    }
  };
  // The choices of the last send (the settings), and the slicers there are.
  def.init = [this, panel, chosenKey, askChanges](CommandState& state, const CommandContext& context) {
    // The last import's line is the last panel's (mitcad#655): gone now.
    panel->imported->summary = PrintImportSummary();
    const PrintSettings settings = PrintSettings::load();
    panel->found = findSlicers();
    panel->saved = settings.slicer;
    panel->other = Slicer{};
    state.setChoice(QStringLiteral("format"), settings.format);
    state.setChoice(QStringLiteral("refinement"), settings.refinement);
    state.setText(QStringLiteral("deviation"), QStringLiteral("%1 mm").arg(settings.deviation));
    state.setText(QStringLiteral("angle"), QStringLiteral("%1 deg").arg(settings.angle));
    const Slicer current = settings.slicer.isValid() ? settings.slicer : panel->found.value(0);
    panel->previous = slicerChoice(current);
    state.setChoice(QStringLiteral("slicer"), panel->previous);
    QStringList names;
    for (const SelectionItem& item : state.items(QStringLiteral("bodies"))) {
      const QString name = context.bodyName(item.owner);
      if (!names.contains(name)) {
        names << name;
      }
    }
    QStringList slicers;
    for (const auto& choice : slicerChoices(panel->found, panel->saved, panel->other)) {
      slicers << choice.second;
    }
    // The design's printer and process (mitcad#655): what a 3MF carries.
    state.setChoice(QStringLiteral("printer"), chosenKey(QStringLiteral("printer")));
    state.setChoice(QStringLiteral("process"), chosenKey(QStringLiteral("process")));
    // A saved one another design changed: asked once, after the panel shows.
    TestSync::singleShot(0, this, askChanges);
    panel->imported = std::make_shared<PrintImport>();
    qDebug().noquote() << QStringLiteral("3D Print printer: %1").arg(designPrinterName());
    qDebug().noquote() << QStringLiteral("3D Print panel: %1 %2: %3; format %4, refinement %5, slicer %6; slicers %7")
                              .arg(names.size())
                              .arg(panel->selected ? QStringLiteral("selected") : QStringLiteral("visible"),
                                   names.join(QStringLiteral(", ")), settings.format, settings.refinement,
                                   current.describe(), slicers.join(QStringLiteral(" | ")));
  };
  def.build = [](const CommandState& state, const CommandContext&) {
    if (state.choice(QStringLiteral("refinement")) == QStringLiteral("custom")) {
      const double deviation = state.value(QStringLiteral("deviation"));
      if (deviation < 0.001 || deviation > 10.0) {
        return Built::failure(tr("The deviation is between 0.001 and 10 mm."), QStringLiteral("deviation"));
      }
      const double angle = qRadiansToDegrees(state.value(QStringLiteral("angle")));
      if (angle < 0.5 || angle > 90.0) {
        return Built::failure(tr("The angle is between 0.5 and 90 deg."), QStringLiteral("angle"));
      }
    }
    return Built::of(QJsonObject());
  };
  def.accept = [this](const CommandState& state, const CommandContext&) { return sendToSlicer(state); };
  return def;
}

bool MainWindow::sendToSlicer(const CommandState& state) {
  // Each body once, in the order picked; the placements picked decide the
  // STL files of a body shown more than once.
  const Selection& chosen = state.items(QStringLiteral("bodies"));
  QStringList sent;
  QStringList names;
  for (const SelectionItem& item : chosen) {
    if (!sent.contains(item.owner)) {
      sent << item.owner;
      names << bodyName(item.owner);
    }
  }
  const QString format = state.choice(QStringLiteral("format"));
  const QString refinementChoice = state.choice(QStringLiteral("refinement"));
  const double deviation = state.value(QStringLiteral("deviation")); // mm
  const double angle = state.value(QStringLiteral("angle"));         // radians
  const Slicer slicer = slicerOfChoice(state.choice(QStringLiteral("slicer")));
  // Remembered; "None" this time keeps the slicer of the settings.
  PrintSettings settings = PrintSettings::load();
  settings.format = format;
  settings.refinement = refinementChoice;
  if (std::isfinite(deviation)) {
    settings.deviation = deviation;
  }
  if (std::isfinite(angle)) {
    settings.angle = qRadiansToDegrees(angle);
  }
  if (slicer.isValid()) {
    settings.slicer = slicer;
  }
  settings.save();
  // The status line, after the "Done." of the panel closing.
  // The export's warnings go with the last line, in the warning colour,
  // one per line (mitcad#655).
  auto warned = std::make_shared<QString>();
  const auto report = [this, warned](const QString& message, bool error) {
    TestSync::singleShot(0, this, [this, message, error, warned] {
      if (error) {
        showError(message);
      } else if (!warned->isEmpty()) {
        showWarning(message + QLatin1Char('\n') + *warned);
      } else {
        showHint(message);
      }
    });
  };

  // A folder of the design's own in the temporary folder, emptied first.
  const QString design = m_filePath.isEmpty() ? documentName() : QFileInfo(m_filePath).completeBaseName();
  QString error;
  const QString folder = preparePrintFolder(QDir::tempPath(), design, &error);
  if (folder.isEmpty()) {
    sheetWarning(this, tr("3D Print"), error);
    return false;
  }
  const QJsonValue refinement =
      refinementChoice == QStringLiteral("custom")
          ? QJsonValue(QJsonObject{{QStringLiteral("deviation"), deviation}, {QStringLiteral("angle"), angle}})
          : QJsonValue(refinementChoice);
  QStringList files;
  QStringList written;
  if (format == QStringLiteral("3mf")) {
    // One object with a part per body, in the colours shown.
    const QString path = QDir(folder).filePath(safeFileName(design) + QStringLiteral(".3mf"));
    QJsonObject command = exportCommand(path, QStringLiteral("3mf"), sent, refinement);
    command.insert(QStringLiteral("colors"), appearanceColors(sent));
    // The print settings in the slicer's own dialect (mitcad#640); without a
    // slicer of a known dialect, in the dialect the design's printer came
    // from, else Bambu Studio's: the printer, the process and the materials
    // always go with a 3MF (mitcad#655).
    QString dialect = slicerDialect(slicer);
    if (dialect.isEmpty()) {
      dialect = designDialect();
    }
    if (!dialect.isEmpty()) {
      command.insert(QStringLiteral("dialect"), dialect);
    }
    QJsonObject result;
    if (!runCommand(command, &result)) {
      sheetWarning(this, tr("3D Print"), tr("Could not write the 3MF file:\n%1").arg(m_lastError));
      return false;
    }
    if (!dialect.isEmpty()) {
      QStringList filaments;
      for (const QJsonValue& slot : result.value(QStringLiteral("filaments")).toArray()) {
        filaments << QString::number(slot.toInt());
      }
      QStringList warnings;
      for (const QJsonValue& warning : result.value(QStringLiteral("warnings")).toArray()) {
        warnings << warning.toString();
      }
      qInfo().noquote() << QStringLiteral("3D Print: print settings for %1, filaments %2%3")
                               .arg(dialect, filaments.join(QLatin1Char(',')),
                                    warnings.isEmpty() ? QString()
                                                       : QStringLiteral("; ") + warnings.join(QStringLiteral("; ")));
      if (!warnings.isEmpty()) {
        // By plain names and the slicers' titles, as the import's report
        // (mitcad#655).
        PrintKeyTable keys;
        try {
          keys = PrintKeyTable(queryObject({{QStringLiteral("query"), QStringLiteral("print_keys")}}));
        } catch (const std::exception&) {
          // Without the names: the slicers' titles still.
        }
        QStringList shown;
        for (const QString& warning : std::as_const(warnings)) {
          shown << keys.plainText(warning);
        }
        qInfo().noquote() << QStringLiteral("3D Print warnings: %1").arg(shown.join(QStringLiteral("; ")));
        // With the last line, in the warning colour (mitcad#655).
        *warned = shown.join(QLatin1Char('\n'));
      }
    } else if (const QStringList warnings = exportWarnings(result); !warnings.isEmpty()) {
      // The print bed faces that cannot be used (mitcad#731).
      *warned = warnings.join(QLatin1Char('\n'));
    }
    qInfo().noquote() << QStringLiteral("3D Print bed: %1").arg(bedLog(result.value(QStringLiteral("bed")).toObject()));
    files << path;
    for (const QJsonValue& value : result.value(QStringLiteral("meshes")).toArray()) {
      const QJsonObject mesh = value.toObject();
      written << QStringLiteral("%1 %2 triangles, %3 mm3")
                     .arg(mesh.value(QStringLiteral("name")).toString())
                     .arg(mesh.value(QStringLiteral("triangles")).toInt())
                     .arg(mesh.value(QStringLiteral("volume")).toDouble(), 0, 'f', 3);
    }
  } else {
    // A file per body, named after it, in the design's coordinates; a body
    // shown more than once a file per occurrence picked (mitcad#17).
    const QVector<PrintPiece> pieces = printPieces(
        sent, names,
        query({{QStringLiteral("query"), QStringLiteral("instances")}, {QStringLiteral("hidden"), true}}).toArray(),
        chosen);
    QStringList pieceNames;
    for (const PrintPiece& piece : pieces) {
      pieceNames << piece.name;
    }
    const QStringList fileNames = printFileNames(pieceNames, QStringLiteral("stl"));
    for (int i = 0; i < pieces.size(); ++i) {
      const QString path = QDir(folder).filePath(fileNames[i]);
      QJsonObject command = exportCommand(path, QStringLiteral("stl"), {pieces[i].body}, refinement);
      if (!pieces[i].occurrence.isEmpty()) {
        command.insert(QStringLiteral("occurrence"), pieces[i].occurrence);
      }
      QJsonObject result;
      if (!runCommand(command, &result)) {
        sheetWarning(this, tr("3D Print"),
                     tr("Could not write %1:\n%2").arg(QDir::toNativeSeparators(path), m_lastError));
        return false;
      }
      // Each file's body on its print bed face (mitcad#731).
      qInfo().noquote() << QStringLiteral("3D Print bed %1: %2")
                               .arg(fileNames[i], bedLog(result.value(QStringLiteral("bed")).toObject()));
      if (const QStringList warnings = exportWarnings(result); !warnings.isEmpty()) {
        *warned += (warned->isEmpty() ? QString() : QStringLiteral("\n")) + warnings.join(QLatin1Char('\n'));
      }
      files << path;
      written << fileNames[i];
    }
  }
  qInfo().noquote() << QStringLiteral("3D Print: %1 bodies as %2 (%3) to %4: %5")
                           .arg(sent.size())
                           .arg(format, refinementChoice, folder, written.join(QStringLiteral(", ")));

  // One start of the slicer with every file.
  const auto openFolder = [folder] {
    QDesktopServices::openUrl(QUrl::fromLocalFile(folder));
    qInfo().noquote() << QStringLiteral("3D Print: opened %1").arg(folder);
  };
  if (!slicer.isValid()) {
    openFolder();
    report(tr("The files are in %1.").arg(QDir::toNativeSeparators(folder)), false);
    return true;
  }
  QStringList arguments = slicer.arguments;
  for (const QString& file : std::as_const(files)) {
    arguments << QDir::toNativeSeparators(file);
  }
  arguments << slicer.argumentsAfter;
  if (!QProcess::startDetached(slicer.program, arguments, folder)) {
    qWarning().noquote() << QStringLiteral("3D Print: %1 did not start").arg(slicer.describe());
    report(tr("%1 did not start; the files are in %2.")
               .arg(slicer.name.isEmpty() ? slicer.program : slicer.name, QDir::toNativeSeparators(folder)),
           true);
    openFolder();
    return true;
  }
  qInfo().noquote() << QStringLiteral("3D Print: started %1 %2")
                           .arg(QDir::toNativeSeparators(slicer.program), arguments.join(QLatin1Char(' ')));
  // A real singular and plural (mitcad#655), not "body(ies)".
  const QString to = slicer.name.isEmpty() ? QFileInfo(slicer.program).fileName() : slicer.name;
  report(sent.size() == 1 ? tr("Sent 1 body to %1.").arg(to)
                          : tr("Sent %1 bodies to %2.").arg(sent.size()).arg(to),
         false);
  return true;
}

} // namespace mitcad
