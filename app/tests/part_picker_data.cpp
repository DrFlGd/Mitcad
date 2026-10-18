// SPDX-License-Identifier: MIT
// The part picker's data (mitcad#418): the order of parts and groups with
// stars (starred first, starred groups first in Favourites, stable within
// each class), the stars' round trip through the settings (an INI file in
// a folder of the test's own) and the design's used library parts read from
// the model's `library_usage` answer (core/model/src/library_tests.rs and
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
      {QStringLiteral("parts"), QStringLiteral("p30"), QStringLiteral("profiles")},     // 0
      {QStringLiteral("parts"), QStringLiteral("m6x12"), QStringLiteral("screws")},     // 1
      {QStringLiteral("parts"), QStringLiteral("p40"), QStringLiteral("profiles")},     // 2
      {QStringLiteral("other"), QStringLiteral("m6x12"), QStringLiteral("screws")},     // 3
      {QStringLiteral("parts"), QStringLiteral("nut-m6"), QStringLiteral("nuts")},      // 4
      {QStringLiteral("parts"), QStringLiteral("m6x16"), QStringLiteral("screws")},     // 5
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
    const QList<QPair<QString, QString>> expected{{QStringLiteral("parts"), QStringLiteral("nut-m6")},
                                                  {QStringLiteral("parts"), QStringLiteral("p40")},
                                                  {QStringLiteral("parts"), QStringLiteral("m6x16")}};
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
  QSettings::setDefaultFormat(format);
  QCoreApplication::setOrganizationName(organization);
  QCoreApplication::setApplicationName(application);
  return failures;
}
