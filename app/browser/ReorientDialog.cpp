// SPDX-License-Identifier: MIT
#include "ReorientDialog.hpp"

#include <utility>

#include <QAbstractButton>
#include <QButtonGroup>
#include <QComboBox>
#include <QDialogButtonBox>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QPushButton>
#include <QRadioButton>
#include <QVBoxLayout>
#include <QtLogging>

namespace mitcad {
namespace {

QJsonObject cmd(const char* name) { return {{QStringLiteral("cmd"), QString::fromLatin1(name)}}; }

// "90 degrees about X" or "XZ becomes XY", as the model names the turn.
QString describe(const QJsonObject& command) {
  const QString plane = command.value(QStringLiteral("plane")).toString();
  if (!plane.isEmpty()) {
    return QStringLiteral("%1 becomes XY").arg(plane.toUpper());
  }
  return QStringLiteral("%1 degrees about %2")
      .arg(command.value(QStringLiteral("degrees")).toInt())
      .arg(command.value(QStringLiteral("axis")).toString().toUpper());
}

} // namespace

ReorientDialog::ReorientDialog(Run run, LastError lastError, QWidget* parent)
    : QDialog(parent), m_run(std::move(run)), m_lastError(std::move(lastError)) {
  setObjectName(QStringLiteral("reorientDesignDialog"));
  setWindowTitle(tr("Reorient Design"));
  auto* layout = new QVBoxLayout(this);
  auto* intro = new QLabel(tr("Turns the whole design. Sketches, construction geometry and features turn "
                              "with it: references to the origin planes and axes become the turned ones, "
                              "and imported bodies and components turn by their placement."));
  intro->setWordWrap(true);
  layout->addWidget(intro);

  auto* grid = new QGridLayout;
  m_aboutAxis = new QRadioButton(tr("&Turn about an axis"));
  m_ontoPlane = new QRadioButton(tr("An &origin plane becomes XY"));
  auto* modes = new QButtonGroup(this);
  modes->addButton(m_aboutAxis);
  modes->addButton(m_ontoPlane);
  m_aboutAxis->setChecked(true);
  grid->addWidget(m_aboutAxis, 0, 0, 1, 2);

  grid->addWidget(new QLabel(tr("Axis:")), 1, 0);
  auto* axes = new QHBoxLayout;
  m_axes = new QButtonGroup(this);
  const QStringList axisLabels{tr("&X"), tr("&Y"), tr("&Z")};
  const QStringList axisIds{QStringLiteral("x"), QStringLiteral("y"), QStringLiteral("z")};
  for (int i = 0; i < axisLabels.size(); ++i) {
    auto* button = new QRadioButton(axisLabels[i]);
    button->setProperty("axis", axisIds[i]);
    m_axes->addButton(button, i);
    axes->addWidget(button);
  }
  axes->addStretch();
  m_axes->button(0)->setChecked(true);
  grid->addLayout(axes, 1, 1);

  grid->addWidget(new QLabel(tr("Angle:")), 2, 0);
  auto* angles = new QHBoxLayout;
  m_angles = new QButtonGroup(this);
  const QStringList angleLabels{tr("&90°"), tr("&180°"), tr("2&70°")};
  const int degrees[] = {90, 180, 270};
  for (int i = 0; i < angleLabels.size(); ++i) {
    auto* button = new QRadioButton(angleLabels[i]);
    m_angles->addButton(button, degrees[i]);
    angles->addWidget(button);
  }
  angles->addStretch();
  m_angles->button(90)->setChecked(true);
  grid->addLayout(angles, 2, 1);

  grid->addWidget(m_ontoPlane, 3, 0, 1, 2);
  auto* planeLabel = new QLabel(tr("&Plane:"));
  m_plane = new QComboBox;
  m_plane->setObjectName(QStringLiteral("reorientDesignPlane"));
  m_plane->addItem(tr("XZ (front)"), QStringLiteral("xz"));
  m_plane->addItem(tr("YZ (side)"), QStringLiteral("yz"));
  planeLabel->setBuddy(m_plane);
  grid->addWidget(planeLabel, 4, 0);
  grid->addWidget(m_plane, 4, 1);
  layout->addLayout(grid);

  m_details = new QLabel;
  m_details->setObjectName(QStringLiteral("reorientDesignDetails"));
  m_details->setWordWrap(true);
  m_details->setMinimumHeight(m_details->fontMetrics().lineSpacing() * 4);
  m_details->setAlignment(Qt::AlignLeft | Qt::AlignTop);
  layout->addWidget(m_details, 1);

  m_buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
  layout->addWidget(m_buttons);
  connect(m_buttons, &QDialogButtonBox::accepted, this, &ReorientDialog::accept);
  connect(m_buttons, &QDialogButtonBox::rejected, this, &ReorientDialog::reject);

  // A choice of axis or angle turns about an axis; one of plane, onto it.
  const auto aboutAxis = [this] {
    m_aboutAxis->setChecked(true);
    preview();
  };
  connect(m_axes, &QButtonGroup::idClicked, this, aboutAxis);
  connect(m_angles, &QButtonGroup::idClicked, this, aboutAxis);
  connect(m_plane, &QComboBox::activated, this, [this] {
    m_ontoPlane->setChecked(true);
    preview();
  });
  connect(m_aboutAxis, &QRadioButton::clicked, this, [this] { preview(); });
  connect(m_ontoPlane, &QRadioButton::clicked, this, [this] { preview(); });

  QJsonObject checkpoint;
  if (m_run(cmd("begin_command"), &checkpoint)) {
    m_token = checkpoint.value(QStringLiteral("token")).toString();
  }
  qDebug() << "Reorient Design dialog opened";
  preview();
}

ReorientDialog::~ReorientDialog() {
  // Closed without OK or Cancel (the window closing): as Cancel.
  if (!m_token.isEmpty()) {
    takeBack();
    release();
  }
}

QJsonObject ReorientDialog::command() const {
  QJsonObject out = cmd("reorient_design");
  if (m_ontoPlane->isChecked()) {
    out.insert(QStringLiteral("plane"), m_plane->currentData().toString());
  } else {
    const QAbstractButton* axis = m_axes->checkedButton();
    out.insert(QStringLiteral("axis"), axis != nullptr ? axis->property("axis").toString() : QStringLiteral("x"));
    out.insert(QStringLiteral("degrees"), m_angles->checkedId());
  }
  return out;
}

void ReorientDialog::takeBack() {
  if (m_turned && !m_token.isEmpty()) {
    QJsonObject restore = cmd("restore_command");
    restore.insert(QStringLiteral("token"), m_token);
    m_run(restore, nullptr);
  }
  m_turned = false;
  m_result = {};
}

void ReorientDialog::release() {
  if (!m_token.isEmpty()) {
    QJsonObject discard = cmd("discard_command");
    discard.insert(QStringLiteral("token"), m_token);
    m_run(discard, nullptr);
    m_token.clear();
  }
}

void ReorientDialog::preview() {
  takeBack();
  const QJsonObject turn = command();
  QJsonObject answer;
  if (m_token.isEmpty()) {
    m_details->setText(tr("The design cannot be previewed now."));
    m_buttons->button(QDialogButtonBox::Ok)->setEnabled(false);
    return;
  }
  if (!m_run(turn, &answer)) {
    const QString reason = m_lastError();
    m_details->setText(reason);
    m_buttons->button(QDialogButtonBox::Ok)->setEnabled(false);
    qDebug().noquote() << QStringLiteral("Reorient Design preview refused: %1: %2").arg(describe(turn), reason);
    return;
  }
  m_turned = true;
  m_result = answer;
  const int features = static_cast<int>(answer.value(QStringLiteral("features")).toArray().size());
  const int moves = static_cast<int>(answer.value(QStringLiteral("moves")).toArray().size());
  const int occurrences = static_cast<int>(answer.value(QStringLiteral("occurrences")).toArray().size());
  QStringList lines;
  lines << tr("Features rewritten: %1").arg(features);
  if (moves > 0) {
    lines << tr("Moves added after imported bodies: %1").arg(moves);
  }
  if (occurrences > 0) {
    lines << tr("Components placed anew: %1").arg(occurrences);
  }
  const int views = answer.value(QStringLiteral("views")).toInt();
  const int analyses = answer.value(QStringLiteral("analyses")).toInt();
  const int lights = answer.value(QStringLiteral("lights")).toInt();
  if (views + analyses + lights > 0) {
    lines << tr("Named views, sections and lights turned: %1").arg(views + analyses + lights);
  }
  m_details->setText(lines.join(QLatin1Char('\n')));
  m_buttons->button(QDialogButtonBox::Ok)->setEnabled(true);
  qDebug().noquote() << QStringLiteral("Reorient Design preview: %1: %2 features, %3 moves, %4 components")
                            .arg(answer.value(QStringLiteral("turn")).toString())
                            .arg(features)
                            .arg(moves)
                            .arg(occurrences);
}

void ReorientDialog::accept() {
  if (!m_turned) {
    return;
  }
  release();
  QDialog::accept();
}

void ReorientDialog::reject() {
  takeBack();
  release();
  qDebug() << "Reorient Design cancelled";
  QDialog::reject();
}

} // namespace mitcad
