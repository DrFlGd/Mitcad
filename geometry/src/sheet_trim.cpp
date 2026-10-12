// SPDX-License-Identifier: MIT
#include "mitcad/geometry/sheet_trim.hpp"

#include <algorithm>
#include <cmath>
#include <map>
#include <memory>
#include <stdexcept>
#include <utility>

#include <BRepAlgoAPI_Common.hxx>
#include <BRepAlgoAPI_Cut.hxx>
#include <BRepAlgoAPI_Fuse.hxx>
#include <BRepBuilderAPI_MakeEdge.hxx>
#include <BRepBuilderAPI_MakeFace.hxx>
#include <BRepBuilderAPI_MakeWire.hxx>
#include <BRepGProp.hxx>
#include <BRepTools.hxx>
#include <BRepTools_WireExplorer.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <GProp_GProps.hxx>
#include <Geom2dAPI_InterCurveCurve.hxx>
#include <Geom2dAPI_ProjectPointOnCurve.hxx>
#include <Geom2d_TrimmedCurve.hxx>
#include <GeomAPI.hxx>
#include <NCollection_List.hxx>
#include <ShapeFix_Face.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <gp_Pln.hxx>

#include "sheet_curve.hpp"
#include "util.hpp"

namespace mitcad::geometry {
namespace {

constexpr double kParamTolerance = 1.0e-10;

double low(ParamSpan span) { return std::min(span.first, span.last); }
double high(ParamSpan span) { return std::max(span.first, span.last); }

struct SourceEdge {
  TrimSource source;
  TopoDS_Edge edge;
  detail::Curve2 curve; // in the common operation chart
};

struct Context {
  const SheetTrimSpec& spec;
  std::vector<SourceEdge> sources;
  // Proven history per returned edge. Splits inherit this relation; never
  // recover identity by proximity to an unrelated geometric edge.
  std::map<std::uint32_t, std::vector<std::size_t>> history;
  std::uint32_t next_edge = 1;
  std::uint32_t next_region = 1;
  std::uint32_t next_split = 1;
};

const TrimDomain& domain_of(const Context& ctx, const SheetDomainRef& id) {
  const auto found = std::find_if(ctx.spec.domains.begin(), ctx.spec.domains.end(),
                                [&](const TrimDomain& d) { return d.domain == id; });
  if (found == ctx.spec.domains.end()) {
    throw std::invalid_argument("sheet seam refers to an unknown domain");
  }
  return *found;
}

TopoDS_Shape compound(const std::vector<TopoDS_Face>& faces) {
  BRep_Builder builder;
  TopoDS_Compound out;
  builder.MakeCompound(out);
  for (const auto& face : faces) {
    builder.Add(out, face);
  }
  return out;
}

TopoDS_Face material_face(Context& ctx, const MaterialRegion& region,
                         const SheetDomainRef& domain, const Isometry2& placement, bool tool) {
  const auto built = detail::make_sheet_face(region, placement, ctx.spec.tolerance);
  for (std::size_t l = 0; l < region.loops.size(); ++l) {
    const auto& loop = region.loops[l];
    for (std::size_t e = 0; e < loop.edges.size(); ++e) {
      throw_if_cancelled();
      const auto& input = loop.edges[e];
      detail::Curve2 curve = detail::make_sheet_curve(input.curve);
      curve.curve = detail::transform_sheet_curve(curve.curve, placement);
      ctx.sources.push_back({{tool ? TrimSourceKind::Tool : TrimSourceKind::Domain,
                              tool ? SheetDomainRef{} : domain, input.piece}, built.edges[l][e], curve});
    }
  }
  return built.face;
}

bool contains(const NCollection_List<TopoDS_Shape>& images, const TopoDS_Edge& edge) {
  return std::any_of(images.begin(), images.end(),
                     [&](const TopoDS_Shape& image) { return image.IsSame(edge); });
}

std::vector<double> parameters_at(const SourceEdge& source, const gp_Pnt2d& point, double tolerance) {
  const double first = low(source.curve.native_range);
  const double last = high(source.curve.native_range);
  std::vector<double> values;
  auto add = [&](double value) {
    if (source.curve.curve->Value(value).Distance(point) <= tolerance &&
        std::none_of(values.begin(), values.end(), [&](double old) {
          return std::abs(old - value) <= kParamTolerance * std::max(1.0, last - first);
        })) {
      values.push_back(value);
    }
  };
  add(first);
  add(last);
  Geom2dAPI_ProjectPointOnCurve project(point, source.curve.curve, first, last);
  for (int i = 1; i <= project.NbPoints(); ++i) {
    if (project.Distance(i) <= tolerance) {
      add(project.Parameter(i));
    }
  }
  return values;
}

SourceSpan source_span(const SourceEdge& source, const detail::Curve2& output, double tolerance) {
  const auto starts = parameters_at(source, output.curve->Value(output.native_range.first), tolerance);
  const auto ends = parameters_at(source, output.curve->Value(output.native_range.last), tolerance);
  const double middle = (output.native_range.first + output.native_range.last) / 2.0;
  const auto middles = parameters_at(source, output.curve->Value(middle), tolerance);
  const double epsilon = kParamTolerance * std::max(1.0, high(source.curve.native_range) -
                                                         low(source.curve.native_range));
  gp_Pnt2d point;
  gp_Vec2d output_tangent;
  output.curve->D1(middle, point, output_tangent);
  if (output.native_range.last < output.native_range.first) {
    output_tangent.Reverse();
  }
  std::vector<ParamSpan> matches;
  for (double a : starts) {
    for (double b : ends) {
      if (std::abs(a - b) <= epsilon) {
        continue;
      }
      bool covered = false;
      for (double m : middles) {
        if (m < std::min(a, b) - epsilon || m > std::max(a, b) + epsilon) {
          continue;
        }
        gp_Vec2d input_tangent;
        source.curve.curve->D1(m, point, input_tangent);
        if (b < a) {
          input_tangent.Reverse();
        }
        covered = covered || input_tangent.Dot(output_tangent) > 0.0;
      }
      if (covered) {
        matches.push_back({a, b});
      }
    }
  }
  if (matches.size() != 1) {
    throw std::runtime_error("sheet boundary history has ambiguous bounded-curve correspondence");
  }
  return {source.source, {source.curve.public_parameter(matches.front().first),
                          source.curve.public_parameter(matches.front().last)}};
}

void lineage(Context& ctx, const SheetDomainRef& domain, TrimEdge& edge) {
  detail::Curve2 output = detail::make_sheet_curve(edge.curve);
  output.curve = detail::transform_sheet_curve(output.curve, domain_of(ctx, domain).to_operation);
  edge.lineage.clear();
  for (std::size_t index : ctx.history.at(edge.token)) {
    const SourceSpan span = source_span(ctx.sources.at(index), output, ctx.spec.tolerance);
    const bool duplicate = std::any_of(edge.lineage.begin(), edge.lineage.end(), [&](const SourceSpan& s) {
      return s.source == span.source && std::abs(s.span.first - span.span.first) < kParamTolerance &&
             std::abs(s.span.last - span.span.last) < kParamTolerance;
    });
    if (!duplicate) {
      edge.lineage.push_back(span);
    }
  }
  if (edge.lineage.empty()) {
    throw std::runtime_error("sheet result boundary has no proven source history");
  }
}

TrimEdge extract(Context& ctx, const TrimDomain& domain, const TopoDS_Face& face,
                 const TopoDS_Edge& edge, BRepAlgoAPI_BooleanOperation* operation,
                 const std::vector<std::size_t>& sources) {
  double first = 0.0;
  double last = 0.0;
  const auto pcurve = BRep_Tool::CurveOnSurface(edge, face, first, last);
  if (pcurve.IsNull()) {
    throw std::runtime_error("sheet result edge has no exact planar curve");
  }
  if (edge.Orientation() == TopAbs_REVERSED) {
    std::swap(first, last);
  }
  // The planar Boolean operates on the canonical XY surface, so its pcurves
  // are in operation coordinates. Do not approximate them through 3D samples.
  const auto local = detail::transform_sheet_curve(pcurve, domain.to_operation, true);
  TrimEdge out{ctx.next_edge++, detail::export_sheet_curve(local, {first, last}), {}};
  auto& ancestors = ctx.history[out.token];
  for (std::size_t index : sources) {
    const TopoDS_Edge& source = ctx.sources[index].edge;
    if (source.IsSame(edge) ||
        (operation && (contains(operation->Modified(source), edge) ||
                       contains(operation->Generated(source), edge)))) {
      ancestors.push_back(index);
    }
  }
  lineage(ctx, domain.domain, out);
  return out;
}

TrimDomainResult trim_domain(Context& ctx, const TrimDomain& domain,
                             const std::vector<TopoDS_Face>& tools, const std::vector<std::size_t>& tool_sources) {
  const std::size_t start = ctx.sources.size();
  std::vector<TopoDS_Face> faces;
  for (const auto& region : domain.regions) {
    faces.push_back(material_face(ctx, region, domain.domain, domain.to_operation, false));
  }
  std::vector<std::size_t> sources = tool_sources;
  for (std::size_t i = start; i < ctx.sources.size(); ++i) {
    sources.push_back(i);
  }
  TopoDS_Shape result = compound(faces);
  std::unique_ptr<BRepAlgoAPI_BooleanOperation> operation;
  if (!ctx.spec.tools.empty()) {
    switch (ctx.spec.operation) {
    case SheetTrimOperation::Union: operation = std::make_unique<BRepAlgoAPI_Fuse>(); break;
    case SheetTrimOperation::Difference: operation = std::make_unique<BRepAlgoAPI_Cut>(); break;
    case SheetTrimOperation::Intersection: operation = std::make_unique<BRepAlgoAPI_Common>(); break;
    }
    NCollection_List<TopoDS_Shape> arguments;
    arguments.Append(result);
    NCollection_List<TopoDS_Shape> cutters;
    // Relief contours can touch or overlap. They are separate Boolean inputs,
    // not a compound that falsely asserts their collective topology is valid.
    // One builder keeps exact source history through all mutual intersections.
    for (const auto& tool : tools) {
      cutters.Append(tool);
    }
    operation->SetArguments(arguments);
    operation->SetTools(cutters);
    operation->SetNonDestructive(true);
    operation->SetFuzzyValue(ctx.spec.tolerance);
    detail::build(*operation);
    if (!operation->IsDone() || operation->HasErrors()) {
      throw std::runtime_error("sheet planar Boolean failed");
    }
    if (ctx.spec.operation == SheetTrimOperation::Union) {
      // One domain owns the union. Adjacent coplanar pieces must become one
      // material region: retaining their internal edge would create duplicate
      // thickness walls. Keep edge splits and the builder's merged history.
      operation->SimplifyResult(false, true);
      throw_if_cancelled();
    }
    result = operation->Shape();
  } else if (ctx.spec.operation == SheetTrimOperation::Intersection) {
    result = compound({});
  }
  TrimDomainResult out{domain.domain, {}};
  for (TopExp_Explorer it(result, TopAbs_FACE); it.More(); it.Next()) {
    throw_if_cancelled();
    const TopoDS_Face face = TopoDS::Face(it.Current());
    detail::require_valid(face, "trimmed sheet material");
    const TopoDS_Wire outer = BRepTools::OuterWire(face);
    if (outer.IsNull()) {
      throw std::runtime_error("trimmed sheet face has no outer boundary");
    }
    std::vector<TopoDS_Wire> wires{outer};
    for (TopExp_Explorer w(face, TopAbs_WIRE); w.More(); w.Next()) {
      if (!w.Current().IsSame(outer)) {
        wires.push_back(TopoDS::Wire(w.Current()));
      }
    }
    TrimRegion region{ctx.next_region++, {}};
    for (const auto& wire : wires) {
      TrimLoop loop;
      for (BRepTools_WireExplorer e(wire, face); e.More(); e.Next()) {
        loop.edges.push_back(extract(ctx, domain, face, e.Current(), operation.get(), sources));
      }
      if (loop.edges.empty()) {
        throw std::runtime_error("trimmed sheet face has an empty boundary");
      }
      region.loops.push_back(std::move(loop));
    }
    out.regions.push_back(std::move(region));
  }
  return out;
}

const SourceEdge& seam_source(const Context& ctx, const TrimSeamSide& side) {
  const SourceEdge* found = nullptr;
  for (const auto& source : ctx.sources) {
    if (source.source.kind != TrimSourceKind::Domain || source.source.domain != side.boundary.domain ||
        source.source.piece != side.boundary.piece) {
      continue;
    }
    const ParamSpan range{source.curve.public_parameter(source.curve.native_range.first),
                          source.curve.public_parameter(source.curve.native_range.last)};
    if (low(side.span) < low(range) - kParamTolerance || high(side.span) > high(range) + kParamTolerance) {
      continue;
    }
    if (found) {
      throw std::invalid_argument("sheet seam source-piece reference is ambiguous");
    }
    found = &source;
  }
  if (!found) {
    throw std::invalid_argument("sheet seam source-piece is unavailable");
  }
  // Axial maps are affine only for a genuine straight line's parameter.
  const auto bounded = detail::export_sheet_curve(found->curve.curve, found->curve.native_range);
  if (bounded.geometry.kind != SegmentKind::Line) {
    throw std::invalid_argument("sheet seam requires an analytic straight boundary");
  }
  return *found;
}

double axial(const TrimSeamSide& side, double parameter) {
  return side.axial.offset + side.axial.scale * parameter;
}

void validate_seam(const Context& ctx, const TrimSeam& seam) {
  if (seam.first.boundary.domain == seam.second.boundary.domain) {
    throw std::invalid_argument("sheet seam must join distinct domains");
  }
  for (const TrimSeamSide* side : {&seam.first, &seam.second}) {
    const auto& source = seam_source(ctx, *side);
    if (!std::isfinite(side->axial.offset) || !std::isfinite(side->axial.scale) ||
        side->axial.scale == 0.0 || !std::isfinite(side->span.first) ||
        !std::isfinite(side->span.last) || side->span.first == side->span.last) {
      throw std::invalid_argument("sheet seam has an invalid parameter map");
    }
    const ParamSpan mapped{axial(*side, side->span.first), axial(*side, side->span.last)};
    if (std::abs(low(mapped) - low(seam.axial_interval)) > ctx.spec.tolerance ||
        std::abs(high(mapped) - high(seam.axial_interval)) > ctx.spec.tolerance) {
      throw std::invalid_argument("sheet seam sides do not cover the declared axial interval");
    }
    const auto a = source.curve.curve->Value(source.curve.native_parameter(side->span.first));
    const auto b = source.curve.curve->Value(source.curve.native_parameter(side->span.last));
    if (std::abs(a.Distance(b) - (high(mapped) - low(mapped))) > ctx.spec.tolerance) {
      throw std::invalid_argument("sheet seam axial map must measure physical millimetres");
    }
  }
  for (double v : {seam.axial_interval.first, seam.axial_interval.last}) {
    const auto& a = seam_source(ctx, seam.first);
    const auto& b = seam_source(ctx, seam.second);
    const auto p = a.curve.curve->Value(a.curve.native_parameter((v - seam.first.axial.offset) / seam.first.axial.scale));
    const auto q = b.curve.curve->Value(b.curve.native_parameter((v - seam.second.axial.offset) / seam.second.axial.scale));
    if (p.Distance(q) > ctx.spec.tolerance) {
      throw std::invalid_argument("sheet seam sides disagree in the operation chart");
    }
  }
}

struct PortEdge {
  std::uint32_t token;
  ParamSpan range;
  ParamSpan axial_range;
};

std::vector<PortEdge> ports_of(const SheetTrimResult& result, const TrimSeamSide& side) {
  std::vector<PortEdge> ports;
  for (const auto& domain : result.domains) {
    if (domain.domain != side.boundary.domain) {
      continue;
    }
    for (const auto& region : domain.regions) {
      for (const auto& loop : region.loops) {
        for (const auto& edge : loop.edges) {
          for (const auto& source : edge.lineage) {
            if (source.source.kind == TrimSourceKind::Domain &&
                source.source.domain == side.boundary.domain && source.source.piece == side.boundary.piece) {
              if (edge.curve.geometry.kind != SegmentKind::Line) {
                throw std::runtime_error("trimmed sheet seam lost its analytic line support");
              }
              ports.push_back({edge.token, edge.curve.range,
                               {axial(side, source.span.first), axial(side, source.span.last)}});
            }
          }
        }
      }
    }
  }
  return ports;
}

double at_axial(const PortEdge& edge, double value) {
  const double fraction = (value - edge.axial_range.first) / (edge.axial_range.last - edge.axial_range.first);
  return edge.range.first + fraction * (edge.range.last - edge.range.first);
}

void add_position(std::vector<double>& values, double value, const TrimSeam& seam, double tolerance) {
  if (value < low(seam.axial_interval) - tolerance || value > high(seam.axial_interval) + tolerance) {
    return;
  }
  value = std::max(low(seam.axial_interval), std::min(high(seam.axial_interval), value));
  if (std::none_of(values.begin(), values.end(), [&](double old) { return std::abs(old - value) <= tolerance; })) {
    values.push_back(value);
  }
}

void split_ports(Context& ctx, SheetTrimResult& result, const TrimSeamSide& side,
                  const std::vector<double>& values) {
  const auto ports = ports_of(result, side);
  for (auto& domain : result.domains) {
    if (domain.domain != side.boundary.domain) {
      continue;
    }
    for (auto& region : domain.regions) {
      for (auto& loop : region.loops) {
        std::vector<TrimEdge> divided;
        for (auto& edge : loop.edges) {
          const auto port = std::find_if(ports.begin(), ports.end(), [&](const PortEdge& p) { return p.token == edge.token; });
          if (port == ports.end()) {
            divided.push_back(std::move(edge));
            continue;
          }
          std::vector<double> fractions{0, 1};
          for (double v : values) {
            if (v > low(port->axial_range) + ctx.spec.tolerance &&
                v < high(port->axial_range) - ctx.spec.tolerance) {
              fractions.push_back((v - port->axial_range.first) / (port->axial_range.last - port->axial_range.first));
            }
          }
          std::sort(fractions.begin(), fractions.end());
          if (fractions.size() == 2) {
            divided.push_back(std::move(edge));
            continue;
          }
          for (std::size_t i = 1; i < fractions.size(); ++i) {
            TrimEdge part = edge;
            part.token = ctx.next_edge++;
            ctx.history[part.token] = ctx.history.at(edge.token);
            part.curve.range = {edge.curve.range.first + fractions[i - 1] * (edge.curve.range.last - edge.curve.range.first),
                                edge.curve.range.first + fractions[i] * (edge.curve.range.last - edge.curve.range.first)};
            lineage(ctx, domain.domain, part);
            divided.push_back(std::move(part));
          }
        }
        loop.edges = std::move(divided);
      }
    }
  }
}

std::vector<TrimEndpoint> endpoints(const Context& ctx, const SheetTrimResult& result,
                                   const TrimSeamSide& side, double v) {
  const auto& source = seam_source(ctx, side);
  const gp_Pnt2d point = source.curve.curve->Value(source.curve.native_parameter((v - side.axial.offset) / side.axial.scale));
  std::vector<TrimEndpoint> out;
  for (const auto& domain : result.domains) {
    if (domain.domain != side.boundary.domain) {
      continue;
    }
    for (const auto& region : domain.regions) {
      for (const auto& loop : region.loops) {
        for (const auto& edge : loop.edges) {
          auto curve = detail::make_sheet_curve(edge.curve);
          curve.curve = detail::transform_sheet_curve(curve.curve, domain_of(ctx, domain.domain).to_operation);
          for (double parameter : {edge.curve.range.first, edge.curve.range.last}) {
            if (curve.curve->Value(curve.native_parameter(parameter)).Distance(point) <= ctx.spec.tolerance) {
              out.push_back({domain.domain, edge.token, parameter});
            }
          }
        }
      }
    }
  }
  return out;
}

const PortEdge* covering(const std::vector<PortEdge>& ports, double value, double tolerance) {
  const PortEdge* found = nullptr;
  for (const auto& port : ports) {
    if (value > low(port.axial_range) + tolerance / 4 && value < high(port.axial_range) - tolerance / 4) {
      if (found) {
        throw std::runtime_error("sheet seam has overlapping material intervals");
      }
      found = &port;
    }
  }
  return found;
}

std::vector<double> seam_positions(Context& ctx, const SheetTrimResult& result, const TrimSeam& seam) {
  validate_seam(ctx, seam);
  std::vector<double> values{low(seam.axial_interval), high(seam.axial_interval)};
  for (const TrimSeamSide* side : {&seam.first, &seam.second}) {
    for (const auto& port : ports_of(result, *side)) {
      add_position(values, port.axial_range.first, seam, ctx.spec.tolerance);
      add_position(values, port.axial_range.last, seam, ctx.spec.tolerance);
    }
  }
  const SourceEdge& source = seam_source(ctx, seam.first);
  const occ::handle<Geom2d_Curve> bounded_seam = new Geom2d_TrimmedCurve(source.curve.curve,
      std::min(source.curve.native_parameter(seam.first.span.first), source.curve.native_parameter(seam.first.span.last)),
      std::max(source.curve.native_parameter(seam.first.span.first), source.curve.native_parameter(seam.first.span.last)));
  for (const auto& tool : ctx.sources) {
    if (tool.source.kind != TrimSourceKind::Tool) {
      continue;
    }
    throw_if_cancelled();
    const occ::handle<Geom2d_Curve> bounded_tool = new Geom2d_TrimmedCurve(tool.curve.curve,
        low(tool.curve.native_range), high(tool.curve.native_range));
    Geom2dAPI_InterCurveCurve intersection(bounded_seam, bounded_tool, ctx.spec.tolerance);
    for (int i = 1; i <= intersection.NbPoints(); ++i) {
      const auto parameters = parameters_at(source, intersection.Point(i), ctx.spec.tolerance);
      if (parameters.size() != 1) {
        throw std::runtime_error("sheet seam intersection has ambiguous source parameter");
      }
      add_position(values, axial(seam.first, source.curve.public_parameter(parameters.front())), seam, ctx.spec.tolerance);
    }
  }
  std::sort(values.begin(), values.end());
  return values;
}

void synchronize(Context& ctx, SheetTrimResult& result, const TrimSeam& seam,
                   const std::vector<double>& values) {
  const auto first = ports_of(result, seam.first);
  const auto second = ports_of(result, seam.second);
  std::vector<bool> retained;
  for (std::size_t i = 1; i < values.size(); ++i) {
    const double middle = (values[i - 1] + values[i]) / 2.0;
    const PortEdge* a = covering(first, middle, ctx.spec.tolerance);
    const PortEdge* b = covering(second, middle, ctx.spec.tolerance);
    // A tool boundary lying exactly on the seam (a cut or relief ending at a
    // tangent line) removes the material of one side only: that interval is
    // no longer a seam, and the surviving side keeps it as a free boundary.
    retained.push_back(a != nullptr && b != nullptr);
    if (a && b) {
      result.ports.push_back({seam.token, {values[i - 1], values[i]},
          {seam.first.boundary.domain, a->token, {at_axial(*a, values[i - 1]), at_axial(*a, values[i])}},
          {seam.second.boundary.domain, b->token, {at_axial(*b, values[i - 1]), at_axial(*b, values[i])}}});
    }
  }
  for (std::size_t i = 0; i < values.size(); ++i) {
    SeamEventKind kind = SeamEventKind::Endpoint;
    if (i > 0 && i + 1 < values.size()) {
      kind = retained[i - 1] == retained[i] ? SeamEventKind::Tangency : SeamEventKind::Crossing;
    }
    result.splits.push_back({seam.token, ctx.next_split++, values[i], kind,
                             endpoints(ctx, result, seam.first, values[i]),
                             endpoints(ctx, result, seam.second, values[i])});
  }
}

} // namespace

SheetTrimResult sheet_trim_domain(const SheetTrimSpec& spec) {
  return detail::run("sheet domain trimming", [&] {
    if (!std::isfinite(spec.tolerance) || spec.tolerance <= 0 || spec.domains.empty()) {
      throw std::invalid_argument("sheet trim requires domains and a positive finite tolerance");
    }
    if (spec.operation == SheetTrimOperation::Union && spec.domains.size() != 1) {
      throw std::invalid_argument("sheet union requires exactly one material domain");
    }
    for (std::size_t i = 0; i < spec.domains.size(); ++i) {
      detail::validate_sheet_isometry(spec.domains[i].to_operation);
      if (spec.domains[i].domain.feature.empty() ||
          std::any_of(spec.domains.begin(), spec.domains.begin() + static_cast<std::ptrdiff_t>(i),
                      [&](const TrimDomain& d) { return d.domain == spec.domains[i].domain; })) {
        throw std::invalid_argument("sheet trim domain identities must be nonempty and unique");
      }
    }
    Context ctx{spec, {}, {}, 1, 1, 1};
    std::vector<TopoDS_Face> faces;
    for (const auto& tool : spec.tools) {
      faces.push_back(material_face(ctx, tool, {}, {}, true));
    }
    std::vector<std::size_t> tool_sources;
    for (std::size_t i = 0; i < ctx.sources.size(); ++i) {
      tool_sources.push_back(i);
    }
    SheetTrimResult result;
    for (const auto& domain : spec.domains) {
      result.domains.push_back(trim_domain(ctx, domain, faces, tool_sources));
    }
    std::vector<std::uint32_t> seam_tokens;
    std::vector<std::vector<double>> positions;
    std::vector<const TrimSeamSide*> used_sides;
    for (const auto& seam : spec.seams) {
      if (!std::isfinite(seam.axial_interval.first) || !std::isfinite(seam.axial_interval.last) ||
          high(seam.axial_interval) - low(seam.axial_interval) <= spec.tolerance ||
          std::find(seam_tokens.begin(), seam_tokens.end(), seam.token) != seam_tokens.end()) {
        throw std::invalid_argument("sheet seam requires a unique token and a finite positive interval");
      }
      seam_tokens.push_back(seam.token);
      for (const TrimSeamSide* side : {&seam.first, &seam.second}) {
        for (const TrimSeamSide* old : used_sides) {
          if (side->boundary == old->boundary &&
              std::min(high(side->span), high(old->span)) - std::max(low(side->span), low(old->span)) > kParamTolerance) {
            throw std::invalid_argument("sheet boundary interval belongs to more than one seam");
          }
        }
        used_sides.push_back(side);
      }
      positions.push_back(seam_positions(ctx, result, seam));
      split_ports(ctx, result, seam.first, positions.back());
      split_ports(ctx, result, seam.second, positions.back());
    }
    // Construct references only after every split has settled the final edge
    // tokens. Several semantic ports can partition one source boundary.
    for (std::size_t i = 0; i < spec.seams.size(); ++i) {
      synchronize(ctx, result, spec.seams[i], positions[i]);
    }
    return result;
  });
}

} // namespace mitcad::geometry
