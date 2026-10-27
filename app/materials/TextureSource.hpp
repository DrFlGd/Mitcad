// SPDX-License-Identifier: MIT
#pragma once

// Texture sources (mitcad#315): where Edit Appearances finds sets of
// physically based texture maps to make appearances of. Each source is one
// class behind TextureSource and one line in textureSourceRegistry()
// (TextureSource.cpp): local folders (FolderTextureSource) and Poly Haven
// online (PolyHavenTextureSource, mitcad#313). A source searches its assets, gives their thumbnails
// and fetches an asset's maps as local image files; the application then
// embeds the images in the design (create_appearance with each map's
// data, mitcad#313), so a design never needs a source or its cache to
// open. The model never sees a source.

#include <functional>
#include <memory>
#include <vector>

#include <QHash>
#include <QImage>
#include <QMap>
#include <QObject>
#include <QPointer>
#include <QSizeF>
#include <QString>
#include <QStringList>
#include <QVector>

namespace mitcad {

// The maps of a set by the model's names (core/model-base/src/appearance.rs):
// `base_color` (the texture's own image), `normal` (OpenGL convention),
// `roughness`, `metalness`, `occlusion`.
const QStringList& textureSetMapNames();

// One asset of a source: a material with its maps in one or more
// resolutions.
struct TextureAsset {
  QString id;   // unique in its source
  QString name; // shown
  // What an appearance's `source.asset` records (the id when empty).
  QString recordedId = QString();
  QStringList tags = {};
  QStringList categories = {};
  // The source's own reference to the thumbnail (a file, an address).
  QString thumbnail = QString();
  // One repeat of the texture on the body, mm; invalid when unknown.
  QSizeF size = QSizeF();
  QString license = QString(); // an SPDX identifier, empty when unknown
  QString url = QString();     // the asset's web page, empty when none
  // The resolutions it comes in (`1k`, `2k`, ...; empty for the files'
  // own size), smallest first, and the bytes of all maps of each (-1 when
  // unknown).
  QStringList resolutions = {};
  QHash<QString, qint64> bytes = {};

  QString recorded() const { return recordedId.isEmpty() ? id : recordedId; }
};

// The local files of an asset's maps in one resolution: map name (above)
// to an absolute file path. `base_color` is always there.
struct TextureFiles {
  QString resolution;
  QMap<QString, QString> maps;
};

// A texture source. Calls are asynchronous: the answer comes later on the
// thread of `context` (the GUI thread), never during the call, and is
// dropped when `context` is gone. An error is a sentence for the user;
// with it the other values are empty.
class TextureSource : public QObject {
  Q_OBJECT

public:
  using CategoriesDone = std::function<void(const QStringList& categories, const QString& error)>;
  using SearchDone = std::function<void(const QVector<TextureAsset>& assets, const QString& error)>;
  using ThumbnailDone = std::function<void(const QImage& image)>;
  using FetchProgress = std::function<void(qint64 done, qint64 total)>;
  using FetchDone = std::function<void(const TextureFiles& files, const QString& error)>;

  explicit TextureSource(QObject* parent = nullptr) : QObject(parent) {}

  // The id an appearance's `source.id` records (letters, digits, `_`,
  // `-`) and the cache's folder is named by.
  virtual QString id() const = 0;
  // The source's name in the selector.
  virtual QString name() const = 0;
  // A line shown under the results when not empty (a source's terms may
  // ask for it).
  virtual QString attribution() const { return QString(); }
  // Whether it uses the network: then it is asked only when the user has
  // allowed it (TextureSettings::allowNetwork).
  virtual bool usesNetwork() const { return false; }
  // What the browser shows when a search finds nothing at all (no folders,
  // ...); empty for the usual "nothing found".
  virtual QString emptyHint() const { return QString(); }

  // The categories to filter by (shown names).
  virtual void categories(QObject* context, CategoriesDone done) = 0;
  // The assets whose name or tags contain every word of `text` (all for
  // none), in `category` (all for an empty one).
  virtual void search(const QString& text, const QString& category, QObject* context, SearchDone done) = 0;
  // The asset's thumbnail, at most `size` pixels square; a null image when
  // there is none.
  virtual void thumbnail(const TextureAsset& asset, int size, QObject* context, ThumbnailDone done) = 0;
  // The asset's maps in `resolution` (one of its resolutions) as local
  // files: the source's own files, or files it fetched into the cache.
  virtual void fetch(const TextureAsset& asset, const QString& resolution, QObject* context, FetchProgress progress,
                     FetchDone done) = 0;
  // What the source knows of an asset only once it is chosen (an online
  // source's resolutions and their bytes, mitcad#313): the asset with them,
  // or it as it was and an error. By default the asset as it was.
  using DetailsDone = std::function<void(const TextureAsset& asset, const QString& error)>;
  virtual void details(const TextureAsset& asset, QObject* context, DetailsDone done) {
    deliver(context, [asset, done = std::move(done)] { done(asset, QString()); });
  }

protected:
  // Calls `call` on the GUI thread later, unless `context` is gone by
  // then; safe from worker threads (take the QPointer on the GUI thread).
  static void deliver(const QPointer<QObject>& context, std::function<void()> call);
};

// Texture settings (Preferences, Textures; the settings textures/folders,
// textures/resolution, textures/allowNetwork).
struct TextureSettings {
  QStringList folders;         // local texture folders, absolute
  QString resolution = QStringLiteral("2k"); // the browser's last choice
  bool allowNetwork = false; // sources that use the network may be asked

  static TextureSettings load();
  void save() const;
  bool operator==(const TextureSettings& o) const {
    return folders == o.folders && resolution == o.resolution && allowNetwork == o.allowNetwork;
  }
  bool operator!=(const TextureSettings& o) const { return !(*this == o); }
};

// The registered sources: adding a source is one entry here (and its
// class). `make` builds it for the settings of the moment.
struct TextureSourceEntry {
  const char* id;
  bool usesNetwork;
  std::function<std::unique_ptr<TextureSource>(const TextureSettings& settings)> make;
};
const std::vector<TextureSourceEntry>& textureSourceRegistry();
// Every registered source, in the registry's order.
std::vector<std::unique_ptr<TextureSource>> makeTextureSources(const TextureSettings& settings);

// The texture cache: fetched files and thumbnails, in the user's data
// folder (`textures`), or MITCAD_TEXTURES_DIR. Never needed to open a
// design.
QString textureCacheFolder();
// `textures/<source>/<asset>/<resolution>/` (`<asset>/` without a
// resolution); the asset's id made a safe folder name.
QString textureAssetFolder(const QString& source, const QString& asset, const QString& resolution = QString());
// A folder name for any id: its letters and digits, and a digest when it
// had to be changed.
QString textureFolderName(const QString& id);

// "12.3 MB", "850 KB".
QString textureBytesText(qint64 bytes);

} // namespace mitcad
