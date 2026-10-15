// SPDX-License-Identifier: MIT
#include "TextureSource.hpp"

#include <QCoreApplication>
#include <QCryptographicHash>
#include <QDir>
#include <QLocale>
#include <QMetaObject>
#include <QSettings>
#include <QStandardPaths>

#include "FolderTextureSource.hpp"
#include "PolyHavenTextureSource.hpp"

namespace mitcad {

const QStringList& textureSetMapNames() {
  static const QStringList names = {QStringLiteral("base_color"), QStringLiteral("normal"),
                                    QStringLiteral("roughness"), QStringLiteral("metalness"),
                                    QStringLiteral("occlusion")};
  return names;
}

void TextureSource::deliver(const QPointer<QObject>& context, std::function<void()> call) {
  QCoreApplication* app = QCoreApplication::instance();
  if (context.isNull() || app == nullptr) {
    return;
  }
  // Queued: never within the call that asked, and on the GUI thread.
  QMetaObject::invokeMethod(
      app,
      [context, call = std::move(call)] {
        if (!context.isNull()) {
          call();
        }
      },
      Qt::QueuedConnection);
}

TextureSettings TextureSettings::load() {
  const QSettings settings;
  TextureSettings textures;
  for (const QString& folder : settings.value(QStringLiteral("textures/folders")).toStringList()) {
    const QString clean = QDir::cleanPath(QDir::fromNativeSeparators(folder.trimmed()));
    if (!folder.trimmed().isEmpty() && !textures.folders.contains(clean)) {
      textures.folders << clean;
    }
  }
  const QString resolution = settings.value(QStringLiteral("textures/resolution")).toString();
  if (!resolution.isEmpty()) {
    textures.resolution = resolution;
  }
  textures.allowNetwork = settings.value(QStringLiteral("textures/allowNetwork"), false).toBool();
  return textures;
}

void TextureSettings::save() const {
  QSettings settings;
  settings.setValue(QStringLiteral("textures/folders"), folders);
  settings.setValue(QStringLiteral("textures/resolution"), resolution);
  settings.setValue(QStringLiteral("textures/allowNetwork"), allowNetwork);
}

const std::vector<TextureSourceEntry>& textureSourceRegistry() {
  static const std::vector<TextureSourceEntry> registry = {
      {"folder", false,
       [](const TextureSettings& settings) -> std::unique_ptr<TextureSource> {
         return std::make_unique<FolderTextureSource>(settings.folders);
       }},
      {"polyhaven", true,
       [](const TextureSettings&) -> std::unique_ptr<TextureSource> {
         return std::make_unique<PolyHavenTextureSource>();
       }},
  };
  return registry;
}

std::vector<std::unique_ptr<TextureSource>> makeTextureSources(const TextureSettings& settings) {
  std::vector<std::unique_ptr<TextureSource>> sources;
  for (const TextureSourceEntry& entry : textureSourceRegistry()) {
    if (std::unique_ptr<TextureSource> source = entry.make(settings)) {
      sources.push_back(std::move(source));
    }
  }
  return sources;
}

QString textureCacheFolder() {
  const QString given = qEnvironmentVariable("MITCAD_TEXTURES_DIR");
  if (!given.isEmpty()) {
    return QDir::cleanPath(given);
  }
  const QString data = QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation);
  return data.isEmpty() ? QString() : data + QStringLiteral("/textures");
}

QString textureFolderName(const QString& id) {
  QString name;
  for (const QChar c : id) {
    const bool plain = c.unicode() < 128 && (c.isLetterOrNumber() || c == QLatin1Char('_') || c == QLatin1Char('-'));
    name += plain ? c : QChar(QLatin1Char('_'));
  }
  name = name.left(48);
  if (name != id || name.isEmpty() || name.startsWith(QLatin1Char('.'))) {
    // Different ids that read alike stay apart.
    name += QLatin1Char('-') +
            QString::fromLatin1(QCryptographicHash::hash(id.toUtf8(), QCryptographicHash::Sha256).toHex().left(12));
  }
  return name;
}

QString textureAssetFolder(const QString& source, const QString& asset, const QString& resolution) {
  const QString cache = textureCacheFolder();
  if (cache.isEmpty()) {
    return QString();
  }
  QString folder = cache + QLatin1Char('/') + textureFolderName(source) + QLatin1Char('/') + textureFolderName(asset);
  if (!resolution.isEmpty()) {
    folder += QLatin1Char('/') + textureFolderName(resolution);
  }
  return folder;
}

QString textureBytesText(qint64 bytes) {
  if (bytes < 0) {
    return QObject::tr("unknown size");
  }
  return QLocale::c().formattedDataSize(bytes, 1, QLocale::DataSizeTraditionalFormat);
}

} // namespace mitcad
