// SPDX-License-Identifier: MIT
#pragma once

#include <QString>
#include <QStringList>
#include <QSet>
#include <QtGlobal>
#include <functional>

class QRegularExpression;
class QOpenGLWidget;
class QPointF;
class QJsonObject;

namespace mitcad::mac::test {

// In-process AppKit input for MITCAD_TEST_INPUT. There is no Accessibility
// API, system event injection, permission prompt or second application loop.
#if defined(Q_OS_MACOS) && defined(MITCAD_MACOS_NATIVE_UI)
bool active();
bool nativeFocused();
bool controlExists(const QString& identifier);
bool click(const QString& identifier, QString& error);
bool hover(const QString& identifier, bool enter, QString& error);
bool clickImmediately(const QString& identifier, QString& error);
bool replaceText(const QString& identifier, const QString& text, QString& error);
bool choose(const QString& identifier, const QString& title, QString& error);
bool triggerAction(const QString& identifier, QString& error);
bool triggerMenu(const QString& path, QString& error);
bool activate(const QString& identifier, QString& error);
bool select(const QString& identifier, QString& error, Qt::KeyboardModifiers modifiers = {});
bool expand(const QString& identifier, QString& error);
bool textOf(const QString& identifier, QString& text);
bool textMatches(const QString& identifier, const QRegularExpression& expression);
bool enabled(const QString& identifier, bool expected);
bool visible(const QString& identifier, bool expected);
bool present(const QString& identifier, bool expected);
bool resizeDocument(int width, int height, QString& error);
bool moveDocument(int x, int y, QString& error);
bool setAppearance(const QString& name, QString& error);
bool floatingLayout(QString& error);
bool historyLayout(const QJsonObject& timeline, const QSet<QString>& filtered, const QStringList& selected,
                   bool checkSelection, QString& error);
bool historyMarkerEvent(const QString& phase, int index, QString& error);
bool trackContextMenu(const QString& choice, const std::function<void()>& open, QString& error);
bool contextMenu(const QString& identifier, const QString& choice, QString& error);
bool pinch(QOpenGLWidget* viewport, const QPointF& position, double magnification, QString& error);
bool buttonExists(const QRegularExpression& expression);
bool clickButton(const QRegularExpression& expression, QString& error);
bool dialogExists(const QRegularExpression& expression);
bool focusDialog(const QRegularExpression& expression);
bool replaceFileName(const QString& name, QString& error);
bool setFileDirectory(const QString& directory, QString& error);
bool cancelFilePanel(QString& error);
void focusMain();
bool sendKey(int key, Qt::KeyboardModifiers modifiers, const QString& text);
bool sendText(const QString& text);
bool screenshot(const QString& path, QOpenGLWidget* viewport = nullptr);
#else
inline bool active() { return false; }
inline bool nativeFocused() { return false; }
inline bool controlExists(const QString&) { return false; }
inline bool click(const QString&, QString&) { return false; }
inline bool hover(const QString&, bool, QString&) { return false; }
inline bool clickImmediately(const QString&, QString&) { return false; }
inline bool replaceText(const QString&, const QString&, QString&) { return false; }
inline bool choose(const QString&, const QString&, QString&) { return false; }
inline bool triggerAction(const QString&, QString&) { return false; }
inline bool triggerMenu(const QString&, QString&) { return false; }
inline bool activate(const QString&, QString&) { return false; }
inline bool select(const QString&, QString&, Qt::KeyboardModifiers = {}) { return false; }
inline bool expand(const QString&, QString&) { return false; }
inline bool textOf(const QString&, QString&) { return false; }
inline bool textMatches(const QString&, const QRegularExpression&) { return false; }
inline bool enabled(const QString&, bool) { return false; }
inline bool visible(const QString&, bool) { return false; }
inline bool present(const QString&, bool) { return false; }
inline bool resizeDocument(int, int, QString&) { return false; }
inline bool moveDocument(int, int, QString&) { return false; }
inline bool setAppearance(const QString&, QString&) { return false; }
inline bool floatingLayout(QString&) { return false; }
inline bool historyLayout(const QJsonObject&, const QSet<QString>&, const QStringList&, bool, QString&) { return false; }
inline bool historyMarkerEvent(const QString&, int, QString&) { return false; }
inline bool trackContextMenu(const QString&, const std::function<void()>&, QString&) { return false; }
inline bool contextMenu(const QString&, const QString&, QString&) { return false; }
inline bool pinch(QOpenGLWidget*, const QPointF&, double, QString&) { return false; }
inline bool buttonExists(const QRegularExpression&) { return false; }
inline bool clickButton(const QRegularExpression&, QString&) { return false; }
inline bool dialogExists(const QRegularExpression&) { return false; }
inline bool focusDialog(const QRegularExpression&) { return false; }
inline bool replaceFileName(const QString&, QString&) { return false; }
inline bool setFileDirectory(const QString&, QString&) { return false; }
inline bool cancelFilePanel(QString&) { return false; }
inline void focusMain() {}
inline bool sendKey(int, Qt::KeyboardModifiers, const QString&) { return false; }
inline bool sendText(const QString&) { return false; }
inline bool screenshot(const QString&, QOpenGLWidget* = nullptr) { return false; }
#endif

} // namespace mitcad::mac::test
