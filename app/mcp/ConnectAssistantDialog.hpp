// SPDX-License-Identifier: MIT
#pragma once

#include "AssistantSetup.hpp"

#include <functional>
#include <QDialog>
#include <QStringList>

class QCheckBox;
class QComboBox;
class QLabel;
class QLineEdit;
class QPlainTextEdit;
class QProcess;
class QPushButton;

namespace mitcad::mcp {

class ConnectAssistantDialog : public QDialog {
public:
  struct Host {
    std::function<QStringList()> clients;
    std::function<void()> disconnect;
    QString suggestedFolder;
  };
  explicit ConnectAssistantDialog(Host host, QWidget* parent = nullptr);

private:
  AssistantClient client() const;
  void updateClient();
  void refreshConnection();
  void setup();
  void setupCodex();
  void setupClaudeCode();
  void setupBundle();
  void grantFolder();
  void message(const QString& text, bool error = false);
  void runRegistration(const QString& program, const QStringList& arguments,
                       std::function<void()> success);
  QString configuredBridge() const;

  Host m_host;
  QString m_bridge;
  QComboBox* m_client = nullptr;
  QLineEdit* m_config = nullptr;
  QLineEdit* m_folder = nullptr;
  QLabel* m_state = nullptr;
  QLabel* m_details = nullptr;
  QLabel* m_result = nullptr;
  QPlainTextEdit* m_fallback = nullptr;
  QPushButton* m_setup = nullptr;
  QPushButton* m_disconnect = nullptr;
  QProcess* m_process = nullptr;
};

} // namespace mitcad::mcp
