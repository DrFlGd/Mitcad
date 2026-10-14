// SPDX-License-Identifier: MIT
// Selecting the edited sketch's entities by id and bringing them into sight
// (the Sketch Palette's warnings, mitcad#264; live MCP, mitcad#265).
#include "SketchController.hpp"

#include <vector>

#include <QStringList>

#include <QDebug>
#include <QtLogging>

#include <gp_Pnt.hxx>

#include "../view/ViewReveal.hpp"

namespace mitcad::sketch {

int SketchController::selectEntities(const QStringList& ids, bool reveal, bool fit) {
  Selection items;
  std::vector<gp_Pnt> places;
  for (const QString& id : ids) {
    SelectionItem item;
    item.owner = m_uid;
    item.name = id;
    if (const PointData* point = m_model.point(id)) {
      item.kind = SelectKind::SketchPoint;
      item.geometry = QStringLiteral("point");
      places.push_back(toModel(m_model.frame, point->at));
    } else if (const CurveData* curve = m_model.curve(id)) {
      item.kind = SelectKind::SketchCurve;
      item.geometry = curve->type;
      for (const V2& at : curve->curve.sample(5.0)) {
        places.push_back(toModel(m_model.frame, at));
      }
    } else {
      continue;
    }
    if (!items.contains(item)) {
      items.append(item);
    }
  }
  m_host.setSelection(items);
  if (reveal) {
    const QString moved = revealInView(m_viewer, places, fit);
    if (moved != QStringLiteral("none")) {
      // "Sketch view to the selection: pan", for UI tests.
      qDebug().noquote() << QStringLiteral("Sketch view to the selection: %1").arg(moved);
    }
  }
  return static_cast<int>(items.size());
}

} // namespace mitcad::sketch
