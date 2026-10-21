// SPDX-License-Identifier: MIT
// The part picker's data (mitcad#418): the order of parts and groups with
// stars (starred first, starred groups first in Favourites, stable within
// each class), the stars of configuration rows (mitcad#491), the stars'
// round trip through the settings (an INI file in a folder of the test's
// own) and the design's used library parts read from the model's
// `library_usage` answer (core/model/src/library_tests.rs and
// build/source_tests.rs check the model's side).
#include "library/PartFavourites.hpp"
#include "library/UsedParts.hpp"

#include <cstdio>
#include <memory>
#include <stdexcept>

#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonObject>
#include <QSettings>
#include <QString>
#include <QTemporaryDir>

#include "framework/Command.hpp"

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "part_picker_data.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using namespace mitcad;

// A model that answers `library_usage` with a fixed answer, or refuses it.
class FakeModel : public CommandContext {
public:
  QJsonObject answer;
  bool refuses = false;
  mutable QJsonObject asked;

  QJsonValue query(const QJsonObject& query) const override {
    asked = query;
    if (refuses) {
      throw std::runtime_error("unknown variant `library_usage`");
    }
    return answer;
  }
  std::shared_ptr<geometry::Shape> bodyShape(const QString&) const override { return nullptr; }
  TopoDS_Shape itemShape(const SelectionItem&) const override { return TopoDS_Shape(); }
  TopoDS_Shape analysisShape(const QJsonObject&) const override { return TopoDS_Shape(); }
};

void usedPartsTests() {
  FakeModel model;
  model.answer = QJsonObject{
      {QStringLiteral("parts"),
       QJsonArray{QJsonObject{{QStringLiteral("library"), QStringLiteral("parts")},
                              {QStringLiteral("url"), QStringLiteral("https://example.org/parts.git")},
                              {QStringLiteral("component"), QStringLiteral("p30")},
                              {QStringLiteral("count"), 4}},
                  QJsonObject{{QStringLiteral("library"), QStringLiteral("parts")},
                              {QStringLiteral("url"), QStringLiteral("")},
                              {QStringLiteral("component"), QStringLiteral("nut-m6")},
                              {QStringLiteral("count"), 1}},
                  // Not a part: left out.
                  QJsonObject{{QStringLiteral("library"), QStringLiteral("parts")}}}}};
  const QList<UsedPart> parts = usedLibraryParts(model);
  CHECK(model.asked.value(QStringLiteral("query")).toString() == QStringLiteral("library_usage"));
  CHECK(parts.size() == 2);
  if (parts.size() == 2) {
    CHECK(parts[0].library == QStringLiteral("parts"));
    CHECK(parts[0].url == QStringLiteral("https://example.org/parts.git"));
    CHECK(parts[0].component == QStringLiteral("p30"));
    CHECK(parts[0].count == 4);
    CHECK(parts[1].component == QStringLiteral("nut-m6"));
    CHECK(parts[1].url.isEmpty());
    CHECK(parts[1].count == 1);
  }
  // A design without library parts, and a model that refuses the query.
  model.answer = QJsonObject{{QStringLiteral("parts"), QJsonArray()}};
  CHECK(usedLibraryParts(model).isEmpty());
  model.refuses = true;
  CHECK(usedLibraryParts(model).isEmpty());
}

QList<OrderedPart> sampleParts() {
  return {
      {QStringLiteral("parts"), QStringLiteral("p30"), QStringLiteral("profiles"), QString(), false},   // 0
      {QStringLiteral("parts"), QStringLiteral("m6x12"), QStringLiteral("screws"), QString(), false},   // 1
      {QStringLiteral("parts"), QStringLiteral("p40"), QStringLiteral("profiles"), QString(), false},   // 2
      {QStringLiteral("other"), QStringLiteral("m6x12"), QStringLiteral("screws"), QString(), false},   // 3
      {QStringLiteral("parts"), QStringLiteral("nut-m6"), QStringLiteral("nuts"), QString(), false},    // 4
      {QStringLiteral("parts"), QStringLiteral("m6x16"), QStringLiteral("screws"), QString(), false},   // 5
  };
}

void orderAndSettingsTests() {
  PartFavourites& favourites = PartFavourites::instance();
  const QList<OrderedPart> parts = sampleParts();
  // Nothing starred: the given order.
  CHECK(pickerOrder(parts, false) == (QList<int>{0, 1, 2, 3, 4, 5}));
  CHECK(groupOrder({QStringLiteral("profiles"), QStringLiteral("screws"), QStringLiteral("nuts")}) ==
        (QStringList{QStringLiteral("profiles"), QStringLiteral("screws"), QStringLiteral("nuts")}));

  int changes = 0;
  const QMetaObject::Connection counting =
      QObject::connect(&favourites, &PartFavourites::changed, [&changes] { ++changes; });
  favourites.setStarred(QStringLiteral("parts"), QStringLiteral("nut-m6"), true);
  favourites.setStarred(QStringLiteral("parts"), QStringLiteral("p40"), true);
  favourites.setStarred(QStringLiteral("parts"), QStringLiteral("m6x16"), true);
  // Starring again changes nothing.
  favourites.setStarred(QStringLiteral("parts"), QStringLiteral("p40"), true);
  CHECK(changes == 3);
  QObject::disconnect(counting);
  // Keyed by library and component: the other library's m6x12 is not the
  // starred one.
  CHECK(favourites.starred(QStringLiteral("parts"), QStringLiteral("m6x16")));
  CHECK(!favourites.starred(QStringLiteral("other"), QStringLiteral("m6x16")));
  // Starred first, each class in the given order.
  CHECK(pickerOrder(parts, false) == (QList<int>{2, 4, 5, 0, 1, 3}));

  // A starred group comes first among the groups, and in Favourites its
  // parts first among the starred ones; elsewhere the group changes nothing.
  favourites.setGroupStarred(QStringLiteral("screws"), true);
  CHECK(favourites.groupStarred(QStringLiteral("screws")));
  CHECK(groupOrder({QStringLiteral("profiles"), QStringLiteral("screws"), QStringLiteral("nuts")}) ==
        (QStringList{QStringLiteral("screws"), QStringLiteral("profiles"), QStringLiteral("nuts")}));
  CHECK(pickerOrder(parts, true) == (QList<int>{5, 2, 4, 0, 1, 3}));
  CHECK(pickerOrder(parts, false) == (QList<int>{2, 4, 5, 0, 1, 3}));
  CHECK(pickerOrder({}, true).isEmpty());

  // The stars are in the settings: another reader finds them, in the order
  // they were starred.
  {
    const PartFavourites reread;
    CHECK(reread.starred(QStringLiteral("parts"), QStringLiteral("p40")));
    CHECK(reread.groupStarred(QStringLiteral("screws")));
    CHECK(!reread.groupStarred(QStringLiteral("nuts")));
    const QList<PartStar> expected{{QStringLiteral("parts"), QStringLiteral("nut-m6"), QString()},
                                   {QStringLiteral("parts"), QStringLiteral("p40"), QString()},
                                   {QStringLiteral("parts"), QStringLiteral("m6x16"), QString()}};
    CHECK(reread.starredParts() == expected);
  }
  CHECK(QSettings().value(QStringLiteral("partPicker/starredParts")).isValid());

  // Unstarred, and read back so.
  favourites.setStarred(QStringLiteral("parts"), QStringLiteral("p40"), false);
  favourites.setGroupStarred(QStringLiteral("screws"), false);
  CHECK(!favourites.starred(QStringLiteral("parts"), QStringLiteral("p40")));
  CHECK(pickerOrder(parts, true) == (QList<int>{4, 5, 0, 1, 2, 3}));
  {
    const PartFavourites reread;
    CHECK(!reread.starred(QStringLiteral("parts"), QStringLiteral("p40")));
    CHECK(!reread.groupStarred(QStringLiteral("screws")));
    CHECK(reread.starredParts().size() == 2);
  }
  // Settings written by hand with damage: what can be read is kept.
  QSettings().setValue(QStringLiteral("partPicker/starredParts"),
                       QByteArray(R"([{"library":"parts","component":"p30"},{"library":""},"x",)"
                                  R"({"library":"parts","component":"p30"}])"));
  {
    const PartFavourites reread;
    CHECK(reread.starredParts().size() == 1);
    CHECK(reread.starred(QStringLiteral("parts"), QStringLiteral("p30")));
  }
  QSettings().setValue(QStringLiteral("partPicker/starredParts"), QByteArray("not json"));
  CHECK(PartFavourites().starredParts().isEmpty());

  favourites.setStarred(QStringLiteral("parts"), QStringLiteral("nut-m6"), false);
  favourites.setStarred(QStringLiteral("parts"), QStringLiteral("m6x16"), false);
  CHECK(favourites.starredParts().isEmpty());
}

// A timber family's sizes, each a tile of its own (mitcad#484), its
// default row 48x148, and a part without rows.
QList<OrderedPart> timberTiles() {
  const QString library = QStringLiteral("timber");
  const QString family = QStringLiteral("dimensioned-48");
  const QString group = QStringLiteral("dimensioned-timber");
  return {
      {library, family, group, QStringLiteral("48x98"), false},                // 0
      {library, family, group, QStringLiteral("48x148"), true},                // 1
      {library, family, group, QStringLiteral("48x198"), false},               // 2
      {library, QStringLiteral("batten-28"), group, QString(), false},         // 3
  };
}

// The tiles of `parts` starred.
QList<int> starredOf(const PartFavourites& favourites, const QList<OrderedPart>& parts) {
  QList<int> starred;
  for (int i = 0; i < parts.size(); ++i) {
    const OrderedPart& part = parts.at(i);
    if (favourites.starred(part.library, part.component, part.row, part.defaultRow)) {
      starred << i;
    }
  }
  return starred;
}

// Stars of configuration rows (mitcad#491): a star stars one size, an old
// star without a row marks only the default row, a part without rows keeps
// its star as it was.
void rowTests() {
  PartFavourites& favourites = PartFavourites::instance();
  const QList<OrderedPart> tiles = timberTiles();
  const QString library = QStringLiteral("timber");
  const QString family = QStringLiteral("dimensioned-48");
  CHECK(favourites.starredParts().isEmpty());

  // One size starred: only its tile, first; the family without a row is not.
  favourites.setStarred(library, family, QStringLiteral("48x198"), false, true);
  CHECK(starredOf(favourites, tiles) == (QList<int>{2}));
  CHECK(!favourites.starred(library, family));
  CHECK(pickerOrder(tiles, false) == (QList<int>{2, 0, 1, 3}));
  CHECK(pickerOrder(tiles, true) == (QList<int>{2, 0, 1, 3}));
  // Kept with its row: another reader finds that size only.
  {
    const PartFavourites reread;
    CHECK(reread.starredParts() == (QList<PartStar>{{library, family, QStringLiteral("48x198")}}));
    CHECK(starredOf(reread, tiles) == (QList<int>{2}));
  }
  CHECK(QSettings().value(QStringLiteral("partPicker/starredParts")).toByteArray().contains("\"row\":\"48x198\""));
  // A second size, then the first one's star off: the second stays.
  favourites.setStarred(library, family, QStringLiteral("48x98"), false, true);
  CHECK(starredOf(favourites, tiles) == (QList<int>{0, 2}));
  favourites.setStarred(library, family, QStringLiteral("48x198"), false, false);
  CHECK(starredOf(favourites, tiles) == (QList<int>{0}));
  favourites.setStarred(library, family, QStringLiteral("48x98"), false, false);
  CHECK(favourites.starredParts().isEmpty());

  // Stars kept before mitcad#491, without rows: the family's marks only its
  // default row; the part without rows is starred as before.
  QSettings().setValue(QStringLiteral("partPicker/starredParts"),
                       QByteArray(R"([{"library":"timber","component":"dimensioned-48"},)"
                                  R"({"library":"timber","component":"batten-28"}])"));
  {
    const PartFavourites reread;
    CHECK(reread.starredParts().size() == 2);
    CHECK(starredOf(reread, tiles) == (QList<int>{1, 3}));
    CHECK(reread.starred(library, QStringLiteral("batten-28")));
    CHECK(reread.starred(library, family, QStringLiteral("48x148"), true));
    CHECK(!reread.starred(library, family, QStringLiteral("48x98"), false));
    CHECK(!reread.starred(library, family, QStringLiteral("48x198"), false));
  }
  // The same in the picker's order (the old stars made here as they were
  // kept: without a row).
  favourites.setStarred(library, family, true);
  favourites.setStarred(library, QStringLiteral("batten-28"), true);
  CHECK(starredOf(favourites, tiles) == (QList<int>{1, 3}));
  CHECK(pickerOrder(tiles, false) == (QList<int>{1, 3, 0, 2}));
  // Starring the default row again changes nothing; its star off turns the
  // old star off too; the part without rows keeps its own.
  int changes = 0;
  const QMetaObject::Connection counting =
      QObject::connect(&favourites, &PartFavourites::changed, [&changes] { ++changes; });
  favourites.setStarred(library, family, QStringLiteral("48x148"), true, true);
  CHECK(changes == 0);
  favourites.setStarred(library, family, QStringLiteral("48x148"), true, false);
  CHECK(changes == 1);
  QObject::disconnect(counting);
  CHECK(starredOf(favourites, tiles) == (QList<int>{3}));
  CHECK(!favourites.starred(library, family));
  CHECK(favourites.starredParts() == (QList<PartStar>{{library, QStringLiteral("batten-28"), QString()}}));
  // Groups' stars as before.
  favourites.setGroupStarred(QStringLiteral("dimensioned-timber"), true);
  CHECK(pickerOrder(tiles, true) == (QList<int>{3, 0, 1, 2}));
  favourites.setGroupStarred(QStringLiteral("dimensioned-timber"), false);
  favourites.setStarred(library, QStringLiteral("batten-28"), false);
  CHECK(favourites.starredParts().isEmpty());
}

} // namespace

int partPickerDataTests() {
  usedPartsTests();
  // The stars in a settings folder of the test's own, never the user's.
  QTemporaryDir directory;
  CHECK(directory.isValid());
  const QSettings::Format format = QSettings::defaultFormat();
  const QString organization = QCoreApplication::organizationName();
  const QString application = QCoreApplication::applicationName();
  QCoreApplication::setOrganizationName(QStringLiteral("MitcadTest"));
  QCoreApplication::setApplicationName(QStringLiteral("part_picker_data"));
  QSettings::setDefaultFormat(QSettings::IniFormat);
  QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, directory.path());
  CHECK(QSettings().fileName().startsWith(directory.path()));
  orderAndSettingsTests();
  rowTests();
  QSettings::setDefaultFormat(format);
  QCoreApplication::setOrganizationName(organization);
  QCoreApplication::setApplicationName(application);
  return failures;
}
