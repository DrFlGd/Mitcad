// SPDX-License-Identifier: MIT
#pragma once

// The requests of the texture sources on the internet (mitcad#313): a
// fetcher gets the bytes of an HTTPS address. The sources take one in their
// constructor, so tests give them a fake one with recorded answers and
// never use the network.
//
// makeTextureFetcher() gives the real one (Qt Network): HTTPS only,
// redirects from HTTPS to HTTPS only, no cookies and no Qt cache, Mitcad's
// own User-Agent (`Mitcad/<version>`), no data for 30 seconds ends a
// request. It asks nothing while Preferences › Textures does not allow
// texture sources on the internet (TextureSettings::allowNetwork). With
// MITCAD_TEXTURE_FIXTURES set (UI tests), the answers are files under that
// folder instead (`<folder>/<host>/<path>`, the query left out).

#include <functional>
#include <memory>
#include <utility>

#include <QByteArray>
#include <QObject>
#include <QPointer>
#include <QString>
#include <QUrl>

class QNetworkAccessManager;

namespace mitcad {

// A request's answer: the bytes, or why there are none.
struct TextureReply {
  enum class Error {
    None,
    Refused,  // not asked: the network is off, or the address is no HTTPS one
    Offline,  // the server could not be reached (no network, no such host, TLS)
    Timeout,  // no data for too long
    Http,     // the server answered with an error status
    TooLarge, // more bytes than the request allows
  };
  QByteArray data;
  Error error = Error::None;
  int status = 0;  // the HTTP status, when there was one
  QString detail;  // the system's words, or the refusal as a sentence

  bool ok() const { return error == Error::None; }
};

class TextureFetcher : public QObject {
  Q_OBJECT

public:
  using Progress = std::function<void(qint64 received, qint64 total)>;
  using Done = std::function<void(const TextureReply& reply)>;

  // `allowed` tells whether the network may be used now; empty: the
  // settings' TextureSettings::allowNetwork (tests give their own).
  using Allowed = std::function<bool()>;
  explicit TextureFetcher(Allowed allowed = {}, QObject* parent = nullptr)
      : QObject(parent), m_allowed(std::move(allowed)) {}

  // A GET of `url`, at most `maxBytes`. The answer comes later on the GUI
  // thread, never during the call, and is dropped (the request stopped)
  // when `context` is gone. `progress` may be empty.
  virtual void get(const QUrl& url, qint64 maxBytes, QObject* context, Progress progress, Done done) = 0;

  // Calls `call` on the GUI thread later, unless `context` is gone by then.
  static void later(const QPointer<QObject>& context, std::function<void()> call);
  // Why `url` is not asked for (a sentence), or empty: the network not
  // allowed in Preferences, an address that is no HTTPS one.
  QString refusal(const QUrl& url) const;

private:
  Allowed m_allowed;
};

// Qt Network.
class NetworkTextureFetcher : public TextureFetcher {
  Q_OBJECT

public:
  explicit NetworkTextureFetcher(Allowed allowed = {}, QObject* parent = nullptr)
      : TextureFetcher(std::move(allowed), parent) {}

  void get(const QUrl& url, qint64 maxBytes, QObject* context, Progress progress, Done done) override;

  // `Mitcad/<version>`.
  static QString userAgent();

private:
  QNetworkAccessManager* m_network = nullptr; // made on the first request
};

// Files under a folder as the answers (MITCAD_TEXTURE_FIXTURES): a missing
// file is HTTP 404. Refuses what the network fetcher refuses, so the
// network's opt-in is tested the same way.
class FileTextureFetcher : public TextureFetcher {
  Q_OBJECT

public:
  explicit FileTextureFetcher(QString folder, Allowed allowed = {}, QObject* parent = nullptr);

  void get(const QUrl& url, qint64 maxBytes, QObject* context, Progress progress, Done done) override;

private:
  QString m_folder;
};

// The fetcher of the running application: FileTextureFetcher when
// MITCAD_TEXTURE_FIXTURES is set, else NetworkTextureFetcher.
std::unique_ptr<TextureFetcher> makeTextureFetcher();

} // namespace mitcad
