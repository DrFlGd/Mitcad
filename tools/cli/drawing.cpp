// SPDX-License-Identifier: MIT
#include "drawing.hpp"

#include <filesystem>
#include <iostream>
#include <sstream>

#include <QByteArray>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QString>

#include "import.hpp"
#include "mitcad_bridge/lib.h"
#include "rust/cxx.h"

namespace mitcad::cli {
namespace {

// "a,b,c" as a JSON array of strings.
std::string strings(const std::string& text) {
  std::string out = "[";
  std::size_t start = 0;
  while (start <= text.size()) {
    const std::size_t comma = text.find(',', start);
    const std::size_t end = comma == std::string::npos ? text.size() : comma;
    if (out.size() > 1) {
      out += ", ";
    }
    out += json_string(text.substr(start, end - start));
    start = end + 1;
  }
  return out + "]";
}

// "x,y,z" as a JSON array of three numbers, or empty.
std::string triple(const std::string& text) {
  std::istringstream in(text);
  std::string part;
  std::string out = "[";
  int count = 0;
  while (std::getline(in, part, ',')) {
    if (!is_number(part)) {
      return {};
    }
    out += (count++ > 0 ? ", " : "") + part;
  }
  return count == 3 ? out + "]" : std::string();
}

} // namespace

int export_drawing(const std::vector<std::string>& args) {
  Arguments a;
  std::string problem;
  if (!parse_arguments(args,
                       {"--bodies", "--sheet", "--scale", "--views", "--section", "--normal",
                        "--section-analysis", "--unit", "--title", "--date"},
                       {"--portrait", "--first-angle", "--hidden", "--tangent-edges", "--no-dimensions",
                        "--sketch-dimensions", "--no-hole-table", "--per-body", "--polygonal", "--json"},
                       a, problem)) {
    return usage(problem);
  }
  if (a.positional.size() != 2) {
    return usage("export-drawing needs a design and an output file");
  }
  std::string command = R"({"cmd": "export_drawing", "path": )" + json_string(a.positional[1]);
  if (const std::string* bodies = a.option("--bodies")) {
    command += R"(, "bodies": )" + strings(*bodies);
  }
  const std::pair<const char*, const char*> texts[] = {
      {"--sheet", "sheet"}, {"--scale", "scale"}, {"--unit", "units"}, {"--date", "date"}};
  for (const auto& [option, field] : texts) {
    if (const std::string* value = a.option(option)) {
      command += std::string(R"(, ")") + field + R"(": )" + json_string(*value);
    }
  }
  // The design's name in the title block: the file's, unless given.
  const std::string* title = a.option("--title");
  command += R"(, "title": )" +
             json_string(title != nullptr ? *title
                                          : std::filesystem::path(a.positional[0]).stem().string());
  if (const std::string* views = a.option("--views")) {
    command += R"(, "views": )" + strings(*views);
  }
  const std::string* section = a.option("--section");
  const std::string* normal = a.option("--normal");
  if (section != nullptr || normal != nullptr) {
    const std::string origin = section != nullptr ? triple(*section) : std::string();
    const std::string direction = normal != nullptr ? triple(*normal) : std::string();
    if (origin.empty() || direction.empty()) {
      return usage("--section x,y,z and --normal x,y,z go together and need three numbers each");
    }
    command += R"(, "section": {"origin": )" + origin + R"(, "normal": )" + direction + "}";
  } else if (const std::string* analysis = a.option("--section-analysis")) {
    command += R"(, "section": {"analysis": )" + json_string(*analysis) + "}";
  }
  const std::pair<const char*, const char*> flags[] = {
      {"--portrait", R"(, "orientation": "portrait")"},
      {"--first-angle", R"(, "projection": "first_angle")"},
      {"--hidden", R"(, "hidden_lines": true)"},
      {"--tangent-edges", R"(, "tangent_edges": true)"},
      {"--no-dimensions", R"(, "dimensions": false)"},
      {"--sketch-dimensions", R"(, "sketch_dimensions": true)"},
      {"--no-hole-table", R"(, "hole_table": false)"},
      {"--per-body", R"(, "per_body": true)"},
      {"--polygonal", R"(, "exact": false)"}};
  for (const auto& [flag, field] : flags) {
    if (a.flag(flag)) {
      command += field;
    }
  }
  command += "}";
  rust::Box<mitcad::Document> document = open_document(a.positional[0]);
  const std::string result(document->command(command));
  if (a.flag("--json")) {
    std::cout << result << "\n";
    return 0;
  }
  // As the application logs it: "Exported <path>: drawing, 3 views,
  // 2 bodies, A3, 1:1".
  const QJsonObject r = QJsonDocument::fromJson(QByteArray::fromStdString(result)).object();
  std::cout << "Exported " << a.positional[1] << ": drawing, " << r.value(QStringLiteral("views")).toInt()
            << " views, " << r.value(QStringLiteral("bodies")).toArray().size() << " bodies, "
            << r.value(QStringLiteral("sheet")).toString().toStdString() << ", "
            << r.value(QStringLiteral("scale")).toString().toStdString() << "\n";
  for (const QJsonValue warning : r.value(QStringLiteral("warnings")).toArray()) {
    std::cerr << "warning: " << warning.toString().toStdString() << "\n";
  }
  return 0;
}

} // namespace mitcad::cli
