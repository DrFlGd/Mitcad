// SPDX-License-Identifier: MIT
// CommandSession: the hover of a tangent chain (mitcad#290). With Tangent
// Chain on, Fillet, Chamfer and the face tools take the whole chain of a
// picked edge or face; the hover shows it before the click. The rest of
// the session is in CommandSession.cpp.
#include <exception>

#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonObject>
#include <QtLogging>

#include <BRep_Builder.hxx>
#include <TopoDS_Compound.hxx>

#include "CommandSession.hpp"

namespace mitcad {
namespace {

// A query for the chains of a body slower than this is logged as slow; its
// answer is still used (the next hovers over the body only look it up).
constexpr qint64 kSlowChainsMs = 1000;

} // namespace

void CommandSession::hovered(const SelectionItem& item) {
  if (m_done) {
    return;
  }
  m_hovered = item;
  updateHoverChain();
}

QStringList CommandSession::hoverChain(const SelectionItem& item) {
  const bool faces = item.kind == SelectKind::Face;
  const QString what = faces ? QStringLiteral("faces") : QStringLiteral("edges");
  const QString key = item.owner + QLatin1Char('/') + what;
  auto known = m_chains.constFind(key);
  if (known == m_chains.cend()) {
    if (m_host.modelBusy()) {
      // Asked again once the model is free; the item alone meanwhile.
      m_host.whenIdle(this, [this] { if (!m_done) { updateHoverChain(); } });
      return {item.name};
    }
    QHash<QString, QStringList> chains;
    QElapsedTimer timer;
    timer.start();
    try {
      QJsonObject request{{QStringLiteral("query"), QStringLiteral("tangent_chains")},
                          {QStringLiteral("body"), item.owner}};
      if (faces) {
        request.insert(QStringLiteral("faces"), true);
      }
      const QJsonArray found = m_host.queryObject(request).value(QStringLiteral("chains")).toArray();
      for (const QJsonValue& value : found) {
        QStringList chain;
        for (const QJsonValue& name : value.toArray()) {
          chain << name.toString();
        }
        for (const QString& name : chain) {
          chains.insert(name, chain);
        }
      }
      const qint64 ms = timer.elapsed();
      qDebug().noquote() << QStringLiteral("Tangent chains of %1 (%2): %3 in %4 ms%5")
                                .arg(item.owner, what)
                                .arg(found.size())
                                .arg(ms)
                                .arg(ms > kSlowChainsMs ? QStringLiteral(" (slow)") : QString());
    } catch (const std::exception& error) {
      // The hover then shows the item alone over this body.
      qWarning().noquote() << QStringLiteral("Tangent chains of %1 (%2) failed after %3 ms: %4")
                                  .arg(item.owner, what)
                                  .arg(timer.elapsed())
                                  .arg(QString::fromUtf8(error.what()));
      chains.clear();
    }
    known = m_chains.insert(key, chains);
  }
  return known->value(item.name, QStringList{item.name});
}

void CommandSession::updateHoverChain() {
  if (m_done) {
    return;
  }
  const InputDef* input = activeInput();
  const SelectionItem& item = m_hovered;
  QStringList chain;
  if (input != nullptr && item.isValid() && !item.owner.isEmpty() && !item.name.isEmpty() &&
      (item.kind == SelectKind::Edge || item.kind == SelectKind::Face) && input->chainKinds.testFlag(item.kind)) {
    chain = !input->chainCheck.isEmpty() && m_state.checked(input->chainCheck) ? hoverChain(item)
                                                                               : QStringList{item.name};
  }
  // The rest of the chain, the item under the mouse left to the view's own
  // hover.
  const QString key = chain.size() > 1 ? QStringList{item.occurrence, item.owner, item.name}.join(QLatin1Char('|'))
                                       : QString();
  if (key != m_chainKey) {
    m_chainKey = key;
    m_chainShape.Nullify();
    if (!key.isEmpty()) {
      BRep_Builder builder;
      TopoDS_Compound compound;
      builder.MakeCompound(compound);
      for (const QString& name : chain) {
        if (name == item.name) {
          continue;
        }
        const TopoDS_Shape shape = m_host.itemShape(SelectionItem{item.kind, item.owner, name, QString(), item.occurrence});
        if (!shape.IsNull()) {
          builder.Add(compound, shape);
        }
      }
      m_chainShape = compound;
    }
    m_host.viewer().setHoverChain(m_chainShape);
  }
  // For UI tests: how many edges or faces the hover shows.
  const QString line =
      chain.isEmpty()
          ? QString()
          : QStringLiteral("Hover chain: %1 %2 of %3")
                .arg(chain.size())
                .arg(item.kind == SelectKind::Face ? QStringLiteral("face(s)") : QStringLiteral("edge(s)"), item.owner);
  if (line != m_loggedHover) {
    m_loggedHover = line;
    if (!line.isEmpty()) {
      qDebug().noquote() << line;
    }
  }
}

} // namespace mitcad
