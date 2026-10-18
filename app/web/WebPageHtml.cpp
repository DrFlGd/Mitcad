// SPDX-License-Identifier: MIT
// The page's HTML from the viewer's files and the design's data
// (web/WebPage.hpp): Qt Core only, so that the unit tests build it.
#include "web/WebPage.hpp"

#include <algorithm>
#include <array>

#include <QFile>
#include <QIODevice>
#include <QJsonDocument>
#include <QList>
#include <QObject>
#include <QRegularExpression>

namespace mitcad::web {

namespace {

const char* const kTemplate = "app/web/viewer/template.html";
const char* const kModules = "app/web/viewer/modules.txt";
const char* const kViewer = "app/web/viewer/viewer.js";

// A script's text inside a <script> element: "</script" would end the
// element and "<!--" changes how the HTML parser reads it; written with a
// backslash, they mean the same in JavaScript's strings and regular
// expressions, where they can occur.
QByteArray scriptText(const QByteArray& source) {
  QString text = QString::fromUtf8(source);
  static const QRegularExpression end(QStringLiteral("</(script)"), QRegularExpression::CaseInsensitiveOption);
  text.replace(end, QStringLiteral("<\\/\\1"));
  text.replace(QStringLiteral("<!--"), QStringLiteral("<\\!--"));
  return text.toUtf8();
}

// JSON inside a <script> element: '<' only occurs in its strings, where
// < is the same character.
QByteArray jsonText(const QJsonObject& object) {
  QByteArray json = QJsonDocument(object).toJson(QJsonDocument::Compact);
  json.replace("<", "\\u003c");
  return json;
}

} // namespace

QByteArray pageHtmlFrom(const QJsonObject& data, const QString& root, QString* error) {
  const auto fail = [error](const QString& why) {
    if (error != nullptr) {
      *error = why;
    }
    return QByteArray();
  };
  QString missing;
  const auto read = [&root, &missing](const QString& path, QByteArray& bytes) {
    QFile file(root + QLatin1Char('/') + path);
    if (!file.open(QIODevice::ReadOnly)) {
      missing = QObject::tr("the web viewer's file %1 is missing from this build (%2)").arg(path, file.fileName());
      return false;
    }
    bytes = file.readAll();
    return true;
  };
  QByteArray page;
  QByteArray modules;
  QByteArray viewer;
  if (!read(QLatin1String(kTemplate), page) || !read(QLatin1String(kModules), modules) ||
      !read(QLatin1String(kViewer), viewer)) {
    return fail(missing);
  }

  // The import map: every module as a data: URL.
  QJsonObject imports;
  int number = 0;
  for (const QByteArray& raw : modules.split('\n')) {
    ++number;
    const QByteArray line = raw.simplified();
    if (line.isEmpty() || line.startsWith('#')) {
      continue;
    }
    const QList<QByteArray> parts = line.split(' ');
    if (parts.size() != 2) {
      return fail(QObject::tr("line %1 of the web viewer's %2 is not \"<specifier> <path>\"")
                      .arg(number)
                      .arg(QLatin1String(kModules)));
    }
    QByteArray module;
    if (!read(QString::fromUtf8(parts.at(1)), module)) {
      return fail(missing);
    }
    imports.insert(QString::fromUtf8(parts.at(0)),
                   QString::fromLatin1("data:text/javascript;base64," + module.toBase64()));
  }

  struct Slot {
    QByteArray placeholder;
    QByteArray content;
    qsizetype at = -1;
  };
  std::array<Slot, 3> inserts{
      Slot{"<!--MITCAD:IMPORTMAP-->",
           "<script type=\"importmap\">" + jsonText(QJsonObject{{QStringLiteral("imports"), imports}}) + "</script>"},
      Slot{"<!--MITCAD:VIEWER-->", "<script type=\"module\">\n" + scriptText(viewer) + "\n</script>"},
      Slot{"<!--MITCAD:DATA-->",
           "<script id=\"mitcad-data\" type=\"application/json\">" + jsonText(data) + "</script>"}};
  for (Slot& slot : inserts) {
    slot.at = page.indexOf(slot.placeholder);
    if (slot.at < 0) {
      return fail(QObject::tr("the web viewer's %1 has no %2")
                      .arg(QLatin1String(kTemplate), QString::fromLatin1(slot.placeholder)));
    }
  }
  std::sort(inserts.begin(), inserts.end(), [](const Slot& a, const Slot& b) { return a.at < b.at; });

  // The template's own text between them, with the title; the inserted
  // parts are taken as they are.
  const QByteArray title = data.value(QStringLiteral("title")).toString().toHtmlEscaped().toUtf8();
  const auto own = [&title](QByteArray part) { return part.replace("{{TITLE}}", title); };
  QByteArray html;
  html.reserve(page.size() + inserts[0].content.size() + inserts[1].content.size() + inserts[2].content.size());
  qsizetype from = 0;
  for (const Slot& slot : inserts) {
    if (slot.at < from) {
      return fail(QObject::tr("the web viewer's %1 has overlapping placeholders").arg(QLatin1String(kTemplate)));
    }
    html += own(page.mid(from, slot.at - from));
    html += slot.content;
    from = slot.at + slot.placeholder.size();
  }
  html += own(page.mid(from));
  return html;
}

QByteArray pageHtml(const QJsonObject& data, QString* error) {
  return pageHtmlFrom(data, QStringLiteral(":/web-viewer"), error);
}

} // namespace mitcad::web
