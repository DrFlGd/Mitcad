// SPDX-License-Identifier: MIT
#include "ExtrudeDragProxy.hpp"

#include <algorithm>
#include <cmath>
#include <initializer_list>

namespace mitcad::cmd {
namespace {

// The same number, both unset (NaN) included.
bool sameValue(double a, double b) {
  return (std::isnan(a) && std::isnan(b)) || a == b;
}

bool tapered(const CommandState& state, const QString& key) {
  const double value = state.value(key);
  return std::isfinite(value) && value != 0.0;
}

} // namespace

std::optional<DragProxyRule> extrudeDragProxy(const CommandState& shown, const CommandState& now, const QString& key,
                                              double value) {
  // Every input but the dragged one as the exact preview had it: the tool
  // it computed is then the one to change.
  for (const char* id : {"distance", "distance2", "start_offset", "taper", "taper2", "thickness", "thickness2",
                         "offset", "offset2"}) {
    const QString input = QString::fromLatin1(id);
    if (input != key && !sameValue(shown.value(input), now.value(input))) {
      return std::nullopt;
    }
  }
  for (const char* id : {"type", "start", "direction", "extent", "extent2", "measurement", "wall", "wall2"}) {
    const QString input = QString::fromLatin1(id);
    if (shown.choice(input) != now.choice(input)) {
      return std::nullopt;
    }
  }
  if (shown.checked(QStringLiteral("flip")) != now.checked(QStringLiteral("flip")) ||
      !shown.items(QStringLiteral("wrap")).isEmpty() || !now.items(QStringLiteral("wrap")).isEmpty()) {
    return std::nullopt;
  }
  const QString start = shown.choice(QStringLiteral("start"));
  if (start == QStringLiteral("object")) {
    return std::nullopt; // the start follows another plane: no straight stretch
  }
  const QString direction = shown.choice(QStringLiteral("direction"));
  const bool two = direction == QStringLiteral("two_sides");
  const bool symmetric = direction == QStringLiteral("symmetric");
  const auto plain = [&shown](const QString& suffix) {
    return shown.choice(QStringLiteral("extent") + suffix) == QStringLiteral("distance") &&
           !tapered(shown, QStringLiteral("taper") + suffix) &&
           std::isfinite(shown.value(QStringLiteral("distance") + suffix));
  };
  if (!plain(QString()) || (two && !plain(QStringLiteral("2")))) {
    return std::nullopt; // to an object, through all or tapered: not a straight prism of known length
  }
  const double distance = shown.value(QStringLiteral("distance"));
  const double whole = std::abs(distance);
  double length = whole; // one side
  if (symmetric) {
    length = shown.choice(QStringLiteral("measurement")) == QStringLiteral("whole") ? whole : 2.0 * whole;
  } else if (two) {
    length = distance + shown.value(QStringLiteral("distance2"));
  }
  const double from = shown.value(key);
  if (!std::isfinite(from) || !std::isfinite(value)) {
    return std::nullopt;
  }
  DragProxyRule rule;
  rule.from = from;
  rule.to = value;
  rule.length = length;
  if (key == QStringLiteral("start_offset")) {
    if (start != QStringLiteral("offset")) {
      return std::nullopt;
    }
    rule.kind = DragProxyRule::Kind::Translate;
    return rule;
  }
  if (key == QStringLiteral("distance") && !two && !symmetric) {
    // From the start towards the arrow, or back from it when negative.
    rule.kind = DragProxyRule::Kind::Stretch;
    rule.reach = std::max(from, 0.0);
    return rule;
  }
  if (key == QStringLiteral("distance") && symmetric) {
    if (from <= 0.0 || value <= 0.0) {
      return std::nullopt;
    }
    rule.kind = DragProxyRule::Kind::Stretch;
    rule.reach = length / 2.0;
    return rule;
  }
  if (two && (key == QStringLiteral("distance") || key == QStringLiteral("distance2"))) {
    // Each side's arrow points away from the profile plane. A side dragged
    // across it (a negative distance, which ends on the other side's side,
    // mitcad#765) or shown so is left to the exact preview.
    if (from <= 0.0 || value <= 0.0 || shown.value(QStringLiteral("distance")) <= 0.0 ||
        shown.value(QStringLiteral("distance2")) <= 0.0) {
      return std::nullopt;
    }
    rule.kind = DragProxyRule::Kind::StretchSide;
    rule.reach = from;
    return rule;
  }
  return std::nullopt;
}

} // namespace mitcad::cmd
