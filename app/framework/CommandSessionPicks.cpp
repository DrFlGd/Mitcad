// SPDX-License-Identifier: MIT
// CommandSession: parameter names picked from the view into a value field,
// and a value field's full tooltip (mitcad#250). The rest of the session is
// in CommandSession.cpp.
#include <exception>
#include <utility>
#include <vector>

#include <QJsonArray>
#include <QJsonObject>
#include <QRegularExpression>
#include <QTimer>
#include <QtLogging>

#include "CommandSession.hpp"
#include "ParameterPicker.hpp"

namespace mitcad {
namespace {

// How long the labels stay after their field lost the keyboard: a click in
// the view takes it before the view sees the click.
constexpr int kPickGraceMs = 400;
// The features before the marker whose values are labelled, nearest first.
constexpr int kLabelledFeatures = 40;

// A number with or without its unit ("20", "-4 mm", "30 deg").
bool plainNumber(const QString& text) {
  static const QRegularExpression number(
      QStringLiteral(R"(^\s*[-+]?(\d+[.,]?\d*|[.,]\d+)([eE][-+]?\d+)?\s*(mm|cm|m|um|in|ft|deg|rad|°)?\s*$)"));
  return number.match(text).hasMatch();
}

} // namespace

void CommandSession::valueFocused(const QString& key, bool focused) {
  if (m_done) {
    return;
  }
  const auto slot = slotOf(key);
  if (!slot || slot->def->type != InputDef::Type::Value) {
    return;
  }
  if (focused) {
    m_valueKeyboard = true;
    if (!m_autoFocusing) {
      m_valueTouched = true;
    }
    const bool same = key == m_pickTarget && !m_picker.isNull();
    m_pickTarget = key;
    m_pickFocused = true;
    // The field takes the keyboard back after a name went in: the labels
    // stay as they are. Nor while the keyboard moved here by itself after a
    // pick (moveKeyboardToValues) and the selection takes more items: the
    // next click in the view is its next pick, not a label's name
    // (mitcad#467); typing in the field shows them.
    if (!(same && m_inserting) && !(m_autoFocusing && activeTakesMore())) {
      showParameterLabels();
    }
    return;
  }
  m_valueKeyboard = false;
  if (key != m_pickTarget) {
    return;
  }
  m_pickFocused = false;
  QTimer::singleShot(kPickGraceMs, this, [this] {
    if (!m_pickFocused) {
      hideParameterLabels();
    }
  });
}

bool CommandSession::activeTakesMore() const {
  const InputDef* input = activeInput();
  if (input == nullptr || input->type != InputDef::Type::Selection || input->max == 1) {
    return false;
  }
  return input->max == 0 || m_state.items(m_active).size() < input->max;
}

void CommandSession::showParameterLabels() {
  if (m_done || m_pickTarget.isEmpty()) {
    return;
  }
  if (m_host.modelBusy()) {
    m_host.whenIdle(this, [this] {
      if (!m_done && m_pickFocused) {
        showParameterLabels();
      }
    });
    return;
  }
  std::vector<ParameterPicker::Sketch> sketches;
  std::vector<ParameterPicker::Value> values;
  QHash<QString, QString> texts;
  gatherParameterLabels(m_host, kLabelledFeatures, sketches, values, texts);
  if (!m_picker) {
    auto* picker = new ParameterPicker(m_host.viewer(), m_def.name);
    m_picker = picker;
    connect(picker, &ParameterPicker::picked, this, [this](const QString& name) { insertParameter(name); });
    // Gone with the command, also when it ends while the field has the
    // keyboard.
    connect(this, &CommandSession::finished, picker, &QObject::deleteLater);
    connect(this, &QObject::destroyed, picker, &QObject::deleteLater);
  }
  m_picker->setContent(std::move(sketches), std::move(values), std::move(texts));
}

void CommandSession::hideParameterLabels() {
  m_pickTarget.clear();
  m_pickFocused = false;
  if (m_picker) {
    m_picker->hide();
    m_picker->deleteLater();
    m_picker = nullptr;
    qDebug().noquote() << QStringLiteral("Parameter labels for %1 hidden").arg(m_def.name);
  }
}

void CommandSession::insertParameter(const QString& name) {
  if (m_done || m_pickTarget.isEmpty() || m_presenter == nullptr) {
    return;
  }
  m_inserting = true;
  const bool inserted = m_presenter->insertIntoValue(m_pickTarget, name);
  m_inserting = false;
  if (!inserted) {
    qDebug().noquote() << QStringLiteral("%1: %2 not inserted, %3 is not shown").arg(m_def.name, name, m_pickTarget);
  }
}

void CommandSession::describeParameter(const QString& key, const QString& rename, const QString& text,
                                       const QString& shown) {
  const QString own = m_state.parameterName(key);
  if (own.isEmpty()) {
    return;
  }
  if (shown.isEmpty()) {
    m_state.setParameterTooltip(key, QString());
    return;
  }
  if (!m_parameterComments.contains(own)) {
    try {
      for (const QJsonValue& entry : m_host.queryArray(QStringLiteral("parameters"))) {
        const QJsonObject p = entry.toObject();
        m_parameterComments.insert(p.value(QStringLiteral("name")).toString(),
                                   p.value(QStringLiteral("comment")).toString());
      }
    } catch (const std::exception&) {
      // Without its comment then.
    }
    if (!m_parameterComments.contains(own)) {
      m_parameterComments.insert(own, QString());
    }
  }
  // Under the name it is given ("depth = 12" renames it on OK).
  const QString name = rename.isEmpty() ? own : rename;
  QString about = plainNumber(text) ? QStringLiteral("%1 = %2").arg(name, shown)
                                    : QStringLiteral("%1 = %2 → %3").arg(name, text, shown);
  if (const QString comment = m_parameterComments.value(own); !comment.isEmpty()) {
    about += QStringLiteral(" (%1)").arg(comment);
  }
  m_state.setParameterTooltip(key, about);
}

} // namespace mitcad
