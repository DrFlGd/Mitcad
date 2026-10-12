// SPDX-License-Identifier: MIT
#include "framework/CutTargetsPanel.hpp"

#include <cstdio>
#include <QJsonArray>
#include <QPushButton>
#include <QTreeWidget>

namespace {
QJsonObject identity(const QString& component, const QString& occurrence) {
  return {{QStringLiteral("component"), component}, {QStringLiteral("occurrence"), occurrence}};
}
QJsonObject candidate(const QString& component, const QString& occurrence, bool checked, bool intersects = true,
                       bool eligible = true) {
  QJsonObject result = identity(component, occurrence);
  result.insert(QStringLiteral("path_name"), QStringLiteral("Root/Part ") + component);
  result.insert(QStringLiteral("eligible"), eligible);
  result.insert(QStringLiteral("checked"), checked);
  result.insert(QStringLiteral("intersects"), intersects);
  result.insert(QStringLiteral("reason"), eligible ? QJsonValue() : QJsonValue(QStringLiteral("Repeated component definition")));
  result.insert(QStringLiteral("bodies"), QJsonArray{QJsonObject{{QStringLiteral("body"), QStringLiteral("F2.b0")},
                                                               {QStringLiteral("checked"), checked}}});
  return result;
}
} // namespace

int cutTargetsTests() {
  int failures = 0;
  const auto check = [&failures](bool ok, const char* message) {
    if (!ok) { std::fprintf(stderr, "cut targets: %s\n", message); ++failures; }
  };
  const QJsonObject context = identity(QStringLiteral("C0"), QString());
  const QJsonObject b = identity(QStringLiteral("C2"), QStringLiteral("O2"));
  QJsonObject scope = mitcad::selectCutTargets({{QStringLiteral("context"), context}}, true);
  mitcad::CutTargetsPanel panel;
  auto* tree = panel.findChild<QTreeWidget*>(QStringLiteral("cut_targets_list"));
  int mutations = 0;
  mitcad::Selection highlighted;
  QObject::connect(&panel, &mitcad::CutTargetsPanel::scopeChanged, &panel, [&](const QJsonObject& value) {
    ++mutations;
    scope = value;
    panel.setScope(scope);
    panel.setPreview({}); // Same immediate stale transition as CommandSession.
  });
  QObject::connect(&panel, &mitcad::CutTargetsPanel::highlightChanged, &panel,
                   [&](const mitcad::Selection& value) { highlighted = value; });
  const auto preview = [&](QJsonArray rows, int selected) {
    panel.setPreview({{QStringLiteral("rows"), rows}, {QStringLiteral("selected_components"), selected},
                      {QStringLiteral("selected_bodies"), selected}, {QStringLiteral("can_commit"), selected > 0}});
  };
  panel.setScope(scope);
  preview({candidate(QStringLiteral("C0"), QString(), true), candidate(QStringLiteral("C2"), QStringLiteral("O2"), true)}, 2);
  check(tree && tree->maximumHeight() <= 180, "checklist has a bounded height");
  tree->topLevelItem(1)->setCheckState(0, Qt::Unchecked);
  check(mutations == 1 && !tree->isEnabled(), "a checkbox submits one change and disables stale candidates");
  check(scope.value(QStringLiteral("context")).toObject() == context, "root occurrence survives explicitly empty");
  check(scope.value(QStringLiteral("targets")).toObject().value(QStringLiteral("excluded")).toArray() == QJsonArray{b},
        "unchecking stores the component occurrence, not all instances of its definition");
  const QJsonObject excluded = scope;
  preview({candidate(QStringLiteral("C0"), QString(), true), candidate(QStringLiteral("C2"), QStringLiteral("O2"), false, false)}, 1);
  check(scope == excluded && mutations == 1, "outside intersection metadata never rewrites persistent exclusions");
  preview({candidate(QStringLiteral("C0"), QString(), true), candidate(QStringLiteral("C2"), QStringLiteral("O2"), false)}, 1);
  check(tree->topLevelItem(1)->checkState(0) == Qt::Unchecked && scope == excluded, "excluded component stays unchecked on re-entry");
  tree->setCurrentItem(tree->topLevelItem(1));
  check(highlighted.size() == 1 && highlighted.first().owner == QStringLiteral("F2.b0") &&
        highlighted.first().occurrence == QStringLiteral("O2") && mutations == 1,
        "keyboard focus highlights the precise occurrence without changing scope");
  QMetaObject::invokeMethod(tree, "entered", Qt::DirectConnection, Q_ARG(QModelIndex, QModelIndex()));
  check(highlighted.isEmpty() && mutations == 1, "hover exit clears only highlights");
  panel.findChild<QPushButton*>(QStringLiteral("cut_targets_none"))->click();
  const QJsonObject none = scope.value(QStringLiteral("targets")).toObject();
  check(none.value(QStringLiteral("type")) == QStringLiteral("explicit") && none.value(QStringLiteral("included")).toArray().isEmpty(),
        "select none is explicit empty, never automatic fallback");
  preview({candidate(QStringLiteral("C2"), QStringLiteral("O2"), false)}, 0);
  check(panel.findChild<QPushButton*>(QStringLiteral("cut_targets_all"))->isEnabled(), "empty/error preview remains repairable");
  panel.findChild<QPushButton*>(QStringLiteral("cut_targets_all"))->click();
  check(scope.value(QStringLiteral("targets")).toObject().value(QStringLiteral("type")) == QStringLiteral("automatic"),
        "select all explicitly restores automatic discovery");
  preview({candidate(QStringLiteral("C2"), QStringLiteral("O2"), false, true, false)}, 0);
  check(!tree->topLevelItem(0)->flags().testFlag(Qt::ItemIsEnabled) &&
        tree->topLevelItem(0)->toolTip(0).contains(QStringLiteral("Repeated")), "unsupported rows are disabled with a reason");
  scope = mitcad::setCutTargetChecked(scope, b, QStringLiteral("F2.b0"), false);
  check(scope.value(QStringLiteral("targets")).toObject().value(QStringLiteral("excluded_bodies")).toArray().size() == 1,
        "body exclusion is retained separately");
  scope = mitcad::setCutTargetChecked(scope, b, QString(), true);
  check(scope.value(QStringLiteral("targets")).toObject().value(QStringLiteral("excluded_bodies")).toArray().isEmpty(),
        "explicitly enabling the whole component clears its finer exclusions");
  return failures;
}
