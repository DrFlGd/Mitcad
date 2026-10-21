// SPDX-License-Identifier: MIT
#include "framework/DialogFolders.hpp"

#include <QDir>
#include <QFileInfo>
#include <QSettings>
#include <QtLogging>

namespace mitcad {

namespace {

QString key(DialogGroup group) {
  const QString name = dialogGroupName(group);
  return name.isEmpty() ? QString() : QStringLiteral("fileDialogs/") + name;
}

} // namespace

QString dialogGroupName(DialogGroup group) {
  switch (group) {
  case DialogGroup::Open:
    return QStringLiteral("open");
  case DialogGroup::Export:
    return QStringLiteral("export");
  case DialogGroup::Import:
    return QStringLiteral("import");
  case DialogGroup::Texture:
    return QStringLiteral("texture");
  case DialogGroup::Program:
    return QStringLiteral("program");
  case DialogGroup::SaveCopy:
    break;
  }
  return {};
}

QString dialogStartFolder(QSettings& settings, DialogGroup group, const QString& fallback) {
  const QString name = key(group);
  if (name.isEmpty()) {
    return fallback;
  }
  const QString remembered = settings.value(name).toString();
  if (!remembered.isEmpty() && QFileInfo(remembered).isDir()) {
    return remembered;
  }
  return fallback;
}

void rememberDialogFolder(QSettings& settings, DialogGroup group, const QString& chosen) {
  const QString name = key(group);
  if (name.isEmpty() || chosen.trimmed().isEmpty()) {
    return;
  }
  settings.setValue(name, QDir::cleanPath(QFileInfo(chosen.trimmed()).absolutePath()));
}

QString dialogStartFolder(DialogGroup group, const QString& fallback) {
  QSettings settings;
  const QString folder = dialogStartFolder(settings, group, fallback);
  if (group != DialogGroup::SaveCopy) {
    qInfo().noquote() << QStringLiteral("Dialog folder %1: %2").arg(dialogGroupName(group), folder);
  }
  return folder;
}

void rememberDialogFolder(DialogGroup group, const QString& chosen) {
  if (group == DialogGroup::SaveCopy || chosen.trimmed().isEmpty()) {
    return;
  }
  QSettings settings;
  rememberDialogFolder(settings, group, chosen);
  settings.sync();
  qInfo().noquote() << QStringLiteral("Dialog folder %1 remembered: %2")
                           .arg(dialogGroupName(group), settings.value(key(group)).toString());
}

} // namespace mitcad
