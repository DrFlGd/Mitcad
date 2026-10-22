// SPDX-License-Identifier: MIT
#pragma once

#include <functional>

#include <QDialog>
#include <QJsonObject>
#include <QString>

class QButtonGroup;
class QComboBox;
class QDialogButtonBox;
class QLabel;
class QRadioButton;

namespace mitcad {

// Reorient Design (mitcad#502), from the root's context menu: the whole
// design turned by 90, 180 or 270 degrees about the origin's X, Y or Z
// axis, or so that an origin plane becomes XY. The design shows turned
// while the dialog is open: the dialog keeps a command checkpoint
// (`begin_command`) and runs `reorient_design` on it for each choice,
// taking the previous one back first (`restore_command`); OK keeps the
// last one (one undo step, `Reorient Design`), Cancel takes it back. A
// turn the model refuses shows its reason and disables OK.
class ReorientDialog : public QDialog {
  Q_OBJECT

public:
  // Runs a model command and shows the model as it is afterwards (the
  // host's runModelCommand); false when the model refused it.
  using Run = std::function<bool(const QJsonObject&, QJsonObject*)>;
  // Why the model refused the last command.
  using LastError = std::function<QString()>;
  ReorientDialog(Run run, LastError lastError, QWidget* parent = nullptr);
  ~ReorientDialog() override;

  // The command of the choice made (`reorient_design` with its fields).
  QJsonObject command() const;
  // The model's answer to it, once OK kept it.
  QJsonObject result() const { return m_result; }

  void accept() override;
  void reject() override;

private:
  void preview();
  void takeBack();
  void release();

  Run m_run;
  LastError m_lastError;
  QString m_token;
  bool m_turned = false;
  QJsonObject m_result;
  QRadioButton* m_aboutAxis = nullptr;
  QRadioButton* m_ontoPlane = nullptr;
  QButtonGroup* m_axes = nullptr;
  QButtonGroup* m_angles = nullptr;
  QComboBox* m_plane = nullptr;
  QLabel* m_details = nullptr;
  QDialogButtonBox* m_buttons = nullptr;
};

} // namespace mitcad
