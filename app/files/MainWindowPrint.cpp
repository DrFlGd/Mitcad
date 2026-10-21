// SPDX-License-Identifier: MIT
// 3D Print (mitcad#13, mitcad#234, Print3d.hpp): a panel of the bodies to
// send (the selected ones, or every visible one, then picked in the view),
// the format, the refinement and the slicer. OK writes the bodies for a
// slicer into a folder of their own (as one 3MF file or an STL file per
// body, through the model's `export`) and gives them to it in one start;
// without a slicer, or when it does not start, the folder opens.
#include "../MainWindow.hpp"

#include <cmath>
#include <memory>
#include <utility>

#include <QColor>
#include <QDesktopServices>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonObject>
#include <QMessageBox>
#include <QProcess>
#include <QUrl>
#include <QtLogging>
#include <QtMath>

#include "../framework/Appearances.hpp"
#include "../framework/TestSync.hpp"
#include "Print3d.hpp"
#include "mitcad/geometry/import.hpp"

#include "../framework/Dialogs.hpp"

namespace mitcad {
namespace {

QJsonObject exportCommand(const QString& path, const QString& format, const QStringList& bodies,
                          const QJsonValue& refinement) {
  return {{QStringLiteral("cmd"), QStringLiteral("export")},
          {QStringLiteral("path"), path},
          {QStringLiteral("format"), format},
          {QStringLiteral("bodies"), QJsonArray::fromStringList(bodies)},
          {QStringLiteral("refinement"), refinement}};
}

// What the panel keeps while it is open, besides its inputs.
struct PrintPanel {
  QVector<Slicer> found; // when the command started
  Slicer saved;          // the settings' slicer
  Slicer other;          // one chosen by hand (Other program...)
  QString previous;      // the Slicer choice before Other program...
  bool selected = false; // the bodies came from the selection
};

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

CommandDef MainWindow::print3dCommand() {
  CommandDef def;
  def.id = QStringLiteral("make.print3d");
  def.name = tr("3D Print");
  def.icon = QStringLiteral("print-3d");
  def.tooltip = tr("Sends the selected bodies, or all visible ones, to a slicer as 3MF or STL files");
  def.group = QStringLiteral("MAKE");
  def.pinned = true;
  def.keywords = {QStringLiteral("print"), QStringLiteral("slicer"), QStringLiteral("3mf"),
                  QStringLiteral("stl"),   QStringLiteral("3d print"), QStringLiteral("make")};
  def.okLabel = tr("Send");
  const auto panel = std::make_shared<PrintPanel>();
  const auto custom = [](const CommandState& state) {
    return state.choice(QStringLiteral("refinement")) == QStringLiteral("custom");
  };
  def.inputs = {
      // Sheet and empty bodies cannot be picked, and are not sent.
      selectionInput(QStringLiteral("bodies"), tr("Bodies"), SelectKind::Body, 1, 0)
          .withAccepts([this](const SelectionItem& item) { return printable(bodyShape(item.owner)); })
          .withTooltip(tr("The bodies to send: a click on a body adds or removes it")),
      choiceInput(QStringLiteral("format"), tr("Format"),
                  {{QStringLiteral("stl"), tr("STL, one file per body")},
                   {QStringLiteral("3mf"), tr("3MF, one object with parts")}},
                  QStringLiteral("stl"))
          .withTooltip(tr("3MF: the bodies as the parts of one object; STL: a file per body")),
      choiceInput(QStringLiteral("refinement"), tr("Refinement"),
                  {{QStringLiteral("low"), tr("Low")},
                   {QStringLiteral("medium"), tr("Medium")},
                   {QStringLiteral("high"), tr("High")},
                   {QStringLiteral("custom"), tr("Custom")}},
                  QStringLiteral("high")),
      valueInput(QStringLiteral("deviation"), tr("Deviation"), ValueKind::Length, QStringLiteral("0.01 mm"))
          .withVisible(custom)
          .withTooltip(tr("The largest distance between the triangles and the surface")),
      valueInput(QStringLiteral("angle"), tr("Angle"), ValueKind::Angle, QStringLiteral("8 deg"))
          .withVisible(custom)
          .withTooltip(tr("The largest angle between neighbouring triangles")),
      choiceInput(QStringLiteral("slicer"), tr("Slicer"), {}, QString())
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
            const QString program = dialogOpenFileName(this, tr("Slicer Program"));
            if (!program.isEmpty()) {
              panel->other = slicerForProgram(QFileInfo(program).completeBaseName(), program);
              panel->previous = slicerChoice(panel->other);
            }
            state.setChoice(QStringLiteral("slicer"), panel->previous);
          })
          .withTooltip(tr("The program the files go to; none: the files' folder opens")),
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
  def.init = [panel](CommandState& state, const CommandContext& context) {
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
  const auto report = [this](const QString& message, bool error) {
    TestSync::singleShot(0, this, [this, message, error] { error ? showError(message) : showHint(message); });
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
    QJsonObject result;
    if (!runCommand(command, &result)) {
      sheetWarning(this, tr("3D Print"), tr("Could not write the 3MF file:\n%1").arg(m_lastError));
      return false;
    }
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
      if (!runCommand(command)) {
        sheetWarning(this, tr("3D Print"),
                     tr("Could not write %1:\n%2").arg(QDir::toNativeSeparators(path), m_lastError));
        return false;
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
  report(tr("Sent %n body(ies) to %1.", nullptr, static_cast<int>(sent.size()))
             .arg(slicer.name.isEmpty() ? QFileInfo(slicer.program).fileName() : slicer.name),
         false);
  return true;
}

} // namespace mitcad
