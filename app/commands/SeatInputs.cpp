// SPDX-License-Identifier: MIT
// Seats on shafts (mitcad#578): the panel inputs every seated feature
// shares (SeatInputs.hpp).
#include "SeatInputs.hpp"

#include <cmath>
#include <limits>

#include <QJsonArray>
#include <QJsonValue>
#include <QObject>
#include <QStringList>

#include <gp_Pnt.hxx>
#include <gp_Vec.hxx>

#include "CommandSupport.hpp"

namespace mitcad::cmd {
namespace {

QString shaftKey() { return QStringLiteral("shaft"); }
QString offsetKey() { return QStringLiteral("seat_offset"); }
QString flipKey() { return QStringLiteral("seat_flip"); }
QString clearanceKey() { return QStringLiteral("bore_clearance"); }
// Kept beside the inputs: where a face was picked, where the feature's
// component is seen, and the seat before the shaft was cleared.
QString pickKey() { return QStringLiteral("seat_pick"); }
QString contextKey() { return QStringLiteral("seat_context"); }
QString lastKey() { return QStringLiteral("seat_last"); }

// A shaft's circular edge or cylindrical face.
bool isRound(const SelectionItem& item) {
  return (item.kind == SelectKind::Edge && isCircular(item)) ||
         (item.kind == SelectKind::Face && item.geometry == QStringLiteral("cylinder"));
}

QJsonArray occurrencesOf(const CommandContext& context) {
  try {
    return context.queryObject({{QStringLiteral("query"), QStringLiteral("components")}})
        .value(QStringLiteral("occurrences"))
        .toArray();
  } catch (...) {
    return QJsonArray();
  }
}

// The first path to `component` in a depth-first walk of the occurrences,
// where the view shows it ("" for the root).
QString pathTo(const QJsonArray& level, const QString& parent, const QString& component) {
  for (const QJsonValue& value : level) {
    const QJsonObject occurrence = value.toObject();
    const QString uid = occurrence.value(QStringLiteral("uid")).toString();
    const QString path = parent.isEmpty() ? uid : parent + QLatin1Char('/') + uid;
    if (occurrence.value(QStringLiteral("component")).toString() == component) {
      return path;
    }
    const QString child = pathTo(occurrence.value(QStringLiteral("children")).toArray(), path, component);
    if (!child.isEmpty()) {
      return child;
    }
  }
  return QString();
}

// The component at the end of an occurrence path ("C0" for the root);
// empty when the path leads nowhere.
QString componentAt(const QJsonArray& occurrences, const QString& path) {
  QString component = QStringLiteral("C0");
  QJsonArray level = occurrences;
  for (const QString& uid : path.split(QLatin1Char('/'), Qt::SkipEmptyParts)) {
    bool found = false;
    for (const QJsonValue& value : level) {
      const QJsonObject occurrence = value.toObject();
      if (occurrence.value(QStringLiteral("uid")).toString() == uid) {
        component = occurrence.value(QStringLiteral("component")).toString();
        level = occurrence.value(QStringLiteral("children")).toArray();
        found = true;
        break;
      }
    }
    if (!found) {
      return QString();
    }
  }
  return component;
}

// Where the feature's component is seen: `path` when given, else the
// first path to it; returns that path.
QString setContext(CommandState& state, const CommandContext& context, const QString& component,
                   const QString& path) {
  QString seen;
  if (component != QStringLiteral("C0")) {
    seen = path.isEmpty() ? pathTo(occurrencesOf(context), QString(), component) : path;
  }
  state.setObject(contextKey(), {{QStringLiteral("component"), component}, {QStringLiteral("path"), seen}});
  return seen;
}

bool offeredIn(const SeatPanel& panel, const CommandState& state) {
  return !panel.offered || panel.offered(state);
}

// The seat of the inputs, whether the panel offers it or not.
QJsonObject seatOf(const CommandState& state, const CommandContext& context) {
  const Selection& shaft = state.items(shaftKey());
  if (shaft.isEmpty()) {
    return QJsonObject();
  }
  const SelectionItem& item = shaft.first();
  QJsonObject seat{{QStringLiteral("shaft"), item.reference()}};
  if (nonZero(state, offsetKey())) {
    seat.insert(QStringLiteral("offset"), state.expression(offsetKey()));
  }
  if (state.checked(flipKey())) {
    seat.insert(QStringLiteral("flip"), true);
  }
  const QJsonArray near = state.object(pickKey()).value(QStringLiteral("near")).toArray();
  if (item.kind == SelectKind::Face && near.size() == 3) {
    seat.insert(QStringLiteral("near"), near);
  }
  // A shaft of another component than the feature's: read where it was
  // picked, the feature's component where the view shows it.
  const QJsonObject where = state.object(contextKey());
  const QString own = where.value(QStringLiteral("component")).toString(QStringLiteral("C0"));
  const QString component = componentAt(occurrencesOf(context), item.occurrence);
  if (!component.isEmpty() && component != own) {
    QJsonObject link{{QStringLiteral("source"), item.occurrence}};
    const QString target = where.value(QStringLiteral("path")).toString();
    if (!target.isEmpty()) {
      link.insert(QStringLiteral("target"), target);
    }
    seat.insert(QStringLiteral("link"), link);
  }
  return seat;
}

// A cleared shaft: the plane and centre where the seat had the part (an
// origin plane square to the shaft's axis when there is one, the centre
// where the axis meets it).
void unseat(CommandState& state, const CommandContext& context, const SeatPanel& panel) {
  QJsonObject seat = state.object(lastKey());
  state.setObject(lastKey(), QJsonObject());
  if (seat.isEmpty() || panel.plane.isEmpty()) {
    return;
  }
  seat.remove(QStringLiteral("offset"));
  seat.remove(QStringLiteral("flip"));
  if (nonZero(state, offsetKey())) {
    seat.insert(QStringLiteral("offset"), state.expression(offsetKey()));
  }
  if (state.checked(flipKey())) {
    seat.insert(QStringLiteral("flip"), true);
  }
  QJsonObject frame;
  try {
    frame = context.queryObject({{QStringLiteral("query"), QStringLiteral("seat_frame")},
                                 {QStringLiteral("seat"), seat},
                                 {QStringLiteral("component"),
                                  state.object(contextKey()).value(QStringLiteral("component")).toString(QStringLiteral("C0"))}});
  } catch (...) {
    return;
  }
  const QJsonArray origin = frame.value(QStringLiteral("origin")).toArray();
  const QJsonArray axis = frame.value(QStringLiteral("axis")).toArray();
  if (origin.size() != 3 || axis.size() != 3) {
    return;
  }
  QString plane;
  if (std::abs(axis.at(2).toDouble()) > 0.999) {
    plane = QStringLiteral("xy");
  } else if (std::abs(axis.at(1).toDouble()) > 0.999) {
    plane = QStringLiteral("xz");
  } else if (std::abs(axis.at(0).toDouble()) > 0.999) {
    plane = QStringLiteral("yz");
  }
  if (!plane.isEmpty() || state.items(panel.plane).isEmpty()) {
    state.setItems(panel.plane, {itemOf(QJsonValue(plane.isEmpty() ? QStringLiteral("xy") : plane), context)});
  }
  const auto planeFrame = frameOf(state.items(panel.plane).first(), context);
  if (!planeFrame || panel.x.isEmpty() || panel.y.isEmpty()) {
    return;
  }
  const gp_Vec offset(planeFrame->origin, gp_Pnt(origin.at(0).toDouble(), origin.at(1).toDouble(), origin.at(2).toDouble()));
  const auto round = [](double value) { return std::round(value * 100.0) / 100.0; };
  state.setText(panel.x, lengthText(round(offset.Dot(gp_Vec(planeFrame->x))), context));
  state.setText(panel.y, lengthText(round(offset.Dot(gp_Vec(planeFrame->y))), context));
  state.setValue(panel.x, std::numeric_limits<double>::quiet_NaN());
  state.setValue(panel.y, std::numeric_limits<double>::quiet_NaN());
}

} // namespace

QVector<InputDef> seatInputs(const SeatPanel& panel) {
  const auto offered = [panel](const CommandState& state) { return offeredIn(panel, state); };
  const auto shown = [panel](const CommandState& state) { return seated(state, panel); };
  QVector<InputDef> inputs = {
      selectionInput(shaftKey(), QObject::tr("Shaft"), SelectKind::Edge | SelectKind::Face, 0, 1)
          .withTooltip(QObject::tr("Pick a shaft edge or face, or a hole rim or wall; the bore follows its diameter"))
          .withAccepts(isRound)
          .withOnPick([](const SelectionItem& item, CommandState& state, const CommandContext&) {
            QJsonObject pick;
            if (item.kind == SelectKind::Face && item.at) {
              pick.insert(QStringLiteral("near"), QJsonArray{(*item.at)[0], (*item.at)[1], (*item.at)[2]});
            }
            state.setObject(pickKey(), pick);
          })
          .withOnChange([panel](CommandState& state, const CommandContext& context) {
            if (state.items(shaftKey()).isEmpty()) {
              unseat(state, context, panel);
            } else {
              state.setObject(lastKey(), seatOf(state, context));
            }
          })
          .withVisible(offered),
      valueInput(offsetKey(), QObject::tr("Offset"), ValueKind::Length, QStringLiteral("0 mm"))
          .withTooltip(QObject::tr("Along the shaft from its end to where the part starts"))
          .withVisible(shown),
      flipInput(flipKey(), QObject::tr("Flip"))
          .withTooltip(QObject::tr("From the shaft's other end, facing back along it"))
          .withVisible(shown),
  };
  if (panel.clearance) {
    inputs.append(valueInput(clearanceKey(), QObject::tr("Bore Clearance"), ValueKind::Length, QStringLiteral("0 mm"))
                      .withTooltip(QObject::tr("Added to the shaft's diameter for the bore (0.2 mm: a printed fit)"))
                      .withVisible(shown));
  }
  return inputs;
}

bool seated(const CommandState& state, const SeatPanel& panel) {
  return !state.items(shaftKey()).isEmpty() && offeredIn(panel, state);
}

void initSeat(CommandState& state, const CommandContext& context) {
  QString active = QStringLiteral("C0");
  try {
    active = context.queryObject({{QStringLiteral("query"), QStringLiteral("components")}})
                 .value(QStringLiteral("active"))
                 .toString(QStringLiteral("C0"));
  } catch (...) {
  }
  setContext(state, context, active, QString());
}

void addSeat(QJsonObject& def, const CommandState& state, const CommandContext& context, const SeatPanel& panel) {
  if (!seated(state, panel)) {
    return;
  }
  def.insert(QStringLiteral("seat"), seatOf(state, context));
  if (panel.clearance && nonZero(state, clearanceKey())) {
    def.insert(QStringLiteral("bore_clearance"), state.expression(clearanceKey()));
  }
}

bool loadSeat(const QJsonObject& feature, CommandState& state, const CommandContext& context) {
  QString component = QStringLiteral("C0");
  try {
    component = context.queryObject({{QStringLiteral("query"), QStringLiteral("feature")},
                                     {QStringLiteral("uid"), feature.value(QStringLiteral("uid"))}})
                    .value(QStringLiteral("component"))
                    .toString(QStringLiteral("C0"));
  } catch (...) {
  }
  const QJsonObject seat = feature.value(QStringLiteral("seat")).toObject();
  const QJsonValue link = seat.value(QStringLiteral("link"));
  const QString seen = setContext(state, context, component,
                                  link.toObject().value(QStringLiteral("target")).toString());
  if (seat.isEmpty()) {
    return false;
  }
  // The shaft where it was picked: a linked one in its occurrence, one of
  // the feature's own component where that is seen.
  SelectionItem shaft = itemOf(seat.value(QStringLiteral("shaft")), context);
  shaft.occurrence = link.isObject() ? link.toObject().value(QStringLiteral("source")).toString() : seen;
  state.setItems(shaftKey(), {shaft});
  QJsonObject pick;
  if (seat.value(QStringLiteral("near")).isArray()) {
    pick.insert(QStringLiteral("near"), seat.value(QStringLiteral("near")));
  }
  state.setObject(pickKey(), pick);
  loadValue(state, offsetKey(), seat.value(QStringLiteral("offset")), feature, context);
  state.setChecked(flipKey(), seat.value(QStringLiteral("flip")).toBool());
  loadValue(state, clearanceKey(), feature.value(QStringLiteral("bore_clearance")), feature, context);
  state.setObject(lastKey(), seatOf(state, context));
  return true;
}

bool seatShowable(const QJsonObject& feature, const CommandContext& context) {
  const QJsonValue seat = feature.value(QStringLiteral("seat"));
  if (seat.isUndefined() || seat.isNull()) {
    return true;
  }
  const QJsonValue shaft = seat.toObject().value(QStringLiteral("shaft"));
  return isShowable(shaft, context) && itemOf(shaft, context).kind != SelectKind::None;
}

QString seatNote(const QJsonObject& values, const CommandContext& context) {
  if (!values.contains(QStringLiteral("shaft_diameter")) || !values.contains(QStringLiteral("bore_diameter"))) {
    return QString();
  }
  return QObject::tr("bore %1 (shaft %2)")
      .arg(lengthText(values.value(QStringLiteral("bore_diameter")).toDouble(), context),
           lengthText(values.value(QStringLiteral("shaft_diameter")).toDouble(), context));
}

} // namespace mitcad::cmd
