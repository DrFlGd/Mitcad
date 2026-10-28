// SPDX-License-Identifier: MIT
#include "RuleBlocks.hpp"

#include <utility>

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonValue>
#include <QSet>

namespace mitcad::rules {

namespace {

// The expression language's tokens, as far as the editor needs them to find
// the blocks (core/expr's grammar): names, numbers, texts and operators,
// each with where it is in the text.
enum class Tok { Name, Number, Text, Op, Bad };

struct Token {
  Tok type = Tok::Bad;
  int start = 0;
  int end = 0;
  QString text; // Text: without its quotes
};

bool nameStart(QChar c) {
  return c.isLetter() || c == QLatin1Char('_') || c == QChar(0x00B0); // ° is a unit
}

bool namePart(QChar c) {
  return c.isLetterOrNumber() || c == QLatin1Char('_');
}

std::vector<Token> tokenize(const QString& s) {
  std::vector<Token> out;
  const int n = static_cast<int>(s.size());
  int i = 0;
  while (i < n) {
    const QChar c = s[i];
    if (c.isSpace()) {
      ++i;
      continue;
    }
    const int start = i;
    if (c.isDigit() || (c == QLatin1Char('.') && i + 1 < n && s[i + 1].isDigit())) {
      while (i < n && s[i].isDigit()) {
        ++i;
      }
      // A decimal point, or a comma between two digits (a decimal comma).
      if (i < n && (s[i] == QLatin1Char('.') ||
                    (s[i] == QLatin1Char(',') && i > start && i + 1 < n && s[i + 1].isDigit()))) {
        ++i;
        while (i < n && s[i].isDigit()) {
          ++i;
        }
      }
      if (i < n && (s[i] == QLatin1Char('e') || s[i] == QLatin1Char('E'))) {
        int j = i + 1;
        if (j < n && (s[j] == QLatin1Char('+') || s[j] == QLatin1Char('-'))) {
          ++j;
        }
        if (j < n && s[j].isDigit()) {
          i = j;
          while (i < n && s[i].isDigit()) {
            ++i;
          }
        }
      }
      out.push_back({Tok::Number, start, i, s.mid(start, i - start)});
      continue;
    }
    if (nameStart(c)) {
      ++i;
      while (i < n && namePart(s[i])) {
        ++i;
      }
      out.push_back({Tok::Name, start, i, s.mid(start, i - start)});
      continue;
    }
    if (c == QLatin1Char('\'') || c == QLatin1Char('"')) {
      const QChar quote = c;
      QString value;
      bool closed = false;
      ++i;
      while (i < n) {
        if (s[i] == quote) {
          if (i + 1 < n && s[i + 1] == quote) {
            value += quote;
            i += 2;
            continue;
          }
          ++i;
          closed = true;
          break;
        }
        value += s[i];
        ++i;
      }
      out.push_back({closed ? Tok::Text : Tok::Bad, start, i, value});
      continue;
    }
    static const char* const pairs[] = {"==", "<>", "!=", "<=", ">="};
    bool paired = false;
    if (i + 1 < n) {
      const QString two = s.mid(i, 2);
      for (const char* pair : pairs) {
        if (two == QLatin1String(pair)) {
          out.push_back({Tok::Op, start, i + 2, two});
          i += 2;
          paired = true;
          break;
        }
      }
    }
    if (!paired) {
      out.push_back({Tok::Op, start, i + 1, QString(c)});
      ++i;
    }
  }
  return out;
}

bool isCompareText(const QString& op) {
  static const QStringList ops{QStringLiteral("=="), QStringLiteral("="),  QStringLiteral("<>"), QStringLiteral("!="),
                               QStringLiteral("<"),  QStringLiteral("<="), QStringLiteral(">"),  QStringLiteral(">=")};
  return ops.contains(op);
}

struct Parser {
  const QString& s;
  std::vector<Token> t;

  QString span(int b, int e) const { return s.mid(t[b].start, t[e - 1].end - t[b].start); }
  bool isOp(int i, const char* op) const {
    return i >= 0 && i < static_cast<int>(t.size()) && t[i].type == Tok::Op && t[i].text == QLatin1String(op);
  }
  bool isCompare(int i) const { return t[i].type == Tok::Op && isCompareText(t[i].text); }
  // The ")" that closes the "(" at `from`, before `e`; -1 when none does.
  int closing(int from, int e) const {
    int depth = 0;
    for (int i = from; i < e; ++i) {
      if (isOp(i, "(")) {
        ++depth;
      } else if (isOp(i, ")")) {
        if (--depth == 0) {
          return i;
        }
      }
    }
    return -1;
  }
  Block formula(int b, int e) const {
    Block f;
    f.kind = BlockKind::Formula;
    f.text = span(b, e);
    f.source = f.text;
    return f;
  }
  Block leaf(BlockKind kind, const QString& text, int b, int e) const {
    Block x;
    x.kind = kind;
    x.text = text;
    x.source = span(b, e);
    return x;
  }

  // A condition or a value: one comparison at the top, else an operand.
  Block parse(int b, int e) const {
    if (b >= e) {
      return {};
    }
    int depth = 0;
    std::vector<int> compares;
    for (int i = b; i < e; ++i) {
      if (isOp(i, "(")) {
        ++depth;
      } else if (isOp(i, ")")) {
        --depth;
      } else if (depth == 0 && isCompare(i)) {
        compares.push_back(i);
      }
    }
    if (compares.empty()) {
      return operand(b, e);
    }
    const int k = compares.front();
    if (compares.size() == 1 && k > b && k < e - 1) {
      Block compare;
      compare.kind = BlockKind::Compare;
      compare.text = t[k].text;
      compare.children.push_back(operand(b, k));
      compare.children.push_back(operand(k + 1, e));
      compare.source = span(b, e);
      return compare;
    }
    return formula(b, e);
  }

  Block operand(int b, int e) const {
    const int n = e - b;
    if (n == 1) {
      switch (t[b].type) {
      case Tok::Name:
        return leaf(BlockKind::Name, t[b].text, b, e);
      case Tok::Number:
        return leaf(BlockKind::Number, t[b].text, b, e);
      case Tok::Text:
        return leaf(BlockKind::Text, t[b].text, b, e);
      case Tok::Op:
      case Tok::Bad:
        break;
      }
      return formula(b, e);
    }
    // A number with a sign, a unit and the unit's power: `-5 mm`, `10 mm^2`.
    int i = b;
    if (isOp(i, "-") || isOp(i, "+")) {
      ++i;
    }
    if (i < e && t[i].type == Tok::Number) {
      int j = i + 1;
      if (j < e && t[j].type == Tok::Name && isUnit(t[j].text)) {
        ++j;
        if (j < e && isOp(j, "^")) {
          ++j;
          if (j < e && (isOp(j, "-") || isOp(j, "+"))) {
            ++j;
          }
          j = j < e && t[j].type == Tok::Number ? j + 1 : -1;
        }
      }
      if (j == e && (j > i + 1 || i > b)) {
        return leaf(BlockKind::Number, span(b, e), b, e);
      }
    }
    // A table column: `Colors.Name`, written without spaces.
    if (n == 3 && t[b].type == Tok::Name && isOp(b + 1, ".") && t[b + 2].type == Tok::Name &&
        t[b + 1].start == t[b].end && t[b + 2].start == t[b + 1].end) {
      return leaf(BlockKind::Column, span(b, e), b, e);
    }
    // Parentheses around everything: the block inside, which prints them as
    // long as it is not changed.
    if (isOp(b, "(") && closing(b, e) == e - 1) {
      Block inner = parse(b + 1, e - 1);
      if (inner.kind == BlockKind::Empty) {
        return formula(b, e);
      }
      inner.source = span(b, e);
      return inner;
    }
    if (n >= 3 && t[b].type == Tok::Name && isOp(b + 1, "(") && closing(b + 1, e) == e - 1) {
      const QString function = t[b].text.toLower();
      if (function == QLatin1String("and") || function == QLatin1String("or") || function == QLatin1String("not")) {
        std::vector<std::pair<int, int>> arguments;
        int depth = 0;
        int start = b + 2;
        for (int k = b + 2; k < e - 1; ++k) {
          if (isOp(k, "(")) {
            ++depth;
          } else if (isOp(k, ")")) {
            --depth;
          } else if (depth == 0 && (isOp(k, ";") || isOp(k, ","))) {
            arguments.emplace_back(start, k);
            start = k + 1;
          }
        }
        arguments.emplace_back(start, e - 1);
        std::vector<Block> children;
        for (const auto& [from, to] : arguments) {
          if (from >= to) {
            return formula(b, e);
          }
          children.push_back(parse(from, to));
        }
        Block logic;
        logic.text = t[b].text;
        logic.source = span(b, e);
        if (function == QLatin1String("not")) {
          if (children.size() != 1) {
            return formula(b, e);
          }
          logic.kind = BlockKind::Not;
        } else if (function == QLatin1String("or") && isOneOf(children)) {
          logic.kind = BlockKind::OneOf;
          logic.text.clear();
          std::vector<Block> values{children.front().children.front()};
          for (const Block& compare : children) {
            values.push_back(compare.children.back());
          }
          children = std::move(values);
        } else {
          logic.kind = function == QLatin1String("and") ? BlockKind::And : BlockKind::Or;
        }
        logic.children = std::move(children);
        return logic;
      }
    }
    return formula(b, e);
  }

  // `or(x == a; x == b; …)`: the same name compared with a value each time.
  static bool isOneOf(const std::vector<Block>& children) {
    if (children.size() < 2) {
      return false;
    }
    const QString* name = nullptr;
    for (const Block& compare : children) {
      if (compare.kind != BlockKind::Compare ||
          (compare.text != QLatin1String("==") && compare.text != QLatin1String("="))) {
        return false;
      }
      const Block& left = compare.children.front();
      const Block& right = compare.children.back();
      if (left.kind != BlockKind::Name || (name != nullptr && left.text != *name)) {
        return false;
      }
      if (right.kind != BlockKind::Number && right.kind != BlockKind::Text && right.kind != BlockKind::Name &&
          right.kind != BlockKind::Column) {
        return false;
      }
      name = &left.text;
    }
    return true;
  }
};

bool hasTopLevelCompare(const QString& text) {
  const std::vector<Token> tokens = tokenize(text);
  int depth = 0;
  for (const Token& token : tokens) {
    if (token.type != Tok::Op) {
      continue;
    }
    if (token.text == QLatin1String("(")) {
      ++depth;
    } else if (token.text == QLatin1String(")")) {
      --depth;
    } else if (depth == 0 && isCompareText(token.text)) {
      return true;
    }
  }
  return false;
}

bool wrapped(const QString& text) {
  if (!text.startsWith(QLatin1Char('(')) || !text.endsWith(QLatin1Char(')'))) {
    return false;
  }
  // The first "(" closes at the end, not earlier: `(a) + (b)` is not wrapped.
  int depth = 0;
  for (int i = 0; i < text.size(); ++i) {
    if (text[i] == QLatin1Char('(')) {
      ++depth;
    } else if (text[i] == QLatin1Char(')') && --depth == 0) {
      return i == text.size() - 1;
    }
  }
  return false;
}

// An operand of a comparison: a comparison in it needs parentheses.
QString operandText(const Block& block) {
  const QString text = printExpression(block);
  const bool compares = block.kind == BlockKind::Compare ||
                        (block.kind == BlockKind::Formula && hasTopLevelCompare(text));
  return compares && !wrapped(text) ? QStringLiteral("(%1)").arg(text) : text;
}

QString quoted(const QString& text) {
  QString escaped = text;
  escaped.replace(QLatin1Char('\''), QStringLiteral("''"));
  return QStringLiteral("'%1'").arg(escaped);
}

QString joined(const std::vector<Block>& blocks, const QString& separator) {
  QStringList parts;
  for (const Block& block : blocks) {
    parts << printExpression(block);
  }
  return parts.join(separator);
}

const char* actionKey(BlockKind kind) {
  switch (kind) {
  case BlockKind::AllowOnly:
    return "values";
  case BlockKind::Exclude:
    return "exclude";
  case BlockKind::Min:
    return "min";
  case BlockKind::Max:
    return "max";
  case BlockKind::Step:
    return "step";
  default:
    return nullptr;
  }
}

bool isOptionAction(BlockKind kind) {
  return actionKey(kind) != nullptr;
}

bool onlyKeys(const QJsonObject& object, const QSet<QString>& keys) {
  for (auto it = object.begin(); it != object.end(); ++it) {
    if (!keys.contains(it.key())) {
      return false;
    }
  }
  return true;
}

bool allStrings(const QJsonValue& value) {
  if (!value.isArray()) {
    return false;
  }
  const QJsonArray array = value.toArray();
  for (const QJsonValue& item : array) {
    if (!item.isString()) {
      return false;
    }
  }
  return true;
}

Block otherAction(const QJsonValue& value) {
  Block other;
  other.kind = BlockKind::OtherAction;
  other.data = value.isObject() ? value.toObject() : QJsonObject{{QStringLiteral("__raw"), value}};
  return other;
}

std::vector<Block> actionsFromJson(const QJsonArray& array, int& group) {
  std::vector<Block> actions;
  for (const QJsonValue& value : array) {
    if (!value.isObject()) {
      actions.push_back(otherAction(value));
      continue;
    }
    const QJsonObject object = value.toObject();
    const int id = group++;
    if (object.contains(QStringLiteral("require"))) {
      const QJsonValue message = object.value(QStringLiteral("message"));
      if (onlyKeys(object, {QStringLiteral("require"), QStringLiteral("message")}) &&
          object.value(QStringLiteral("require")).isString() && (message.isUndefined() || message.isString())) {
        Block require;
        require.kind = BlockKind::Require;
        require.children.push_back(parseExpression(object.value(QStringLiteral("require")).toString()));
        require.message = message.toString();
        if (!message.isUndefined()) {
          require.data.insert(QStringLiteral("message"), true);
        }
        require.group = id;
        actions.push_back(require);
        continue;
      }
    } else if (object.contains(QStringLiteral("forbid"))) {
      if (onlyKeys(object, {QStringLiteral("forbid")}) && object.value(QStringLiteral("forbid")).isString()) {
        Block forbid;
        forbid.kind = BlockKind::Forbid;
        forbid.message = object.value(QStringLiteral("forbid")).toString();
        forbid.group = id;
        actions.push_back(forbid);
        continue;
      }
    } else if (object.contains(QStringLiteral("in_table"))) {
      const QJsonValue columns = object.value(QStringLiteral("columns"));
      if (onlyKeys(object, {QStringLiteral("in_table"), QStringLiteral("columns")}) &&
          object.value(QStringLiteral("in_table")).isString() && (columns.isUndefined() || columns.isObject())) {
        Block table;
        table.kind = BlockKind::InTable;
        table.text = object.value(QStringLiteral("in_table")).toString();
        table.data = columns.toObject();
        table.group = id;
        actions.push_back(table);
        continue;
      }
    } else if (object.value(QStringLiteral("option")).isString() &&
               onlyKeys(object, {QStringLiteral("option"), QStringLiteral("values"), QStringLiteral("exclude"),
                                 QStringLiteral("min"), QStringLiteral("max"), QStringLiteral("step")})) {
      const QString option = object.value(QStringLiteral("option")).toString();
      std::vector<Block> parts;
      bool readable = true;
      for (const BlockKind kind :
           {BlockKind::AllowOnly, BlockKind::Exclude, BlockKind::Min, BlockKind::Max, BlockKind::Step}) {
        const QString key = QLatin1String(actionKey(kind));
        if (!object.contains(key)) {
          continue;
        }
        Block part;
        part.kind = kind;
        part.text = option;
        part.group = id;
        const QJsonValue item = object.value(key);
        if (kind == BlockKind::AllowOnly || kind == BlockKind::Exclude) {
          if (!allStrings(item)) {
            readable = false;
            break;
          }
          const QJsonArray values = item.toArray();
          for (const QJsonValue& entry : values) {
            part.children.push_back(parseExpression(entry.toString()));
          }
        } else {
          if (!item.isString()) {
            readable = false;
            break;
          }
          part.children.push_back(parseExpression(item.toString()));
        }
        parts.push_back(std::move(part));
      }
      if (readable && !parts.empty()) {
        for (Block& part : parts) {
          actions.push_back(std::move(part));
        }
        continue;
      }
    }
    Block other = otherAction(object);
    other.group = id;
    actions.push_back(other);
  }
  return actions;
}

QJsonValue singleActionJson(const Block& action) {
  switch (action.kind) {
  case BlockKind::Require: {
    QJsonObject object{{QStringLiteral("require"), printExpression(action.children.front())}};
    if (!action.message.isEmpty() || action.data.contains(QStringLiteral("message"))) {
      object.insert(QStringLiteral("message"), action.message);
    }
    return object;
  }
  case BlockKind::Forbid:
    return QJsonObject{{QStringLiteral("forbid"), action.message}};
  case BlockKind::InTable: {
    QJsonObject object{{QStringLiteral("in_table"), action.text}};
    if (!action.data.isEmpty()) {
      object.insert(QStringLiteral("columns"), action.data);
    }
    return object;
  }
  case BlockKind::OtherAction:
    if (action.data.size() == 1 && action.data.contains(QStringLiteral("__raw"))) {
      return action.data.value(QStringLiteral("__raw"));
    }
    return action.data;
  default:
    return QJsonObject{};
  }
}

QJsonArray actionsToJson(const std::vector<Block>& actions) {
  QJsonArray out;
  QJsonObject pending;
  int pendingGroup = -1;
  bool started = false;
  const auto flush = [&out, &pending, &started] {
    if (started) {
      out.append(pending);
    }
    pending = QJsonObject();
    started = false;
  };
  for (const Block& action : actions) {
    if (!isOptionAction(action.kind)) {
      flush();
      out.append(singleActionJson(action));
      continue;
    }
    const QString key = QLatin1String(actionKey(action.kind));
    QJsonValue value;
    if (action.kind == BlockKind::AllowOnly || action.kind == BlockKind::Exclude) {
      QJsonArray values;
      for (const Block& child : action.children) {
        if (child.kind != BlockKind::Empty) {
          values.append(printExpression(child));
        }
      }
      value = values;
    } else {
      value = action.children.empty() ? QString() : printExpression(action.children.front());
    }
    if (started && action.group >= 0 && action.group == pendingGroup &&
        pending.value(QStringLiteral("option")).toString() == action.text && !pending.contains(key)) {
      pending.insert(key, value);
      continue;
    }
    flush();
    pending = QJsonObject{{QStringLiteral("option"), action.text}, {key, value}};
    pendingGroup = action.group;
    started = true;
  }
  flush();
  return out;
}

QString sectionName(int section) {
  switch (section) {
  case 0:
    return QStringLiteral("when");
  case 1:
    return QStringLiteral("then");
  default:
    return QStringLiteral("else");
  }
}

std::vector<Block>* sectionList(Rule& rule, int section) {
  if (section == 1) {
    return &rule.then;
  }
  if (section == 2 && rule.hasElse) {
    return &rule.otherwise;
  }
  return nullptr;
}

BlockPath parentOf(const BlockPath& path) {
  BlockPath parent = path;
  if (!parent.indices.empty()) {
    parent.indices.pop_back();
  }
  return parent;
}

bool isPrefix(const std::vector<int>& prefix, const std::vector<int>& path) {
  if (prefix.size() > path.size()) {
    return false;
  }
  for (std::size_t i = 0; i < prefix.size(); ++i) {
    if (prefix[i] != path[i]) {
      return false;
    }
  }
  return true;
}

} // namespace

bool isAction(BlockKind kind) {
  switch (kind) {
  case BlockKind::AllowOnly:
  case BlockKind::Exclude:
  case BlockKind::Min:
  case BlockKind::Max:
  case BlockKind::Step:
  case BlockKind::Require:
  case BlockKind::Forbid:
  case BlockKind::InTable:
  case BlockKind::OtherAction:
    return true;
  default:
    return false;
  }
}

bool isValue(BlockKind kind) {
  return kind != BlockKind::Empty && !isAction(kind);
}

QString groupOf(BlockKind kind) {
  switch (kind) {
  case BlockKind::Compare:
  case BlockKind::And:
  case BlockKind::Or:
  case BlockKind::Not:
  case BlockKind::OneOf:
    return QStringLiteral("Logic");
  default:
    return isAction(kind) ? QStringLiteral("Actions") : QStringLiteral("Values");
  }
}

QString kindLabel(BlockKind kind) {
  switch (kind) {
  case BlockKind::Empty:
    return QStringLiteral("empty");
  case BlockKind::Name:
    return QStringLiteral("name");
  case BlockKind::Number:
    return QStringLiteral("number");
  case BlockKind::Text:
    return QStringLiteral("text");
  case BlockKind::Column:
    return QStringLiteral("table column");
  case BlockKind::Formula:
    return QStringLiteral("formula");
  case BlockKind::Compare:
    return QStringLiteral("compare");
  case BlockKind::And:
    return QStringLiteral("and");
  case BlockKind::Or:
    return QStringLiteral("or");
  case BlockKind::Not:
    return QStringLiteral("not");
  case BlockKind::OneOf:
    return QStringLiteral("is one of");
  case BlockKind::AllowOnly:
    return QStringLiteral("allow only");
  case BlockKind::Exclude:
    return QStringLiteral("exclude");
  case BlockKind::Min:
    return QStringLiteral("min");
  case BlockKind::Max:
    return QStringLiteral("max");
  case BlockKind::Step:
    return QStringLiteral("step");
  case BlockKind::Require:
    return QStringLiteral("require");
  case BlockKind::Forbid:
    return QStringLiteral("forbid");
  case BlockKind::InTable:
    return QStringLiteral("combination in table");
  case BlockKind::OtherAction:
    return QStringLiteral("action");
  }
  return {};
}

Block parseExpression(const QString& text) {
  const Parser parser{text, tokenize(text)};
  if (parser.t.empty()) {
    return {};
  }
  Block block = parser.parse(0, static_cast<int>(parser.t.size()));
  // The whole text, with any spaces around it, is the source of the top.
  block.source = text;
  return block;
}

QString printExpression(const Block& block) {
  if (!block.source.isEmpty()) {
    return block.source;
  }
  switch (block.kind) {
  case BlockKind::Name:
  case BlockKind::Number:
  case BlockKind::Column:
  case BlockKind::Formula:
    return block.text;
  case BlockKind::Text:
    return quoted(block.text);
  case BlockKind::Compare:
    if (block.children.size() != 2) {
      return {};
    }
    return QStringLiteral("%1 %2 %3")
        .arg(operandText(block.children.front()), block.text, operandText(block.children.back()));
  case BlockKind::And:
  case BlockKind::Or:
  case BlockKind::Not: {
    QString name = block.text.toLower();
    if (name.isEmpty()) {
      name = block.kind == BlockKind::And ? QStringLiteral("and")
             : block.kind == BlockKind::Or ? QStringLiteral("or")
                                            : QStringLiteral("not");
    }
    return QStringLiteral("%1(%2)").arg(name, joined(block.children, QStringLiteral("; ")));
  }
  case BlockKind::OneOf: {
    if (block.children.empty()) {
      return {};
    }
    const QString subject = printExpression(block.children.front());
    QStringList parts;
    for (std::size_t i = 1; i < block.children.size(); ++i) {
      parts << QStringLiteral("%1 == %2").arg(subject, operandText(block.children[i]));
    }
    return QStringLiteral("or(%1)").arg(parts.join(QStringLiteral("; ")));
  }
  default:
    return {};
  }
}

bool isComplete(const Block& block) {
  const auto all = [&block] {
    for (const Block& child : block.children) {
      if (!isComplete(child)) {
        return false;
      }
    }
    return true;
  };
  switch (block.kind) {
  case BlockKind::Empty:
    return false;
  case BlockKind::Name:
  case BlockKind::Number:
  case BlockKind::Column:
  case BlockKind::Formula:
    return !block.text.trimmed().isEmpty();
  case BlockKind::Text:
  case BlockKind::Forbid:
  case BlockKind::OtherAction:
    return true;
  case BlockKind::Compare:
    return block.children.size() == 2 && all();
  case BlockKind::And:
  case BlockKind::Or:
    return !block.children.empty() && all();
  case BlockKind::Not:
  case BlockKind::Require:
    return block.children.size() == 1 && all();
  case BlockKind::OneOf:
    return block.children.size() >= 2 && all();
  case BlockKind::AllowOnly:
  case BlockKind::Exclude:
    return !block.text.isEmpty() && !block.children.empty() && all();
  case BlockKind::Min:
  case BlockKind::Max:
  case BlockKind::Step:
    return !block.text.isEmpty() && block.children.size() == 1 && all();
  case BlockKind::InTable:
    return !block.text.isEmpty();
  }
  return false;
}

bool isUnit(const QString& name) {
  static const QStringList units{
      QStringLiteral("um"),   QStringLiteral("micron"), QStringLiteral("µm"), QStringLiteral("mm"),
      QStringLiteral("cm"),   QStringLiteral("m"),      QStringLiteral("km"),      QStringLiteral("in"),
      QStringLiteral("inch"), QStringLiteral("ft"),     QStringLiteral("foot"),    QStringLiteral("yd"),
      QStringLiteral("mi"),   QStringLiteral("mil"),    QStringLiteral("rad"),     QStringLiteral("deg"),
      QStringLiteral("°"), QStringLiteral("grad")};
  return units.contains(name);
}

QStringList compareOperators() {
  return {QStringLiteral("=="), QStringLiteral("<>"), QStringLiteral("<"),
          QStringLiteral("<="), QStringLiteral(">"),  QStringLiteral(">=")};
}

QString operatorLabel(const QString& op) {
  if (op == QLatin1String("==") || op == QLatin1String("=")) {
    return QStringLiteral("=");
  }
  if (op == QLatin1String("<>") || op == QLatin1String("!=")) {
    return QStringLiteral("≠");
  }
  if (op == QLatin1String("<=")) {
    return QStringLiteral("≤");
  }
  if (op == QLatin1String(">=")) {
    return QStringLiteral("≥");
  }
  return op;
}

Rule ruleFromJson(const QJsonObject& json) {
  Rule rule;
  rule.name = json.value(QStringLiteral("name")).toString();
  rule.enabledWritten = json.contains(QStringLiteral("enabled"));
  rule.enabled = json.value(QStringLiteral("enabled")).toBool(true);
  int group = 0;
  if (json.contains(QStringLiteral("when"))) {
    rule.hasWhen = true;
    rule.when = parseExpression(json.value(QStringLiteral("when")).toString());
  }
  rule.then = actionsFromJson(json.value(QStringLiteral("then")).toArray(), group);
  if (json.contains(QStringLiteral("else"))) {
    rule.hasElse = true;
    rule.otherwise = actionsFromJson(json.value(QStringLiteral("else")).toArray(), group);
  }
  rule.extra = json;
  for (const char* key : {"name", "enabled", "when", "then", "else"}) {
    rule.extra.remove(QLatin1String(key));
  }
  return rule;
}

QJsonObject ruleToJson(const Rule& rule, bool* complete) {
  QJsonObject json = rule.extra;
  json.insert(QStringLiteral("name"), rule.name);
  if (rule.enabledWritten || !rule.enabled) {
    json.insert(QStringLiteral("enabled"), rule.enabled);
  }
  if (rule.hasWhen && rule.when.kind != BlockKind::Empty) {
    json.insert(QStringLiteral("when"), printExpression(rule.when));
  }
  json.insert(QStringLiteral("then"), actionsToJson(rule.then));
  if (rule.hasElse) {
    json.insert(QStringLiteral("else"), actionsToJson(rule.otherwise));
  }
  if (complete != nullptr) {
    *complete = isComplete(rule);
  }
  return json;
}

bool isComplete(const Rule& rule) {
  // An empty else is not complete either: the model does not keep one.
  if (rule.name.trimmed().isEmpty() || (rule.hasWhen && !isComplete(rule.when)) ||
      (rule.hasElse && rule.otherwise.empty())) {
    return false;
  }
  for (const auto* list : {&rule.then, &rule.otherwise}) {
    for (const Block& action : *list) {
      if (!isComplete(action)) {
        return false;
      }
    }
  }
  return true;
}

QString actionText(const Block& action) {
  const auto value = [](const Block& block) {
    const QString text = printExpression(block);
    return text.trimmed().isEmpty() ? QStringLiteral("…") : text.trimmed();
  };
  const auto values = [&action, &value] {
    QStringList parts;
    for (const Block& child : action.children) {
      parts << value(child);
    }
    return parts.isEmpty() ? QStringLiteral("…") : parts.join(QStringLiteral("; "));
  };
  const QString option = action.text.isEmpty() ? QStringLiteral("…") : action.text;
  switch (action.kind) {
  case BlockKind::AllowOnly:
    return QStringLiteral("%1 only %2").arg(option, values());
  case BlockKind::Exclude:
    return QStringLiteral("%1 not %2").arg(option, values());
  case BlockKind::Min:
  case BlockKind::Max:
  case BlockKind::Step:
    return QStringLiteral("%1 %2 %3")
        .arg(option, kindLabel(action.kind), action.children.empty() ? QStringLiteral("…") : value(action.children.front()));
  case BlockKind::Require: {
    const QString condition = action.children.empty() ? QStringLiteral("…") : value(action.children.front());
    return action.message.isEmpty() ? QStringLiteral("require %1").arg(condition)
                                    : QStringLiteral("require %1: %2").arg(condition, action.message);
  }
  case BlockKind::Forbid:
    return QStringLiteral("forbid: %1").arg(action.message);
  case BlockKind::InTable: {
    QStringList columns;
    for (auto it = action.data.begin(); it != action.data.end(); ++it) {
      columns << QStringLiteral("%1 = %2").arg(it.key(), it.value().toString());
    }
    return columns.isEmpty() ? QStringLiteral("combination in table %1").arg(option)
                             : QStringLiteral("combination in table %1 (%2)")
                                   .arg(option, columns.join(QStringLiteral(", ")));
  }
  case BlockKind::OtherAction:
    return QString::fromUtf8(QJsonDocument(action.data).toJson(QJsonDocument::Compact));
  default:
    return value(action);
  }
}

QJsonObject inTableColumns(const QStringList& tableColumns, const QStringList& options, const QJsonObject& current) {
  QJsonObject columns;
  for (auto it = current.begin(); it != current.end(); ++it) {
    if (options.contains(it.key()) && tableColumns.contains(it.value().toString())) {
      columns.insert(it.key(), it.value());
    }
  }
  for (const QString& option : options) {
    if (columns.contains(option)) {
      continue;
    }
    if (tableColumns.contains(option)) {
      columns.insert(option, option);
      continue;
    }
    for (const QString& column : tableColumns) {
      if (column.compare(option, Qt::CaseInsensitive) == 0) {
        columns.insert(option, column);
        break;
      }
    }
  }
  return columns;
}

QStringList ruleText(const Rule& rule) {
  QStringList lines;
  if (rule.hasWhen) {
    const QString condition = printExpression(rule.when).trimmed();
    lines << QStringLiteral("when %1").arg(condition.isEmpty() ? QStringLiteral("…") : condition);
  } else {
    lines << QStringLiteral("always");
  }
  if (rule.then.empty()) {
    lines << QStringLiteral("then nothing");
  }
  for (const Block& action : rule.then) {
    lines << QStringLiteral("then %1").arg(actionText(action));
  }
  if (rule.hasElse && rule.otherwise.empty()) {
    lines << QStringLiteral("else nothing");
  }
  for (const Block& action : rule.otherwise) {
    lines << QStringLiteral("else %1").arg(actionText(action));
  }
  return lines;
}

QString BlockPath::id() const {
  QString id = sectionName(section);
  for (const int index : indices) {
    id += QStringLiteral(".%1").arg(index);
  }
  return id;
}

Block* blockAt(Rule& rule, const BlockPath& path) {
  Block* block = nullptr;
  std::size_t next = 0;
  if (path.section == 0) {
    if (!rule.hasWhen) {
      return nullptr;
    }
    block = &rule.when;
  } else {
    std::vector<Block>* list = sectionList(rule, path.section);
    if (list == nullptr || path.indices.empty() || path.indices.front() < 0 ||
        path.indices.front() >= static_cast<int>(list->size())) {
      return nullptr;
    }
    block = &(*list)[static_cast<std::size_t>(path.indices.front())];
    next = 1;
  }
  for (; next < path.indices.size(); ++next) {
    const int index = path.indices[next];
    if (index < 0 || index >= static_cast<int>(block->children.size())) {
      return nullptr;
    }
    block = &block->children[static_cast<std::size_t>(index)];
  }
  return block;
}

const Block* blockAt(const Rule& rule, const BlockPath& path) {
  return blockAt(const_cast<Rule&>(rule), path);
}

std::vector<Block>* listOf(Rule& rule, const BlockPath& path) {
  if (path.indices.empty()) {
    return nullptr;
  }
  if (path.section != 0 && path.indices.size() == 1) {
    return sectionList(rule, path.section);
  }
  Block* parent = blockAt(rule, parentOf(path));
  return parent != nullptr ? &parent->children : nullptr;
}

bool growsAtEnd(BlockKind kind) {
  switch (kind) {
  case BlockKind::And:
  case BlockKind::Or:
  case BlockKind::OneOf:
  case BlockKind::AllowOnly:
  case BlockKind::Exclude:
    return true;
  default:
    return false;
  }
}

bool removable(BlockKind parent, int index) {
  return growsAtEnd(parent) && !(parent == BlockKind::OneOf && index == 0);
}

bool fits(const Rule& rule, const Target& target, BlockKind kind) {
  const BlockPath& path = target.path;
  if (path.section == 0 && path.indices.empty()) {
    return !target.insert && rule.hasWhen && isValue(kind);
  }
  if (path.indices.empty()) {
    return false;
  }
  const int index = path.indices.back();
  if (path.section != 0 && path.indices.size() == 1) {
    const std::vector<Block>* list = sectionList(const_cast<Rule&>(rule), path.section);
    return list != nullptr && target.insert && isAction(kind) && index >= 0 &&
           index <= static_cast<int>(list->size());
  }
  const Block* parent = blockAt(rule, parentOf(path));
  if (parent == nullptr || !isValue(kind)) {
    return false;
  }
  const int size = static_cast<int>(parent->children.size());
  if (target.insert) {
    const int first = parent->kind == BlockKind::OneOf ? 1 : 0;
    return growsAtEnd(parent->kind) && index >= first && index <= size;
  }
  return index >= 0 && index < size;
}

void touch(Rule& rule, const BlockPath& path) {
  BlockPath prefix = path;
  for (;;) {
    if (Block* block = blockAt(rule, prefix)) {
      block->source.clear();
    }
    if (prefix.indices.empty() || (prefix.section != 0 && prefix.indices.size() == 1)) {
      return;
    }
    prefix.indices.pop_back();
  }
}

bool place(Rule& rule, const Target& target, Block block) {
  if (!fits(rule, target, block.kind)) {
    return false;
  }
  const BlockPath& path = target.path;
  if (path.section == 0 && path.indices.empty()) {
    rule.when = std::move(block);
    return true;
  }
  std::vector<Block>* list = listOf(rule, path);
  if (list == nullptr) {
    return false;
  }
  const auto index = static_cast<std::size_t>(path.indices.back());
  if (target.insert) {
    list->insert(list->begin() + static_cast<std::ptrdiff_t>(index), std::move(block));
  } else {
    (*list)[index] = std::move(block);
  }
  if (path.indices.size() > 1 || path.section == 0) {
    touch(rule, parentOf(path));
  }
  return true;
}

Block take(Rule& rule, const BlockPath& path) {
  if (path.section == 0 && path.indices.empty()) {
    Block taken = std::move(rule.when);
    rule.when = Block();
    return taken;
  }
  std::vector<Block>* list = listOf(rule, path);
  if (list == nullptr || path.indices.back() < 0 || path.indices.back() >= static_cast<int>(list->size())) {
    return {};
  }
  const auto index = static_cast<std::size_t>(path.indices.back());
  Block taken = std::move((*list)[index]);
  const bool inSection = path.section != 0 && path.indices.size() == 1;
  const Block* parent = inSection ? nullptr : blockAt(rule, parentOf(path));
  if (inSection || (parent != nullptr && removable(parent->kind, path.indices.back()))) {
    list->erase(list->begin() + static_cast<std::ptrdiff_t>(index));
  } else {
    (*list)[index] = Block();
  }
  if (!inSection) {
    touch(rule, parentOf(path));
  }
  return taken;
}

bool move(Rule& rule, const BlockPath& from, const Target& to) {
  const Block* block = blockAt(rule, from);
  if (block == nullptr || block->kind == BlockKind::Empty) {
    return false;
  }
  if (to.path.section == from.section && isPrefix(from.indices, to.path.indices)) {
    return false; // into itself
  }
  if (!fits(rule, to, block->kind)) {
    return false;
  }
  // Whether taking it out removes it from a list (the places after it move
  // up by one) or leaves its socket empty.
  bool erases = false;
  if (!from.indices.empty()) {
    if (from.section != 0 && from.indices.size() == 1) {
      erases = true;
    } else if (const Block* parent = blockAt(rule, parentOf(from))) {
      erases = removable(parent->kind, from.indices.back());
    }
  }
  Rule changed = rule;
  Block moved = take(changed, from);
  Target adjusted = to;
  if (erases && to.path.section == from.section && from.indices.size() <= to.path.indices.size()) {
    const std::size_t level = from.indices.size() - 1;
    bool samePrefix = true;
    for (std::size_t i = 0; i < level; ++i) {
      samePrefix = samePrefix && from.indices[i] == to.path.indices[i];
    }
    if (samePrefix && to.path.indices[level] > from.indices[level]) {
      --adjusted.path.indices[level];
    }
  }
  if (!place(changed, adjusted, std::move(moved))) {
    return false;
  }
  rule = std::move(changed);
  return true;
}

Block newBlock(const QString& paletteId, const QString& option) {
  Block block;
  const auto empties = [&block](int count) { block.children.assign(static_cast<std::size_t>(count), Block()); };
  if (paletteId == QLatin1String("compare")) {
    block.kind = BlockKind::Compare;
    block.text = QStringLiteral("==");
    empties(2);
  } else if (paletteId == QLatin1String("and") || paletteId == QLatin1String("or")) {
    block.kind = paletteId == QLatin1String("and") ? BlockKind::And : BlockKind::Or;
    block.text = paletteId;
    empties(2);
  } else if (paletteId == QLatin1String("not")) {
    block.kind = BlockKind::Not;
    block.text = paletteId;
    empties(1);
  } else if (paletteId == QLatin1String("one_of")) {
    block.kind = BlockKind::OneOf;
    empties(2);
  } else if (paletteId == QLatin1String("option") || paletteId == QLatin1String("parameter")) {
    block.kind = BlockKind::Name;
    block.text = option;
  } else if (paletteId == QLatin1String("number")) {
    block.kind = BlockKind::Number;
  } else if (paletteId == QLatin1String("text")) {
    block.kind = BlockKind::Text;
  } else if (paletteId == QLatin1String("column")) {
    block.kind = BlockKind::Column;
  } else if (paletteId == QLatin1String("formula")) {
    block.kind = BlockKind::Formula;
  } else if (paletteId == QLatin1String("allow_only") || paletteId == QLatin1String("exclude")) {
    block.kind = paletteId == QLatin1String("exclude") ? BlockKind::Exclude : BlockKind::AllowOnly;
    block.text = option;
    empties(1);
  } else if (paletteId == QLatin1String("min") || paletteId == QLatin1String("max") ||
             paletteId == QLatin1String("step")) {
    block.kind = paletteId == QLatin1String("min")   ? BlockKind::Min
                 : paletteId == QLatin1String("max") ? BlockKind::Max
                                                      : BlockKind::Step;
    block.text = option;
    empties(1);
  } else if (paletteId == QLatin1String("require")) {
    block.kind = BlockKind::Require;
    empties(1);
  } else if (paletteId == QLatin1String("forbid")) {
    block.kind = BlockKind::Forbid;
  } else if (paletteId == QLatin1String("in_table")) {
    block.kind = BlockKind::InTable;
  }
  return block;
}

std::vector<PaletteEntry> paletteEntries() {
  const QString rule = QStringLiteral("Rule");
  const QString logic = QStringLiteral("Logic");
  const QString values = QStringLiteral("Values");
  const QString actions = QStringLiteral("Actions");
  return {
      {QStringLiteral("when"), QStringLiteral("when … then"), rule},
      {QStringLiteral("always"), QStringLiteral("then (always)"), rule},
      {QStringLiteral("else"), QStringLiteral("else"), rule},
      {QStringLiteral("and"), QStringLiteral("and"), logic},
      {QStringLiteral("or"), QStringLiteral("or"), logic},
      {QStringLiteral("not"), QStringLiteral("not"), logic},
      {QStringLiteral("compare"), QStringLiteral("compare"), logic},
      {QStringLiteral("one_of"), QStringLiteral("is one of"), logic},
      {QStringLiteral("option"), QStringLiteral("option"), values},
      {QStringLiteral("parameter"), QStringLiteral("parameter"), values},
      {QStringLiteral("number"), QStringLiteral("number with unit"), values},
      {QStringLiteral("text"), QStringLiteral("text"), values},
      {QStringLiteral("column"), QStringLiteral("table column"), values},
      {QStringLiteral("formula"), QStringLiteral("formula"), values},
      {QStringLiteral("allow_only"), QStringLiteral("allow only"), actions},
      {QStringLiteral("exclude"), QStringLiteral("exclude"), actions},
      {QStringLiteral("min"), QStringLiteral("min"), actions},
      {QStringLiteral("max"), QStringLiteral("max"), actions},
      {QStringLiteral("step"), QStringLiteral("step"), actions},
      {QStringLiteral("require"), QStringLiteral("require"), actions},
      {QStringLiteral("forbid"), QStringLiteral("forbid"), actions},
      {QStringLiteral("in_table"), QStringLiteral("combination in table"), actions},
  };
}

bool isRuleEntry(const QString& id) {
  return id == QLatin1String("when") || id == QLatin1String("always") || id == QLatin1String("else");
}

bool applyRuleEntry(Rule& rule, const QString& id) {
  if (id == QLatin1String("when")) {
    if (rule.hasWhen) {
      return false;
    }
    rule.hasWhen = true;
    rule.when = Block();
    return true;
  }
  if (id == QLatin1String("always")) {
    if (!rule.hasWhen) {
      return false;
    }
    rule.hasWhen = false;
    rule.when = Block();
    return true;
  }
  if (id == QLatin1String("else")) {
    if (rule.hasElse) {
      return false;
    }
    rule.hasElse = true;
    return true;
  }
  return false;
}

} // namespace mitcad::rules
