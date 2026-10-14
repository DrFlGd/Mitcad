// SPDX-License-Identifier: MIT
#pragma once

#include <stdexcept>
#include <utility>

#include <QJsonObject>
#include <QString>

namespace mitcad::mcp {

// A refused live MCP call whose result carries fields besides the message
// (mitcad#265): `busy` with `retry_after_ms` while the model computes, a
// command panel's state when its OK was refused. The tool result is an
// error with these fields and `error`.
class Refusal : public std::runtime_error {
public:
  Refusal(const QString& message, QJsonObject fields)
      : std::runtime_error(message.toStdString()), m_fields(std::move(fields)) {}
  const QJsonObject& fields() const { return m_fields; }

private:
  QJsonObject m_fields;
};

} // namespace mitcad::mcp
