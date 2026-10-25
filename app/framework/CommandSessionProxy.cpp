// SPDX-License-Identifier: MIT
// CommandSession: the provisional drag presentation (mitcad#636,
// framework/DragProxy.hpp, CommandDef::dragProxy). While a handle is
// dragged, the shown exact preview's tool is changed to each dragged value
// at once and drawn provisional; the exact preview, computed on the
// model's worker as before (the bounded latest-input scheduling of
// mitcad#634, the interactive mesh tier of mitcad#635), replaces it once
// its value is the one dragged to, and stays what OK commits. The rest of
// the session is in CommandSession.cpp.
#include <algorithm>
#include <cmath>
#include <utility>
#include <vector>

#include <BRep_Builder.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS_Compound.hxx>

#include <QtLogging>

#include "CommandSession.hpp"
#include "Diagnostics.hpp"
#include "ManipulatorOverlay.hpp"

namespace mitcad {
namespace {

// A value's text for the log: millimetres.
QString millimetres(double value) {
  return QStringLiteral("%1 mm").arg(std::abs(value) < 1e-12 ? 0.0 : value, 0, 'g', 10);
}

// The time between consecutive events (ms), as median and 95th percentile
// (nearest rank); zero for fewer than two.
std::pair<double, double> intervals(const std::vector<qint64>& at) {
  std::vector<double> gaps;
  for (std::size_t i = 1; i < at.size(); ++i) {
    gaps.push_back(static_cast<double>(at[i] - at[i - 1]));
  }
  if (gaps.empty()) {
    return {0.0, 0.0};
  }
  std::sort(gaps.begin(), gaps.end());
  const auto rank = [&gaps](double fraction) {
    const auto index = static_cast<std::size_t>(std::ceil(fraction * static_cast<double>(gaps.size())));
    return gaps[std::min(gaps.size(), std::max<std::size_t>(index, 1)) - 1];
  };
  return {rank(0.5), rank(0.95)};
}

// Where a mesh is drawn at a placement: its box's middle, as "x,y,z".
QString middleOf(const ProxyMesh& mesh, const gp_Trsf& placement) {
  gp_XYZ low(1e300, 1e300, 1e300);
  gp_XYZ high(-1e300, -1e300, -1e300);
  for (const gp_Pnt& node : mesh.nodes) {
    const gp_XYZ at = node.Transformed(placement).XYZ();
    low.SetCoord(std::min(low.X(), at.X()), std::min(low.Y(), at.Y()), std::min(low.Z(), at.Z()));
    high.SetCoord(std::max(high.X(), at.X()), std::max(high.Y(), at.Y()), std::max(high.Z(), at.Z()));
  }
  const gp_XYZ middle = (low + high) / 2.0;
  const auto coordinate = [](double value) { return QString::number(std::abs(value) < 1e-6 ? 0.0 : value, 'f', 3); };
  return QStringLiteral("%1,%2,%3").arg(coordinate(middle.X()), coordinate(middle.Y()), coordinate(middle.Z()));
}

double medianOf(std::vector<double> values) {
  if (values.empty()) {
    return 0.0;
  }
  std::sort(values.begin(), values.end());
  return values[(values.size() - 1) / 2];
}

} // namespace

TopoDS_Shape CommandSession::placedTool(const TopoDS_Shape& tool, const std::vector<gp_Trsf>& placements) {
  if (tool.IsNull() || placements.empty()) {
    return TopoDS_Shape();
  }
  const auto placed = [&tool](const gp_Trsf& placement) {
    return placement.Form() == gp_Identity ? tool : tool.Moved(TopLoc_Location(placement));
  };
  if (placements.size() == 1) {
    return placed(placements.front());
  }
  // One copy per occurrence of its component, sharing its faces and their
  // triangulations.
  TopoDS_Compound compound;
  BRep_Builder builder;
  builder.MakeCompound(compound);
  for (const gp_Trsf& placement : placements) {
    builder.Add(compound, placed(placement));
  }
  return compound;
}

void CommandSession::exactPreviewShown(const Preview& preview) {
  if (!m_def.dragProxy) {
    return;
  }
  const std::shared_ptr<const ProxyMesh> before = m_proxyBasis ? m_proxyBasis->mesh : nullptr;
  if (preview.state && preview.toolMesh && !preview.toolPlacements.empty()) {
    m_proxyBasis = ProxyBasis{*preview.state, preview.structure, preview.toolMesh, preview.toolPlacements};
  } else {
    m_proxyBasis.reset();
  }
  const std::shared_ptr<const ProxyMesh> after = m_proxyBasis ? m_proxyBasis->mesh : nullptr;
  if (m_proxyStats.active && m_proxyStats.releasedAt < 0 && after != before) {
    m_proxyStats.exactAt.push_back(m_proxyStats.clock.elapsed());
  }
  if (!m_proxyKey.isEmpty()) {
    updateDragProxy();
  }
}

void CommandSession::dragProxyMoved(const QString& key, double value) {
  if (!m_def.dragProxy || m_done) {
    return;
  }
  if (!m_proxyKey.isEmpty() && key == m_proxyKey && value == m_proxyValue) {
    return; // the pending input's own call once the model is idle
  }
  if (!m_proxyStats.active || m_proxyStats.releasedAt >= 0) {
    // A new drag: its own counts.
    m_proxyStats = ProxyStats();
    m_proxyStats.active = true;
    m_proxyStats.clock.start();
    m_proxyStats.frames = m_host.viewer().framesDrawn();
  }
  ++m_proxyStats.inputs;
  m_proxyKey = key;
  m_proxyValue = value;
  updateDragProxy();
}

void CommandSession::updateDragProxy() {
  if (m_proxyKey.isEmpty() || m_done) {
    return;
  }
  QString why;
  std::optional<DragProxyRule> rule;
  if (!m_proxyBasis) {
    why = QStringLiteral("no exact preview to start from");
  } else if (m_proxyBasis->structure != m_structureGeneration) {
    why = QStringLiteral("the exact preview is of other inputs");
  } else {
    rule = m_def.dragProxy(m_proxyBasis->state, m_state, m_proxyKey, m_proxyValue);
    if (!rule) {
      why = QStringLiteral("not for these inputs");
    }
  }
  const bool dragging = m_manipulators && m_manipulators->isDragging();
  if (rule && std::abs(rule->to - rule->from) <= 1e-9) {
    // The exact preview shows the value dragged to: it is what is shown
    // and committed.
    if (m_proxyStats.active && m_proxyStats.releasedAt >= 0) {
      qDebug().noquote() << QStringLiteral("Drag proxy %1: the exact preview of %2 replaced it %3 ms after the release")
                                .arg(m_def.name, millimetres(m_proxyValue))
                                .arg(m_proxyStats.clock.elapsed() - m_proxyStats.releasedAt);
      m_proxyStats.active = false;
    }
    hideDragProxy(QStringLiteral("the exact preview shows %1").arg(millimetres(m_proxyValue)), !dragging);
    return;
  }
  if (rule && m_proxyShown && m_proxyDrawnFrom.lock() == m_proxyBasis->mesh && m_proxyDrawnValue == m_proxyValue) {
    return; // drawn so already
  }
  std::optional<Manipulator> arrow;
  if (rule) {
    arrow = m_manipulators ? m_manipulators->arrowOf(m_proxyKey) : std::nullopt;
    if (!arrow) {
      why = QStringLiteral("no arrow");
    }
  }
  std::optional<DragProxy> proxy;
  QElapsedTimer timer;
  timer.start();
  if (rule && arrow) {
    // The arrow is drawn in the design; the tool's mesh is in its
    // component's coordinates: the arrow's direction as each placement
    // gives it there, and as it is (a direction the placement keeps).
    std::vector<gp_Dir> axes;
    for (const gp_Trsf& placement : m_proxyBasis->placements) {
      axes.push_back(arrow->direction.Transformed(placement.Inverted()));
    }
    axes.push_back(arrow->direction);
    proxy = applyDragProxy(*m_proxyBasis->mesh, *rule, axes);
    if (!proxy) {
      why = QStringLiteral("the exact preview's tool is not the prism of its values");
    }
  }
  if (!proxy) {
    hideDragProxy(why, false);
    return;
  }
  OcctViewer& viewer = m_host.viewer();
  viewer.setProvisional(proxy->mesh, m_proxyBasis->placements);
  m_proxyViewer = &viewer;
  m_proxyDrawnFrom = m_proxyBasis->mesh;
  m_proxyDrawnValue = m_proxyValue;
  const double spent = static_cast<double>(timer.nsecsElapsed()) / 1e6;
  if (m_proxyStats.active) {
    m_proxyStats.proxyAt.push_back(m_proxyStats.clock.elapsed());
    m_proxyStats.updateMs.push_back(spent);
  }
  if (!m_proxyShown) {
    m_proxyShown = true;
    qDebug().noquote() << QStringLiteral("Drag proxy %1: shown for %2 %3 (the exact preview at %4) at %5")
                              .arg(m_def.name, m_proxyKey, millimetres(rule->to), millimetres(rule->from),
                                   middleOf(*proxy->mesh, m_proxyBasis->placements.front()));
  }
  m_proxyWhy.clear();
  if (timingLogged()) {
    qDebug().noquote() << QStringLiteral("Drag proxy %1 %2: %3 over the exact %4 at %5 in %6 ms")
                              .arg(m_def.name, m_proxyKey, millimetres(rule->to), millimetres(rule->from),
                                   middleOf(*proxy->mesh, m_proxyBasis->placements.front()))
                              .arg(spent, 0, 'f', 3);
  }
}

void CommandSession::hideDragProxy(const QString& why, bool forget) {
  if (m_proxyShown) {
    m_proxyShown = false;
    if (m_proxyViewer) {
      m_proxyViewer->setProvisional(nullptr, {});
    }
    qDebug().noquote() << QStringLiteral("Drag proxy %1: hidden: %2").arg(m_def.name, why);
    m_proxyWhy = why;
  } else if (!m_proxyKey.isEmpty() && why != m_proxyWhy) {
    m_proxyWhy = why;
    qDebug().noquote() << QStringLiteral("Drag proxy %1: not shown: %2").arg(m_def.name, why);
  }
  if (forget) {
    m_proxyKey.clear();
    m_proxyWhy.clear();
    if (m_proxyStats.releasedAt >= 0) {
      m_proxyStats.active = false;
    }
  }
}

void CommandSession::dragProxyFinished(const QString& key) {
  if (!m_proxyStats.active || m_proxyStats.releasedAt >= 0) {
    return;
  }
  ProxyStats& stats = m_proxyStats;
  stats.releasedAt = stats.clock.elapsed();
  const auto proxy = intervals(stats.proxyAt);
  const auto exact = intervals(stats.exactAt);
  double slowest = 0.0;
  for (const double ms : stats.updateMs) {
    slowest = std::max(slowest, ms);
  }
  // Delivered semantic inputs, proxy updates drawn and exact previews shown
  // while the button was held, and the frames submitted meanwhile: each
  // counted on its own (mitcad#636).
  qDebug().noquote() << QStringLiteral("Drag proxy %1 %2: %3 inputs, %4 proxy updates (every %5 ms median, %6 ms p95), "
                                       "%7 exact previews (every %8 ms median, %9 ms p95), %10 frames in %11 ms; "
                                       "proxy update %12 ms median, %13 ms max")
                            .arg(m_def.name, key)
                            .arg(stats.inputs)
                            .arg(stats.proxyAt.size())
                            .arg(proxy.first, 0, 'f', 1)
                            .arg(proxy.second, 0, 'f', 1)
                            .arg(stats.exactAt.size())
                            .arg(exact.first, 0, 'f', 1)
                            .arg(exact.second, 0, 'f', 1)
                            .arg(m_host.viewer().framesDrawn() - stats.frames)
                            .arg(stats.releasedAt)
                            .arg(medianOf(stats.updateMs), 0, 'f', 3)
                            .arg(slowest, 0, 'f', 3);
  if (!m_proxyShown) {
    // Nothing provisional is left: the exact preview has the value.
    stats.active = false;
    m_proxyKey.clear();
    m_proxyWhy.clear();
  }
}

} // namespace mitcad
