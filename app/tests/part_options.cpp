// SPDX-License-Identifier: MIT
// The options of configurable components in the pickers (mitcad#810): the
// `configure` answer read (a list option narrowed by a rule, a range, a
// driven option, the row, the moves and a refusal), the requests sent for
// a library component and an inserted one, the tables of library_show
// (open, closed, the old form) and the constants written.
#include "library/PartOptions.hpp"

#include <cmath>
#include <cstdio>

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QString>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "part_options.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using namespace mitcad;

QJsonObject parse(const char* text) { return QJsonDocument::fromJson(QByteArray(text)).object(); }

// The design note's example after Color = Black: Length narrowed by "Black
// is short" and moved from 1200 mm, Power driven.
const char* kAnswer = R"({
  "options": [
    {"name": "Color", "kind": "text", "unit": "", "value": "Black", "text": "Black", "expression": "'Black'",
     "driven": false, "restricted_by": [],
     "domain": {"kind": "list", "values": [{"value": "Black", "text": "Black", "expression": "'Black'"},
                                           {"value": "White", "text": "White", "expression": "'White'"},
                                           {"value": "Red", "text": "Red", "expression": "'Red'"}]},
     "allowed": {"kind": "list", "values": [{"value": "Black", "text": "Black", "expression": "'Black'"},
                                            {"value": "White", "text": "White", "expression": "'White'"}]}},
    {"name": "Length", "kind": "number", "unit": "mm", "value": 1000, "text": "1000 mm", "expression": "1000 mm",
     "driven": false, "restricted_by": ["Black is short"],
     "domain": {"kind": "range", "min": 300, "max": 2000, "step": 10},
     "allowed": {"kind": "range", "min": 300, "max": 1000, "step": 10,
                 "min_text": "300 mm", "max_text": "1000 mm", "step_text": "10 mm"}},
    {"name": "Power", "kind": "number", "unit": "", "value": 1, "text": "1", "expression": "Tables",
     "driven": true, "restricted_by": [], "allowed": {"kind": "free"}}
  ],
  "row": null,
  "designation_values": {"Color": "Black", "Length": "1000"},
  "moves": [{"option": "Length", "from": "1200 mm", "to": "1000 mm", "from_expression": "1200 mm",
             "to_expression": "1000 mm", "rule": "Black is short", "text": "Length 1200 mm → 1000 mm: Black is short"}],
  "refused": null,
  "problems": []
})";

void answerTests() {
  const OptionsAnswer answer = optionsAnswer(parse(kAnswer));
  CHECK(answer.ok());
  CHECK(answer.options.size() == 3);
  CHECK(answer.row.isEmpty());
  CHECK(answer.refused.isEmpty());
  const PartOption* color = answer.option(QStringLiteral("Color"));
  CHECK(color != nullptr && color->text && color->kind == PartOption::Kind::List);
  // The allowed values, not the domain: Red is left out by the rule.
  CHECK(color != nullptr && color->values.size() == 2);
  CHECK(color != nullptr && color->allows(QStringLiteral("White")) && !color->allows(QStringLiteral("Red")));
  CHECK(color != nullptr && color->allowedText() == QStringLiteral("Black, White"));
  const PartOption* length = answer.option(QStringLiteral("Length"));
  CHECK(length != nullptr && !length->text && length->kind == PartOption::Kind::Range);
  CHECK(length != nullptr && length->value == 1000.0 && length->valueText == QStringLiteral("1000 mm"));
  CHECK(length != nullptr && length->restrictedBy == QStringList{QStringLiteral("Black is short")});
  CHECK(length != nullptr && length->allowedText() == QStringLiteral("300 mm … 1000 mm, step 10 mm"));
  CHECK(length != nullptr && length->allowsNumber(800.0) && !length->allowsNumber(1200.0) &&
        !length->allowsNumber(805.0));
  const PartOption* power = answer.option(QStringLiteral("Power"));
  CHECK(power != nullptr && power->driven && power->kind == PartOption::Kind::Free);
  CHECK(answer.moves.size() == 1 &&
        answer.moves.first().text() == QStringLiteral("Length 1200 mm → 1000 mm: Black is short"));
  CHECK(optionsSummary(answer) == QStringLiteral("Color = Black, Length = 1000 mm, Power = 1 (driven) (no row)"));

  // A row by name; a refusal; a failed command.
  const OptionsAnswer rowed = optionsAnswer(parse(R"({"options": [], "row": "Standard", "refused": null})"));
  CHECK(rowed.row == QStringLiteral("Standard"));
  const OptionsAnswer refused = optionsAnswer(
      parse(R"({"options": [], "refused": {"rule": "No red XL", "message": "Red is not made in XL",
                 "text": "the rule No red XL: Red is not made in XL"}})"));
  CHECK(refused.refused == QStringLiteral("the rule No red XL: Red is not made in XL"));
  const OptionsAnswer failed =
      optionsAnswer(parse(R"({"error": {"message": "unknown library command 'library_configure'"}})"));
  CHECK(!failed.ok() && failed.error.contains(QStringLiteral("library_configure")));
}

void requestTests() {
  QJsonObject asked;
  const JsonRunner run = [&asked](const QJsonObject& request) {
    asked = request;
    return parse(kAnswer);
  };
  const OptionsQuery library = libraryOptionsQuery(run, QStringLiteral("/libs/boards"), QStringLiteral("boards"),
                                                   QStringLiteral("3f9a2c1"), QStringLiteral("shelf-board"));
  const OptionsAnswer answer = library(QJsonObject{{QStringLiteral("Color"), QStringLiteral("Black")}});
  CHECK(answer.options.size() == 3);
  CHECK(asked.value(QStringLiteral("cmd")).toString() == QStringLiteral("library_configure"));
  CHECK(asked.value(QStringLiteral("url")).toString() == QStringLiteral("/libs/boards"));
  CHECK(asked.value(QStringLiteral("id")).toString() == QStringLiteral("boards"));
  CHECK(asked.value(QStringLiteral("rev")).toString() == QStringLiteral("3f9a2c1"));
  CHECK(asked.value(QStringLiteral("component")).toString() == QStringLiteral("shelf-board"));
  CHECK(asked.value(QStringLiteral("values")).toObject().value(QStringLiteral("Color")).toString() ==
        QStringLiteral("Black"));

  // An inserted component: its own expressions first (none given), then
  // with the parent's formula for Length.
  QJsonValue expressions;
  const OptionsQuery component = componentOptionsQuery(run, QStringLiteral("C3"), [&expressions] { return expressions; });
  component(QJsonObject());
  CHECK(asked.value(QStringLiteral("query")).toString() == QStringLiteral("configure"));
  CHECK(asked.value(QStringLiteral("component")).toString() == QStringLiteral("C3"));
  CHECK(!asked.contains(QStringLiteral("parameters")));
  expressions = QJsonObject{{QStringLiteral("Length"), QStringLiteral("Shelf_L")}};
  component(QJsonObject{{QStringLiteral("Color"), QStringLiteral("White")}});
  CHECK(asked.value(QStringLiteral("parameters")).toObject().value(QStringLiteral("Length")).toString() ==
        QStringLiteral("Shelf_L"));
  const QJsonObject set = setComponentParametersCommand(
      QStringLiteral("C3"), QJsonObject{{QStringLiteral("Length"), QStringLiteral("Shelf_L")}});
  CHECK(set.value(QStringLiteral("cmd")).toString() == QStringLiteral("set_component_parameters"));
  CHECK(set.value(QStringLiteral("component")).toString() == QStringLiteral("C3"));
}

void tableTests() {
  // The new form, open: rows set the options; a closed one; the old form.
  const QJsonObject open = parse(R"({"options": ["Color", "Length"], "closed": false, "default": "Standard",
    "rows": [{"name": "Standard", "values": {"Color": "'White'", "Length": "1200 mm"}}]})");
  CHECK(isOpenTable(open));
  // As library_show gives it: `selectors` and the rows' `select` too.
  const QJsonObject shown = parse(R"({"options": ["Color"], "closed": false, "selectors": ["Color"],
    "rows": [{"name": "Standard", "select": {"Color": "White"}}]})");
  CHECK(isOpenTable(shown));
  CHECK(rowOptionText(shown.value(QStringLiteral("rows")).toArray().first().toObject(), QStringLiteral("Color")) ==
        QStringLiteral("White"));
  CHECK(tableOptions(open) == (QStringList{QStringLiteral("Color"), QStringLiteral("Length")}));
  const QJsonObject row = open.value(QStringLiteral("rows")).toArray().first().toObject();
  CHECK(rowOptionText(row, QStringLiteral("Color")) == QStringLiteral("White"));
  CHECK(rowOptionText(row, QStringLiteral("Length")) == QStringLiteral("1200 mm"));
  const QJsonObject start = rowOptionValues(open, row);
  CHECK(start.value(QStringLiteral("Color")).toString() == QStringLiteral("White"));
  CHECK(start.value(QStringLiteral("Length")).toString() == QStringLiteral("1200 mm"));
  const QJsonObject closed = parse(R"({"options": ["Size"], "closed": true,
    "rows": [{"name": "Short", "values": {"Size": "'S'"}}]})");
  CHECK(!isOpenTable(closed));
  const QJsonObject old = parse(R"({"selectors": ["Size", "Length"], "default": "M5x16",
    "rows": [{"name": "M5x16", "select": {"Size": "M5", "Length": "16"}}]})");
  CHECK(!isOpenTable(old));
  CHECK(tableOptions(old) == (QStringList{QStringLiteral("Size"), QStringLiteral("Length")}));
  CHECK(rowOptionText(old.value(QStringLiteral("rows")).toArray().first().toObject(), QStringLiteral("Size")) ==
        QStringLiteral("M5"));
}

void valueTests() {
  PartOption color;
  color.name = QStringLiteral("Color");
  color.text = true;
  color.valueText = QStringLiteral("Black");
  PartOption length;
  length.name = QStringLiteral("Length");
  length.unit = QStringLiteral("mm");
  length.value = 800.0;
  CHECK(constantExpression(color, QStringLiteral("Black")) == QStringLiteral("'Black'"));
  CHECK(constantExpression(color, QStringLiteral("Jack's")) == QStringLiteral("'Jack''s'"));
  CHECK(constantExpression(length, QStringLiteral("800")) == QStringLiteral("800 mm"));
  CHECK(constantExpression(length, QStringLiteral("0.8 m")) == QStringLiteral("0.8 m"));
  CHECK(queryValue(color, QStringLiteral("'White'")).toString() == QStringLiteral("White"));
  CHECK(queryValue(length, QStringLiteral("800")).toString() == QStringLiteral("800 mm"));
  CHECK(valueOfExpression(QStringLiteral("'Jack''s'")) == QStringLiteral("Jack's"));
  CHECK(isConstantExpression(QStringLiteral("'Black'")));
  CHECK(isConstantExpression(QStringLiteral("800 mm")));
  CHECK(isConstantExpression(QStringLiteral("1,5 in")));
  CHECK(!isConstantExpression(QStringLiteral("Shelf_L")));
  CHECK(!isConstantExpression(QStringLiteral("Shelf_L - 2 * Wall")));
  CHECK(!isConstantExpression(QStringLiteral("'a' + 'b'")));
  CHECK(std::abs(constantNumber(QStringLiteral("1 in"), QStringLiteral("mm")) - 25.4) < 1e-12);
  CHECK(constantNumber(QStringLiteral("800"), QStringLiteral("cm")) == 8000.0);
  CHECK(std::isnan(constantNumber(QStringLiteral("Shelf_L"), QStringLiteral("mm"))));
  CHECK(sameValue(length, QJsonValue(QStringLiteral("80 cm"))));
  CHECK(!sameValue(length, QJsonValue(QStringLiteral("810 mm"))));
  CHECK(sameValue(color, QJsonValue(QStringLiteral("Black"))));
  CHECK(numberText(25.4, QStringLiteral("in")) == QStringLiteral("1 in"));
  CHECK(numberText(1000.0, QStringLiteral("mm")) == QStringLiteral("1000 mm"));
  CHECK(numberText(2.0, QString()) == QStringLiteral("2"));
}

} // namespace

int partOptionsTests() {
  answerTests();
  requestTests();
  tableTests();
  valueTests();
  return failures;
}
