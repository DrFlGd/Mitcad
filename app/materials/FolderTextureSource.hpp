// SPDX-License-Identifier: MIT
#pragma once

// Local texture folders (mitcad#315): the folders the user adds in
// Preferences, Textures, searched as one source. A material is a set of
// PNG or JPEG images whose file names end in a map's suffix
// (parseTextureFileName): those with the same name stem in one folder, or
// all of a folder whose files are named by the suffix alone. A material
// needs a base colour image; its thumbnail is made from it and kept in the
// texture cache.

#include <QHash>
#include <QMap>
#include <QString>
#include <QStringList>
#include <QVector>

#include "TextureSource.hpp"

namespace mitcad {

// What a texture file's name says (`oak_veneer_01_nor_gl_2k.png`,
// `Bricks051_2K_Color.jpg`, `roughness.png`): its map (a name of
// textureSetMapNames(); `normal_directx` for a DirectX normal map, which is
// not used; empty when the name has no map suffix), the stem before the
// suffix (as written, without the separators around it), and the
// resolution token (`2k`; `1024` is `1k`; empty when there is none).
struct TextureFileName {
  QString map;
  QString stem;
  QString resolution;
};
TextureFileName parseTextureFileName(const QString& fileName);

// The materials under the folders (and their subfolders, a few levels
// deep; hidden ones and links skipped): one asset each, id the folder and
// stem, its maps by resolution. Plain and synchronous (FolderTextureSource
// runs it on a worker thread).
struct FolderMaterial {
  TextureAsset asset;
  // resolution -> map name -> file; files without a resolution token
  // under "".
  QMap<QString, QMap<QString, QString>> files;
};
QVector<FolderMaterial> scanTextureFolders(const QStringList& folders);

// The files of a material in `resolution`: that resolution's maps, and
// maps it lacks from the files without a resolution token; no
// `base_color`: empty.
QMap<QString, QString> folderMaterialFiles(const FolderMaterial& material, const QString& resolution);

// The folders the user names as a source.
class FolderTextureSource : public TextureSource {
  Q_OBJECT

public:
  explicit FolderTextureSource(QStringList folders, QObject* parent = nullptr);

  QString id() const override { return QStringLiteral("folder"); }
  QString name() const override { return tr("Local Folders"); }
  QString emptyHint() const override;

  void categories(QObject* context, CategoriesDone done) override;
  void search(const QString& text, const QString& category, QObject* context, SearchDone done) override;
  void thumbnail(const TextureAsset& asset, int size, QObject* context, ThumbnailDone done) override;
  void fetch(const TextureAsset& asset, const QString& resolution, QObject* context, FetchProgress progress,
             FetchDone done) override;

  // Looks at the folders again on the next call.
  void rescan();

private:
  // Calls `then` once the folders are scanned (at once when they are).
  void whenScanned(std::function<void()> then);

  QStringList m_folders;
  bool m_scanned = false;
  bool m_scanning = false;
  QVector<FolderMaterial> m_materials;
  QHash<QString, int> m_byId;
  std::vector<std::function<void()>> m_waiting;
  int m_generation = 0;
};

// The thumbnail of an image file: at most `size` pixels square, the
// middle of the image; kept as `thumbnail-<size>.png` in `cacheFolder`
// (when not empty) and taken from there while it is newer than the image.
// A null image when the file cannot be read.
QImage textureThumbnail(const QString& imageFile, int size, const QString& cacheFolder);

} // namespace mitcad
