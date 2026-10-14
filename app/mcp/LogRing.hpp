// SPDX-License-Identifier: MIT
#pragma once

#include <vector>

#include <QString>
#include <QtGlobal>

namespace mitcad::mcp {

// The application's latest log lines (mitcad#265, mitcad#269): its own
// qDebug, qInfo, qWarning and qCritical messages (command refusals among
// them), each with a sequence number, in a ring of the newest 5000 lines,
// whether or not the debug log writes them to a file. Live MCP's ui_log
// and the problem reports (report/ReportCenter) read it, and the debug log
// (framework/DebugLog.hpp) starts its file with it.
//
// main() installs the handler once, for the whole run, after the UI test
// driver's: it passes every message on to the one installed before it
// (stderr, the test driver), then to the sink (the debug log's file).
struct LogLine {
  quint64 sequence = 0;
  QString level; // debug, info, warning, critical, fatal
  QString text;
  qint64 time = 0; // milliseconds since the epoch
  QString thread;  // the thread's name when it is not the GUI thread, else empty
};

// Installs the handler once; removeLogRing() puts the previous one back.
void installLogRing();
void removeLogRing();
// The newest lines after `since` (0: all kept), at most `limit` of them, in
// order; `next` gets the sequence number the next line will have and
// `dropped` whether lines after `since` are no longer kept.
std::vector<LogLine> logLines(quint64 since, int limit, quint64* next = nullptr, bool* dropped = nullptr);

// "2026-10-14 09:30:12.345 warning  [model worker] text": the time in ms
// (local), the level, the thread when it is not the GUI thread, the text
// (its further lines indented).
QString formatLogLine(const LogLine& line);

// Gets every line once it is in the ring, on the thread that logged it,
// outside the ring's lock (the debug log's file); nullptr for none. A
// message the sink itself causes is not passed to it again.
using LogSink = void (*)(QtMsgType type, const LogLine& line);
void setLogSink(LogSink sink);

} // namespace mitcad::mcp
