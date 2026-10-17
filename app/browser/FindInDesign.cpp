// SPDX-License-Identifier: MIT
#include "FindInDesign.hpp"

#include <functional>

namespace mitcad {

QString FindResult::key() const {
  return QStringLiteral("%1|%2|%3").arg(kind == Kind::Body ? 'b' : 'c').arg(uid, occurrence);
}

NameMatcher::NameMatcher(const QString& text) : m_text(text.trimmed()) {
  m_wildcards = m_text.contains(QLatin1Char('*')) || m_text.contains(QLatin1Char('?'));
  if (!m_wildcards) {
    return;
  }
  // Anywhere in the name, as the plain text is.
  QString pattern;
  for (const QChar c : std::as_const(m_text)) {
    if (c == QLatin1Char('*')) {
      pattern += QStringLiteral(".*");
    } else if (c == QLatin1Char('?')) {
      pattern += QLatin1Char('.');
    } else {
      pattern += QRegularExpression::escape(QString(c));
    }
  }
  m_pattern = QRegularExpression(pattern, QRegularExpression::CaseInsensitiveOption |
                                              QRegularExpression::DotMatchesEverythingOption);
}

bool NameMatcher::matches(const QString& name) const {
  if (m_text.isEmpty()) {
    return true;
  }
  return m_wildcards ? m_pattern.match(name).hasMatch() : name.contains(m_text, Qt::CaseInsensitive);
}

QVector<FindResult> findInDesign(const DocumentSnapshot& snapshot, const FindOptions& options) {
  const NameMatcher matcher(options.text);
  QVector<FindResult> found;
  const auto wanted = [&options](bool visible) {
    return options.visibility == FindOptions::Visibility::Any ||
           (options.visibility == FindOptions::Visibility::Visible) == visible;
  };
  const QString rootName = snapshot.components.isEmpty() ? QStringLiteral("Root") : snapshot.components.first().name;

  // The bodies of a component at one place.
  const auto bodiesOf = [&](const QString& component, const QString& occurrence, const QString& where, bool shown) {
    if (!options.bodies) {
      return;
    }
    const DocumentSnapshot::Component* def = snapshot.component(component);
    for (const QString& uid : def != nullptr ? def->bodies : QStringList()) {
      const DocumentSnapshot::Body body = snapshot.bodies.value(uid);
      const QString name = body.name.isEmpty() ? uid : body.name;
      const bool visible = shown && body.visible;
      if (!wanted(visible) || !matcher.matches(name)) {
        continue;
      }
      FindResult result;
      result.kind = FindResult::Kind::Body;
      result.name = name;
      result.uid = uid;
      result.occurrence = occurrence;
      result.component = component;
      result.path = where;
      result.visible = visible;
      found.append(result);
    }
  };

  const QString root = snapshot.components.isEmpty() ? QStringLiteral("C0") : snapshot.components.first().uid;
  bodiesOf(root, QString(), rootName, true);

  std::function<void(const QVector<DocumentSnapshot::Occurrence>&, const QString&, bool)> walk =
      [&](const QVector<DocumentSnapshot::Occurrence>& level, const QString& where, bool parentShown) {
        for (const DocumentSnapshot::Occurrence& occurrence : level) {
          const bool shown = parentShown && occurrence.visible;
          if (options.components && wanted(shown) && matcher.matches(occurrence.name)) {
            FindResult result;
            result.kind = FindResult::Kind::Component;
            result.name = occurrence.name;
            result.uid = occurrence.component;
            result.occurrence = occurrence.path;
            result.component = occurrence.component;
            result.path = where;
            result.visible = shown;
            found.append(result);
          }
          const QString here = where + QLatin1Char('/') + occurrence.name;
          bodiesOf(occurrence.component, occurrence.path, here, shown);
          walk(occurrence.children, here, shown);
        }
      };
  walk(snapshot.occurrences, rootName, true);
  return found;
}

} // namespace mitcad
