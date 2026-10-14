// SPDX-License-Identifier: MIT
#pragma once

#include <QObject>
#include <QString>

class QEvent;

namespace mitcad::mcp {

// Live MCP never waits for the user's answer to a dialog (mitcad#265):
// while a guard lives, every modal dialog that opens is closed at once as
// if cancelled, and `opened()` names the first. A long computation's
// progress dialog (ComputeProgress) is not one of them: it stays.
class ModalGuard : public QObject {
public:
  ModalGuard();
  ~ModalGuard() override;
  ModalGuard(const ModalGuard&) = delete;
  ModalGuard& operator=(const ModalGuard&) = delete;

  // The title (else the class) of the first dialog that opened, or empty.
  const QString& opened() const { return m_opened; }

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  QString m_opened;
};

} // namespace mitcad::mcp
