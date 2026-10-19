// SPDX-License-Identifier: MIT
#include "DisplayMeshes.hpp"

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

DisplayMeshing meshForDisplay(const DisplayShapes& shapes, ModelJob& job) {
  DisplayMeshing meshing;
  QElapsedTimer clock;
  clock.start();
  // The shapes the view would mesh, meshed together so that OCCT spreads
  // the faces of all of them over the cores (mitcad#438); one at a time,
  // a design of many small bodies used a core or two.
  std::vector<TopoDS_Shape> pending;
  for (const std::shared_ptr<geometry::Shape>& shape : shapes) {
    const TopoDS_Shape& occt = shape->occt();
    if (!io::has_triangulation(occt) && !io::is_mesh(occt)) {
      pending.push_back(occt);
    }
  }
  if (!pending.empty() && !job.control().is_cancelled()) {
    job.setStage(QCoreApplication::translate("DisplayMeshes", "Meshing the bodies for display"));
    const occ::handle<JobBreak> progress = new JobBreak(job);
    try {
      // A body OCCT fails on is left to the view, which meshes it when it
      // draws it, as before.
      meshing.bodies = io::mesh_all_in_place(pending, viewMeshOptions(), progress->Start());
    } catch (const Standard_Failure&) {
    } catch (const std::exception&) {
    }
  }
  meshing.milliseconds = clock.elapsed();
  return meshing;
}

void logDisplayMeshing(const DisplayMeshing& meshing, const QString& label) {
  if (meshing.bodies > 0) {
    qDebug().noquote() << QStringLiteral("Display meshes of %1: %2 bodies in %3 ms on the model's thread")
                              .arg(label)
                              .arg(static_cast<qulonglong>(meshing.bodies))
                              .arg(meshing.milliseconds);
  }
}

} // namespace mitcad
