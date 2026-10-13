// SPDX-License-Identifier: MIT
#include "AppSettings.hpp"

#include <algorithm>
#include <optional>

#include <QByteArray>
#include <QDir>
#include <QSettings>
#include <QStandardPaths>
#include <QtGlobal>

namespace mitcad {
namespace {

// Not in a group "general": Qt's INI files keep their top-level keys in a
// section [General], and would write that group as [%General].
const QString kAutosave = QStringLiteral("autosave/enabled");
const QString kAutosaveMinutes = QStringLiteral("autosave/minutes");
const QString kImportTolerance = QStringLiteral("import/geometricTolerance");

} // namespace

GeneralSettings GeneralSettings::load() {
  const QSettings settings;
  GeneralSettings general;
  general.autosave = settings.value(kAutosave, general.autosave).toBool();
  general.autosaveMinutes = std::clamp(settings.value(kAutosaveMinutes, general.autosaveMinutes).toInt(),
                                       kMinAutosaveMinutes, kMaxAutosaveMinutes);
  general.importTolerance = std::clamp(settings.value(kImportTolerance, general.importTolerance).toDouble(),
                                       kMinImportTolerance, kMaxImportTolerance);
  return general;
}

void GeneralSettings::save() const {
  QSettings settings;
  settings.setValue(kAutosave, autosave);
  settings.setValue(kAutosaveMinutes, autosaveMinutes);
  settings.setValue(kImportTolerance, importTolerance);
}

bool VersionSettings::complete() const {
  return !name.trimmed().isEmpty() && !email.trimmed().isEmpty();
}

QString VersionSettings::author() const {
  return QStringLiteral("%1 <%2>").arg(name.trimmed(), email.trimmed());
}

VersionSettings VersionSettings::load() {
  const QSettings settings;
  VersionSettings versions;
  versions.name = settings.value(QStringLiteral("versions/name")).toString();
  versions.email = settings.value(QStringLiteral("versions/email")).toString();
  return versions;
}

void VersionSettings::save() const {
  QSettings settings;
  settings.setValue(QStringLiteral("versions/name"), name.trimmed());
  settings.setValue(QStringLiteral("versions/email"), email.trimmed());
  // Of the author's dialog before mitcad#89.
  settings.remove(QStringLiteral("versions/useGit"));
  settings.remove(QStringLiteral("versions/confirmed"));
}

RemoteSettings RemoteSettings::load() {
  const QSettings settings;
  RemoteSettings remote;
  remote.git = settings.value(QStringLiteral("remote/git")).toString().trimmed();
  remote.checkMinutes = std::clamp(settings.value(QStringLiteral("remote/checkMinutes"), remote.checkMinutes).toInt(),
                                   0, kMaxCheckMinutes);
  remote.autoPush = settings.value(QStringLiteral("remote/autoPush"), remote.autoPush).toBool();
  remote.allowLive = settings.value(QStringLiteral("remote/allowLive"), remote.allowLive).toBool();
  remote.defaultBroker = settings.value(QStringLiteral("remote/defaultBroker")).toString().trimmed();
  return remote;
}

void RemoteSettings::save() const {
  QSettings settings;
  settings.setValue(QStringLiteral("remote/git"), git.trimmed());
  settings.setValue(QStringLiteral("remote/checkMinutes"), checkMinutes);
  settings.setValue(QStringLiteral("remote/autoPush"), autoPush);
  settings.setValue(QStringLiteral("remote/allowLive"), allowLive);
  settings.setValue(QStringLiteral("remote/defaultBroker"), defaultBroker.trimmed());
}

void RemoteSettings::apply() const {
  // MITCAD_GIT as the environment had it before Mitcad set it.
  static const std::optional<QByteArray> original =
      qEnvironmentVariableIsSet("MITCAD_GIT") ? std::optional<QByteArray>(qgetenv("MITCAD_GIT")) : std::nullopt;
  if (!git.trimmed().isEmpty()) {
    qputenv("MITCAD_GIT", QDir::toNativeSeparators(git.trimmed()).toUtf8());
  } else if (original) {
    qputenv("MITCAD_GIT", *original);
  } else {
    qunsetenv("MITCAD_GIT");
  }
}

QString projectsDirectory() {
  const QString given = qEnvironmentVariable("MITCAD_PROJECTS_DIR");
  if (!given.isEmpty()) {
    return QDir::cleanPath(QDir(given).absolutePath());
  }
  const QString documents = QStandardPaths::writableLocation(QStandardPaths::DocumentsLocation);
  return documents.isEmpty() ? QString() : QDir(documents).filePath(QStringLiteral("Mitcad"));
}

QString recoveryDirectory() {
  const QString given = qEnvironmentVariable("MITCAD_AUTOSAVE_DIR");
  if (!given.isEmpty()) {
    return QDir::cleanPath(QDir(given).absolutePath());
  }
  const QString data = QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation);
  // Never relative to the working directory, which may be a project's.
  return data.isEmpty() ? QString() : QDir(data).filePath(QStringLiteral("autosave"));
}

namespace {

// A project's folder as the lists keep it, and whether two are the same.
QString projectKey(const QString& root) { return QDir::cleanPath(QDir::fromNativeSeparators(root)); }

void removePath(QStringList& list, const QString& root) {
  const QString key = projectKey(root);
  list.erase(std::remove_if(list.begin(), list.end(),
                            [&key](const QString& other) {
#ifdef _WIN32
                              return projectKey(other).compare(key, Qt::CaseInsensitive) == 0;
#else
                              return projectKey(other) == key;
#endif
                            }),
             list.end());
}

} // namespace

ProjectsSettings ProjectsSettings::load() {
  const QSettings settings;
  ProjectsSettings projects;
  projects.known = settings.value(QStringLiteral("projects/known")).toStringList();
  projects.hidden = settings.value(QStringLiteral("projects/hidden")).toStringList();
  if (settings.contains(QStringLiteral("projects/panel"))) {
    projects.panelShown = settings.value(QStringLiteral("projects/panel")).toBool();
  }
  return projects;
}

void ProjectsSettings::save() const {
  QSettings settings;
  settings.setValue(QStringLiteral("projects/known"), known);
  settings.setValue(QStringLiteral("projects/hidden"), hidden);
  // Never written before a choice: the default follows the projects.
  if (panelShown) {
    settings.setValue(QStringLiteral("projects/panel"), *panelShown);
  }
}

void ProjectsSettings::remember(const QString& root) {
  if (root.isEmpty()) {
    return;
  }
  ProjectsSettings projects = load();
  removePath(projects.known, root);
  removePath(projects.hidden, root);
  projects.known.prepend(projectKey(root));
  while (projects.known.size() > kMaxKnown) {
    projects.known.removeLast();
  }
  projects.save();
}

void ProjectsSettings::forget(const QString& root) {
  ProjectsSettings projects = load();
  removePath(projects.known, root);
  removePath(projects.hidden, root);
  projects.hidden.append(projectKey(root));
  projects.save();
}

} // namespace mitcad
