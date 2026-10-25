// SPDX-License-Identifier: MIT
// Blend surfaces of imported bodies stored without their approximation
// (mitcad#621), made with OCCT's own blend machinery.
#pragma once

#include <Geom2d_Curve.hxx>
#include <Geom_Curve.hxx>
#include <Geom_Surface.hxx>

namespace mitcad::brep {

// What a rolling ball touches on one side: a surface, or a curve (an edge).
// Exactly one of the two is set.
struct BlendSupport {
  Handle(Geom_Surface) surface;
  Handle(Geom_Curve) curve;
};

// The surface a ball of `radius` sweeps while its centre runs along `spine`
// from parameter v0 to v1 touching both supports, at least over [n0, n1]
// (where the ball may leave a support in the rest). OCCT walks the ball along
// the spine (BRepBlend_Walking with BlendFunc_ConstRad between two surfaces,
// BRepBlend_SurfRstLineBuilder with the rolling ball of
// BRepBlend_SurfRstConstRad between a surface and a curve) and approximates
// the sections to a B-spline (BRepBlend_AppSurface). The sections are the
// minor arcs between the contacts; u runs across them in [0, 1] from the
// first support's contact to the second's, v is the spine's parameter. Over
// one whole period of a closed spine the surface is closed, periodic in v
// (mitcad#622). Throws std::runtime_error saying why it cannot.
Handle(Geom_Surface) rolling_ball_surface(const BlendSupport& first, const BlendSupport& second,
                                          const Handle(Geom_Curve)& spine, double radius, double v0, double v1,
                                          double n0, double n1);

// The surface along a curve in its parameter space, at the curve's
// parameter: the surface's iso curve where the parameter curve is one along
// the other parameter, else its approximation. Throws std::runtime_error.
Handle(Geom_Curve) curve_on_surface(const Handle(Geom_Surface)& surface, const Handle(Geom2d_Curve)& pcurve);

} // namespace mitcad::brep
