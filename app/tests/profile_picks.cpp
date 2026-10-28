// SPDX-License-Identifier: MIT
// What Extrude's and Revolve's Profiles take with a click (mitcad#868): a
// planar face alone, or sketch profiles and curves; a click on a face
// while they hold a profile (an edited boss whose Start is Object, the arm
// face it starts at clicked while Profiles was still the active input) is
// not taken, and the status bar says to click Start Object first, instead
// of the panel's "not both" error; the face they hold is taken again so
// that its click removes it.
#include "commands/ProfilePicks.hpp"

#include <cstdio>

#include <QString>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "profile_picks.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using mitcad::SelectionItem;
using mitcad::SelectKind;
using mitcad::cmd::ProfilesCommand;
using mitcad::cmd::profilesRefused;
using mitcad::cmd::profilesTake;

SelectionItem face(const char* name, const char* occurrence = "") {
  return {SelectKind::Face, QStringLiteral("F2.b0"), QString::fromLatin1(name), QStringLiteral("plane"),
          QString::fromLatin1(occurrence)};
}

const SelectionItem kProfile{SelectKind::Profile, QStringLiteral("F5"), QStringLiteral("r{c8[c6+c10,c1+c10]}"),
                             QString(), QStringLiteral("O1")};
const SelectionItem kCurve{SelectKind::SketchCurve, QStringLiteral("F5"), QStringLiteral("c3"), QStringLiteral("line"),
                           QStringLiteral("O1")};

void takeTests() {
  const SelectionItem inner = face("F2:side(c9[c15,c16])", "O1");
  const SelectionItem outer = face("F2:side(c4[c7,c15])", "O1");
  // Empty: either is the first pick.
  CHECK(profilesTake(inner, {}));
  CHECK(profilesTake(kProfile, {}));
  CHECK(profilesTake(kCurve, {}));
  // The edited boss: its profile held, the arm's face clicked.
  CHECK(!profilesTake(inner, {kProfile}));
  CHECK(!profilesTake(inner, {kCurve}));
  // More profiles of the sketch still go in.
  CHECK(profilesTake(kProfile, {kProfile}));
  // A face held: no profile beside it, no second face; its own click is
  // taken (and removes it), also when the edited definition's item names
  // no occurrence.
  CHECK(!profilesTake(kProfile, {inner}));
  CHECK(!profilesTake(kCurve, {inner}));
  CHECK(!profilesTake(outer, {inner}));
  CHECK(profilesTake(inner, {inner}));
  CHECK(profilesTake(inner, {face("F2:side(c9[c15,c16])")}));
  // The same face in another occurrence is another face.
  CHECK(!profilesTake(inner, {face("F2:side(c9[c15,c16])", "O2")}));
}

void hintTests() {
  const SelectionItem inner = face("F2:side(c9[c15,c16])", "O1");
  // Taken: no hint.
  CHECK(profilesRefused(inner, {}, true, ProfilesCommand::Extrude, QStringLiteral("object")).isEmpty());
  // A face that is not planar is refused as before, without this hint.
  CHECK(profilesRefused(inner, {kProfile}, false, ProfilesCommand::Extrude, QStringLiteral("object")).isEmpty());
  // Start: Object shown: pick there.
  const QString object = profilesRefused(inner, {kProfile}, true, ProfilesCommand::Extrude, QStringLiteral("object"));
  CHECK(object.contains(QStringLiteral("click Start Object")));
  // Another start: set it to Object.
  const QString plane =
      profilesRefused(inner, {kProfile}, true, ProfilesCommand::Extrude, QStringLiteral("profile_plane"));
  CHECK(plane.contains(QStringLiteral("set Start to Object")));
  // Revolve has no start: clear Profiles to revolve the face.
  const QString revolve = profilesRefused(inner, {kProfile}, true, ProfilesCommand::Revolve);
  CHECK(revolve.contains(QStringLiteral("to revolve the face")) && !revolve.contains(QStringLiteral("Start")));
  CHECK(profilesRefused(face("F2:side(c4[c7,c15])", "O1"), {inner}, true, ProfilesCommand::Extrude)
            .contains(QStringLiteral("one planar face")));
  CHECK(profilesRefused(kProfile, {inner}, true, ProfilesCommand::Extrude).contains(QStringLiteral("extruded alone")));
  CHECK(profilesRefused(kProfile, {inner}, true, ProfilesCommand::Revolve).contains(QStringLiteral("revolved alone")));
}

} // namespace

int profilePicksTests() {
  failures = 0;
  takeTests();
  hintTests();
  if (failures == 0) {
    std::printf("profile picks tests passed\n");
  }
  return failures;
}
