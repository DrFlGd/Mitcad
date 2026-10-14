// SPDX-License-Identifier: MIT
#include "ParameterPicker.hpp"

#include <algorithm>
#include <cmath>
#include <exception>
#include <optional>
#include <utility>

#include <QFontMetricsF>
#include <QJsonArray>
#include <QJsonObject>
#include <QMouseEvent>
#include <QPainter>
#include <QPointer>
#include <QStringList>
#include <QtLogging>

#include <BRepAdaptor_Surface.hxx>
#include <BRepTools.hxx>
#include <Standard_Failure.hxx>
#include <TopoDS_Face.hxx>
#include <gp_Vec.hxx>

#include "../OcctViewer.hpp"
#include "CommandSession.hpp"

namespace mitcad {
namespace {

// A sketch's dimensions in the colour of picking, a feature's values in
// the handles' colour; the one under the pointer brighter.
const QColor kDimensionColor(0x2a, 0x6e, 0xd8);
const QColor kFeatureColor(0xe0, 0x7b, 0x1a);
const QColor kHoverColor(0x1f, 0x7a, 0xff);
const QColor kBoxColor(0xff, 0xff, 0xff, 235);
const QColor kTextColor(0x1a, 0x2a, 0x40);
const QColor kDrivenTextColor(0x60, 0x66, 0x70);
constexpr double kPadding = 4.0;
// A feature's label sits this far above its point (pixels), off the handle.
constexpr double kFeatureLift = 16.0;

double screenDistance(const QPointF& a, const QPointF& b) { return std::hypot(a.x() - b.x(), a.y() - b.y()); }

QPointF unitOf(const QPointF& p) {
  const double n = std::hypot(p.x(), p.y());
  return n > 1e-12 ? QPointF(p.x() / n, p.y() / n) : QPointF(1, 0);
}

// An input's state (by its key: a list row's inputs by their row's) where
// nothing set it, as a command's panel starts (CommandSession::setDefaults):
// before a definition is loaded, and for the rows it loaded.
void setDefault(const InputDef& input, const QString& key, CommandState& state) {
  switch (input.type) {
  case InputDef::Type::Value:
  case InputDef::Type::Text:
    if (!state.hasText(key)) {
      state.setText(key, input.defaultText);
    }
    break;
  case InputDef::Type::Choice:
    if (!state.hasChoice(key)) {
      state.setChoice(key, input.defaultChoice);
    }
    break;
  case InputDef::Type::Check:
  case InputDef::Type::Flip:
    if (!state.hasCheck(key)) {
      state.setChecked(key, input.defaultChecked);
    }
    break;
  case InputDef::Type::List:
    if (state.rows(key) < input.minRows) {
      state.setRows(key, input.minRows);
    }
    break;
  case InputDef::Type::Selection:
  case InputDef::Type::CutTargets:
    break;
  }
}

// A value input of a feature's panel: its key in the whole state (a list
// row's "sets.0.radius"), its definition and the state it is shown in (its
// row's).
struct ValueSlot {
  QString key;
  const InputDef* input = nullptr;
  CommandState scope;
  Selection items; // what the inputs beside it picked (a set's edges)
};

// The value inputs with handles that a loaded state shows, and those of its
// list rows (fillet and chamfer sets).
std::vector<ValueSlot> valueSlots(const CommandDef& command, const CommandState& state) {
  Selection picked;
  for (const InputDef& input : command.inputs) {
    if (input.type == InputDef::Type::Selection) {
      picked += state.items(input.id);
    }
  }
  std::vector<ValueSlot> found;
  for (const InputDef& input : command.inputs) {
    if (input.visible && !input.visible(state)) {
      continue;
    }
    if (input.type == InputDef::Type::Value && input.manipulator) {
      found.push_back({input.id, &input, state, picked});
      continue;
    }
    if (input.type != InputDef::Type::List) {
      continue;
    }
    for (int row = 0; row < state.rows(input.id); ++row) {
      const CommandState scope = state.row(input.id, row);
      Selection items;
      for (const InputDef& child : input.children) {
        if (child.type == InputDef::Type::Selection) {
          items += scope.items(child.id);
        }
      }
      for (const InputDef& child : input.children) {
        // A row's values without handles too: a set's place on the model
        // is the face it made (madeFaceMiddle; a chamfer's distances).
        if (child.type == InputDef::Type::Value && (!child.visible || child.visible(scope))) {
          found.push_back({CommandState::rowKey(input.id, row, child.id), &child, scope, items});
        }
      }
    }
  }
  return found;
}

// Where the handle of a value of a feature that rounds or bevels edges was,
// now that the edges are gone (the marker is after the feature): the
// middle of the face the feature made of the first edge
// (`F3:fillet(E{…})`), on its rounding or bevel where the handle sat.
std::optional<gp_Pnt> madeFaceMiddle(const QString& feature, const Selection& items, const CommandHost& host) {
  const std::string prefix = (feature + QLatin1Char(':')).toStdString();
  for (const SelectionItem& item : items) {
    if (item.kind != SelectKind::Edge) {
      continue;
    }
    const auto body = host.bodyShape(item.owner);
    if (!body) {
      continue;
    }
    const std::string edge = item.name.toStdString();
    for (int i = 0; i < body->face_count(); ++i) {
      for (const std::string& name : body->face_names(i)) {
        if (name.rfind(prefix, 0) == 0 && name.find(edge) != std::string::npos) {
          const TopoDS_Face& face = body->face(i);
          double u0 = 0.0;
          double u1 = 0.0;
          double v0 = 0.0;
          double v1 = 0.0;
          BRepTools::UVBounds(face, u0, u1, v0, v1);
          const BRepAdaptor_Surface surface(face);
          return surface.Value((u0 + u1) / 2.0, (v0 + v1) / 2.0);
        }
      }
    }
  }
  return std::nullopt;
}

// The parameter a loaded value input stands for: the feature's own, or
// one its text names; empty when it is an expression.
QString parameterOf(const CommandState& state, const QString& id, const QHash<QString, double>& numbers) {
  QString name = state.parameterName(id);
  if (name.isEmpty()) {
    name = state.text(id).trimmed();
  }
  return numbers.contains(name) ? name : QString();
}

} // namespace

ParameterPicker::ParameterPicker(OcctViewer& viewer, const QString& command)
    : QWidget(&viewer), m_viewer(viewer), m_command(command) {
  setObjectName(QStringLiteral("parameterPicker"));
  setAttribute(Qt::WA_TransparentForMouseEvents);
  setAttribute(Qt::WA_NoSystemBackground);
  setFocusPolicy(Qt::NoFocus);
  QFont small = font();
  small.setPointSizeF(9.0);
  setFont(small);
  resize(viewer.size());
  show();
  raise();
  m_viewerFilter.install(&viewer);
  connect(&viewer, &OcctViewer::viewChanged, this, [this] {
    update();
    logPlaces();
  });
}

ParameterPicker::~ParameterPicker() = default;

void ParameterPicker::setContent(std::vector<Sketch> sketches, std::vector<Value> values,
                                 QHash<QString, QString> texts) {
  m_sketches = std::move(sketches);
  m_values = std::move(values);
  m_texts = std::move(texts);
  m_hover = -1;
  QStringList shown;
  for (const Label& label : layout()) {
    shown << QStringLiteral("%1 %2").arg(label.source, label.name);
  }
  qDebug().noquote() << QStringLiteral("Parameter labels for %1: %2")
                            .arg(m_command, shown.isEmpty() ? QStringLiteral("none") : shown.join(QStringLiteral(", ")));
  m_logged.clear();
  update();
  logPlaces();
}

std::vector<ParameterPicker::Label> ParameterPicker::layout() const {
  std::vector<Label> labels;
  const QFontMetricsF metrics(font());
  const auto box = [&metrics](const QPointF& at, const QString& text) {
    const double across = metrics.horizontalAdvance(text) + 2.0 * kPadding;
    const double tall = metrics.height() + kPadding;
    return QRectF(at.x() - across / 2.0, at.y() - tall / 2.0, across, tall);
  };
  for (const Sketch& s : m_sketches) {
    const sketch::SketchModel& model = s.model;
    const gp_Trsf placement = s.placement;
    const OcctViewer& viewer = m_viewer;
    const auto toScreen = [&viewer, &model, placement](const sketch::V2& p) {
      return viewer.toWidgetF(sketch::toModel(model.frame, p).Transformed(placement));
    };
    const QPointF origin = toScreen(sketch::V2{0.0, 0.0});
    const double perMm = std::max(screenDistance(toScreen(sketch::V2{1.0, 0.0}), origin),
                                  screenDistance(toScreen(sketch::V2{0.0, 1.0}), origin));
    const sketch::Projection projection{toScreen, perMm > 1e-9 ? 1.0 / perMm : 1.0};
    for (const sketch::DimensionData& d : model.dimensions) {
      if (d.parameter.isEmpty()) {
        continue;
      }
      Label label;
      label.name = d.parameter;
      label.source = QStringLiteral("%1 %2").arg(model.name, d.id);
      label.driven = d.driven;
      label.text = QStringLiteral("%1 = %2").arg(d.parameter, m_texts.value(d.parameter));
      if (d.driven) {
        label.text = QStringLiteral("(%1)").arg(label.text);
      }
      label.shape = sketch::dimensionShape(d, model, projection, label.text);
      if (!label.shape.valid) {
        // Only the value then, where the dimension shows it.
        label.shape.textAt = toScreen(d.text ? *d.text : sketch::defaultTextPosition(d, model, projection.pixel));
      }
      label.rect = box(label.shape.textAt, label.text);
      labels.push_back(std::move(label));
    }
  }
  // A feature's labels apart from each other (a revolve's two angles share
  // the axis).
  std::vector<QRectF> placed;
  for (const Value& value : m_values) {
    Label label;
    label.name = value.name;
    label.source = QStringLiteral("%1 %2").arg(value.feature, value.input);
    label.text = QStringLiteral("%1 = %2").arg(value.name, m_texts.value(value.name));
    label.feature = true;
    QRectF place = box(m_viewer.toWidgetF(value.at) - QPointF(0.0, kFeatureLift), label.text);
    for (const QRectF& other : placed) {
      if (place.intersects(other)) {
        place.moveTop(other.bottom() + 2.0);
      }
    }
    placed.push_back(place);
    label.rect = place;
    labels.push_back(std::move(label));
  }
  return labels;
}

int ParameterPicker::labelAt(const QPointF& position) const {
  const std::vector<Label> labels = layout();
  // The one drawn last is on top.
  for (int i = static_cast<int>(labels.size()) - 1; i >= 0; --i) {
    if (labels[static_cast<std::size_t>(i)].rect.adjusted(-2, -2, 2, 2).contains(position)) {
      return i;
    }
  }
  return -1;
}

bool ParameterPicker::eventFilter(QObject* watched, QEvent* event) {
  if (watched != &m_viewer) {
    return QWidget::eventFilter(watched, event);
  }
  // An exception must not leave a Qt event handler: geometry that cannot be
  // drawn takes no clicks.
  try {
    switch (event->type()) {
    case QEvent::Resize:
      resize(m_viewer.size());
      return false;
    case QEvent::MouseButtonPress:
    case QEvent::MouseButtonDblClick: {
      auto* e = static_cast<QMouseEvent*>(event);
      if (e->button() != Qt::LeftButton) {
        return false;
      }
      const int at = labelAt(e->position());
      if (at < 0) {
        return false;
      }
      m_pressed = true;
      if (event->type() == QEvent::MouseButtonPress) {
        const Label label = layout()[static_cast<std::size_t>(at)];
        qDebug().noquote() << QStringLiteral("Parameter label picked: %1 (%2)").arg(label.name, label.source);
        const QPointer<ParameterPicker> alive(this);
        emit picked(label.name, label.source);
        if (!alive) {
          return true;
        }
      }
      return true;
    }
    case QEvent::MouseButtonRelease: {
      auto* e = static_cast<QMouseEvent*>(event);
      if (e->button() == Qt::LeftButton && m_pressed) {
        m_pressed = false;
        return true;
      }
      return false;
    }
    case QEvent::MouseMove: {
      auto* e = static_cast<QMouseEvent*>(event);
      const int at = labelAt(e->position());
      if (at != m_hover) {
        m_hover = at;
        update();
      }
      return m_pressed;
    }
    default:
      return false;
    }
  } catch (const Standard_Failure&) {
    return false;
  } catch (const std::exception&) {
    return false;
  }
}

void ParameterPicker::paintEvent(QPaintEvent*) {
  QPainter painter(this);
  painter.setRenderHint(QPainter::Antialiasing);
  std::vector<Label> labels;
  try {
    labels = layout();
  } catch (const Standard_Failure&) {
    return;
  } catch (const std::exception&) {
    return;
  }
  for (std::size_t i = 0; i < labels.size(); ++i) {
    const Label& label = labels[i];
    const bool hover = static_cast<int>(i) == m_hover;
    const QColor color = hover ? kHoverColor : label.feature ? kFeatureColor : kDimensionColor;
    if (label.shape.valid) {
      painter.setPen(QPen(color, 1.0));
      painter.setBrush(Qt::NoBrush);
      for (const QLineF& line : label.shape.lines) {
        painter.drawLine(line);
      }
      for (const QPolygonF& arc : label.shape.arcs) {
        painter.drawPolyline(arc);
      }
      painter.setBrush(color);
      for (const auto& [tip, towards] : label.shape.arrows) {
        const QPointF along = unitOf(towards);
        const QPointF across(-along.y(), along.x());
        QPolygonF head;
        head << tip << tip - along * 8.0 + across * 3.0 << tip - along * 8.0 - across * 3.0;
        painter.drawPolygon(head);
      }
    }
    painter.setPen(QPen(color, hover ? 2.0 : 1.0));
    painter.setBrush(kBoxColor);
    painter.drawRoundedRect(label.rect, 3.0, 3.0);
    painter.setPen(label.driven ? kDrivenTextColor : kTextColor);
    painter.drawText(label.rect, Qt::AlignCenter, label.text);
  }
}

void ParameterPicker::logPlaces() {
  if (!isVisible()) {
    return;
  }
  QStringList lines;
  try {
    for (const Label& label : layout()) {
      const QPoint at = mapTo(window(), label.rect.center().toPoint());
      lines << QStringLiteral("Parameter label %1 %2 at %3,%4").arg(label.source, label.name).arg(at.x()).arg(at.y());
    }
  } catch (const Standard_Failure&) {
    return;
  } catch (const std::exception&) {
    return;
  }
  const QString logged = lines.join(QLatin1Char('\n'));
  if (logged != m_logged) {
    m_logged = logged;
    for (const QString& line : lines) {
      qDebug().noquote() << line;
    }
  }
}

void gatherParameterLabels(const CommandHost& host, int features, std::vector<ParameterPicker::Sketch>& sketches,
                           std::vector<ParameterPicker::Value>& values, QHash<QString, QString>& texts) {
  // Every parameter's value: in its unit for the labels, in mm or radians
  // for the handles.
  QHash<QString, double> numbers;
  try {
    for (const QJsonValue& entry : host.queryArray(QStringLiteral("parameters"))) {
      const QJsonObject p = entry.toObject();
      const QString name = p.value(QStringLiteral("name")).toString();
      texts.insert(name, p.value(QStringLiteral("text")).toString());
      numbers.insert(name, p.value(QStringLiteral("value")).toDouble());
    }
  } catch (const std::exception&) {
    return;
  }

  // The dimensions of the sketches shown.
  for (const CommandHost::ShownSketch& shown : host.shownSketches()) {
    ParameterPicker::Sketch entry;
    try {
      if (!entry.model.load(host.queryObject(
              {{QStringLiteral("query"), QStringLiteral("sketch")}, {QStringLiteral("uid"), shown.uid}}))) {
        continue;
      }
    } catch (const std::exception&) {
      continue; // a sketch that was just taken back
    }
    const auto& dimensions = entry.model.dimensions;
    if (std::none_of(dimensions.begin(), dimensions.end(),
                     [](const sketch::DimensionData& d) { return !d.parameter.isEmpty(); })) {
      continue;
    }
    entry.placement = shown.placement;
    sketches.push_back(std::move(entry));
  }

  // The values of the features before the marker, where their panels'
  // handles would be.
  QJsonObject timeline;
  try {
    timeline = host.queryObject({{QStringLiteral("query"), QStringLiteral("timeline")}});
  } catch (const std::exception&) {
    return;
  }
  const QJsonArray list = timeline.value(QStringLiteral("features")).toArray();
  const int marker = std::min(timeline.value(QStringLiteral("marker")).toInt(static_cast<int>(list.size())),
                              static_cast<int>(list.size()));
  int taken = 0;
  for (int i = marker - 1; i >= 0 && taken < features; --i) {
    const QJsonObject f = list[i].toObject();
    const QString status = f.value(QStringLiteral("status")).toString();
    if ((status != QStringLiteral("ok") && status != QStringLiteral("warning")) ||
        f.value(QStringLiteral("type")).toString() == QStringLiteral("sketch")) {
      continue;
    }
    const QString uid = f.value(QStringLiteral("uid")).toString();
    const QString name = f.value(QStringLiteral("name")).toString();
    try {
      QJsonObject def = host.queryObject({{QStringLiteral("query"), QStringLiteral("feature")},
                                          {QStringLiteral("uid"), uid}})
                            .value(QStringLiteral("def"))
                            .toObject();
      const CommandDef* command = host.featureEditor(def);
      if (command == nullptr || !command->load) {
        continue;
      }
      ++taken;
      // As its panel would show it, with its parameters' values.
      def.insert(QStringLiteral("uid"), uid);
      CommandState state;
      for (const InputDef& input : command->inputs) {
        setDefault(input, input.id, state);
      }
      command->load(def, state, host);
      // The rows it loaded (fillet and chamfer sets) as the panel fills
      // them, and every value input's value, the rows' too.
      const auto valueOf = [&state, &numbers](const QString& key) {
        if (const QString parameter = parameterOf(state, key, numbers); !parameter.isEmpty()) {
          state.setValue(key, numbers.value(parameter));
        }
      };
      for (const InputDef& input : command->inputs) {
        if (input.type == InputDef::Type::Value) {
          valueOf(input.id);
        }
        if (input.type != InputDef::Type::List) {
          continue;
        }
        for (int row = 0; row < state.rows(input.id); ++row) {
          for (const InputDef& child : input.children) {
            const QString key = CommandState::rowKey(input.id, row, child.id);
            setDefault(child, key, state);
            if (child.type == InputDef::Type::Value) {
              valueOf(key);
            }
          }
        }
      }
      // A label at each value's handle, where the panel would show it.
      for (const ValueSlot& slot : valueSlots(*command, state)) {
        const QString parameter = parameterOf(state, slot.key, numbers);
        if (parameter.isEmpty()) {
          continue;
        }
        const std::optional<Manipulator> handle =
            slot.input->manipulator ? slot.input->manipulator(slot.scope, host) : std::nullopt;
        if (handle && (handle->kind == Manipulator::Kind::Arrow || handle->kind == Manipulator::Kind::Ring)) {
          gp_Pnt at = handle->origin;
          if (handle->kind == Manipulator::Kind::Arrow) {
            at.Translate(gp_Vec(handle->direction) * ((numbers.value(parameter) - handle->base) * handle->factor));
          }
          values.push_back({name, slot.key, parameter, at});
        } else if (!handle) {
          // A handle on edges the feature took away (a fillet's, a
          // chamfer's set): where it was, on the face made of them.
          if (const auto at = madeFaceMiddle(uid, slot.items, host)) {
            values.push_back({name, slot.key, parameter, *at});
          }
        }
      }
    } catch (const Standard_Failure&) {
      continue; // a feature whose handles cannot be worked out
    } catch (const std::exception&) {
      continue;
    }
  }
}

} // namespace mitcad
