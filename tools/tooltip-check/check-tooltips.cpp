// SPDX-License-Identifier: MIT
//
// Keeps the app's tooltips short (docs/development.md, code conventions):
// a command's tooltip (CommandDef::tooltip, the ribbon and the menus) has at
// most 80 characters, any other control's at most 100. The detail belongs in
// the user guide.
//
//   mitcad-check-tooltips DIR...   scans the .cpp, .hpp and .mm files under
//                                  each DIR; prints every tooltip over its
//                                  limit as file:line and fails if any
//   mitcad-check-tooltips --self-test
//                                  checks the scan on planted sources
//
// The scan reads the source text, not the compiled program. It tokenises
// each file (comments and preprocessor lines left out, adjacent string
// literals joined, escapes counted as one character, UTF-8 counted in
// characters) and takes the string literals of:
// - calls of withTooltip (its argument), setToolTip, setTabToolTip and
//   QToolTip::showText (the text), withButton (the button's tip), and
//   setData/setItemData with Qt::ToolTipRole;
// - assignments and initialisations of a name that is a tooltip (tooltip,
//   toolTip, tip, or ending in Tip, Tooltip or ToolTip): `def.tooltip = ...`,
//   `item.toolTip = @"..."`, `const QString tip = ...`, and the lines
//   streamed into one (`tip << tr("...")`);
// - calls of a helper of the same file (or its .hpp/.cpp/.mm sibling) that
//   takes a string parameter named like a tooltip: the argument in that
//   place (`def("id", tr("Name"), "icon", tr("Tooltip"), ...)`);
// - return statements of a function named like a tooltip.
// Each run of adjacent literals in such an expression is one text, so the
// branches of `cond ? tr("a") : tr("b")` count separately. A command's
// limit applies to `x.tooltip` of a variable declared as a CommandDef in
// the file, and to a helper whose definition makes a CommandDef; every other
// tooltip has the input limit. Texts built at run time are not seen.

#include <algorithm>
#include <cctype>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <set>
#include <sstream>
#include <string>
#include <vector>

namespace {

constexpr int kCommandLimit = 80;
constexpr int kInputLimit = 100;

enum class Kind { Ident, String, Number, Punct };

struct Token {
  Kind kind;
  std::string text; // a string literal's decoded content
  int line;
};

bool identStart(char c) {
  return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || c == '_';
}
bool identChar(char c) {
  return identStart(c) || (c >= '0' && c <= '9');
}
bool digit(char c) {
  return c >= '0' && c <= '9';
}

// The characters of a UTF-8 text (continuation bytes not counted).
int characters(const std::string& text) {
  int count = 0;
  for (unsigned char c : text) {
    if ((c & 0xC0) != 0x80) {
      ++count;
    }
  }
  return count;
}

class Lexer {
public:
  explicit Lexer(const std::string& source) : m_s(source) {}

  std::vector<Token> run() {
    std::vector<Token> tokens;
    bool lineStart = true;
    while (m_i < m_s.size()) {
      const char c = m_s[m_i];
      if (c == '\n') {
        ++m_line;
        ++m_i;
        lineStart = true;
        continue;
      }
      if (c == ' ' || c == '\t' || c == '\r' || c == '\f' || c == '\v') {
        ++m_i;
        continue;
      }
      if (c == '/' && peek(1) == '/') {
        while (m_i < m_s.size() && m_s[m_i] != '\n') {
          ++m_i;
        }
        continue;
      }
      if (c == '/' && peek(1) == '*') {
        m_i += 2;
        while (m_i < m_s.size() && !(m_s[m_i] == '*' && peek(1) == '/')) {
          if (m_s[m_i] == '\n') {
            ++m_line;
          }
          ++m_i;
        }
        m_i += 2;
        continue;
      }
      if (c == '#' && lineStart) {
        skipDirective();
        continue;
      }
      lineStart = false;
      if (c == '@' && peek(1) == '"') { // an Objective-C string
        ++m_i;
        continue;
      }
      if (identStart(c)) {
        const std::size_t start = m_i;
        while (m_i < m_s.size() && identChar(m_s[m_i])) {
          ++m_i;
        }
        const std::string word = m_s.substr(start, m_i - start);
        if (m_i < m_s.size() && m_s[m_i] == '"') {
          if (word == "R" || word == "u8R" || word == "uR" || word == "UR" || word == "LR") {
            tokens.push_back(rawString());
            continue;
          }
          if (word == "u8" || word == "u" || word == "U" || word == "L") {
            tokens.push_back(string());
            continue;
          }
        }
        if (m_i < m_s.size() && m_s[m_i] == '\'' &&
            (word == "u8" || word == "u" || word == "U" || word == "L")) {
          tokens.push_back(character());
          continue;
        }
        tokens.push_back({Kind::Ident, word, m_line});
        continue;
      }
      if (digit(c) || (c == '.' && digit(peek(1)))) {
        const std::size_t start = m_i;
        while (m_i < m_s.size()) {
          const char d = m_s[m_i];
          if ((d == '+' || d == '-') && m_i > start &&
              (m_s[m_i - 1] == 'e' || m_s[m_i - 1] == 'E' || m_s[m_i - 1] == 'p' || m_s[m_i - 1] == 'P')) {
            ++m_i;
          } else if (identChar(d) || d == '.' || (d == '\'' && m_i + 1 < m_s.size() && identChar(m_s[m_i + 1]))) {
            ++m_i;
          } else {
            break;
          }
        }
        tokens.push_back({Kind::Number, m_s.substr(start, m_i - start), m_line});
        continue;
      }
      if (c == '"') {
        tokens.push_back(string());
        continue;
      }
      if (c == '\'') {
        tokens.push_back(character());
        continue;
      }
      static const char* const pairs[] = {"::", "->", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=",
                                          "&=", "|=", "^=", "&&", "||", "<<", ">>", "++", "--"};
      std::string op(1, c);
      for (const char* pair : pairs) {
        if (c == pair[0] && peek(1) == pair[1]) {
          op = pair;
          break;
        }
      }
      m_i += op.size();
      tokens.push_back({Kind::Punct, op, m_line});
    }
    return tokens;
  }

private:
  char peek(std::size_t ahead) const {
    return m_i + ahead < m_s.size() ? m_s[m_i + ahead] : '\0';
  }

  void skipDirective() {
    while (m_i < m_s.size() && m_s[m_i] != '\n') {
      if (m_s[m_i] == '\\' && peek(1) == '\n') {
        ++m_line;
        m_i += 2;
        continue;
      }
      if (m_s[m_i] == '\\' && peek(1) == '\r' && peek(2) == '\n') {
        ++m_line;
        m_i += 3;
        continue;
      }
      ++m_i;
    }
  }

  // An escape sequence after the backslash, as one character.
  std::string escape() {
    const char e = m_s[m_i++];
    switch (e) {
    case 'n': return "\n";
    case 't': return "\t";
    case 'r': return "\r";
    case '0': case '1': case '2': case '3': case '4': case '5': case '6': case '7': {
      for (int k = 0; k < 2 && m_i < m_s.size() && m_s[m_i] >= '0' && m_s[m_i] <= '7'; ++k) {
        ++m_i;
      }
      return "?";
    }
    case 'x':
      while (m_i < m_s.size() && std::isxdigit(static_cast<unsigned char>(m_s[m_i]))) {
        ++m_i;
      }
      return "?";
    case 'u': m_i += 4; return "?";
    case 'U': m_i += 8; return "?";
    default: return std::string(1, e);
    }
  }

  Token string() {
    const int line = m_line;
    std::string text;
    ++m_i; // the opening quote
    while (m_i < m_s.size() && m_s[m_i] != '"' && m_s[m_i] != '\n') {
      if (m_s[m_i] == '\\' && m_i + 1 < m_s.size()) {
        ++m_i;
        text += escape();
      } else {
        text += m_s[m_i++];
      }
    }
    ++m_i; // the closing quote
    return {Kind::String, text, line};
  }

  Token rawString() {
    const int line = m_line;
    ++m_i; // the quote
    const std::size_t open = m_s.find('(', m_i);
    if (open == std::string::npos) {
      m_i = m_s.size();
      return {Kind::String, "", line};
    }
    const std::string close = ")" + m_s.substr(m_i, open - m_i) + "\"";
    const std::size_t end = m_s.find(close, open + 1);
    const std::size_t stop = end == std::string::npos ? m_s.size() : end;
    const std::string text = m_s.substr(open + 1, stop - open - 1);
    m_line += static_cast<int>(std::count(text.begin(), text.end(), '\n'));
    m_i = end == std::string::npos ? m_s.size() : end + close.size();
    return {Kind::String, text, line};
  }

  Token character() {
    const int line = m_line;
    ++m_i;
    while (m_i < m_s.size() && m_s[m_i] != '\'' && m_s[m_i] != '\n') {
      m_i += m_s[m_i] == '\\' ? 2 : 1;
    }
    ++m_i;
    return {Kind::Number, "'c'", line};
  }

  const std::string& m_s;
  std::size_t m_i = 0;
  int m_line = 1;
};

struct Violation {
  std::string file;
  int line;
  int length;
  int limit;
  std::string text;
};

struct Source {
  std::string path; // as printed
  std::vector<Token> tokens;
};

// A helper that takes a tooltip: the place of that parameter and its limit.
struct Helper {
  std::size_t index;
  int limit;
  bool lambda; // a local lambda: `x.name(` is something else
};

bool isTipName(const std::string& name) {
  const auto endsWith = [&name](const std::string& tail) {
    return name.size() > tail.size() && name.compare(name.size() - tail.size(), tail.size(), tail) == 0;
  };
  return name == "tooltip" || name == "toolTip" || name == "tip" || endsWith("Tip") || endsWith("Tooltip") ||
         endsWith("ToolTip");
}

bool opens(const Token& t) {
  return t.kind == Kind::Punct && (t.text == "(" || t.text == "[" || t.text == "{");
}
bool closes(const Token& t) {
  return t.kind == Kind::Punct && (t.text == ")" || t.text == "]" || t.text == "}");
}
bool punct(const Token& t, const char* text) {
  return t.kind == Kind::Punct && t.text == text;
}

// The index of the bracket that closes the one at `open` (or the end).
std::size_t matching(const std::vector<Token>& tokens, std::size_t open) {
  int depth = 0;
  for (std::size_t i = open; i < tokens.size(); ++i) {
    if (opens(tokens[i])) {
      ++depth;
    } else if (closes(tokens[i]) && --depth == 0) {
      return i;
    }
  }
  return tokens.size();
}

// The arguments of a call whose "(" is at `open`, as [begin, end) ranges.
std::vector<std::pair<std::size_t, std::size_t>> arguments(const std::vector<Token>& tokens, std::size_t open) {
  std::vector<std::pair<std::size_t, std::size_t>> args;
  const std::size_t close = matching(tokens, open);
  std::size_t begin = open + 1;
  int depth = 0;
  for (std::size_t i = open + 1; i < close; ++i) {
    if (opens(tokens[i])) {
      ++depth;
    } else if (closes(tokens[i])) {
      --depth;
    } else if (depth == 0 && punct(tokens[i], ",")) {
      args.emplace_back(begin, i);
      begin = i + 1;
    }
  }
  if (close > begin) {
    args.emplace_back(begin, close);
  }
  return args;
}

// The end of the expression that starts at `begin`: the ";" or "," at its
// own depth, or the bracket that closes around it.
std::size_t expressionEnd(const std::vector<Token>& tokens, std::size_t begin) {
  int depth = 0;
  for (std::size_t i = begin; i < tokens.size(); ++i) {
    if (opens(tokens[i])) {
      ++depth;
    } else if (closes(tokens[i])) {
      if (--depth < 0) {
        return i;
      }
    } else if (depth == 0 && (punct(tokens[i], ";") || punct(tokens[i], ","))) {
      return i;
    }
  }
  return tokens.size();
}

class Checker {
public:
  std::vector<Violation> violations;

  void check(const std::vector<Source>& sources) {
    // Siblings share their helpers and CommandDef names (Foo.hpp, Foo.cpp).
    std::map<std::string, std::vector<const Source*>> families;
    for (const Source& source : sources) {
      families[stem(source.path)].push_back(&source);
    }
    for (const auto& family : families) {
      std::map<std::string, Helper> helpers;
      std::set<std::string> commandDefs;
      std::set<std::pair<const Source*, std::size_t>> definitions;
      for (const Source* source : family.second) {
        collect(*source, helpers, commandDefs, definitions);
      }
      for (const Source* source : family.second) {
        scan(*source, helpers, commandDefs, definitions);
      }
    }
  }

private:
  static std::string stem(const std::string& path) {
    const std::size_t dot = path.rfind('.');
    return dot == std::string::npos ? path : path.substr(0, dot);
  }

  // The helpers with a tooltip parameter and the names declared CommandDef.
  static void collect(const Source& source, std::map<std::string, Helper>& helpers,
                      std::set<std::string>& commandDefs,
                      std::set<std::pair<const Source*, std::size_t>>& definitions) {
    const std::vector<Token>& t = source.tokens;
    for (std::size_t i = 0; i < t.size(); ++i) {
      if (t[i].kind == Kind::Ident && t[i].text == "CommandDef") {
        std::size_t j = i + 1;
        while (j < t.size() && (punct(t[j], "&") || punct(t[j], "*") || punct(t[j], "&&") ||
                                (t[j].kind == Kind::Ident && t[j].text == "const"))) {
          ++j;
        }
        if (j < t.size() && t[j].kind == Kind::Ident) {
          commandDefs.insert(t[j].text);
        }
      }
      if (!punct(t[i], "(") || i == 0) {
        continue;
      }
      // The name of a function (`name(`) or a lambda (`name = [...](`).
      std::string name;
      std::size_t nameAt = 0;
      bool lambda = false;
      if (t[i - 1].kind == Kind::Ident) {
        name = t[i - 1].text;
        nameAt = i - 1;
      } else if (punct(t[i - 1], "]")) {
        std::size_t depth = 0;
        std::size_t k = i - 1;
        for (;; --k) {
          if (punct(t[k], "]")) {
            ++depth;
          } else if (punct(t[k], "[") && --depth == 0) {
            break;
          }
          if (k == 0) {
            break;
          }
        }
        if (k >= 2 && punct(t[k - 1], "=") && t[k - 2].kind == Kind::Ident) {
          name = t[k - 2].text;
          nameAt = k - 2;
          lambda = true;
        }
      }
      if (name.empty()) {
        continue;
      }
      const auto params = arguments(t, i);
      for (std::size_t p = 0; p < params.size(); ++p) {
        // A parameter: a string type and a tooltip's name (before a default).
        bool stringType = false;
        std::string last;
        for (std::size_t k = params[p].first; k < params[p].second; ++k) {
          if (punct(t[k], "=")) {
            break;
          }
          if (t[k].kind == Kind::Ident) {
            if (t[k].text == "QString" || t[k].text == "char" || t[k].text == "NSString") {
              stringType = true;
            }
            last = t[k].text;
          }
        }
        if (!stringType || last == "QString" || !isTipName(last)) {
          continue;
        }
        // Its limit: a command's when its definition makes a CommandDef.
        int limit = kInputLimit;
        for (std::size_t k = nameAt; k > 0 && k + 8 > nameAt; --k) {
          if (t[k - 1].kind == Kind::Ident && t[k - 1].text == "CommandDef") {
            limit = kCommandLimit;
          }
          if (punct(t[k - 1], ";") || punct(t[k - 1], "}") || punct(t[k - 1], "{")) {
            break;
          }
        }
        std::size_t after = matching(t, i) + 1;
        while (after < t.size() && !punct(t[after], "{") && !punct(t[after], ";") && !punct(t[after], ",") &&
               !punct(t[after], ")")) {
          ++after;
        }
        if (after < t.size() && punct(t[after], "{")) {
          const std::size_t end = matching(t, after);
          for (std::size_t k = after; k < end; ++k) {
            if (t[k].kind == Kind::Ident && t[k].text == "CommandDef") {
              limit = kCommandLimit;
            }
          }
        }
        helpers[name] = {p, limit, lambda};
        definitions.insert({&source, nameAt});
      }
    }
  }

  void scan(const Source& source, const std::map<std::string, Helper>& helpers,
            const std::set<std::string>& commandDefs,
            const std::set<std::pair<const Source*, std::size_t>>& definitions) {
    const std::vector<Token>& t = source.tokens;
    std::set<std::size_t> seen; // literal runs already measured
    const auto measure = [&](std::size_t begin, std::size_t end, int limit) {
      for (std::size_t k = begin; k < end; ++k) {
        if (t[k].kind != Kind::String || seen.count(k)) {
          continue;
        }
        std::string text;
        const std::size_t first = k;
        while (k < end && t[k].kind == Kind::String) {
          seen.insert(k);
          text += t[k].text;
          ++k;
        }
        const int length = characters(text);
        if (length > limit) {
          violations.push_back({source.path, t[first].line, length, limit, text});
        }
      }
    };
    const auto argument = [&](std::size_t open, std::size_t index, int limit) {
      const auto args = arguments(t, open);
      if (index < args.size()) {
        measure(args[index].first, args[index].second, limit);
      }
    };
    for (std::size_t i = 0; i + 1 < t.size(); ++i) {
      if (t[i].kind != Kind::Ident) {
        continue;
      }
      const std::string& name = t[i].text;
      if (punct(t[i + 1], "(")) {
        if (name == "withTooltip" || name == "setToolTip") {
          argument(i + 1, 0, kInputLimit);
        } else if (name == "setTabToolTip" || name == "showText" || name == "withButton") {
          argument(i + 1, 1, kInputLimit);
        } else if (name == "setData" || name == "setItemData") {
          const std::size_t close = matching(t, i + 1);
          bool role = false;
          for (std::size_t k = i + 1; k < close; ++k) {
            role = role || (t[k].kind == Kind::Ident && t[k].text == "ToolTipRole");
          }
          if (role) {
            measure(i + 2, close, kInputLimit);
          }
        } else if (helpers.count(name) && !definitions.count({&source, i})) {
          const Helper& helper = helpers.at(name);
          const bool member = i > 0 && (punct(t[i - 1], ".") || punct(t[i - 1], "->"));
          if (!(helper.lambda && member)) {
            argument(i + 1, helper.index, helper.limit);
          }
        }
        if (isTipName(name)) {
          // A function named like a tooltip: what it returns.
          std::size_t after = matching(t, i + 1) + 1;
          while (after < t.size() && t[after].kind == Kind::Ident) {
            ++after; // const, override, noexcept
          }
          if (after < t.size() && punct(t[after], "{")) {
            const std::size_t end = matching(t, after);
            for (std::size_t k = after; k < end; ++k) {
              if (t[k].kind == Kind::Ident && t[k].text == "return") {
                measure(k + 1, expressionEnd(t, k + 1), kInputLimit);
              }
            }
          }
        }
        continue;
      }
      if (isTipName(name) && punct(t[i + 1], "<<")) { // a list of lines: each one
        std::size_t end = i + 2;
        while (end < t.size() && !punct(t[end], ";")) {
          ++end;
        }
        measure(i + 2, end, kInputLimit);
      }
      if (isTipName(name) && punct(t[i + 1], "=")) {
        int limit = kInputLimit;
        if (name == "tooltip" && i >= 2 && (punct(t[i - 1], ".") || punct(t[i - 1], "->")) &&
            t[i - 2].kind == Kind::Ident && commandDefs.count(t[i - 2].text)) {
          limit = kCommandLimit;
        }
        measure(i + 2, expressionEnd(t, i + 2), limit);
      }
    }
  }
};

std::vector<Source> read(const std::vector<std::string>& dirs) {
  namespace fs = std::filesystem;
  std::vector<Source> sources;
  for (const std::string& dir : dirs) {
    const fs::path root = fs::path(dir).lexically_normal();
    const fs::path base = root.has_parent_path() ? root.parent_path() : fs::path();
    std::vector<fs::path> files;
    for (const auto& entry : fs::recursive_directory_iterator(root)) {
      const std::string ext = entry.path().extension().string();
      if (entry.is_regular_file() && (ext == ".cpp" || ext == ".hpp" || ext == ".mm")) {
        files.push_back(entry.path());
      }
    }
    std::sort(files.begin(), files.end());
    for (const fs::path& file : files) {
      std::ifstream in(file, std::ios::binary);
      std::stringstream text;
      text << in.rdbuf();
      const std::string content = text.str();
      sources.push_back({file.lexically_relative(base).generic_string(), Lexer(content).run()});
    }
  }
  return sources;
}

// Planted sources: what the scan must find, and what it must not.
int selfTest() {
  const std::string long81(81, 'c');
  const std::string long101(101, 'i');
  const std::string half(60, 'h');
  std::string umlauts; // 99 characters in 198 bytes
  for (int k = 0; k < 99; ++k) {
    umlauts += "\xC3\xA4";
  }
  struct Planted {
    std::string path;
    std::string text;
  };
  const std::vector<Planted> planted = {
      {"app/planted/Commands.cpp",
       "// SPDX-License-Identifier: MIT\n"                                              // 1
       "CommandDef command() {\n"                                                       // 2
       "  CommandDef def;\n"                                                            // 3
       "  def.tooltip = QObject::tr(\"Short: what the command does\");\n"               // 4
       "  def.tooltip = QObject::tr(\"" + half + "\"\n"                                  // 5: 120, joined
       "                            \"" + half + "\");\n"                               // 6
       "  def.tooltip = cond ? QObject::tr(\"" + long81 + "\") : QObject::tr(\"b\");\n"   // 7: 81
       "  InputDef input = InputDef::value(\"d\", QObject::tr(\"" + long101 + "\"));\n"  // 8: a label
       "  input.withTooltip(QObject::tr(\"" + long81 + "\"));\n"                         // 9: 81 < 100
       "  input.withTooltip(QObject::tr(\"" + long101 + "\"));\n"                        // 10: 101
       "  // setToolTip(tr(\"" + long101 + "\"));\n"                                     // 11: a comment
       "  const QString text = tr(\"" + long101 + "\");\n"                              // 12: not a tooltip
       "  const QString tip = tr(\"" + long101 + "\").arg(1);\n"                         // 13: 101
       "  input.withButton(tr(\"" + long101 + "\"), tr(\"Pick\"), {});\n"               // 14: a label
       "  return def;\n"
       "}\n"
       "const auto make = [](const char* id, const QString& name, const QString& tooltip) {\n" // 17
       "  CommandDef d;\n"
       "  d.tooltip = tooltip;\n"
       "  return d;\n"
       "};\n"
       "CommandDef a = make(\"a\", tr(\"" + long101 + "\"), tr(\"Fits\"));\n"           // 22: a name
       "CommandDef b = make(\"b\", tr(\"B\"), tr(\"" + long81 + "\"));\n"               // 23: 81
       "QString panelTooltip() {\n"
       "  return tr(\"" + long101 + "\");\n"                                             // 25: 101
       "}\n"},
      {"app/planted/Native.mm",
       "// SPDX-License-Identifier: MIT\n"
       "void show() {\n"
       "  button.toolTip = @\"" + long101 + "\";\n"                                     // 3: 101
       "  button.toolTip = ns(QObject::tr(\"" + umlauts + "\\\"\"));\n"                 // 4: 100 characters
       "  label.setToolTip(QStringLiteral(\"" + std::string(50, 'u') + "\" \"" + std::string(25, 'v') +
           "\" R\"(" + std::string(26, 'w') + ")\"));\n"                                 // 5: 101
       "  m_tabs->setTabToolTip(0, tr(\"" + long101 + "\"));\n"                          // 6: 101
       "  combo->setItemData(0, tr(\"" + long101 + "\"), Qt::ToolTipRole);\n"            // 7: 101
       "  combo->setItemData(0, tr(\"" + long101 + "\"), Qt::UserRole);\n"               // 8: not a tip
       "  tip << tr(\"Fits\") << tr(\"" + long101 + "\");\n"                             // 9: 101
       "}\n"},
  };
  std::vector<Source> sources;
  for (const Planted& p : planted) {
    sources.push_back({p.path, Lexer(p.text).run()});
  }
  Checker checker;
  checker.check(sources);
  const std::set<std::string> expected = {
      "app/planted/Commands.cpp:5 120/80", "app/planted/Commands.cpp:7 81/80",
      "app/planted/Commands.cpp:10 101/100", "app/planted/Commands.cpp:13 101/100",
      "app/planted/Commands.cpp:23 81/80", "app/planted/Commands.cpp:25 101/100",
      "app/planted/Native.mm:3 101/100", "app/planted/Native.mm:5 101/100",
      "app/planted/Native.mm:6 101/100", "app/planted/Native.mm:7 101/100",
      "app/planted/Native.mm:9 101/100"};
  std::set<std::string> found;
  for (const Violation& v : checker.violations) {
    found.insert(v.file + ":" + std::to_string(v.line) + " " + std::to_string(v.length) + "/" +
                 std::to_string(v.limit));
  }
  int failures = 0;
  for (const std::string& e : expected) {
    if (!found.count(e)) {
      std::cout << "self-test: not found: " << e << "\n";
      ++failures;
    }
  }
  for (const std::string& f : found) {
    if (!expected.count(f)) {
      std::cout << "self-test: found, not planted: " << f << "\n";
      ++failures;
    }
  }
  std::cout << "self-test: " << found.size() << " planted long tooltips found, " << failures << " failures\n";
  return failures == 0 ? 0 : 1;
}

} // namespace

int main(int argc, char** argv) {
  if (argc == 2 && std::string(argv[1]) == "--self-test") {
    return selfTest();
  }
  if (argc < 2) {
    std::cerr << "usage: mitcad-check-tooltips DIR... | --self-test\n";
    return 2;
  }
  std::vector<std::string> dirs(argv + 1, argv + argc);
  std::vector<Source> sources;
  try {
    sources = read(dirs);
  } catch (const std::exception& error) {
    std::cerr << "mitcad-check-tooltips: " << error.what() << "\n";
    return 2;
  }
  Checker checker;
  checker.check(sources);
  for (const Violation& v : checker.violations) {
    std::string shown = v.text.substr(0, 60);
    while (!shown.empty() && (static_cast<unsigned char>(shown.back()) & 0xC0) == 0x80) {
      shown.pop_back(); // not in the middle of a character
    }
    if (!shown.empty() && (static_cast<unsigned char>(shown.back()) & 0x80)) {
      shown.pop_back();
    }
    std::cout << v.file << ":" << v.line << ": tooltip of " << v.length << " characters, over the "
              << (v.limit == kCommandLimit ? "command" : "input") << " limit of " << v.limit << ": \"" << shown
              << "...\"\n";
  }
  std::cout << sources.size() << " files, " << checker.violations.size() << " tooltips over their limit"
            << " (commands " << kCommandLimit << ", other controls " << kInputLimit
            << " characters: one short line, the detail in the user guide)\n";
  return checker.violations.empty() ? 0 : 1;
}
