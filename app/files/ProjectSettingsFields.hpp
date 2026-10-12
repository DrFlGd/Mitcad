// SPDX-License-Identifier: MIT
#pragma once

// Editable project settings shared by New Project and Project Settings.
// No persistence happens here: creation writes them with the first version,
// whereas existing projects apply edits through set_project_settings.
#include <QJsonObject>
#include <QWidget>

class QCheckBox;
class QComboBox;
class QGroupBox;
class QLabel;
class QLineEdit;
class QPushButton;
class QRadioButton;
class QSpinBox;

namespace mitcad {

class ProjectSettingsFields : public QWidget {
public:
  explicit ProjectSettingsFields(QWidget* parent = nullptr);
  QJsonObject sharedChosen() const;
  QJsonObject localChosen(bool includeName = false) const;

  QLineEdit* m_displayName = nullptr;
  QGroupBox* m_everyone = nullptr;
  QCheckBox* m_locks = nullptr;
  QSpinBox* m_idle = nullptr;
  QSpinBox* m_poll = nullptr;
  QLabel* m_locksNote = nullptr;
  QRadioButton* m_liveNone = nullptr;
  QRadioButton* m_liveBroker = nullptr;
  QLineEdit* m_broker = nullptr;
  QLineEdit* m_prefix = nullptr;
  QPushButton* m_test = nullptr;
  QLineEdit* m_name = nullptr;
  QLineEdit* m_email = nullptr;
  QCheckBox* m_sendAtOnce = nullptr;
  QComboBox* m_check = nullptr;
  QComboBox* m_liveHere = nullptr;
  QLineEdit* m_hereBroker = nullptr;
  QLineEdit* m_herePrefix = nullptr;
  QWidget* m_hereRow = nullptr;
};

} // namespace mitcad
