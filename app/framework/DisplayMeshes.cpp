// SPDX-License-Identifier: MIT
#include "DisplayMeshes.hpp"

#include <algorithm>
#include <set>
#include <string>
#include <vector>

#include <QCoreApplication>
#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonValue>
#include <QtLogging>

#include <Message_ProgressIndicator.hxx>
#include <Message_ProgressScope.hxx>
#include <Standard_Failure.hxx>

#include "Json.hpp"
#include "mitcad/io/mesh.hpp"
#include "view/ViewTessellation.hpp"

namespace mitcad {
namespace {

// The job's cancel as OCCT's user break (asked from BRepMesh's threads).
class JobBreak : public Message_ProgressIndicator {
public:
  explicit JobBreak(const ModelJob& job) : m_job(job) {}
  bool UserBreak() override { return m_job.control().is_cancelled(); }
  void Show(const Message_ProgressScope&, const bool) override {}

private:
  const ModelJob& m_job;
};

void addShape(DisplayShapes& shapes, std::shared_ptr<geometry::Shape> shape) {
  if (shape && !shape->occt().IsNull()) {
    shapes.push_back(std::move(shape));
  }
}

} // namespace

DisplayShapes shownBodyShapes(const Document& document) {
  DisplayShapes shapes;
  const QJsonDocument instances =
      parseJson(document.query(rustStr(compactJson({{QStringLiteral("query"), QStringLiteral("instances")}}))));
  std::set<std::string> seen;
  for (const QJsonValue& value : instances.array()) {
    const QJsonObject instance = value.toObject();
    if (!instance.value(QStringLiteral("visible")).toBool(true)) {
      continue;
    }
    const std::string uid = instance.value(QStringLiteral("body")).toString().toStdString();
    if (!uid.empty() && seen.insert(uid).second) {
      addShape(shapes, document.body_shape(uid));
    }
  }
  return shapes;
}

DisplayShapes previewShapes(const Document& document, const QJsonObject& report) {
  DisplayShapes shapes;
  if (report.value(QStringLiteral("status")).toString() != QStringLiteral("ok")) {
    return shapes;
  }
  std::set<std::string> seen;
  const auto add = [&](const QString& uid) {
    if (!uid.isEmpty() && seen.insert(uid.toStdString()).second) {
      addShape(shapes, document.preview_body_shape(uid.toStdString()));
    }
  };
  for (const QJsonValue& value : report.value(QStringLiteral("bodies")).toArray()) {
    const QJsonObject body = value.toObject();
    if (body.value(QStringLiteral("changed")).toBool()) {
      add(body.value(QStringLiteral("uid")).toString());
    }
  }
  for (const QJsonValue& value : report.value(QStringLiteral("instances")).toArray()) {
    add(value.toObject().value(QStringLiteral("body")).toString());
  }
  addShape(shapes, document.preview_tool_shape());
  return shapes;
}

DisplayShapes previewDonors(const Document& document, const QJsonObject& report) {
  DisplayShapes shapes;
  if (report.value(QStringLiteral("status")).toString() != QStringLiteral("ok")) {
    return shapes;
  }
  for (const QJsonValue& value : report.value(QStringLiteral("bodies")).toArray()) {
    const QJsonObject body = value.toObject();
    if (body.value(QStringLiteral("changed")).toBool()) {
      addShape(shapes, document.body_shape(body.value(QStringLiteral("uid")).toString().toStdString()));
    }
  }
  return shapes;
}

DisplayMeshing meshForDisplay(const DisplayShapes& shapes, ModelJob& job, DisplayTier tier,
                              const DisplayShapes& donors) {
  DisplayMeshing meshing;
  meshing.tier = tier;
  QElapsedTimer clock;
  clock.start();
  // The shapes the view would mesh, meshed together so that OCCT spreads
  // the faces of all of them over the cores (mitcad#438); one at a time,
  // a design of many small bodies used a core or two. In the interactive
  // tier their stand-ins instead (mitcad#635).
  std::vector<TopoDS_Shape> pending;
  std::vector<std::shared_ptr<geometry::Shape>> meshed; // whose faces are meshed
  std::vector<std::shared_ptr<geometry::Shape>> bodies; // the body each is drawn for
  for (const std::shared_ptr<geometry::Shape>& shape : shapes) {
    const TopoDS_Shape& occt = shape->occt();
    if (io::has_triangulation(occt) || io::is_mesh(occt) || shape->display_meshed()) {
      ++meshing.ready;
      continue;
    }
    if (tier == DisplayTier::Final) {
      pending.push_back(occt);
      meshed.push_back(shape);
      bodies.push_back(shape);
      continue;
    }
    if (const auto standIn = shape->display_stand_in(); standIn && standIn->display_meshed()) {
      // Drawn again (a result the model had cached): the caller keeps it
      // as it keeps a new one, or it would go with the previous preview's
      // and the view would mesh the body itself (mitcad#636).
      ++meshing.ready;
      meshing.standIns.push_back(standIn);
      continue;
    }
    try {
      const std::shared_ptr<geometry::Shape> copy = geometry::independent_copy(*shape);
      pending.push_back(copy->occt());
      meshed.push_back(copy);
      bodies.push_back(shape);
    } catch (const Standard_Failure&) {
      // Left to the view, as a shape the meshing did not finish.
    } catch (const std::exception&) {
    }
  }
  if (!pending.empty() && !job.control().is_cancelled()) {
    job.setStage(QCoreApplication::translate("DisplayMeshes", "Meshing the bodies for display"));
    const occ::handle<JobBreak> progress = new JobBreak(job);
    try {
      // The faces the command left as they were, as the view shows them:
      // only bodies a job meshed to the end in the final tier, which
      // nothing writes into any more.
      std::vector<TopoDS_Shape> given;
      for (const std::shared_ptr<geometry::Shape>& donor : donors) {
        if (donor && donor->display_meshed() && !donor->display_interactive() &&
            io::has_triangulation(donor->occt())) {
          given.push_back(donor->occt());
        }
      }
      if (!given.empty()) {
        for (const TopoDS_Shape& shape : pending) {
          meshing.reused += io::reuse_triangulations(shape, given);
        }
      }
      const io::MeshOptions options = tier == DisplayTier::Final ? viewMeshOptions() : previewMeshOptions();
      meshing.bodies = io::mesh_all_in_place(pending, options, progress->Start(), &meshing.report);
      // Meshed to the end: a face still without a triangulation is one
      // OCCT's mesher fails on. The view draws the others and does not
      // mesh the body again on the UI thread, where meshing such a face
      // again could hold the window without end (mitcad#589).
      if (!job.control().is_cancelled() && meshing.bodies == pending.size()) {
        for (std::size_t i = 0; i < pending.size(); ++i) {
          meshed[i]->set_display_meshed();
          if (!io::has_triangulation(pending[i])) {
            ++meshing.incomplete;
          }
          if (tier == DisplayTier::Interactive) {
            meshed[i]->set_stand_in_for(bodies[i]);
            bodies[i]->set_display_stand_in(meshed[i]);
            meshing.standIns.push_back(meshed[i]);
          } else {
            bodies[i]->set_display_stand_in(nullptr);
          }
        }
      }
    } catch (const Standard_Failure&) {
      // The view meshes them when it draws them, as before.
    } catch (const std::exception&) {
    }
  }
  meshing.milliseconds = clock.elapsed();
  return meshing;
}

std::shared_ptr<geometry::Shape> displayShape(const std::shared_ptr<geometry::Shape>& shape) {
  if (shape) {
    if (std::shared_ptr<geometry::Shape> standIn = shape->display_stand_in()) {
      return standIn;
    }
  }
  return shape;
}

DisplayShapes refinableShapes(const DisplayShapes& shown) {
  DisplayShapes bodies;
  for (const std::shared_ptr<geometry::Shape>& shape : shown) {
    if (!shape || !shape->display_interactive()) {
      continue;
    }
    std::shared_ptr<geometry::Shape> body = shape->stand_in_for();
    if (body && !body->display_meshed() && !io::has_triangulation(body->occt()) &&
        std::find(bodies.begin(), bodies.end(), body) == bodies.end()) {
      bodies.push_back(std::move(body));
    }
  }
  return bodies;
}

void logDisplayMeshing(const DisplayMeshing& meshing, const QString& label) {
  if (io::mesh_tracing()) {
    qDebug().noquote() << QStringLiteral("Mesh trace %1 %2: %3 bodies ready, %4 meshed, %5 faces reused; %6")
                              .arg(label, meshing.tier == DisplayTier::Final ? QStringLiteral("final")
                                                                             : QStringLiteral("interactive"))
                              .arg(static_cast<qulonglong>(meshing.ready))
                              .arg(static_cast<qulonglong>(meshing.bodies))
                              .arg(static_cast<qulonglong>(meshing.reused))
                              .arg(QString::fromStdString(meshing.report.line()));
  }
  if (meshing.bodies > 0) {
    qDebug().noquote() << QStringLiteral("Display meshes of %1: %2 bodies in %3 ms on the model's thread%4")
                              .arg(label)
                              .arg(static_cast<qulonglong>(meshing.bodies))
                              .arg(meshing.milliseconds)
                              .arg(meshing.tier == DisplayTier::Interactive ? QStringLiteral(" (interactive)")
                                                                            : QString());
  }
  if (meshing.incomplete > 0) {
    qWarning().noquote() << QStringLiteral("Display meshes of %1: %2 bodies with faces that could not be meshed "
                                           "(not drawn)")
                                .arg(label)
                                .arg(static_cast<qulonglong>(meshing.incomplete));
  }
}

} // namespace mitcad
