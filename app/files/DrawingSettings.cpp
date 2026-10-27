// SPDX-License-Identifier: MIT
#include "DrawingSettings.hpp"

#include <utility>

#include <QJsonArray>
#include <QRegularExpression>
#include <QSettings>

namespace mitcad {
namespace {

const QString kGroup = QStringLiteral("drawing");

} // namespace

QList<QPair<QString, QString>> DrawingSettings::sheets() {
  return {{QStringLiteral("A4"), QStringLiteral("A4")},
          {QStringLiteral("A3"), QStringLiteral("A3")},
          {QStringLiteral("A2"), QStringLiteral("A2")},
          {QStringLiteral("A1"), QStringLiteral("A1")},
          {QStringLiteral("A0"), QStringLiteral("A0")},
          {QStringLiteral("ANSI_A"), QStringLiteral("ANSI A (Letter)")},
          {QStringLiteral("ANSI_B"), QStringLiteral("ANSI B (Tabloid)")},
          {QStringLiteral("ANSI_C"), QStringLiteral("ANSI C")},
          {QStringLiteral("ANSI_D"), QStringLiteral("ANSI D")},
          {QStringLiteral("ANSI_E"), QStringLiteral("ANSI E")}};
}

QString normalizedScale(const QString& text) {
  const QString t = text.trimmed().toLower();
  if (t.isEmpty() || t == QStringLiteral("fit") || t.startsWith(QStringLiteral("fit "))) {
    return QStringLiteral("fit");
  }
  static const QRegularExpression ratio(QStringLiteral(R"(^(\d+(?:\.\d+)?)\s*:\s*(\d+(?:\.\d+)?)$)"));
  const QRegularExpressionMatch m = ratio.match(t);
  if (!m.hasMatch() || m.captured(1).toDouble() <= 0.0 || m.captured(2).toDouble() <= 0.0) {
    return QString();
  }
  return m.captured(1) + QLatin1Char(':') + m.captured(2);
}

DrawingSettings DrawingSettings::load(QSettings& settings) {
  DrawingSettings s;
  settings.beginGroup(kGroup);
  const QString format = settings.value(QStringLiteral("format"), s.format).toString();
  if (format == QStringLiteral("pdf") || format == QStringLiteral("svg") || format == QStringLiteral("dxf") ||
      format == QStringLiteral("dwg")) {
    s.format = format;
  }
  const QString sheet = settings.value(QStringLiteral("sheet"), s.sheet).toString();
  for (const auto& [value, label] : sheets()) {
    if (value == sheet) {
      s.sheet = sheet;
    }
  }
  const QString scale = normalizedScale(settings.value(QStringLiteral("scale"), s.scale).toString());
  if (!scale.isEmpty()) {
    s.scale = scale;
  }
  const auto flag = [&settings](const char* key, bool& value) {
    value = settings.value(QLatin1String(key), value).toBool();
  };
  flag("portrait", s.portrait);
  flag("firstAngle", s.firstAngle);
  flag("front", s.front);
  flag("top", s.top);
  flag("right", s.right);
  flag("iso", s.iso);
  flag("hiddenLines", s.hiddenLines);
  flag("tangentEdges", s.tangentEdges);
  flag("dimensions", s.dimensions);
  flag("holeTable", s.holeTable);
  flag("sketchDimensions", s.sketchDimensions);
  flag("perBody", s.perBody);
  settings.endGroup();
  return s;
}

void DrawingSettings::save(QSettings& settings) const {
  settings.beginGroup(kGroup);
  settings.setValue(QStringLiteral("format"), format);
  settings.setValue(QStringLiteral("sheet"), sheet);
  settings.setValue(QStringLiteral("scale"), scale);
  settings.setValue(QStringLiteral("portrait"), portrait);
  settings.setValue(QStringLiteral("firstAngle"), firstAngle);
  settings.setValue(QStringLiteral("front"), front);
  settings.setValue(QStringLiteral("top"), top);
  settings.setValue(QStringLiteral("right"), right);
  settings.setValue(QStringLiteral("iso"), iso);
  settings.setValue(QStringLiteral("hiddenLines"), hiddenLines);
  settings.setValue(QStringLiteral("tangentEdges"), tangentEdges);
  settings.setValue(QStringLiteral("dimensions"), dimensions);
  settings.setValue(QStringLiteral("holeTable"), holeTable);
  settings.setValue(QStringLiteral("sketchDimensions"), sketchDimensions);
  settings.setValue(QStringLiteral("perBody"), perBody);
  settings.endGroup();
}

QStringList DrawingSettings::views() const {
  QStringList out;
  const std::pair<bool, const char*> all[] = {{front, "front"}, {top, "top"}, {right, "right"}, {iso, "iso"}};
  for (const auto& [on, name] : all) {
    if (on) {
      out << QLatin1String(name);
    }
  }
  return out;
}

QJsonObject DrawingSettings::command(const QString& path, const QStringList& bodies, const QString& title,
                                     const QString& section) const {
  QJsonObject command{{QStringLiteral("cmd"), QStringLiteral("export_drawing")},
                      {QStringLiteral("path"), path},
                      {QStringLiteral("format"), format},
                      {QStringLiteral("sheet"), sheet},
                      {QStringLiteral("orientation"), portrait ? QStringLiteral("portrait") : QStringLiteral("landscape")},
                      {QStringLiteral("scale"), scale},
                      {QStringLiteral("projection"),
                       firstAngle ? QStringLiteral("first_angle") : QStringLiteral("third_angle")},
                      {QStringLiteral("views"), QJsonArray::fromStringList(views())},
                      {QStringLiteral("hidden_lines"), hiddenLines},
                      {QStringLiteral("tangent_edges"), tangentEdges},
                      {QStringLiteral("dimensions"), dimensions},
                      {QStringLiteral("hole_table"), holeTable},
                      {QStringLiteral("sketch_dimensions"), sketchDimensions},
                      {QStringLiteral("per_body"), perBody},
                      {QStringLiteral("title"), title}};
  if (!bodies.isEmpty()) {
    command.insert(QStringLiteral("bodies"), QJsonArray::fromStringList(bodies));
  }
  if (!section.isEmpty()) {
    command.insert(QStringLiteral("section"), QJsonObject{{QStringLiteral("analysis"), section}});
  }
  return command;
}

} // namespace mitcad
