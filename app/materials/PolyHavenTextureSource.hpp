// SPDX-License-Identifier: MIT
#pragma once

// Poly Haven (https://polyhaven.com) as a texture source (mitcad#313): its
// CC0 textures through its public API (https://api.polyhaven.com, no key).
// The API's terms ask for a User-Agent naming the software (the fetcher's
// `Mitcad/<version>`), for the content to be shown as Poly Haven's (the
// source's name in the selector and the attribution line under the
// results; nothing suggests Poly Haven makes or endorses Mitcad), and for
// no load that degrades the service: the list of textures, each texture's
// list of files and the thumbnails are kept in the texture cache, a
// texture's files are asked for only when it is chosen, and only the maps
// of the chosen resolution are downloaded. The API may change or stop
// without notice: every failure is a sentence in the browser, and the
// other sources keep working.
//
//   GET /assets?t=textures   every texture: name, tags, categories,
//                            dimensions (mm), max_resolution,
//                            thumbnail_url, files_hash, download_count
//   GET /files/<id>          per map (Diffuse, nor_gl, Rough, Metal, AO,
//                            ...), per resolution (1k, 2k, ...), per format
//                            (jpg, png, exr): url, size, md5
//
// JPEG maps are taken where there are (smaller design files), else PNG.

#include <functional>
#include <memory>
#include <vector>

#include <QByteArray>
#include <QHash>
#include <QJsonObject>
#include <QMap>
#include <QString>
#include <QStringList>
#include <QVector>

#include "TextureFetcher.hpp"
#include "TextureSource.hpp"

namespace mitcad {

// The answer of `/assets?t=textures`, read: the assets by popularity, the
// categories (shown names, sorted), each asset's files_hash; an error (a
// sentence) when it is no such list.
struct PolyHavenList {
  QVector<TextureAsset> assets;
  QStringList categories;
  QHash<QString, QString> filesHash;
  QString error;
};
PolyHavenList parsePolyHavenAssets(const QByteArray& json);

// One map image of a resolution in `/files/<id>`.
struct PolyHavenFile {
  QString map; // a name of textureSetMapNames()
  QString url;
  qint64 size = -1;
  QString md5; // lower-case hex, empty when not given
};
// The answer of `/files/<id>`, read: resolution -> the maps' files (those
// with a base colour image only), the resolutions smallest first; an
// error (a sentence) when it is no such list.
struct PolyHavenFiles {
  QMap<QString, QVector<PolyHavenFile>> byResolution;
  QStringList resolutions;
  QString error;
};
PolyHavenFiles parsePolyHavenFiles(const QByteArray& json);

class PolyHavenTextureSource : public TextureSource {
  Q_OBJECT

public:
  // `fetcher` null: makeTextureFetcher().
  explicit PolyHavenTextureSource(std::unique_ptr<TextureFetcher> fetcher = nullptr, QObject* parent = nullptr);
  ~PolyHavenTextureSource() override;

  QString id() const override { return QStringLiteral("polyhaven"); }
  QString name() const override { return QStringLiteral("Poly Haven"); }
  QString attribution() const override;
  bool usesNetwork() const override { return true; }

  void categories(QObject* context, CategoriesDone done) override;
  void search(const QString& text, const QString& category, QObject* context, SearchDone done) override;
  void thumbnail(const TextureAsset& asset, int size, QObject* context, ThumbnailDone done) override;
  void fetch(const TextureAsset& asset, const QString& resolution, QObject* context, FetchProgress progress,
             FetchDone done) override;
  void details(const TextureAsset& asset, QObject* context, DetailsDone done) override;

  // The API's address (tests' fixtures use the same one).
  static QString apiBase() { return QStringLiteral("https://api.polyhaven.com"); }
  // How long the cached list of textures and lists of files are used
  // without asking again.
  static constexpr qint64 kListMaxAgeSeconds = 24 * 60 * 60;

private:
  using Then = std::function<void(const QString& error)>;
  using FilesThen = std::function<void(const PolyHavenFiles& files)>;
  // Calls `then` once the list of textures is there (from the cache or the
  // API), with an error when it is not.
  void whenListed(Then then);
  void listed(const PolyHavenList& list);
  // The asset's files, from the cache or the API.
  void whenFiles(const QString& asset, FilesThen then);
  // Downloads `files[index...]` into `folder`, then calls `finish`.
  struct Download;
  void downloadNext(const std::shared_ptr<Download>& download);
  // A failed request as a sentence.
  static QString errorText(const TextureReply& reply);
  QString listCacheFile() const;
  QString filesCacheFile(const QString& asset) const;

  std::unique_ptr<TextureFetcher> m_fetcher;
  bool m_listed = false;
  bool m_listing = false;
  PolyHavenList m_list;
  QHash<QString, int> m_byId;
  std::vector<Then> m_waiting;
  QHash<QString, PolyHavenFiles> m_files;
  QHash<QString, std::vector<FilesThen>> m_filesWaiting;
};

} // namespace mitcad
