// SPDX-License-Identifier: MIT
#include "MoveDragMotion.hpp"

#include <cmath>

#include <QString>

#include <gp_Ax1.hxx>
#include <gp_Dir.hxx>
#include <gp_Vec.hxx>
#include <gp_XYZ.hxx>

namespace mitcad::cmd {

gp_Trsf freeRotation(double rx, double ry, double rz) {
  gp_Trsf x;
  x.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(1, 0, 0)), rx);
  gp_Trsf y;
  y.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 1, 0)), ry);
  gp_Trsf z;
  z.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), rz);
  return z * y * x;
}

std::optional<gp_Trsf> moveMotion(const CommandState& state, const gp_Pnt& pivot) {
  const QString type = state.choice(QStringLiteral("type"));
  const auto value = [&state](const char* id) { return state.value(QString::fromLatin1(id)); };
  if (type == QStringLiteral("free")) {
    const double rx = value("rx");
    const double ry = value("ry");
    const double rz = value("rz");
    const gp_XYZ t(value("tx"), value("ty"), value("tz"));
    if (!std::isfinite(rx) || !std::isfinite(ry) || !std::isfinite(rz) || !std::isfinite(t.X()) ||
        !std::isfinite(t.Y()) || !std::isfinite(t.Z())) {
      return std::nullopt;
    }
    // p -> R (p - c) + c + t, as the definition's matrix (transformOf).
    const gp_Trsf rotation = freeRotation(rx, ry, rz);
    gp_XYZ turned = pivot.XYZ();
    rotation.Transforms(turned);
    const gp_XYZ moved = pivot.XYZ() - turned + t;
    gp_Trsf motion;
    motion.SetValues(rotation.Value(1, 1), rotation.Value(1, 2), rotation.Value(1, 3), moved.X(),
                     rotation.Value(2, 1), rotation.Value(2, 2), rotation.Value(2, 3), moved.Y(),
                     rotation.Value(3, 1), rotation.Value(3, 2), rotation.Value(3, 3), moved.Z());
    return motion;
  }
  if (type == QStringLiteral("translate")) {
    const gp_XYZ t(value("x"), value("y"), value("z"));
    if (!std::isfinite(t.X()) || !std::isfinite(t.Y()) || !std::isfinite(t.Z())) {
      return std::nullopt;
    }
    gp_Trsf motion;
    motion.SetTranslation(gp_Vec(t));
    return motion;
  }
  return std::nullopt;
}

} // namespace mitcad::cmd
