// SPDX-License-Identifier: MIT
#pragma once

#include <vector>

#include <QHash>
#include <QPointF>
#include <QRectF>
#include <QString>
#include <QWidget>

#include <gp_Pnt.hxx>
#include <gp_Trsf.hxx>

#include "../sketch/SketchModel.hpp"
#include "../sketch/SketchOverlay.hpp"
#include "EventFilterGuard.hpp"

namespace mitcad {

class CommandHost;
class OcctViewer;

// The parameters a click in the view puts into a feature panel's value
// field while that field has the keyboard (mitcad#250): the dimensions of
// the sketches the view shows (drawn as sketch mode draws them, their
// names and values in their boxes) and the values of the features before
// the one the panel adds or edits (a label at the handle the feature's
// panel would drag the value by). A left click on a label names its
// parameter (picked); the view gets no click then. Drawn with QPainter in
// view pixels over the 3D view; the mouse goes through to the view except
// on a label.
class ParameterPicker : public QWidget {
  Q_OBJECT

public:
  // A sketch the view shows, as the `sketch` query gives it, placed by the
  // occurrence that places it.
  struct Sketch {
    sketch::SketchModel model;
    gp_Trsf placement;
  };
  // A feature's value: its parameter at a point of the design.
  struct Value {
    QString feature; // "Extrude1"
    QString input;   // the input of its panel: "distance", a list row's "sets.0.radius"
    QString name;    // its parameter: "d4"
    gp_Pnt at;
  };

  ParameterPicker(OcctViewer& viewer, const QString& command);
  ~ParameterPicker() override;

  // What it shows; `texts` are the parameters' values in their units, by
  // name ("40 mm"), as the `parameters` query gives them.
  void setContent(std::vector<Sketch> sketches, std::vector<Value> values, QHash<QString, QString> texts);
  bool isEmpty() const { return m_sketches.empty() && m_values.empty(); }

signals:
  // A label was clicked: its parameter, and what it stands for ("Sketch1
  // k5", "Extrude1 distance").
  void picked(const QString& name, const QString& source);

protected:
  void paintEvent(QPaintEvent* event) override;
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  struct Label {
    QString name;
    QString source;
    QString text;
    QRectF rect;
    bool driven = false;
    bool feature = false;
    sketch::DimensionShape shape; // a sketch dimension's lines and arrows
  };

  // The labels as the view shows them now, in the order they are drawn.
  std::vector<Label> layout() const;
  int labelAt(const QPointF& position) const;
  void logPlaces();

  OcctViewer& m_viewer;
  EventFilterGuard m_viewerFilter{this};
  QString m_command;
  std::vector<Sketch> m_sketches;
  std::vector<Value> m_values;
  QHash<QString, QString> m_texts;
  int m_hover = -1;
  bool m_pressed = false; // a press on a label: its release is not the view's
  QString m_logged;
};

// Gathers what a picker shows from the model: the dimensions with
// parameters of the sketches the view shows (CommandHost::shownSketches)
// and the values with parameters (list rows' too: fillet and chamfer sets)
// of the features before the timeline's marker that a feature command can
// edit (CommandHost::featureEditor), each at its handle's place, at
// most `features` of them, the nearest the marker first; `texts` gets
// every parameter's value in its unit.
void gatherParameterLabels(const CommandHost& host, int features, std::vector<ParameterPicker::Sketch>& sketches,
                           std::vector<ParameterPicker::Value>& values, QHash<QString, QString>& texts);

} // namespace mitcad
