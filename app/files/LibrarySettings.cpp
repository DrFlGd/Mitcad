// SPDX-License-Identifier: MIT
// The library sources the user chose (settings libraries/sources): plain Qt
// Core, so that the unit tests (app/tests/library_settings.cpp) use it
// without the bridge.
#include "Libraries.hpp"

#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonValue>
#include <QSettings>

namespace mitcad {
namespace {

const QString kSourcesKey = QStringLiteral("libraries/sources");

} // namespace

QList<LibrarySource> defaultLibrarySources() {
  // Where Mitcad's own repositories are published; fetched only when the
  // user asks (Auto-update is off).
  return {{QStringLiteral("https://github.com/mitcad/parts.git"), true, false},
          {QStringLiteral("https://github.com/mitcad/community-index.git"), true, false}};
}

QString normalLibraryUrl(const QString& url) {
  QString out = QDir::fromNativeSeparators(url.trimmed());
  while (out.endsWith(QLatin1Char('/'))) {
    out.chop(1);
  }
  return out;
}

LibrarySettings LibrarySettings::load() {
  LibrarySettings settings;
  QSettings store;
  if (!store.contains(kSourcesKey)) {
    settings.sources = defaultLibrarySources();
    return settings;
  }
  const QJsonArray list = QJsonDocument::fromJson(store.value(kSourcesKey).toByteArray()).array();
  for (const QJsonValue& value : list) {
    const QJsonObject source = value.toObject();
    const QString url = source.value(QStringLiteral("url")).toString().trimmed();
    if (!url.isEmpty()) {
      // Settings written before Auto-update (mitcad#576) have no key: off.
      settings.sources.append({url, source.value(QStringLiteral("enabled")).toBool(true),
                               source.value(QStringLiteral("autoUpdate")).toBool(false)});
    }
  }
  return settings;
}

void LibrarySettings::save() const {
  QJsonArray list;
  for (const LibrarySource& source : sources) {
    list.append(QJsonObject{{QStringLiteral("url"), source.url},
                            {QStringLiteral("enabled"), source.enabled},
                            {QStringLiteral("autoUpdate"), source.autoUpdate}});
  }
  QSettings().setValue(kSourcesKey, QJsonDocument(list).toJson(QJsonDocument::Compact));
}

QStringList LibrarySettings::enabledUrls() const {
  QStringList urls;
  for (const LibrarySource& source : sources) {
    if (source.enabled) {
      urls << source.url;
    }
  }
  return urls;
}

QStringList LibrarySettings::autoUpdateUrls() const {
  QStringList urls;
  for (const LibrarySource& source : sources) {
    if (source.enabled && source.autoUpdate) {
      urls << source.url;
    }
  }
  return urls;
}

bool LibrarySettings::add(const QString& url) {
  const QString trimmed = url.trimmed();
  for (const LibrarySource& source : sources) {
    if (source.url == trimmed) {
      return false;
    }
  }
  sources.append({trimmed, true, false});
  return true;
}

} // namespace mitcad
