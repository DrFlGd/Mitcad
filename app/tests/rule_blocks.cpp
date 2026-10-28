// SPDX-License-Identifier: MIT
// The rule block editor (mitcad#808): the expression <-> block round trip
// (every rule of docs/configurations.md and of commands.md keeps its text
// exactly, and so does a rule with a part that has no block, which is a
// formula block), the canonical text of edited blocks, the edits (put,
// take, move, the Rule group), and the Rules page on a design of the
// test's own: a rule built from the palette at the canvas's focus, saved
// once complete, kept unsaved while a socket is empty, Ctrl+Z, the keys,
// View as Text and Check Rules; a "combination in table" block whose
// table is chosen among the design's and whose options' columns follow
// it (mitcad#849). Qt Widgets on the offscreen platform.
#include <cstdio>
#include <functional>
#include <vector>

#include <QApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QKeyEvent>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QString>
#include <QStringList>

#include "configurations/RuleBlocks.hpp"
#include "configurations/RuleCanvas.hpp"
#include "configurations/RulesPage.hpp"

namespace {

int failures = 0;

void check(bool value, const char* expression, int line) {
  if (!value) {
    std::fprintf(stderr, "rule_blocks.cpp:%d: %s\n", line, expression);
    ++failures;
  }
}
#define CHECK(value) check((value), #value, __LINE__)

using namespace mitcad::rules;

QJsonObject json(const char* text) {
  return QJsonDocument::fromJson(QByteArray(text)).object();
}

QString compact(const QJsonObject& object) {
  return QString::fromUtf8(QJsonDocument(object).toJson(QJsonDocument::Compact));
}

// Every block's source cleared: the block prints canonically.
void forget(Block& block) {
  block.source.clear();
  for (Block& child : block.children) {
    forget(child);
  }
}

QString canonical(const QString& text) {
  Block block = parseExpression(text);
  forget(block);
  return printExpression(block);
}

void roundTrips() {
  // The design note's rules (docs/configurations.md, "Rules") and the
  // commands' (commands.md, "Rules"), and rules with parts that have no
  // block: each comes back exactly, its conditions character for
  // character.
  const char* const rules[] = {
      R"json({"name": "Black is short", "enabled": true, "when": "Color = 'Black'",
          "then": [{"option": "Length", "max": "1000 mm"}]})json",
      R"json({"name": "Power needs tables", "when": "Tables = 0", "then": [{"option": "Power", "values": ["0"]}]})json",
      R"json({"name": "Made sizes", "then": [{"in_table": "Made", "columns": {"Color": "Color", "Length": "Length"}}]})json",
      R"json({"name": "No red XL", "when": "and(Color = 'Red'; Size = 'XL')",
          "then": [{"forbid": "Red is not made in XL"}]})json",
      R"json({"name": "Black is short", "when": "Color == 'Black'", "then": [{"option": "Length", "max": "1000 mm"}]})json",
      R"json({"name": "Power needs tables", "when": "Tables == 0", "then": [{"option": "Power", "values": ["0"]}]})json",
      R"json({"name": "No red XL", "when": "and(Color == 'Red'; Size == 'XL')",
          "then": [{"forbid": "Red is not made in XL"}]})json",
      R"json({"name": "Deep enough", "then": [{"require": "Depth >= Length / 4",
          "message": "the shelf is too shallow for its length"}]})json",
      // No block for `if`, `*` or a lookup: formula blocks.
      R"json({"name": "Long XL", "when": "if(Size = 'XL'; Length > 1000 mm; 0)",
          "then": [{"option": "Depth", "min": "400 mm", "max": "800 mm", "step": "10 mm"}]})json",
      R"json({"name": "Wide", "comment": "kept", "enabled": false, "when": "and(Color == 'Red';Length*2 > 3000 mm)",
          "then": [{"option": "Color", "exclude": ["'Red'", "'Black'"]}],
          "else": [{"option": "Length", "values": ["300 mm", "XLOOKUP(Size; Sizes.Name; Sizes.Length)"]}]})json",
      R"json({"name": "Colours", "when": "or(Color = 'Red'; Color = 'Black')", "then": [{"forbid": ""}]})json",
      R"json({"name": "Spaces", "when": "  not( Tables == 0 )  ", "then": []})json",
      R"json({"name": "Unknown", "then": [{"something": "else"}, {"option": "Length", "max": "1 m", "later": 1}]})json",
  };
  for (const char* text : rules) {
    const QJsonObject rule = json(text);
    CHECK(!rule.isEmpty());
    const QJsonObject back = ruleToJson(ruleFromJson(rule));
    if (back != rule) {
      std::fprintf(stderr, "rule_blocks.cpp: %s\n  came back as %s\n", qPrintable(compact(rule)),
                   qPrintable(compact(back)));
    }
    CHECK(back == rule);
  }

  // The blocks read from them.
  const Block black = parseExpression(QStringLiteral("Color = 'Black'"));
  CHECK(black.kind == BlockKind::Compare && black.text == QStringLiteral("="));
  CHECK(black.children.size() == 2 && black.children[0].kind == BlockKind::Name &&
        black.children[0].text == QStringLiteral("Color"));
  CHECK(black.children[1].kind == BlockKind::Text && black.children[1].text == QStringLiteral("Black"));
  const Block red = parseExpression(QStringLiteral("and(Color == 'Red'; Size == 'XL')"));
  CHECK(red.kind == BlockKind::And && red.children.size() == 2 && red.children[1].kind == BlockKind::Compare);
  const Block length = parseExpression(QStringLiteral("Length >= 1000 mm"));
  CHECK(length.kind == BlockKind::Compare && length.children[1].kind == BlockKind::Number &&
        length.children[1].text == QStringLiteral("1000 mm"));
  const Block wide = parseExpression(QStringLiteral("and(Color == 'Red';Length*2 > 3000 mm)"));
  CHECK(wide.kind == BlockKind::And && wide.children[1].kind == BlockKind::Compare &&
        wide.children[1].children[0].kind == BlockKind::Formula &&
        wide.children[1].children[0].text == QStringLiteral("Length*2"));
  const Block formula = parseExpression(QStringLiteral("if(Size = 'XL'; Length > 1000 mm; 0)"));
  CHECK(formula.kind == BlockKind::Formula);
  const Block colours = parseExpression(QStringLiteral("or(Color = 'Red'; Color = 'Black')"));
  CHECK(colours.kind == BlockKind::OneOf && colours.children.size() == 3 &&
        colours.children[0].text == QStringLiteral("Color") && colours.children[2].text == QStringLiteral("Black"));
  CHECK(parseExpression(QStringLiteral("Colors.Name")).kind == BlockKind::Column);
  CHECK(parseExpression(QStringLiteral("-5 mm")).kind == BlockKind::Number);
  CHECK(parseExpression(QStringLiteral("10 mm^2")).kind == BlockKind::Number);
  CHECK(parseExpression(QStringLiteral("a = b = c")).kind == BlockKind::Formula);
  CHECK(parseExpression(QStringLiteral("'it''s'")).text == QStringLiteral("it's"));
  CHECK(parseExpression(QStringLiteral("and(a = 1,5)")).children.size() == 1); // a decimal comma

  // Canonical text, as an edited block prints it.
  for (const char* text : {"Color == 'Black'", "and(Color == 'Red'; Size == 'XL')", "not(Tables == 0)",
                           "or(Color == 'Red'; Color == 'Black')", "Length >= 1000 mm", "Depth >= Length / 4",
                           "(a < b) == 1", "'it''s' <> Name"}) {
    CHECK(canonical(QString::fromUtf8(text)) == QString::fromUtf8(text));
  }
  CHECK(canonical(QStringLiteral("AND(a=1;b=2)")) == QStringLiteral("and(a = 1; b = 2)"));
}

void edits() {
  // "Black is short" made block by block.
  Rule rule;
  rule.name = QStringLiteral("Black is short");
  CHECK(isComplete(rule));
  CHECK(applyRuleEntry(rule, QStringLiteral("when")));
  CHECK(!applyRuleEntry(rule, QStringLiteral("when")));
  CHECK(!isComplete(rule));
  const Target when{BlockPath{0, {}}, false};
  CHECK(!fits(rule, when, BlockKind::Max)); // an action does not fit a value socket
  CHECK(place(rule, when, newBlock(QStringLiteral("compare"))));
  CHECK(place(rule, Target{BlockPath{0, {0}}, false}, newBlock(QStringLiteral("option"), QStringLiteral("Color"))));
  Block black = newBlock(QStringLiteral("text"));
  black.text = QStringLiteral("Black");
  CHECK(place(rule, Target{BlockPath{0, {1}}, false}, black));
  CHECK(!fits(rule, Target{BlockPath{1, {0}}, true}, BlockKind::Compare)); // a value does not fit a then slot
  CHECK(place(rule, Target{BlockPath{1, {0}}, true}, newBlock(QStringLiteral("max"), QStringLiteral("Length"))));
  CHECK(!isComplete(rule));
  CHECK(place(rule, Target{BlockPath{1, {0, 0}}, false}, parseExpression(QStringLiteral("1000 mm"))));
  bool complete = false;
  const QJsonObject made = ruleToJson(rule, &complete);
  CHECK(complete);
  CHECK(made == json(R"json({"name": "Black is short", "when": "Color == 'Black'",
                         "then": [{"option": "Length", "max": "1000 mm"}]})json"));
  CHECK(ruleText(rule) == QStringList({QStringLiteral("when Color == 'Black'"),
                                       QStringLiteral("then Length max 1000 mm")}));

  // An edit inside a read rule: the rest keeps its text, the edited parts
  // print canonically.
  Rule read = ruleFromJson(json(R"json({"name": "R", "when": "and(Color = 'Red';  Size='XL')", "then": []})json"));
  Block* size = blockAt(read, BlockPath{0, {1, 1}});
  CHECK(size != nullptr && size->kind == BlockKind::Text);
  size->text = QStringLiteral("XXL");
  touch(read, BlockPath{0, {1, 1}});
  CHECK(printExpression(read.when) == QStringLiteral("and(Color = 'Red'; Size = 'XXL')"));

  // Taken out: a list's entry goes, a socket stays empty.
  Rule list = ruleFromJson(json(R"json({"name": "L", "when": "and(a == 1; b == 2; c == 3)", "then": []})json"));
  const Block taken = take(list, BlockPath{0, {1}});
  CHECK(taken.kind == BlockKind::Compare && list.when.children.size() == 2);
  take(list, BlockPath{0, {0, 1}});
  CHECK(list.when.children[0].children[1].kind == BlockKind::Empty && !isComplete(list));

  // Moves: an action up, a value into another socket, not into itself.
  Rule moves = ruleFromJson(json(R"json({"name": "M", "when": "a == 1", "then": [{"forbid": "x"},
      {"option": "L", "min": "1 mm"}, {"option": "L", "max": "2 mm"}]})json"));
  CHECK(mitcad::rules::move(moves, BlockPath{1, {2}}, Target{BlockPath{1, {0}}, true}));
  CHECK(moves.then[0].kind == BlockKind::Max && moves.then[1].kind == BlockKind::Forbid);
  CHECK(mitcad::rules::move(moves, BlockPath{1, {0}}, Target{BlockPath{1, {3}}, true}));
  CHECK(moves.then[2].kind == BlockKind::Max);
  CHECK(!mitcad::rules::move(moves, BlockPath{0, {}}, Target{BlockPath{0, {0}}, false}));
  CHECK(!mitcad::rules::move(moves, BlockPath{0, {1}}, Target{BlockPath{1, {0, 0}}, false})); // forbid has no socket
  CHECK(mitcad::rules::move(moves, BlockPath{0, {1}}, Target{BlockPath{1, {1, 0}}, false}));
  CHECK(moves.when.children[1].kind == BlockKind::Empty &&
        printExpression(moves.then[1].children[0]) == QStringLiteral("1"));

  // The actions of one object go back as one, also after an edit.
  Rule range = ruleFromJson(json(R"json({"name": "D", "then": [{"option": "Depth", "min": "4 mm", "max": "8 mm"}]})json"));
  CHECK(range.then.size() == 2);
  range.then[1].children[0] = parseExpression(QStringLiteral("9 mm"));
  CHECK(ruleToJson(range) == json(R"json({"name": "D", "then": [{"option": "Depth", "min": "4 mm", "max": "9 mm"}]})json"));

  // The Rule group: else, always.
  Rule sections;
  sections.name = QStringLiteral("S");
  CHECK(applyRuleEntry(sections, QStringLiteral("else")) && !isComplete(sections)); // an empty else
  CHECK(place(sections, Target{BlockPath{2, {0}}, true}, newBlock(QStringLiteral("forbid"))));
  CHECK(isComplete(sections));
  CHECK(applyRuleEntry(sections, QStringLiteral("when")) && applyRuleEntry(sections, QStringLiteral("always")));
  CHECK(!sections.hasWhen);
}

// A design of the test's own behind the page's source.
struct Design {
  QJsonObject table = json(R"json({"options": ["Color", "Length"], "rows": [], "rules": []})json");
  std::vector<QJsonArray> saves;
  bool refuse = false;

  RulesSource source() {
    RulesSource source;
    source.configurations = [this] { return QJsonObject{{QStringLiteral("configurations"), table}}; };
    source.configure = [] {
      return json(R"json({"options": [{"name": "Color", "kind": "text"}, {"name": "Length", "kind": "number"}]})json");
    };
    source.setRules = [this](const QJsonArray& rules, QString* error) {
      if (refuse) {
        *error = QStringLiteral("the rule Black is short: no");
        return false;
      }
      table.insert(QStringLiteral("rules"), rules);
      saves.push_back(rules);
      return true;
    };
    source.checkRules = [] {
      return json(R"json({"never_allowed": [{"option": "Length", "text": "2500 mm"}],
                      "lines": ["Length 2500 mm is never allowed"]})json");
    };
    source.tables = [] {
      return QJsonDocument::fromJson(QByteArray(R"json([{"name": "Made", "columns": [
          {"name": "Color", "unit": "Text"}, {"name": "Length", "unit": "mm"}, {"name": "Note", "unit": "Text"}]}])json"))
          .array();
    };
    return source;
  }
};

void type(RuleCanvas* canvas, const QString& text) {
  QLineEdit* editor = canvas->editor();
  CHECK(editor != nullptr);
  if (editor != nullptr) {
    editor->setText(text);
    emit editor->returnPressed();
  }
}

void key(QWidget* widget, int code) {
  QKeyEvent press(QEvent::KeyPress, code, Qt::NoModifier);
  QApplication::sendEvent(widget, &press);
}

void pageTests() {
  Design design;
  RulesPage page(design.source());
  RuleCanvas* canvas = page.canvas();
  CHECK(page.rules().empty() && !canvas->hasRule());

  page.addRule();
  CHECK(design.saves.size() == 1 && page.rules().size() == 1 && page.rules()[0].name == QStringLiteral("Rule 1"));

  // when … then from the palette, a compare, Color and 'Black': a socket
  // left empty keeps the rule unsaved.
  CHECK(canvas->insertAtFocus(QStringLiteral("when")));
  CHECK(page.hasUnsaved() && design.saves.size() == 1);
  CHECK(page.problemOf(0) == QStringLiteral("Not saved: fill its empty sockets"));
  CHECK(canvas->focusPiece(QStringLiteral("when socket")));
  CHECK(canvas->insertAtFocus(QStringLiteral("compare")));
  CHECK(canvas->focusName() == QStringLiteral("when.0 socket")); // its first empty socket
  CHECK(canvas->insertAtFocus(QStringLiteral("option")));
  type(canvas, QStringLiteral("Color")); // its editor opened at once
  CHECK(canvas->focusPiece(QStringLiteral("when.1 socket")));
  CHECK(canvas->insertAtFocus(QStringLiteral("text")));
  // The new text and its value are one edit: nothing saved while its
  // editor is open (an empty text would make `Color == ''` a step).
  CHECK(canvas->editor() != nullptr && design.saves.size() == 1);
  type(canvas, QStringLiteral("Black"));
  CHECK(!page.hasUnsaved() && design.saves.size() == 2);
  // max goes to the then; Length, the number option, is its option.
  CHECK(canvas->insertAtFocus(QStringLiteral("max")));
  CHECK(canvas->focusName() == QStringLiteral("then.0.0 socket"));
  CHECK(page.hasUnsaved());
  CHECK(canvas->insertAtFocus(QStringLiteral("number")));
  type(canvas, QStringLiteral("1000 mm"));
  CHECK(!page.hasUnsaved() && design.saves.size() == 3);
  CHECK(design.saves.back() ==
        QJsonArray{json(R"json({"name": "Rule 1", "when": "Color == 'Black'",
                            "then": [{"option": "Length", "max": "1000 mm"}]})json")});

  // The keys: Right from the when's word to its compare, Delete on the
  // number, Ctrl+Z.
  CHECK(canvas->focusPiece(QStringLiteral("when label")));
  key(canvas, Qt::Key_Right);
  CHECK(canvas->focusName() == QStringLiteral("when block"));
  CHECK(canvas->focusPiece(QStringLiteral("then.0.0 block")));
  key(canvas, Qt::Key_Delete);
  CHECK(page.hasUnsaved() && design.saves.size() == 3);
  CHECK(page.undo() && !page.hasUnsaved());
  CHECK(printExpression(page.rules()[0].then[0].children[0]) == QStringLiteral("1000 mm"));

  // What the design refuses stays unsaved, marked with the reason.
  design.refuse = true;
  CHECK(canvas->focusPiece(QStringLiteral("then.1 slot")));
  CHECK(canvas->insertAtFocus(QStringLiteral("forbid")));
  type(canvas, QStringLiteral("not black"));
  CHECK(page.hasUnsaved() && page.problemOf(0).contains(QStringLiteral("no")));
  // The forbid and its message were one edit: one Ctrl+Z.
  CHECK(page.undo() && !page.hasUnsaved());
  design.refuse = false;

  page.setTextShown(true);
  const QString text = page.textView()->toPlainText();
  CHECK(text.contains(QStringLiteral("when Color == 'Black'")) && text.contains(QStringLiteral("then Length max 1000 mm")));

  page.checkRules();
  CHECK(page.report()->toPlainText().contains(QStringLiteral("Length 2500 mm is never allowed")));

  page.duplicateRule();
  CHECK(page.rules().size() == 2 && page.rules()[1].name == QStringLiteral("Rule 1 2"));
  page.deleteRule();
  CHECK(page.rules().size() == 1 && design.saves.size() == 5);
}

// A "combination in table" block (mitcad#849): its columns follow the
// table chosen, by the options' names; one the user mapped stays while the
// table has its column.
void tableTests() {
  const QStringList columns{QStringLiteral("Color"), QStringLiteral("LENGTH"), QStringLiteral("Price")};
  const QStringList options{QStringLiteral("Color"), QStringLiteral("Length"), QStringLiteral("Size")};
  CHECK(inTableColumns(columns, options, QJsonObject()) == json(R"json({"Color": "Color", "Length": "LENGTH"})json"));
  CHECK(inTableColumns(columns, options, json(R"json({"Color": "Price", "Length": "Gone", "Other": "Color"})json")) ==
        json(R"json({"Color": "Price", "Length": "LENGTH"})json"));
  CHECK(inTableColumns({}, options, json(R"json({"Color": "Color"})json")).isEmpty());

  // On the page: the block's table typed (the design's Made), its options'
  // columns mapped, and the rule saved complete, as the model takes it.
  Design design;
  RulesPage page(design.source());
  RuleCanvas* canvas = page.canvas();
  page.addRule();
  CHECK(design.saves.size() == 1);
  CHECK(canvas->focusPiece(QStringLiteral("then.0 slot")));
  CHECK(canvas->insertAtFocus(QStringLiteral("in_table")));
  // The new block and its table are one edit: nothing saved while the
  // table's editor is open.
  CHECK(canvas->editor() != nullptr && design.saves.size() == 1);
  type(canvas, QStringLiteral("Made"));
  CHECK(!page.hasUnsaved() && design.saves.size() == 2);
  CHECK(design.saves.back() ==
        QJsonArray{json(R"json({"name": "Rule 1",
                            "then": [{"in_table": "Made", "columns": {"Color": "Color", "Length": "Length"}}]})json")});
  page.setTextShown(true);
  CHECK(page.textView()->toPlainText().contains(
      QStringLiteral("combination in table Made (Color = Color, Length = Length)")));
  // A table the design does not have keeps the columns as they are.
  CHECK(canvas->focusPiece(QStringLiteral("then.0 table")));
  CHECK(canvas->editFocused());
  type(canvas, QStringLiteral("Elsewhere"));
  CHECK(design.saves.size() == 3);
  CHECK(design.saves.back() ==
        QJsonArray{json(R"json({"name": "Rule 1",
                            "then": [{"in_table": "Elsewhere", "columns": {"Color": "Color", "Length": "Length"}}]})json")});
}

} // namespace

int ruleBlocksTests() {
  roundTrips();
  edits();
  pageTests();
  tableTests();
  return failures;
}
