// SPDX-License-Identifier: MIT
// The folders a live MCP agent may save to and open from (mitcad#402): a
// path inside the allowed folder is taken (relative ones too), '..', other
// folders, other suffixes, missing folders and files, an existing file
// without overwrite and links out of the folder are refused.
#include "mcp/AgentFolders.hpp"

#include <cstdio>

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QString>
#include <QTemporaryDir>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "agent_folders.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

bool touch(const QString& path) {
  QFile file(path);
  return file.open(QIODevice::WriteOnly) && file.write("{}") == 2;
}

} // namespace

int agentFolderTests() {
  using mitcad::mcp::agentDesignPath;
  QTemporaryDir directory;
  CHECK(directory.isValid());
  const QString root = QFileInfo(directory.path()).canonicalFilePath();
  const QString allowed = root + QStringLiteral("/allowed");
  const QString outside = root + QStringLiteral("/outside");
  CHECK(QDir().mkpath(allowed + QStringLiteral("/sub")));
  CHECK(QDir().mkpath(outside));
  CHECK(touch(allowed + QStringLiteral("/kept.mitcad")));
  CHECK(touch(outside + QStringLiteral("/other.mitcad")));
  mitcad::mcp::setRunAgentFolders({allowed});
  CHECK(mitcad::mcp::agentFolders().contains(allowed));

  QString reason;
  // New files in the folder and below it; relative to the folder.
  CHECK(agentDesignPath(allowed + QStringLiteral("/new.mitcad"), true, false, reason) ==
        allowed + QStringLiteral("/new.mitcad"));
  CHECK(reason.isEmpty());
  CHECK(agentDesignPath(QStringLiteral("sub/new.mitcad"), true, false, reason) ==
        allowed + QStringLiteral("/sub/new.mitcad"));
  CHECK(agentDesignPath(QStringLiteral("NEW.MITCAD"), true, false, reason) == allowed + QStringLiteral("/NEW.MITCAD"));
  // Refused: '..', another folder, another suffix, a missing folder.
  CHECK(agentDesignPath(QStringLiteral("../outside/x.mitcad"), true, false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("..")));
  CHECK(agentDesignPath(allowed + QStringLiteral("/sub/../../outside/x.mitcad"), true, false, reason).isEmpty());
  CHECK(agentDesignPath(outside + QStringLiteral("/x.mitcad"), true, false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("not in a folder agents may use")));
  CHECK(agentDesignPath(allowed + QStringLiteral("/x.step"), true, false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral(".mitcad")));
  CHECK(agentDesignPath(QStringLiteral("missing/x.mitcad"), true, false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("does not exist")));
  // A folder whose name only starts like the allowed one.
  CHECK(QDir().mkpath(allowed + QStringLiteral("-not")));
  CHECK(agentDesignPath(allowed + QStringLiteral("-not/x.mitcad"), true, false, reason).isEmpty());
  // An existing file: only with overwrite; reading needs it there.
  CHECK(agentDesignPath(QStringLiteral("kept.mitcad"), true, false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("overwrite")));
  CHECK(agentDesignPath(QStringLiteral("kept.mitcad"), true, true, reason) == allowed + QStringLiteral("/kept.mitcad"));
  CHECK(agentDesignPath(QStringLiteral("kept.mitcad"), false, false, reason) == allowed + QStringLiteral("/kept.mitcad"));
  CHECK(agentDesignPath(QStringLiteral("absent.mitcad"), false, false, reason).isEmpty());
  CHECK(agentDesignPath(outside + QStringLiteral("/other.mitcad"), false, false, reason).isEmpty());
#ifndef Q_OS_WIN
  // Links out of the folder: a linked folder, a linked file.
  CHECK(QFile::link(outside, allowed + QStringLiteral("/linked")));
  CHECK(agentDesignPath(QStringLiteral("linked/x.mitcad"), true, false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("not in a folder agents may use")));
  CHECK(QFile::link(outside + QStringLiteral("/other.mitcad"), allowed + QStringLiteral("/link.mitcad")));
  CHECK(agentDesignPath(QStringLiteral("link.mitcad"), true, true, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("not a regular file")));
  CHECK(agentDesignPath(QStringLiteral("link.mitcad"), false, false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("not in a folder agents may use")));
#endif
  // An agent's output (mitcad#689: model_export, ui_screenshot's path): a
  // new file of any or of the given suffixes, by the same rules.
  using mitcad::mcp::agentOutputPath;
  CHECK(agentOutputPath(QStringLiteral("part.step"), false, reason) == allowed + QStringLiteral("/part.step"));
  CHECK(reason.isEmpty());
  CHECK(agentOutputPath(QStringLiteral("sub/view.JPG"), false, reason, {QStringLiteral("jpg"), QStringLiteral("jpeg")}) ==
        allowed + QStringLiteral("/sub/view.JPG"));
  CHECK(agentOutputPath(QStringLiteral("view.png"), false, reason, {QStringLiteral("jpg")}).isEmpty());
  CHECK(reason.contains(QStringLiteral(".jpg")));
  CHECK(agentOutputPath(QStringLiteral("kept.mitcad"), false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("overwrite")));
  CHECK(agentOutputPath(QStringLiteral("kept.mitcad"), true, reason) == allowed + QStringLiteral("/kept.mitcad"));
  CHECK(agentOutputPath(outside + QStringLiteral("/part.stl"), false, reason).isEmpty());
  CHECK(reason.contains(QStringLiteral("not in a folder agents may use")));
  CHECK(agentOutputPath(QStringLiteral("../outside/part.stl"), false, reason).isEmpty());
  mitcad::mcp::setRunAgentFolders({});
  return failures;
}
