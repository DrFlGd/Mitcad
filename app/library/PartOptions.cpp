// SPDX-License-Identifier: MIT
#include "PartOptions.hpp"

#include <algorithm>
#include <cmath>

#include <QJsonArray>
#include <QJsonValue>
#include <QObject>
#include <QRegularExpression>

namespace mitcad {
namespace {

constexpr double kPi = 3.14159265358979323846;

QString textOf(const QJsonObject& object, const char* key) { return object.value(QLatin1String(key)).toString(); }

double numberOf(const QJsonObject& object, const char* key) {
  const QJsonValue value = object.value(QLatin1String(key));
  return value.isDouble() ? value.toDouble() : std::numeric_limits<double>::quiet_NaN();
}

QStringList textsOf(const QJsonValue& list) {
  QStringList out;
  for (const QJsonValue& value : list.toArray()) {
    if (value.isString()) {
      out << value.toString();
    } else if (value.isObject()) {
      // A problem or a name given as an object: its words.
      const QJsonObject object = value.toObject();
      const QString text = textOf(object, "text").isEmpty() ? textOf(object, "message") : textOf(object, "text");
      out << (text.isEmpty() ? textOf(object, "name") : text);
    }
  }
  return out;
}

// A value of the answer as text: the text given, else the number in the unit.
QString shownValue(const QJsonValue& value, const QString& text, const QString& unit) {
  if (!text.isEmpty()) {
    return text;
  }
  if (value.isString()) {
    return value.toString();
  }
  if (value.isDouble()) {
    return numberText(value.toDouble(), unit);
  }
  return QString();
}

PartOption optionOf(const QJsonObject& object) {
  PartOption option;
  option.name = textOf(object, "name");
  option.text = textOf(object, "kind") == QStringLiteral("text");
  option.unit = textOf(object, "unit");
  option.expression = textOf(object, "expression");
  const QJsonValue value = object.value(QStringLiteral("value"));
  if (value.isDouble()) {
    option.value = value.toDouble();
  }
  option.valueText = shownValue(value, textOf(object, "text"), option.unit);
  if (option.valueText.isEmpty()) {
    option.valueText = valueOfExpression(option.expression);
  }
  option.parameter = textOf(object, "parameter");
  option.bound = object.value(QStringLiteral("bound")).toBool();
  // A value bound to the parent is not chosen here either.
  option.driven = object.value(QStringLiteral("driven")).toBool() || option.bound;
  option.restrictedBy = textsOf(object.value(QStringLiteral("restricted_by")));
  // What the rules allow; the domain when the answer has no `allowed`.
  QJsonObject allowed = object.value(QStringLiteral("allowed")).toObject();
  if (allowed.isEmpty()) {
    allowed = object.value(QStringLiteral("domain")).toObject();
  }
  const QString kind = textOf(allowed, "kind");
  if (kind == QStringLiteral("list")) {
    option.kind = PartOption::Kind::List;
    for (const QJsonValue& entry : allowed.value(QStringLiteral("values")).toArray()) {
      OptionValueChoice choice;
      if (entry.isObject()) {
        const QJsonObject one = entry.toObject();
        choice.expression = textOf(one, "expression");
        const QJsonValue v = one.value(QStringLiteral("value"));
        if (v.isDouble()) {
          choice.value = v.toDouble();
        }
        choice.text = shownValue(v, textOf(one, "text"), option.unit);
      } else if (entry.isString()) {
        choice.text = entry.toString();
      } else if (entry.isDouble()) {
        choice.value = entry.toDouble();
        choice.text = numberText(choice.value, option.unit);
      }
      if (choice.text.isEmpty()) {
        choice.text = valueOfExpression(choice.expression);
      }
      if (choice.expression.isEmpty()) {
        choice.expression = constantExpression(option, choice.text);
      }
      option.values.append(choice);
    }
  } else if (kind == QStringLiteral("range")) {
    option.kind = PartOption::Kind::Range;
    option.min = numberOf(allowed, "min");
    option.max = numberOf(allowed, "max");
    option.step = numberOf(allowed, "step");
    option.minText = textOf(allowed, "min_text");
    option.maxText = textOf(allowed, "max_text");
    option.stepText = textOf(allowed, "step_text");
  }
  return option;
}

} // namespace

QString PartOption::allowedText() const {
  switch (kind) {
  case Kind::List: {
    QStringList texts;
    for (const OptionValueChoice& choice : values) {
      texts << choice.text;
    }
    return texts.isEmpty() ? QObject::tr("none") : texts.join(QStringLiteral(", "));
  }
  case Kind::Range: {
    const QString low = !minText.isEmpty() ? minText : std::isfinite(min) ? numberText(min, unit) : QStringLiteral("-");
    const QString high =
        !maxText.isEmpty() ? maxText : std::isfinite(max) ? numberText(max, unit) : QStringLiteral("-");
    QString out = QStringLiteral("%1 … %2").arg(low, high);
    if (!stepText.isEmpty() || (std::isfinite(step) && step > 0.0)) {
      out += QStringLiteral(", step %1").arg(!stepText.isEmpty() ? stepText : numberText(step, unit));
    }
    return out;
  }
  case Kind::Free:
    break;
  }
  return QObject::tr("any");
}

bool PartOption::allows(const QString& candidate) const {
  if (kind == Kind::List) {
    for (const OptionValueChoice& choice : values) {
      if (choice.text == candidate || choice.expression == candidate) {
        return true;
      }
    }
    return false;
  }
  return true;
}

bool PartOption::allowsNumber(double candidate) const {
  if (!std::isfinite(candidate)) {
    return false;
  }
  if (kind == Kind::List) {
    for (const OptionValueChoice& choice : values) {
      if (std::isfinite(choice.value) && std::abs(choice.value - candidate) <= 1e-9 * std::max(1.0, std::abs(candidate))) {
        return true;
      }
    }
    return false;
  }
  if (kind != Kind::Range) {
    return true;
  }
  const double tolerance = 1e-9 * std::max(1.0, std::abs(candidate));
  if ((std::isfinite(min) && candidate < min - tolerance) || (std::isfinite(max) && candidate > max + tolerance)) {
    return false;
  }
  if (std::isfinite(step) && step > 0.0) {
    const double base = std::isfinite(min) ? min : 0.0;
    const double steps = (candidate - base) / step;
    return std::abs(steps - std::round(steps)) <= 1e-6;
  }
  return true;
}

QString OptionMove::text() const {
  QString out = QStringLiteral("%1 %2 → %3").arg(option, from, to);
  if (!rule.isEmpty()) {
    out += QStringLiteral(": ") + rule;
  }
  return out;
}

const PartOption* OptionsAnswer::option(const QString& name) const {
  for (const PartOption& one : options) {
    if (one.name == name) {
      return &one;
    }
  }
  return nullptr;
}

OptionsAnswer optionsAnswer(const QJsonObject& answer) {
  OptionsAnswer out;
  const QJsonValue error = answer.value(QStringLiteral("error"));
  if (error.isObject()) {
    out.error = textOf(error.toObject(), "message");
    if (out.error.isEmpty()) {
      out.error = QObject::tr("the options could not be read");
    }
    return out;
  }
  if (error.isString()) {
    out.error = error.toString();
    return out;
  }
  for (const QJsonValue& value : answer.value(QStringLiteral("options")).toArray()) {
    if (value.isObject()) {
      out.options.append(optionOf(value.toObject()));
    }
  }
  const QJsonValue row = answer.value(QStringLiteral("row"));
  out.row = row.isObject() ? textOf(row.toObject(), "name") : row.toString();
  for (const QJsonValue& value : answer.value(QStringLiteral("moves")).toArray()) {
    const QJsonObject move = value.toObject();
    out.moves.append({textOf(move, "option"), textOf(move, "from"), textOf(move, "to"), textOf(move, "rule")});
  }
  const QJsonValue refused = answer.value(QStringLiteral("refused"));
  if (refused.isObject()) {
    const QJsonObject why = refused.toObject();
    out.refused = textOf(why, "text");
    if (out.refused.isEmpty()) {
      out.refused = QStringList{textOf(why, "rule"), textOf(why, "message")}.join(QStringLiteral(": "));
    }
  } else if (refused.isString()) {
    out.refused = refused.toString();
  }
  out.problems = textsOf(answer.value(QStringLiteral("problems")));
  return out;
}

OptionsQuery libraryOptionsQuery(const JsonRunner& run, const QString& url, const QString& library, const QString& rev,
                                 const QString& component) {
  return [run, url, library, rev, component](const QJsonObject& values) {
    QJsonObject request{{QStringLiteral("cmd"), QStringLiteral("library_configure")},
                        {QStringLiteral("component"), component},
                        {QStringLiteral("values"), values}};
    if (!url.isEmpty()) {
      request.insert(QStringLiteral("url"), url);
    }
    if (!library.isEmpty()) {
      request.insert(QStringLiteral("id"), library);
    }
    if (!rev.isEmpty()) {
      request.insert(QStringLiteral("rev"), rev);
    }
    return optionsAnswer(run(request));
  };
}

OptionsQuery componentOptionsQuery(const JsonRunner& query, const QString& component,
                                   const std::function<QJsonValue()>& parameters) {
  return [query, component, parameters](const QJsonObject& values) {
    QJsonObject request{{QStringLiteral("query"), QStringLiteral("configure")},
                        {QStringLiteral("component"), component},
                        {QStringLiteral("values"), values}};
    const QJsonValue expressions = parameters ? parameters() : QJsonValue();
    if (expressions.isObject()) {
      request.insert(QStringLiteral("parameters"), expressions);
    }
    return optionsAnswer(query(request));
  };
}

QJsonObject setComponentParametersCommand(const QString& component, const QJsonObject& parameters) {
  return {{QStringLiteral("cmd"), QStringLiteral("set_component_parameters")},
          {QStringLiteral("component"), component},
          {QStringLiteral("parameters"), parameters}};
}

bool isOpenTable(const QJsonObject& configurations) {
  // library_show gives `selectors` beside `options` for the pickers of
  // before mitcad#798; a table with `selectors` alone is of the old form,
  // closed.
  return !configurations.value(QStringLiteral("options")).toArray().isEmpty() &&
         !configurations.value(QStringLiteral("closed")).toBool();
}

QStringList tableOptions(const QJsonObject& configurations) {
  QStringList out;
  const QJsonValue options = configurations.value(QStringLiteral("options"));
  for (const QJsonValue& value : (options.isArray() ? options : configurations.value(QStringLiteral("selectors"))).toArray()) {
    if (!value.toString().isEmpty()) {
      out << value.toString();
    }
  }
  return out;
}

QString rowOptionText(const QJsonObject& row, const QString& option) {
  const QJsonValue value = row.value(QStringLiteral("values")).toObject().value(option);
  if (value.isString()) {
    return valueOfExpression(value.toString());
  }
  if (value.isDouble()) {
    return QString::number(value.toDouble(), 'g', 12);
  }
  return row.value(QStringLiteral("select")).toObject().value(option).toString();
}

QJsonObject rowOptionValues(const QJsonObject& configurations, const QJsonObject& row) {
  QJsonObject out;
  for (const QString& option : tableOptions(configurations)) {
    const QString text = rowOptionText(row, option);
    if (!text.isEmpty()) {
      out.insert(option, text);
    }
  }
  return out;
}

QString valueOfExpression(const QString& expression) {
  const QString trimmed = expression.trimmed();
  if (trimmed.size() >= 2 && trimmed.startsWith(QLatin1Char('\'')) && trimmed.endsWith(QLatin1Char('\''))) {
    QString inner = trimmed.mid(1, trimmed.size() - 2);
    inner.replace(QStringLiteral("''"), QStringLiteral("'"));
    return inner;
  }
  return trimmed;
}

bool isConstantExpression(const QString& expression) {
  const QString trimmed = expression.trimmed();
  if (trimmed.size() >= 2 && trimmed.startsWith(QLatin1Char('\'')) && trimmed.endsWith(QLatin1Char('\''))) {
    // One quoted text: no quote inside but doubled ones.
    QString inner = trimmed.mid(1, trimmed.size() - 2);
    inner.remove(QStringLiteral("''"));
    return !inner.contains(QLatin1Char('\''));
  }
  static const QRegularExpression number(
      QStringLiteral("^[+-]?(\\d+([.,]\\d*)?|[.,]\\d+)([eE][+-]?\\d+)?\\s*([A-Za-z]+)?$"));
  return number.match(trimmed).hasMatch();
}

QString constantExpression(const PartOption& option, const QString& text) {
  if (option.text) {
    QString quoted = valueOfExpression(text);
    quoted.replace(QLatin1Char('\''), QStringLiteral("''"));
    return QLatin1Char('\'') + quoted + QLatin1Char('\'');
  }
  const QString trimmed = text.trimmed();
  bool bare = false;
  trimmed.toDouble(&bare);
  if (bare && !option.unit.isEmpty()) {
    return trimmed + QLatin1Char(' ') + option.unit;
  }
  return trimmed;
}

QJsonValue queryValue(const PartOption& option, const QString& text) {
  if (option.text) {
    return valueOfExpression(text);
  }
  return constantExpression(option, text);
}

double unitFactor(const QString& unit) {
  if (unit == QStringLiteral("cm")) {
    return 10.0;
  }
  if (unit == QStringLiteral("m")) {
    return 1000.0;
  }
  if (unit == QStringLiteral("in")) {
    return 25.4;
  }
  if (unit == QStringLiteral("ft")) {
    return 304.8;
  }
  if (unit == QStringLiteral("um")) {
    return 0.001;
  }
  if (unit == QStringLiteral("deg")) {
    return kPi / 180.0;
  }
  return 1.0;
}

double constantNumber(const QString& expression, const QString& unit) {
  static const QRegularExpression number(
      QStringLiteral("^\\s*([+-]?(?:\\d+(?:[.,]\\d*)?|[.,]\\d+)(?:[eE][+-]?\\d+)?)\\s*([A-Za-z]*)\\s*$"));
  const QRegularExpressionMatch match = number.match(expression);
  if (!match.hasMatch()) {
    return std::numeric_limits<double>::quiet_NaN();
  }
  QString digits = match.captured(1);
  digits.replace(QLatin1Char(','), QLatin1Char('.'));
  bool ok = false;
  const double value = digits.toDouble(&ok);
  if (!ok) {
    return std::numeric_limits<double>::quiet_NaN();
  }
  const QString written = match.captured(2);
  return value * unitFactor(written.isEmpty() ? unit : written);
}

bool sameValue(const PartOption& option, const QJsonValue& value) {
  if (option.text) {
    return value.toString() == option.valueText;
  }
  const double number = value.isDouble() ? value.toDouble() : constantNumber(value.toString(), option.unit);
  return std::isfinite(number) && std::isfinite(option.value) &&
         std::abs(number - option.value) <= 1e-9 * std::max(1.0, std::abs(number));
}

QString numberText(double value, const QString& unit) {
  if (!std::isfinite(value)) {
    return QString();
  }
  double shown = value / unitFactor(unit);
  // Rounding noise of the unit's conversion away: 25.4 mm is 1 in.
  const double rounded = std::round(shown * 1e9) / 1e9;
  shown = std::abs(rounded) < 1e15 ? rounded : shown;
  const QString number = QString::number(shown, 'g', 12);
  return unit.isEmpty() ? number : QStringLiteral("%1 %2").arg(number, unit);
}

QString optionsSummary(const OptionsAnswer& answer) {
  QStringList parts;
  for (const PartOption& option : answer.options) {
    parts << QStringLiteral("%1 = %2%3").arg(option.name, option.valueText,
                                             option.driven ? QStringLiteral(" (driven)") : QString());
  }
  return QStringLiteral("%1 (%2)").arg(parts.join(QStringLiteral(", ")),
                                       answer.row.isEmpty() ? QStringLiteral("no row")
                                                            : QStringLiteral("row %1").arg(answer.row));
}

} // namespace mitcad
