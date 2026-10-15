// SPDX-License-Identifier: MIT
#pragma once

// The texture browser of Edit Appearances (mitcad#315): a source's assets
// as a grid of thumbnails, loaded as they come into sight; the chosen
// asset's resolutions with the bytes they take; Use; the source's
// attribution line and messages. The dialog chooses the source, the
// search text and the category, and does Use (useRequested).

#include <QAbstractListModel>
#include <QHash>
#include <QPixmap>
#include <QPointer>
#include <QSet>
#include <QVector>
#include <QWidget>

#include "TextureSource.hpp"

class QComboBox;
class QLabel;
class QListView;
class QPushButton;

namespace mitcad {

// A source's assets with their thumbnails, asked of the source the first
// time a view shows them.
class TextureAssetModel : public QAbstractListModel {
  Q_OBJECT

public:
  static constexpr int kThumbnailSize = 96;
  static constexpr int kIdRole = Qt::UserRole;

  explicit TextureAssetModel(QObject* parent = nullptr);

  void setAssets(TextureSource* source, const QVector<TextureAsset>& assets);
  // An asset with more of its details (TextureSource::details); false when
  // it is not listed or did not change.
  bool updateAsset(const TextureAsset& asset);
  const TextureAsset* asset(int row) const;
  int rowCount(const QModelIndex& parent = QModelIndex()) const override;
  QVariant data(const QModelIndex& index, int role) const override;

signals:
  // A thumbnail came (UI tests' log).
  void thumbnailShown(const QString& name);

private:
  QPointer<TextureSource> m_source;
  QVector<TextureAsset> m_assets;
  QHash<QString, QPixmap> m_thumbnails;
  mutable QSet<QString> m_asked;
  QPixmap m_placeholder;
  int m_generation = 0;
};

class TextureBrowser : public QWidget {
  Q_OBJECT

public:
  explicit TextureBrowser(QWidget* parent = nullptr);

  // Searches `source` (null: shows nothing); the results replace the
  // grid's when they come.
  void search(TextureSource* source, const QString& text, const QString& category);
  const TextureAsset* currentAsset() const;
  QString resolution() const;
  // While a Use fetches: the message, Use disabled; empty: done.
  void setBusy(const QString& message);
  // A line under the grid (errors in the error colour).
  void setMessage(const QString& message, bool error);

  QListView* view() const { return m_view; }
  QComboBox* resolutionBox() const { return m_resolution; }
  QPushButton* useButton() const { return m_use; }
  // The assets in sight: name and centre in the view's viewport.
  QVector<QPair<QString, QPoint>> assetsInSight() const;

signals:
  void useRequested(const TextureAsset& asset, const QString& resolution);
  // The grid's contents or places changed (the dialog logs them).
  void placesChanged();

private:
  void showAsset();
  void showSize();

  QPointer<TextureSource> m_source;
  TextureAssetModel* m_model = nullptr;
  QListView* m_view = nullptr;
  QLabel* m_attribution = nullptr;
  QComboBox* m_resolution = nullptr;
  QLabel* m_size = nullptr;
  QPushButton* m_use = nullptr;
  QLabel* m_message = nullptr;
  QString m_busy;
  int m_searches = 0;
  // The assets whose details were asked for, of the source shown.
  QSet<QString> m_detailed;
};

} // namespace mitcad
