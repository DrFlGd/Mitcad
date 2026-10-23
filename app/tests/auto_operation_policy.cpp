// SPDX-License-Identifier: MIT
#include "framework/AutoOperationPolicy.hpp"

#include <cstdio>

namespace {
QJsonObject stored(const QString& operation, const QJsonArray& bodies = {}) {
  return {{QStringLiteral("operation"), operation}, {QStringLiteral("participants"), bodies},
           {QStringLiteral("extent"), QJsonObject{{QStringLiteral("distance"), 10}}}};
}
QJsonObject report(const QString& operation, const QJsonArray& bodies = {}) {
  return {{QStringLiteral("status"), QStringLiteral("ok")},
           {QStringLiteral("auto_operation"), QJsonObject{{QStringLiteral("operation"), operation},
                                                         {QStringLiteral("objects"), bodies}}}};
}
QJsonObject cutReport(const QJsonArray& checked, const QJsonArray& unchecked = {}) {
  QJsonObject result = report(QStringLiteral("cut"), {QStringLiteral("F1.b0")});
  QJsonArray bodies;
  for (const QJsonValue& body : checked) bodies.append(QJsonObject{{QStringLiteral("body"), body}, {QStringLiteral("checked"), true}});
  for (const QJsonValue& body : unchecked) bodies.append(QJsonObject{{QStringLiteral("body"), body}, {QStringLiteral("checked"), false}});
  result.insert(QStringLiteral("cut_targets"), QJsonObject{{QStringLiteral("rows"),
      QJsonArray{QJsonObject{{QStringLiteral("bodies"), bodies}}}}});
  return result;
}
} // namespace

int autoOperationPolicyTests() {
  int failures = 0;
  const auto check = [&failures](bool ok, const char* message) {
    if (!ok) { std::fprintf(stderr, "auto operation policy: %s\n", message); ++failures; }
  };
  using mitcad::AutoOperationPolicy;
  AutoOperationPolicy state;
  state.start(true, {});
  check(state.active() && !state.pending(), "new commands start automatically without an edit probe");

  QJsonObject baseline = stored(QStringLiteral("join"), {QStringLiteral("F1.b0")});
  state.start(true, baseline);
  check(!state.active() && state.pending(), "an edit must verify its stored geometry first");
  baseline[QStringLiteral("operation")] = QStringLiteral("cut");
  baseline[QStringLiteral("extent")] = QJsonObject{{QStringLiteral("distance"), -10}};
  check(state.stored() == stored(QStringLiteral("join"), {QStringLiteral("F1.b0")}),
        "early input cannot replace the baseline retained for cancel and retry");
  check(state.resolve(report(QStringLiteral("join"), {QStringLiteral("F1.b0")})) && state.active(),
        "the stored match enables automatic operation after early geometry edits");
  check(!state.resolve(report(QStringLiteral("cut"))) && state.active(),
        "a later changed-geometry answer cannot replace the initial decision");

  state.start(true, stored(QStringLiteral("join"), {QStringLiteral("F1.b0")}));
  state.disable();
  check(!state.resolve(report(QStringLiteral("join"), {QStringLiteral("F1.b0")})) && !state.active(),
        "manual override while a probe is pending wins over a late matching reply");
  state.start(true, stored(QStringLiteral("new_body")));
  state.resolve(report(QStringLiteral("join"), {QStringLiteral("F1.b0")}));
  check(!state.active(), "a deliberate touching New Body keeps its stored operation");
  state.start(true, stored(QStringLiteral("join"), {QStringLiteral("F2.b0")}));
  state.resolve(report(QStringLiteral("join"), {QStringLiteral("F1.b0")}));
  check(!state.active(), "different explicit target bodies keep their stored policy");
  state.start(true, stored(QStringLiteral("join"), {QStringLiteral("F2.b0"), QStringLiteral("F1.b0")}));
  state.resolve(report(QStringLiteral("join"), {QStringLiteral("F1.b0"), QStringLiteral("F2.b0")}));
  check(state.active(), "target matching uses body sets rather than incidental order");

  QJsonObject cut = stored(QStringLiteral("cut"));
  cut.insert(QStringLiteral("cut_scope"), QJsonObject{{QStringLiteral("targets"),
      QJsonObject{{QStringLiteral("type"), QStringLiteral("automatic")}}}});
  state.start(true, cut);
  state.resolve(cutReport({QStringLiteral("F1.b0")}, {QStringLiteral("F2.b0")}));
  check(state.active(), "modern Cut compares checked bodies, including retained hidden exclusions");
  state.disable();
  check(!state.active(), "a later checklist choice stops automatic mode");
  state.start(true, cut);
  state.resolve(cutReport({QStringLiteral("F1.b0"), QStringLiteral("F2.b0")}));
  check(!state.active(), "selected foreign or additional bodies are not silently localized");
  state.start(true, cut);
  state.resolve(report(QStringLiteral("cut"), {QStringLiteral("F1.b0")}));
  check(!state.active(), "a scoped Cut without actual target metadata cannot opt in");
  state.start(true, stored(QStringLiteral("join"), {QStringLiteral("F1.b0")}));
  state.resolve({});
  check(!state.active() && !state.pending(), "an unverifiable initial policy stays stored");
  state.start(false, stored(QStringLiteral("join"), {QStringLiteral("F1.b0")}));
  check(!state.active() && !state.pending(), "commands without automatic operation never probe");
  return failures;
}
