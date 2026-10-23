// SPDX-License-Identifier: MIT
// Parameter suggestions of expression fields (mitcad#567, mitcad#568): the
// name at the caret, what matches it and in what order, what a choice does
// to the text, the choices a field cannot take, new names; and on a plain
// field of a window on the offscreen platform, the list's keys (Enter puts
// the reference in without the field's own Enter, Esc only closes the
// list), the fx browser searching comments, and `name = …`.
#include "framework/ParameterField.hpp"
#include "framework/ParameterSuggest.hpp"

#include <cstdio>
#include <vector>

#include <QApplication>
#include <QEvent>
#include <QJsonArray>
#include <QJsonObject>
#include <QKeyEvent>
#include <QLineEdit>
#include <QString>
#include <QStringList>
#include <QWidget>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "parameter_suggest.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using namespace mitcad;

QJsonObject parameter(const char* name, const char* expression, const char* text, const char* unit,
                      const char* kind, const char* comment, const QStringList& uses = {},
                      const char* owner = "") {
  return {{QStringLiteral("name"), QString::fromUtf8(name)},
          {QStringLiteral("expression"), QString::fromUtf8(expression)},
          {QStringLiteral("text"), QString::fromUtf8(text)},
          {QStringLiteral("unit"), QString::fromUtf8(unit)},
          {QStringLiteral("kind"), QString::fromUtf8(kind)},
          {QStringLiteral("comment"), QString::fromUtf8(comment)},
          {QStringLiteral("owner"), QString::fromUtf8(owner)},
          {QStringLiteral("dependencies"), QJsonArray::fromStringList(uses)}};
}

// A small design's parameters, as the `parameters` query gives them: three
// user parameters (two lengths, an angle), model parameters of which d1
// uses width and d2 uses d1, and Sketch1's (F1) driven reference d3.
QJsonArray designParameters() {
  return {parameter("width", "30 mm", "30 mm", "mm", "user", ""),
          parameter("thickness", "4 mm", "4 mm", "mm", "user", "Wall thickness"),
          parameter("tilt", "15 deg", "15 deg", "deg", "user", ""),
          parameter("d1", "width + 5 mm", "35 mm", "mm", "model", "Extrude1 distance", {QStringLiteral("width")},
                    "F2"),
          parameter("d10", "2 mm", "2 mm", "mm", "model", "Fillet1 radius", {}, "F3"),
          parameter("d2", "d1 * 2", "70 mm", "mm", "model", "Sketch1 length", {QStringLiteral("d1")}, "F1"),
          parameter("d3", "25", "25 mm", "mm", "reference", "Sketch1 length (driven)", {}, "F1")};
}

QStringList names(const std::vector<ParameterChoice>& choices) {
  QStringList found;
  for (const ParameterChoice& choice : choices) {
    found << choice.parameter->name;
  }
  return found;
}

const ParameterChoice* choiceOf(const std::vector<ParameterChoice>& choices, const QString& name) {
  for (const ParameterChoice& choice : choices) {
    if (choice.parameter->name == name) {
      return &choice;
    }
  }
  return nullptr;
}

void fragmentTests() {
  // In `width / 2 + thi` the list is for `thi`.
  NameFragment f = nameFragmentAt(QStringLiteral("width / 2 + thi"), 15);
  CHECK(f.active);
  CHECK(f.typed == QStringLiteral("thi"));
  CHECK(f.start == 12 && f.length == 3);
  // In the middle of an expression and of a name: filtered by what is
  // before the caret, the whole name replaced.
  f = nameFragmentAt(QStringLiteral("a + thi * 2"), 6);
  CHECK(f.active && f.typed == QStringLiteral("th") && f.start == 4 && f.length == 3);
  // After an operator: a name.
  CHECK(nameFragmentAt(QStringLiteral("1.5 + x"), 7).active);
  CHECK(nameFragmentAt(QStringLiteral("max(a; wi"), 9).active);
  // Units, numbers, functions: no list.
  CHECK(!nameFragmentAt(QStringLiteral("30 mm"), 5).active);
  CHECK(!nameFragmentAt(QStringLiteral("d1 mm"), 5).active);
  CHECK(!nameFragmentAt(QStringLiteral("(a + b) in"), 10).active);
  CHECK(!nameFragmentAt(QStringLiteral("12mm"), 4).active);
  CHECK(!nameFragmentAt(QStringLiteral("2e3"), 3).active);
  CHECK(!nameFragmentAt(QStringLiteral("ma(1; 2)"), 2).active);
  CHECK(!nameFragmentAt(QStringLiteral("d1 * "), 5).active);
  CHECK(!nameFragmentAt(QString(), 0).active);
  // `name = …` names the field's own parameter: not a reference.
  f = nameFragmentAt(QStringLiteral("width = 3"), 2);
  CHECK(!f.active && f.assigned);
  CHECK(nameFragmentAt(QStringLiteral("wi == 3"), 2).active); // a comparison
  CHECK(nameFragmentAt(QStringLiteral("width = thi"), 11).active);
}

void insertionTests() {
  // Only the name typed is replaced; the rest of the expression stays.
  Insertion in = insertParameter(QStringLiteral("width / 2 + thi"), 15, -1, 0, QStringLiteral("thickness"));
  CHECK(in.text == QStringLiteral("width / 2 + thickness"));
  CHECK(in.caret == 21);
  CHECK(in.from == 12 && in.length == 3);
  in = insertParameter(QStringLiteral("a + thi * 2"), 6, -1, 0, QStringLiteral("thickness"));
  CHECK(in.text == QStringLiteral("a + thickness * 2"));
  CHECK(in.caret == 13);
  // At the caret after an operator, and into an empty field.
  in = insertParameter(QStringLiteral("d1 * "), 5, -1, 0, QStringLiteral("width"));
  CHECK(in.text == QStringLiteral("d1 * width") && in.caret == 10);
  CHECK(insertParameter(QString(), 0, -1, 0, QStringLiteral("width")).text == QStringLiteral("width"));
  // A selection (a value not edited yet) is replaced.
  CHECK(insertParameter(QStringLiteral("10 mm"), 5, 0, 5, QStringLiteral("width")).text == QStringLiteral("width"));
  // At the caret where no name is; never in place of a unit.
  CHECK(insertParameter(QStringLiteral("2 * 3 mm"), 3, -1, 0, QStringLiteral("w")).text ==
        QStringLiteral("2 *w 3 mm"));
  CHECK(insertParameter(QStringLiteral("2 * 3 mm"), 8, -1, 0, QStringLiteral("w")).text ==
        QStringLiteral("2 * 3 mmw"));
  CHECK(expressionPart(QStringLiteral("width = d1 * 2")) == QStringLiteral("d1 * 2"));
  CHECK(expressionPart(QStringLiteral(" w=3")) == QStringLiteral("3"));
  CHECK(expressionPart(QStringLiteral("d1 == 2")) == QStringLiteral("d1 == 2"));
}

void nameTests() {
  CHECK(isValidParameterName(QStringLiteral("width")));
  CHECK(isValidParameterName(QStringLiteral("_a1")));
  CHECK(isValidParameterName(QStringLiteral("breite_ä")));
  CHECK(!isValidParameterName(QStringLiteral("1a")));
  CHECK(!isValidParameterName(QStringLiteral("a b")));
  CHECK(!isValidParameterName(QString()));
  // Units, functions and constants are reserved.
  CHECK(!isValidParameterName(QStringLiteral("mm")));
  CHECK(!isValidParameterName(QStringLiteral("deg")));
  CHECK(!isValidParameterName(QStringLiteral("sin")));
  CHECK(!isValidParameterName(QStringLiteral("PI")));
  CHECK(isValidParameterName(QStringLiteral("pi"))); // case-sensitive
  CHECK(kindOfUnit(QString()) == ParameterKind::Unitless);
  CHECK(kindOfUnit(QStringLiteral("deg")) == ParameterKind::Angle);
  CHECK(kindOfUnit(QStringLiteral("rad")) == ParameterKind::Angle);
  CHECK(kindOfUnit(QStringLiteral("mm")) == ParameterKind::Length);
  CHECK(kindOfUnit(QStringLiteral("in")) == ParameterKind::Length);
  CHECK(naturalLess(QStringLiteral("d2"), QStringLiteral("d10")));
  CHECK(!naturalLess(QStringLiteral("d10"), QStringLiteral("d2")));
  CHECK(naturalLess(QStringLiteral("a"), QStringLiteral("B")));
  CHECK(naturalLess(QStringLiteral("d1"), QStringLiteral("d1a")));
  CHECK(kindProblem(ParameterKind::Length, ParameterKind::Unitless).isEmpty());
  CHECK(kindProblem(ParameterKind::Angle, ParameterKind::Angle).isEmpty());
  CHECK(!kindProblem(ParameterKind::Angle, ParameterKind::Length).isEmpty());
  CHECK(!kindProblem(ParameterKind::Unitless, ParameterKind::Length).isEmpty());
  CHECK(numberWithUnit(QStringLiteral("30"), QStringLiteral("mm")) == QStringLiteral("30 mm"));
  CHECK(numberWithUnit(QStringLiteral("2,5"), QStringLiteral("mm")) == QStringLiteral("2.5 mm"));
  CHECK(numberWithUnit(QStringLiteral("d1 * 2"), QStringLiteral("mm")) == QStringLiteral("d1 * 2"));
  CHECK(numberWithUnit(QStringLiteral("4"), QString()) == QStringLiteral("4"));
}

void matchTests() {
  const std::vector<SuggestedParameter> parameters = suggestedParameters(designParameters());
  CHECK(parameters.size() == 7);
  CHECK(parameters[0].group == SuggestedParameter::Group::User);
  CHECK(parameters[3].group == SuggestedParameter::Group::Model);
  CHECK(parameters[6].reference);
  CHECK(parameters[2].kind == ParameterKind::Angle);
  const auto whole = [](const QString& name) { return name; };
  SuggestContext length;
  // Names that start with the text first, then those that contain it.
  CHECK(names(matchParameters(parameters, QStringLiteral("th"), false, length, whole)) ==
        QStringList({QStringLiteral("thickness"), QStringLiteral("width")}));
  // User parameters first, then the model's in natural order.
  CHECK(names(matchParameters(parameters, QStringLiteral("d"), false, length, whole)) ==
        QStringList({QStringLiteral("width"), QStringLiteral("d1"), QStringLiteral("d2"), QStringLiteral("d3"),
                     QStringLiteral("d10")}));
  // Case-insensitive; similar names all listed, the exact one first.
  CHECK(names(matchParameters(parameters, QStringLiteral("D1"), false, length, whole)) ==
        QStringList({QStringLiteral("d1"), QStringLiteral("d10")}));
  // No match.
  CHECK(matchParameters(parameters, QStringLiteral("xyz"), false, length, whole).empty());
  // Comments only in the browser's search.
  CHECK(matchParameters(parameters, QStringLiteral("wall"), false, length, whole).empty());
  CHECK(names(matchParameters(parameters, QStringLiteral("wall"), true, length, whole)) ==
        QStringList({QStringLiteral("thickness")}));
  // Everything for an empty filter.
  CHECK(matchParameters(parameters, QString(), true, length, whole).size() == 7);

  // An angle as a length field's whole expression is explained, not
  // offered as valid; inside an expression it is fine.
  std::vector<ParameterChoice> choices = matchParameters(parameters, QStringLiteral("ti"), false, length, whole);
  CHECK(choices.size() == 1 && !choices.front().problem.isEmpty());
  const auto inside = [](const QString& name) { return QStringLiteral("d1 * tan(%1)").arg(name); };
  choices = matchParameters(parameters, QStringLiteral("ti"), false, length, inside);
  CHECK(choices.size() == 1 && choices.front().problem.isEmpty());
  SuggestContext angle;
  angle.kind = ParameterKind::Angle;
  choices = matchParameters(parameters, QStringLiteral("t"), false, angle, whole);
  CHECK(choiceOf(choices, QStringLiteral("tilt")) && choiceOf(choices, QStringLiteral("tilt"))->problem.isEmpty());
  CHECK(choiceOf(choices, QStringLiteral("thickness")) &&
        !choiceOf(choices, QStringLiteral("thickness"))->problem.isEmpty());

  // The field's own parameter is not offered; those that use it would
  // make a cycle.
  SuggestContext own;
  own.own = QStringLiteral("d1");
  choices = matchParameters(parameters, QStringLiteral("d"), false, own, whole);
  CHECK(!choiceOf(choices, QStringLiteral("d1")));
  CHECK(choiceOf(choices, QStringLiteral("d2")) && !choiceOf(choices, QStringLiteral("d2"))->problem.isEmpty());
  CHECK(choiceOf(choices, QStringLiteral("d10")) && choiceOf(choices, QStringLiteral("d10"))->problem.isEmpty());
  CHECK(dependentsOf(parameters, QStringLiteral("width")) == QStringList({QStringLiteral("d1"), QStringLiteral("d2")}));
  // A sketch cannot use what its own driven dimensions measure.
  SuggestContext sketch;
  sketch.sketch = QStringLiteral("F1");
  choices = matchParameters(parameters, QStringLiteral("d3"), false, sketch, whole);
  CHECK(choices.size() == 1 && !choices.front().problem.isEmpty());
  sketch.sketch = QStringLiteral("F4");
  choices = matchParameters(parameters, QStringLiteral("d3"), false, sketch, whole);
  CHECK(choices.size() == 1 && choices.front().problem.isEmpty());

  // New names: taken, reserved, invalid, free.
  const SuggestedParameter* existing = nullptr;
  CHECK(!newNameProblem(parameters, QStringLiteral("width"), &existing).isEmpty());
  CHECK(existing != nullptr && existing->name == QStringLiteral("width"));
  CHECK(!newNameProblem(parameters, QStringLiteral("mm"), &existing).isEmpty() && existing == nullptr);
  CHECK(!newNameProblem(parameters, QStringLiteral("1x")).isEmpty());
  CHECK(!newNameProblem(parameters, QString()).isEmpty());
  CHECK(newNameProblem(parameters, QStringLiteral("depth")).isEmpty());
}

// Sends a key to the field as typing does: its shortcut override first
// (the window's Enter and Esc shortcuts only fire when it is not
// accepted), then the press. Returns whether the override was accepted.
bool press(QLineEdit& edit, int key, const QString& text = QString()) {
  QKeyEvent override(QEvent::ShortcutOverride, key, Qt::NoModifier, text);
  override.ignore();
  QApplication::sendEvent(&edit, &override);
  const bool taken = override.isAccepted();
  QKeyEvent down(QEvent::KeyPress, key, Qt::NoModifier, text);
  QApplication::sendEvent(&edit, &down);
  QKeyEvent up(QEvent::KeyRelease, key, Qt::NoModifier, text);
  QApplication::sendEvent(&edit, &up);
  return taken;
}

// Letters, spaces and `+`, as typed.
void type(QLineEdit& edit, const QString& text) {
  for (const QChar c : text) {
    int key = static_cast<int>(Qt::Key_Space);
    if (c.isLetter()) {
      key = static_cast<int>(Qt::Key_A) + (static_cast<int>(c.toUpper().unicode()) - static_cast<int>('A'));
    } else if (c == QLatin1Char('+')) {
      key = static_cast<int>(Qt::Key_Plus);
    }
    press(edit, key, QString(c));
  }
}

void fieldTests() {
  QWidget window;
  window.resize(640, 480);
  int added = 0;
  setParameterSource(&window, {[] { return designParameters(); },
                               [&added](const QJsonObject&, QString*) {
                                 ++added;
                                 return true;
                               },
                               [] { return QStringLiteral("mm"); }});
  auto* edit = new QLineEdit(&window);
  edit->setGeometry(20, 20, 200, 24);
  ParameterFieldOptions options;
  options.label = QStringLiteral("test distance");
  options.create = false; // no modal dialog in a unit test
  ParameterField* field = ParameterField::attach(edit, options);
  CHECK(field != nullptr && ParameterField::of(edit) == field);
  CHECK(ParameterField::attach(edit, options) == field); // once per field
  CHECK(parameterSourceOf(edit) != nullptr);
  int returns = 0;
  int edits = 0;
  QString edited;
  QObject::connect(edit, &QLineEdit::returnPressed, [&returns] { ++returns; });
  QObject::connect(edit, &QLineEdit::textEdited, [&edits, &edited](const QString& text) {
    ++edits;
    edited = text;
  });
  window.show();

  // Completion after an operator: Enter puts the reference in place of
  // the name typed, and is not the field's Enter.
  edit->setText(QStringLiteral("width / 2 + "));
  edit->setCursorPosition(static_cast<int>(edit->text().size()));
  type(*edit, QStringLiteral("thi"));
  CHECK(field->isOpen());
  edits = 0;
  CHECK(press(*edit, Qt::Key_Return));
  CHECK(edit->text() == QStringLiteral("width / 2 + thickness"));
  CHECK(edit->cursorPosition() == 21);
  CHECK(!field->isOpen());
  CHECK(returns == 0);
  CHECK(edits == 1 && edited == QStringLiteral("width / 2 + thickness")); // the field's reactions follow
  // With the list closed, Enter is the field's (and the window's) again.
  CHECK(!press(*edit, Qt::Key_Return));
  CHECK(returns == 1);

  // Esc closes the list only: the text stays as typed.
  type(*edit, QStringLiteral(" + w"));
  CHECK(field->isOpen());
  CHECK(press(*edit, Qt::Key_Escape));
  CHECK(!field->isOpen());
  CHECK(edit->text() == QStringLiteral("width / 2 + thickness + w"));
  CHECK(!press(*edit, Qt::Key_Escape)); // the window's Esc again

  // In the middle of an expression; Down moves to the next choice.
  edit->setText(QStringLiteral("2 *  + 1"));
  edit->setCursorPosition(4);
  type(*edit, QStringLiteral("d"));
  CHECK(field->isOpen()); // width (contains d), d1, d2, d3, d10
  CHECK(press(*edit, Qt::Key_Down));
  CHECK(press(*edit, Qt::Key_Return));
  CHECK(edit->text() == QStringLiteral("2 * d1 + 1"));

  // A choice the field cannot take is explained, not put in.
  edit->clear();
  type(*edit, QStringLiteral("ti"));
  CHECK(field->isOpen());
  press(*edit, Qt::Key_Return);
  CHECK(edit->text() == QStringLiteral("ti"));
  CHECK(field->isOpen());
  press(*edit, Qt::Key_Escape);

  // Exactly the one name typed: no list, Enter is the field's.
  edit->clear();
  type(*edit, QStringLiteral("width"));
  CHECK(!field->isOpen());

  // No match: the list says so.
  edit->clear();
  type(*edit, QStringLiteral("xyz"));
  CHECK(field->isOpen());
  press(*edit, Qt::Key_Escape);
  // A unit: no list.
  edit->setText(QStringLiteral("30 "));
  edit->setCursorPosition(3);
  type(*edit, QStringLiteral("mm"));
  CHECK(!field->isOpen());

  // The fx browser: what is typed searches names and comments, the field
  // keeps its text until a choice.
  edit->setText(QStringLiteral("2 * "));
  edit->setCursorPosition(4);
  field->openBrowser();
  CHECK(field->isOpen());
  type(*edit, QStringLiteral("wall"));
  CHECK(edit->text() == QStringLiteral("2 * "));
  CHECK(field->isOpen());
  CHECK(press(*edit, Qt::Key_Return));
  CHECK(edit->text() == QStringLiteral("2 * thickness"));
  // Opened on an empty field: the first choice it can take.
  edit->clear();
  field->openBrowser();
  CHECK(field->isOpen());
  press(*edit, Qt::Key_Return);
  CHECK(edit->text() == QStringLiteral("thickness"));
  // fx again closes it.
  field->openBrowser();
  field->openBrowser();
  CHECK(!field->isOpen());

  // The field's own parameter is not offered (mitcad#231 fields): nothing
  // matches, and Enter puts nothing in.
  field->setOwnParameter(QStringLiteral("width"));
  edit->clear();
  type(*edit, QStringLiteral("wid"));
  CHECK(field->isOpen());
  press(*edit, Qt::Key_Return);
  CHECK(edit->text() == QStringLiteral("wid"));
  press(*edit, Qt::Key_Escape);
  CHECK(!field->isOpen());
  field->setOwnParameter(QString());

  // `name = …` (mitcad#568): a new name typed first is offered as the
  // field's own parameter's name.
  auto* named = new QLineEdit(&window);
  named->setGeometry(20, 60, 200, 24);
  ParameterFieldOptions naming;
  naming.label = QStringLiteral("test dimension");
  naming.naming = true;
  naming.create = false;
  ParameterField* namedField = ParameterField::attach(named, naming);
  named->show();
  type(*named, QStringLiteral("depth"));
  CHECK(namedField->isOpen()); // no match, and the action
  press(*named, Qt::Key_Return);
  CHECK(named->text() == QStringLiteral("depth = "));
  CHECK(!namedField->isOpen());
  // On a value: its parameter's name before it, selected to type over.
  named->setText(QStringLiteral("30 mm"));
  namedField->setOwnParameter(QStringLiteral("d4"));
  namedField->startNaming();
  CHECK(named->text() == QStringLiteral("d4 = 30 mm"));
  CHECK(named->selectedText() == QStringLiteral("d4"));
  CHECK(added == 0);
}

} // namespace

int parameterSuggestTests() {
  failures = 0;
  fragmentTests();
  insertionTests();
  nameTests();
  matchTests();
  fieldTests();
  return failures;
}
