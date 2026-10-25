// SPDX-License-Identifier: MIT
// Isolated client stub: verifies the CLI contract and writes only the test
// home's TOML or JSON. It is never installed or linked into the application.
#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSaveFile>

int main(int argc, char** argv) {
  QCoreApplication app(argc, argv);
  const QStringList args = app.arguments().mid(1);
  const QString work = qEnvironmentVariable("MITCAD_ASSISTANT_TEST_WORK");
  if (args.size() == 9 && args.mid(0, 6) == QStringList({QStringLiteral("mcp"), QStringLiteral("add"),
      QStringLiteral("--scope"), QStringLiteral("user"), QStringLiteral("mitcad"), QStringLiteral("--")}) &&
      args.mid(7) == QStringList({QStringLiteral("mcp"), QStringLiteral("--connect")})) {
    const QString home = qEnvironmentVariable("CLAUDE_CONFIG_DIR");
    if (work.isEmpty() || !QDir::fromNativeSeparators(home).startsWith(QDir::fromNativeSeparators(work) + QLatin1Char('/'))) return 3;
    QFile recorded(QDir(work).filePath(QStringLiteral("claude-registration-argv.json")));
    if (!recorded.open(QIODevice::WriteOnly) ||
        recorded.write(QJsonDocument(QJsonArray::fromStringList(args)).toJson()) < 0) return 4;
    const QString path = QDir(home).filePath(QStringLiteral(".claude.json"));
    QFile original(path);
    if (!original.open(QIODevice::ReadOnly)) return 5;
    QJsonObject content = QJsonDocument::fromJson(original.readAll()).object(); original.close();
    auto servers = content.value(QStringLiteral("mcpServers")).toObject();
    servers.insert(QStringLiteral("mitcad"), QJsonObject{{QStringLiteral("type"), QStringLiteral("stdio")},
        {QStringLiteral("command"), args.at(6)},
        {QStringLiteral("args"), QJsonArray{QStringLiteral("mcp"), QStringLiteral("--connect")}}});
    content.insert(QStringLiteral("mcpServers"), servers);
    if (qEnvironmentVariable("MITCAD_TEST_ASSISTANT_CORRUPT") == QStringLiteral("1")) content.remove(QStringLiteral("keep"));
    if (qEnvironmentVariable("MITCAD_TEST_ASSISTANT_CONCURRENT") == QStringLiteral("1")) {
      QFile concurrent(QDir(work).filePath(QStringLiteral("claude.json")));
      if (!concurrent.open(QIODevice::ReadOnly)) return 7;
      auto changed = QJsonDocument::fromJson(concurrent.readAll()).object(); concurrent.close();
      changed.insert(QStringLiteral("concurrent"), true);
      if (!concurrent.open(QIODevice::WriteOnly | QIODevice::Truncate) || concurrent.write(QJsonDocument(changed).toJson()) < 0) return 7;
    }
    QSaveFile output(path);
    const QByteArray bytes = QJsonDocument(content).toJson();
    if (!output.open(QIODevice::WriteOnly) || output.write(bytes) != bytes.size() || !output.commit()) return 6;
    return qEnvironmentVariable("MITCAD_TEST_ASSISTANT_FAIL") == QStringLiteral("1") ? 9 : 0;
  }
  if (args.size() != 7 || args.mid(0, 4) != QStringList({QStringLiteral("mcp"), QStringLiteral("add"),
      QStringLiteral("mitcad-desktop"), QStringLiteral("--")}) ||
      args.mid(5) != QStringList({QStringLiteral("mcp"), QStringLiteral("--connect")})) return 2;
  const QString home = qEnvironmentVariable("CODEX_HOME");
  if (work.isEmpty() || !QDir::fromNativeSeparators(home).startsWith(QDir::fromNativeSeparators(work) + QLatin1Char('/'))) return 3;
  QFile recorded(QDir(work).filePath(QStringLiteral("registration-argv.json")));
  if (!recorded.open(QIODevice::WriteOnly) ||
      recorded.write(QJsonDocument(QJsonArray::fromStringList(args)).toJson()) < 0) return 4;
  if (qEnvironmentVariable("MITCAD_TEST_ASSISTANT_FAIL") == QStringLiteral("1")) return 9;
  const QString path = QDir(home).filePath(QStringLiteral("config.toml"));
  QFile original(path);
  if (!original.open(QIODevice::ReadOnly)) return 5;
  QByteArray content = original.readAll(); original.close();
  const QByteArray encoded = QJsonDocument(QJsonArray{args.at(4)}).toJson(QJsonDocument::Compact);
  content += "\n[mcp_servers.mitcad-desktop]\ncommand = " + encoded.mid(1, encoded.size() - 2) + "\nargs = [\"mcp\", \"--connect\"]\n";
  if (qEnvironmentVariable("MITCAD_TEST_ASSISTANT_CORRUPT") == QStringLiteral("1")) {
    content = "unexpected = 'changed by the stub'\n";
  }
  if (qEnvironmentVariable("MITCAD_TEST_ASSISTANT_CONCURRENT") == QStringLiteral("1")) {
    QFile concurrent(QDir(work).filePath(QStringLiteral("codex user's settings/config.toml")));
    if (!concurrent.open(QIODevice::Append) || concurrent.write("# Concurrent user edit\n") < 0) return 7;
  }
  QSaveFile output(path);
  return output.open(QIODevice::WriteOnly) && output.write(content) == content.size() && output.commit() ? 0 : 6;
}
