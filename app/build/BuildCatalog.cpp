// SPDX-License-Identifier: MIT
#include "BuildCatalog.hpp"

#include <cmath>
#include <exception>
#include <limits>

#include <QJsonDocument>
#include <QObject>
#include <QRegularExpression>
#include <QSet>
#include <QtLogging>

#include "../files/Libraries.hpp"
#include "../framework/Command.hpp"
#include "../framework/Json.hpp"

namespace mitcad::build {
namespace {

QString text(const QJsonObject& object, const char* key) { return object.value(QLatin1String(key)).toString(); }

// A component of `library_search` or `library_build_candidates` in each of
// its configuration rows (the default first when it has none).
QVector<Part> partsOf(const QJsonObject& component) {
  Part part;
  // library_search: the library's id beside the component's fields; a
  // recorded part (library_parts): the record as `library`.
  if (component.value(QStringLiteral("library")).isObject()) {
    const QJsonObject record = component.value(QStringLiteral("library")).toObject();
    part.library = text(record, "library");
    part.url = text(record, "url");
    part.rev = text(record, "rev");
    part.component = text(record, "component");
    part.version = text(record, "label");
    // A source's row is the entry's own (library_parts).
    part.config = text(component, "config").isEmpty() ? text(record, "config") : text(component, "config");
  } else {
    part.library = text(component, "library");
    part.url = text(component, "url");
    part.rev = text(component, "rev");
    part.component = text(component, "id");
    part.version = text(component, "version");
  }
  if (part.component.isEmpty()) {
    part.component = text(component, "component");
  }
  part.name = text(component, "name");
  part.designation = text(component, "designation");
  part.supplier = text(component, "supplier");
  part.partNumber = text(component, "part_number");
  part.supplierUrl = text(component, "supplier_url");
  part.build = component.value(QStringLiteral("build")).toObject();
  if (part.name.isEmpty()) {
    part.name = part.designation.isEmpty() ? part.component : part.designation;
  }
  QVector<Part> parts;
  const QJsonArray rows =
      component.value(QStringLiteral("configurations")).toObject().value(QStringLiteral("rows")).toArray();
  if (rows.isEmpty()) {
    parts.append(part);
    return parts;
  }
  for (const QJsonValue& value : rows) {
    const QJsonObject row = value.toObject();
    Part one = part;
    one.config = text(row, "name");
    // A row's own part number and designation come first (docs/libraries.md).
    if (!text(row, "part_number").isEmpty()) {
      one.partNumber = text(row, "part_number");
    }
    if (!text(row, "designation").isEmpty()) {
      one.designation = text(row, "designation");
    }
    parts.append(one);
  }
  return parts;
}

QString keyOf(const Part& part) {
  return QStringLiteral("%1|%2|%3").arg(part.library, part.component, part.config);
}

QString junctionKey(const QJsonObject& junction) {
  return text(junction, "id") + QLatin1Char('|') +
         QString::fromUtf8(QJsonDocument(junction.value(QStringLiteral("systems")).toArray()).toJson(QJsonDocument::Compact));
}

// The part's ids by which a source of the design is found.
QString libraryComponent(const Part& part) { return part.library + QLatin1Char('|') + part.component; }

QString lowered(const QStringList& words) { return words.join(QLatin1Char(' ')).toLower(); }

} // namespace

double millimetres(const QJsonValue& value) {
  if (value.isDouble()) {
    return value.toDouble();
  }
  static const QRegularExpression number(QStringLiteral(R"(^\s*([-+]?[0-9]*[.,]?[0-9]+(?:[eE][-+]?[0-9]+)?)\s*(mm|cm|m|in)?\s*$)"));
  const QRegularExpressionMatch match = number.match(value.toString());
  if (!match.hasMatch()) {
    return std::numeric_limits<double>::quiet_NaN();
  }
  double amount = QString(match.captured(1)).replace(QLatin1Char(','), QLatin1Char('.')).toDouble();
  const QString unit = match.captured(2);
  if (unit == QStringLiteral("cm")) {
    amount *= 10.0;
  } else if (unit == QStringLiteral("m")) {
    amount *= 1000.0;
  } else if (unit == QStringLiteral("in")) {
    amount *= 25.4;
  }
  return amount;
}

QString Part::role() const { return text(build, "role"); }

QString Part::systemText() const {
  const auto one = [](const QJsonObject& system) {
    const QString type = text(system, "type");
    const int slot = system.value(QStringLiteral("slot")).toInt();
    const int grid = system.value(QStringLiteral("grid")).toInt();
    QString shown = type;
    if (slot > 0) {
      shown += QString::number(slot);
    }
    if (grid > 0) {
      shown += QStringLiteral(" ") + QString::number(grid);
    }
    return shown.trimmed();
  };
  if (build.value(QStringLiteral("system")).isObject()) {
    return one(build.value(QStringLiteral("system")).toObject());
  }
  QStringList systems;
  for (const QJsonValue& value : build.value(QStringLiteral("systems")).toArray()) {
    const QString shown = one(value.toObject());
    if (!shown.isEmpty() && !systems.contains(shown)) {
      systems << shown;
    }
  }
  return systems.join(QStringLiteral(", "));
}

QString Part::sizeText() const {
  const double width = sectionWidth();
  const double height = sectionHeight();
  if (!(width > 0.0) || !(height > 0.0)) {
    return QString();
  }
  return QStringLiteral("%1x%2").arg(width, 0, 'g', 6).arg(height, 0, 'g', 6);
}

QString Part::slotsText() const {
  int slotCount = 0;
  for (const QJsonValue& value : build.value(QStringLiteral("frames")).toArray()) {
    if (text(value.toObject(), "kind") == QStringLiteral("slot")) {
      ++slotCount;
    }
  }
  if (slotCount == 0) {
    return QString();
  }
  return slotCount == 1 ? QObject::tr("1 slot") : QObject::tr("%1 slots").arg(slotCount);
}

QString Part::label() const {
  QStringList about;
  const QString system = systemText();
  if (!system.isEmpty()) {
    about << system;
  }
  if (!slotsText().isEmpty()) {
    about << slotsText();
  }
  if (!config.isEmpty()) {
    about << config;
  }
  QString shown = name;
  if (!about.isEmpty()) {
    shown += QStringLiteral(" - ") + about.join(QStringLiteral(", "));
  }
  const QString seller = QStringList{supplier, partNumber}.join(QLatin1Char(' ')).trimmed();
  if (!seller.isEmpty()) {
    shown += QStringLiteral(" - ") + seller;
  }
  return shown;
}

QJsonObject Part::ref() const {
  QJsonObject part;
  if (!source.isEmpty()) {
    part.insert(QStringLiteral("source"), source);
  } else {
    part.insert(QStringLiteral("library"), library);
    part.insert(QStringLiteral("url"), url);
    part.insert(QStringLiteral("rev"), rev);
    part.insert(QStringLiteral("component"), component);
  }
  if (!config.isEmpty()) {
    part.insert(QStringLiteral("config"), config);
  }
  return part;
}

bool Part::matches(const QString& words) const {
  const QString haystack = lowered({name, designation, component, config, supplier, partNumber, systemText(),
                                    sizeText(), slotsText(), library, role()});
  for (const QString& word : words.toLower().split(QRegularExpression(QStringLiteral("\\s+")), Qt::SkipEmptyParts)) {
    if (!haystack.contains(word)) {
      return false;
    }
  }
  return true;
}

bool Part::cutToLength() const { return build.value(QStringLiteral("length")).isObject(); }

double Part::defaultLength() const {
  // The length block's `default`, else its shortest stock length (mitcad#394).
  const QJsonObject length = build.value(QStringLiteral("length")).toObject();
  double shown = millimetres(length.value(QStringLiteral("default")));
  if (!std::isfinite(shown) || shown <= 0.0) {
    shown = std::numeric_limits<double>::quiet_NaN();
    for (const QJsonValue& stock : length.value(QStringLiteral("stock")).toArray()) {
      const double mm = millimetres(stock);
      if (std::isfinite(mm) && mm > 0.0 && !(mm >= shown)) {
        shown = mm;
      }
    }
  }
  return std::isfinite(shown) && shown > 0.0 ? shown : 500.0;
}

double Part::sectionWidth() const {
  const double width = millimetres(build.value(QStringLiteral("section")).toObject().value(QStringLiteral("width")));
  return std::isfinite(width) ? width : 0.0;
}

double Part::sectionHeight() const {
  const double height = millimetres(build.value(QStringLiteral("section")).toObject().value(QStringLiteral("height")));
  return std::isfinite(height) ? height : 0.0;
}

void Catalog::refresh(const QString& role, const CommandContext& context) {
  // The design's sources (commands.md, "Sources"): placed again by their id.
  QHash<QString, QString> sources;
  QVector<Part> recorded;
  try {
    const QJsonObject answer = context.queryObject({{QStringLiteral("query"), QStringLiteral("library_parts")}});
    for (const QJsonValue& value : answer.value(QStringLiteral("parts")).toArray()) {
      const QJsonObject entry = value.toObject();
      const QString source = text(entry, "source");
      if (source.isEmpty()) {
        continue;
      }
      for (Part part : partsOf(entry)) {
        part.source = source;
        sources.insert(libraryComponent(part), source);
        if (part.role() == role) {
          part.key = QStringLiteral("source:%1|%2").arg(source, part.config);
          recorded.append(part);
        }
      }
    }
  } catch (const std::exception& error) {
    qWarning().noquote() << QStringLiteral("BUILD parts: the design's sources not read: %1").arg(errorText(error));
  }

  // The fetched libraries' parts of the role.
  QVector<Part> fetched;
  QString problem;
  QJsonObject answer = libraryCommand({{QStringLiteral("cmd"), QStringLiteral("library_build_candidates")},
                                       {QStringLiteral("role"), role}});
  if (answer.contains(QStringLiteral("error"))) {
    // As Insert from Library searches, filtered by the role.
    answer = libraryCommand({{QStringLiteral("cmd"), QStringLiteral("library_search")}, {QStringLiteral("role"), role}});
  }
  if (answer.contains(QStringLiteral("error"))) {
    problem = answer.value(QStringLiteral("error")).toObject().value(QStringLiteral("message")).toString();
  }
  for (const QJsonValue& value : answer.value(QStringLiteral("components")).toArray()) {
    for (Part part : partsOf(value.toObject())) {
      if (!part.build.isEmpty() && part.role() != role) {
        continue;
      }
      part.source = sources.value(libraryComponent(part));
      part.key = keyOf(part);
      fetched.append(part);
    }
  }
  // A source of the design comes once: as the fetched part that names it,
  // else on its own (its library not fetched here).
  QVector<Part> all = fetched;
  QSet<QString> named;
  for (const Part& part : fetched) {
    if (!part.source.isEmpty()) {
      named.insert(part.source + QLatin1Char('|') + part.config);
    }
  }
  for (const Part& part : recorded) {
    if (!named.contains(part.source + QLatin1Char('|') + part.config)) {
      all.prepend(part);
    }
  }
  qInfo().noquote() << QStringLiteral("BUILD parts %1: %2 (%3 in the design)")
                           .arg(role)
                           .arg(all.size())
                           .arg(recorded.size());
  const std::lock_guard<std::mutex> lock(m_mutex);
  m_byRole.insert(role, all);
  m_problems.insert(role, problem);
  m_sources = sources;
  m_byJunction.clear();
  m_refused.clear();
}

QVector<Part> Catalog::parts(const QString& role) const {
  const std::lock_guard<std::mutex> lock(m_mutex);
  return m_byRole.value(role);
}

bool Catalog::find(const QString& key, Part& part) const {
  const std::lock_guard<std::mutex> lock(m_mutex);
  for (const QVector<Part>& parts : m_byRole) {
    for (const Part& one : parts) {
      if (one.key == key) {
        part = one;
        return true;
      }
    }
  }
  for (const QVector<Part>& parts : m_byJunction) {
    for (const Part& one : parts) {
      if (one.key == key) {
        part = one;
        return true;
      }
    }
  }
  return false;
}

QString Catalog::problem(const QString& role) const {
  const std::lock_guard<std::mutex> lock(m_mutex);
  return m_problems.value(role);
}

QVector<Part> Catalog::forJunction(const QJsonObject& junction, QStringList* refused) {
  const QString key = junctionKey(junction);
  {
    const std::lock_guard<std::mutex> lock(m_mutex);
    const auto found = m_byJunction.constFind(key);
    if (found != m_byJunction.cend()) {
      if (refused != nullptr) {
        *refused = m_refused.value(key);
      }
      return found.value();
    }
  }
  QHash<QString, QString> sources;
  {
    const std::lock_guard<std::mutex> lock(m_mutex);
    sources = m_sources;
  }
  const QJsonObject answer = libraryCommand({{QStringLiteral("cmd"), QStringLiteral("library_build_candidates")},
                                             {QStringLiteral("junction"), junction}});
  QVector<Part> parts;
  QStringList nearly;
  if (answer.contains(QStringLiteral("error"))) {
    nearly << answer.value(QStringLiteral("error")).toObject().value(QStringLiteral("message")).toString();
  }
  for (const QJsonValue& value : answer.value(QStringLiteral("components")).toArray()) {
    for (Part part : partsOf(value.toObject())) {
      part.source = sources.value(libraryComponent(part));
      part.key = keyOf(part);
      parts.append(part);
    }
  }
  for (const QJsonValue& value : answer.value(QStringLiteral("refused")).toArray()) {
    const QJsonObject entry = value.toObject();
    // {id, name, library, reason}
    const QString name = text(entry, "name").isEmpty() ? text(entry, "id") : text(entry, "name");
    nearly << QStringLiteral("%1 (%2): %3").arg(name, text(entry, "library"), text(entry, "reason"));
  }
  qInfo().noquote() << QStringLiteral("BUILD connectors for junction %1 (%2): %3 fit, %4 nearly")
                           .arg(text(junction, "id"), text(junction, "kind"))
                           .arg(parts.size())
                           .arg(nearly.size());
  const std::lock_guard<std::mutex> lock(m_mutex);
  m_byJunction.insert(key, parts);
  m_refused.insert(key, nearly);
  if (refused != nullptr) {
    *refused = nearly;
  }
  return parts;
}

Choices partChoices(const QVector<Part>& parts, const QString& words, const QString& system, const QString& current) {
  Choices choices;
  for (const Part& part : parts) {
    const bool chosen = part.key == current;
    if (chosen || (part.matches(words) && (system.isEmpty() || part.systemText().contains(system)))) {
      choices.append({part.key, part.label()});
    }
  }
  return choices;
}

Choices systemChoices(const QVector<Part>& parts) {
  Choices choices{{QString(), QObject::tr("All systems")}};
  QStringList seen;
  for (const Part& part : parts) {
    const QString system = part.systemText();
    if (!system.isEmpty() && !system.contains(QLatin1Char(',')) && !seen.contains(system)) {
      seen << system;
    }
  }
  seen.sort();
  for (const QString& system : seen) {
    choices.append({system, system});
  }
  return choices;
}

} // namespace mitcad::build
