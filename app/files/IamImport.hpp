// SPDX-License-Identifier: MIT
#pragma once

#include <optional>

#include <QJsonObject>
#include <QSettings>
#include <QString>

namespace mitcad {

// The .iam import's choices and progress in the application (mitcad#507),
// plain Qt Core (app.unit).

// Parts with their design history: whether an .iam assembly's parts come
// in with their parameters and features replayed (the model's import_iam
// `history`), else with their stored bodies only. On by default; File >
// Open and Import remember the last choice (the setting
// import/iamPartsHistory).
bool iamPartsHistory(const QSettings& settings);
void rememberIamPartsHistory(QSettings& settings, bool history);

// A line of the import worker's trace about an assembly's part (the
// model's "import: [<s>] part <n> of <m>: <file>" when its import starts,
// "...: <file>: <outcome> in <s> s" when it is done).
struct IamPartProgress {
  int part = 0;
  int parts = 0;
  QString file;
  QString outcome; // replayed or bodies when done; empty when it starts
  double seconds = 0.0;
};
std::optional<IamPartProgress> parseIamPartProgress(const QString& line);

// How an imported assembly's part file came in (its `part` in the
// import_iam report's `files`), for the Import Report and the log:
// "replayed, 12 items (10 parametric, 1 partial, 1 fallback, 0 skipped),
// 9 verified, 3 parameters" or "stored bodies: <reason>"; empty when the
// report does not say (an older report, a file not imported).
QString iamPartOutcome(const QJsonObject& part);

} // namespace mitcad
