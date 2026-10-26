// SPDX-License-Identifier: MIT
// Drawings kept in the design (mitcad#707, app/COMMANDS.md "Drawings"):
// Create Drawing (the File menu, the browser's Drawings folders and a
// component's menu) with its sheet's size, orientation and projection
// method, a drawing opened in its own view over the 3D view
// (DrawingWorkspace) and back, the drawing tools as commands of the
// window, and the view following the model.
#include "../MainWindow.hpp"

#include <algorithm>
#include <optional>

#include <QApplication>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QFileInfo>
#include <QFormLayout>
#include <QJsonArray>
#include <QJsonObject>
#include <QKeySequence>
#include <QtLogging>

#include <gp_Dir.hxx>

#include "../OcctViewer.hpp"
#include "../framework/CommandRegistry.hpp"
#include "../framework/Dialogs.hpp"
#include "DrawingTemplates.hpp"
#include "DrawingTools.hpp"
#include "DrawingWorkspace.hpp"

namespace mitcad {
namespace {

// The 3D view's direction as a base view's Current View takes it
// (mitcad#725): toward the eye (the camera looks the other way) and up.
QJsonObject cameraOf(const OcctViewer& viewer) {
  const gp_Dir eye = viewer.viewDirection().Reversed();
  const gp_Dir up = viewer.viewUp();
  return {{QStringLiteral("toward_eye"), QJsonArray{eye.X() + 0.0, eye.Y() + 0.0, eye.Z() + 0.0}},
          {QStringLiteral("up"), QJsonArray{up.X() + 0.0, up.Y() + 0.0, up.Z() + 0.0}}};
}

// Create Drawing's question: the first sheet's template, size, orientation
// and projection method (the model's names), or none when cancelled. A
// template chosen brings its own sheet size; ISO is chosen first.
std::optional<QJsonObject> askNewDrawing(QWidget* parent, const QList<drawings::TemplateChoice>& templates) {
  QDialog dialog(parent);
  dialog.setWindowTitle(QObject::tr("Create Drawing"));
  auto* form = new QFormLayout(&dialog);
  const auto combo = [](const QList<QPair<QString, QString>>& choices) {
    auto* box = new QComboBox;
    for (const auto& [value, label] : choices) {
      box->addItem(label, value);
    }
    return box;
  };
  auto* templateBox = new QComboBox;
  templateBox->setObjectName(QStringLiteral("drawingTemplate"));
  for (const drawings::TemplateChoice& choice : templates) {
    templateBox->addItem(choice.label, choice.choice);
  }
  templateBox->setCurrentIndex(
      std::max(0, drawings::templateIndex(templates, drawings::defaultTemplate(QStringLiteral("A4")))));
  form->addRow(QObject::tr("&Template"), templateBox);
  QComboBox* sizes = combo({{QStringLiteral("A4"), QStringLiteral("A4")},
                            {QStringLiteral("A3"), QStringLiteral("A3")},
                            {QStringLiteral("A2"), QStringLiteral("A2")},
                            {QStringLiteral("A1"), QStringLiteral("A1")},
                            {QStringLiteral("A0"), QStringLiteral("A0")},
                            {QStringLiteral("ANSI_A"), QStringLiteral("ANSI A")},
                            {QStringLiteral("ANSI_B"), QStringLiteral("ANSI B")},
                            {QStringLiteral("ANSI_C"), QStringLiteral("ANSI C")},
                            {QStringLiteral("ANSI_D"), QStringLiteral("ANSI D")},
                            {QStringLiteral("ANSI_E"), QStringLiteral("ANSI E")}});
  sizes->setObjectName(QStringLiteral("drawingSize"));
  form->addRow(QObject::tr("Size"), sizes);
  QComboBox* orientation = combo({{QStringLiteral("landscape"), QObject::tr("Landscape")},
                                  {QStringLiteral("portrait"), QObject::tr("Portrait")}});
  form->addRow(QObject::tr("Orientation"), orientation);
  QComboBox* projection = combo({{QStringLiteral("third_angle"), QObject::tr("Third Angle")},
                                 {QStringLiteral("first_angle"), QObject::tr("First Angle")}});
  projection->setObjectName(QStringLiteral("drawingProjection"));
  form->addRow(QObject::tr("Projection"), projection);
  // A template chosen brings its sheet size and its projection method
  // (ISO: A3, first angle; ANSI: B, third angle; mitcad#725).
  const auto templateSheet = [templateBox, sizes, projection, &templates] {
    const int at = drawings::templateIndex(templates, templateBox->currentData().toString());
    const int size = at < 0 ? -1 : sizes->findData(templates.at(at).size);
    if (size >= 0) {
      sizes->setCurrentIndex(size);
    }
    const int method = at < 0 ? -1 : projection->findData(templates.at(at).projection);
    if (method >= 0) {
      projection->setCurrentIndex(method);
    }
  };
  QObject::connect(templateBox, &QComboBox::currentIndexChanged, sizes, templateSheet);
  // The template chosen first brings its sheet too, as the handler was not
  // connected when it was chosen (mitcad#722).
  templateSheet();
  auto* buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
  QObject::connect(buttons, &QDialogButtonBox::accepted, &dialog, &QDialog::accept);
  QObject::connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
  form->addRow(buttons);
  qDebug().noquote() << QStringLiteral("Create Drawing dialog opened");
  if (execDialog(&dialog) != QDialog::Accepted) {
    qDebug().noquote() << QStringLiteral("Create Drawing cancelled");
    return std::nullopt;
  }
  QJsonObject out{{QStringLiteral("size"), sizes->currentData().toString()},
                  {QStringLiteral("orientation"), orientation->currentData().toString()},
                  {QStringLiteral("projection"), projection->currentData().toString()}};
  const int chosen = drawings::templateIndex(templates, templateBox->currentData().toString());
  if (chosen >= 0) {
    out.insert(QStringLiteral("template"), templates.at(chosen).value);
  }
  qDebug().noquote() << QStringLiteral("Create Drawing dialog: template %1, %2, %3")
                            .arg(chosen >= 0 && !templates.at(chosen).choice.isEmpty() ? templates.at(chosen).choice
                                                                                       : QStringLiteral("simple"),
                                 sizes->currentData().toString(), projection->currentData().toString());
  return out;
}

} // namespace

drawings::DrawingWorkspace* MainWindow::drawingWorkspace() {
  if (m_drawingWorkspace == nullptr) {
    m_drawingWorkspace = new drawings::DrawingWorkspace(
        m_viewer, *this, [this] { return drawingTitle(); }, [this] { return cameraOf(*m_viewer); },
        [this](const QKeySequence& key) {
          // A shortcut of the model's commands (a feature, a command of a
          // ribbon tab), which would leave the drawing (trigger, mitcad#725).
          for (const auto& def : m_registry->commands()) {
            if ((def->kind == CommandDef::Kind::Feature || !def->tab.isEmpty()) &&
                m_registry->effectiveShortcuts(def->id).contains(key)) {
              return true;
            }
          }
          return false;
        });
    connect(m_drawingWorkspace, &drawings::DrawingWorkspace::closed, this, [this] {
      m_drawingDesign.clear();
      m_viewer->setFocus(Qt::OtherFocusReason);
      updateActions();
    });
  }
  return m_drawingWorkspace;
}

QString MainWindow::drawingTitle() const {
  return m_filePath.isEmpty() ? documentName() : QFileInfo(m_filePath).completeBaseName();
}

bool MainWindow::drawingOpen() const { return m_drawingWorkspace != nullptr && m_drawingWorkspace->isOpen(); }

void MainWindow::registerDrawingCommands() {
  // The drawing tools and Close Drawing as the window's commands: the
  // command search and the shortcuts find them, enabled while a drawing
  // is open. On no ribbon tab: the drawing's own bar shows them, and a
  // command with a tab would lead back to the 3D view (trigger).
  const auto add = [this](const QString& id, const QString& name, const QString& icon, const QString& tooltip,
                          std::function<void()> run) {
    CommandDef def;
    def.id = id;
    def.name = name;
    def.icon = icon;
    def.tooltip = tooltip;
    def.kind = CommandDef::Kind::Action;
    def.mode = CommandDef::Mode::Model;
    def.tab.clear();
    // The tools' other words (Leader, Chain, BOM, …; mitcad#725).
    def.keywords = drawings::toolKeywords(id);
    def.run = std::move(run);
    def.enabled = [this] { return drawingOpen(); };
    m_registry->add(def);
  };
  for (const drawings::DrawingToolDef& tool : drawings::drawingTools()) {
    const QString id = tool.id;
    // The drawing's Parts List is told apart from the Build tab's in the
    // command search (mitcad#718); on the drawing's bar it keeps its name.
    const QString name = id == QStringLiteral("drawing.parts_list") ? tr("Parts List (Drawing)") : tool.name;
    add(id, name, tool.icon, tool.tooltip, [this, id] {
      if (drawingOpen()) {
        m_drawingWorkspace->startTool(id);
      }
    });
  }
  add(QStringLiteral("drawing.close"), tr("Close Drawing"), QStringLiteral("view-arrow-left"),
      tr("Shows the 3D model again"), [this] { leaveDrawing(); });
  // A row of the drawing's revision table (mitcad#707 phase 4).
  add(QStringLiteral("drawing.add_revision"), tr("Add Revision…"), QStringLiteral("version-history"),
      tr("Adds a row to the drawing's revision table"), [this] {
        if (drawingOpen()) {
          m_drawingWorkspace->addRevision();
        }
      });
}

void MainWindow::openDrawing(const QString& uid) {
  if (m_mode != Mode::Idle) {
    showStatus(changeBlockedReason(), true);
    return;
  }
  m_drawingDesign = designId();
  if (!drawingWorkspace()->open(uid)) {
    m_drawingDesign.clear();
    showStatus(tr("There is no drawing %1.").arg(uid), true);
  }
  updateActions();
}

void MainWindow::createDrawing(const QString& component) {
  if (!canChangeModel()) {
    showStatus(changeBlockedReason(), true);
    return;
  }
  const std::optional<QJsonObject> sheet = askNewDrawing(this, drawings::availableTemplates(*this));
  if (!sheet) {
    return;
  }
  QJsonObject command = *sheet;
  command.insert(QStringLiteral("cmd"), QStringLiteral("add_drawing"));
  if (!component.isEmpty() && component != QStringLiteral("C0")) {
    command.insert(QStringLiteral("component"), component);
  }
  QJsonObject result;
  if (!runModelCommand(command, &result)) {
    return;
  }
  const QString uid = result.value(QStringLiteral("drawing")).toString();
  qDebug().noquote() << QStringLiteral("Drawing created: %1 %2 in %3")
                            .arg(uid, result.value(QStringLiteral("name")).toString(),
                                 result.value(QStringLiteral("component")).toString());
  openDrawing(uid);
  // A new drawing's first view comes next (mitcad#725): Base View is on.
  if (drawingOpen()) {
    m_drawingWorkspace->startTool(QStringLiteral("drawing.base_view"));
  }
}

void MainWindow::followDrawing() {
  if (!drawingOpen()) {
    return;
  }
  // Another design: its drawings are not this one's.
  if (designId() != m_drawingDesign) {
    m_drawingWorkspace->closeDrawing();
    return;
  }
  m_drawingWorkspace->refresh();
}

bool MainWindow::escapeDrawing() {
  if (!drawingOpen()) {
    return false;
  }
  const QWidget* focus = QApplication::focusWidget();
  if (focus != nullptr && focus != m_viewer && !m_drawingWorkspace->isAncestorOf(focus)) {
    return false; // the browser's, a panel's, ...
  }
  m_drawingWorkspace->escape();
  return true;
}

void MainWindow::leaveDrawing() {
  if (drawingOpen()) {
    m_drawingWorkspace->closeDrawing();
  }
}

} // namespace mitcad
