// SPDX-License-Identifier: MIT
// The web viewer's page (mitcad#423, app/web): pageHtml from fixture files
// of the viewer (the placeholders replaced, the import map's modules as
// data: URLs, the data JSON read back, the title escaped, script text that
// would end its element written safely, missing files named), and the
// bodies' glb: one node per placed body, named as the tree names them,
// shared meshes, faces with materials of their own, an embedded texture,
// placements in model units.
#include "web/WebGlb.hpp"
#include "web/WebPage.hpp"

#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <vector>

#include <QByteArray>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSet>
#include <QStringList>
#include <QTemporaryDir>

namespace {

int failures = 0;

void check(bool condition, const char* expression, int line) {
  if (!condition) {
    std::fprintf(stderr, "web_page.cpp:%d: check failed: %s\n", line, expression);
    ++failures;
  }
}

#define CHECK(expr) check((expr), #expr, __LINE__)

using namespace mitcad;

bool writeFile(const QString& path, const QByteArray& bytes) {
  QDir().mkpath(QFileInfo(path).absolutePath());
  QFile file(path);
  return file.open(QIODevice::WriteOnly) && file.write(bytes) == bytes.size();
}

// The text between `open` and the next "</script>" after it.
QByteArray between(const QByteArray& html, const QByteArray& open) {
  const qsizetype start = html.indexOf(open);
  if (start < 0) {
    return {};
  }
  const qsizetype from = start + open.size();
  const qsizetype end = html.indexOf("</script>", from);
  return end < 0 ? QByteArray() : html.mid(from, end - from);
}

void pageHtmlTests() {
  QTemporaryDir root;
  CHECK(root.isValid());
  const QString viewer = root.filePath(QStringLiteral("app/web/viewer"));
  const QByteArray page = "<!doctype html>\n<html><head><title>{{TITLE}}</title>\n"
                          "<!--MITCAD:IMPORTMAP-->\n</head><body><h1>{{TITLE}}</h1>\n"
                          "<!--MITCAD:VIEWER-->\n<!--MITCAD:DATA-->\n</body></html>\n";
  const QByteArray three = "export const REVISION = '1';\n";
  const QByteArray logic = "export function step(a) { return a + 1; }\n";
  // A module that writes the placeholders and the end of a script in its
  // strings: inlined, neither may act as one.
  const QByteArray script = "import * as THREE from 'three';\n"
                          "const tag = '</script><!--MITCAD:DATA-->';\nconsole.log(tag, THREE.REVISION);\n";
  CHECK(writeFile(viewer + QStringLiteral("/template.html"), page));
  CHECK(writeFile(viewer + QStringLiteral("/modules.txt"),
                  "# specifier and path\nthree third_party/three/three.module.js\n\n"
                  "mitcad/logic.js   app/web/viewer/logic.js\n"));
  CHECK(writeFile(viewer + QStringLiteral("/viewer.js"), script));
  CHECK(writeFile(viewer + QStringLiteral("/logic.js"), logic));
  CHECK(writeFile(root.filePath(QStringLiteral("third_party/three/three.module.js")), three));

  const QJsonObject data{{QStringLiteral("format"), 1},
                         {QStringLiteral("title"), QStringLiteral("Bracket <A&B> \"2\"")},
                         {QStringLiteral("units"), QStringLiteral("mm")},
                         {QStringLiteral("unit_scale"), 0.001},
                         {QStringLiteral("glb"), QStringLiteral("Z2xURgIAAAA=")},
                         {QStringLiteral("tree"), QJsonArray{}},
                         {QStringLiteral("views"),
                          QJsonArray{QJsonObject{{QStringLiteral("name"), QStringLiteral("</script><!-- x")}}}}};
  QString error;
  const QByteArray html = web::pageHtmlFrom(data, root.path(), &error);
  CHECK(!html.isEmpty());
  CHECK(error.isEmpty());
  // Every placeholder replaced, the title escaped where the template has it.
  CHECK(!html.contains("<!--MITCAD:"));
  CHECK(!html.contains("{{TITLE}}"));
  CHECK(html.contains("<title>Bracket &lt;A&amp;B&gt; &quot;2&quot;</title>"));
  CHECK(html.contains("<h1>Bracket &lt;A&amp;B&gt; &quot;2&quot;</h1>"));
  // Exactly the three scripts end: the module's and the data's texts cannot.
  int ends = 0;
  for (qsizetype at = html.indexOf("</script>"); at >= 0; at = html.indexOf("</script>", at + 1)) {
    ++ends;
  }
  CHECK(ends == 3);
  CHECK(html.indexOf("<script type=\"importmap\">") < html.indexOf("<script type=\"module\">"));
  CHECK(html.indexOf("<script type=\"module\">") < html.indexOf("<script id=\"mitcad-data\""));

  // The import map parses; each module is its file as a data: URL.
  const QJsonDocument map = QJsonDocument::fromJson(between(html, "<script type=\"importmap\">"));
  CHECK(map.isObject());
  const QJsonObject imports = map.object().value(QStringLiteral("imports")).toObject();
  CHECK(imports.size() == 2);
  const QString prefix = QStringLiteral("data:text/javascript;base64,");
  const QString threeUrl = imports.value(QStringLiteral("three")).toString();
  CHECK(threeUrl.startsWith(prefix));
  CHECK(QByteArray::fromBase64(threeUrl.mid(prefix.size()).toLatin1()) == three);
  const QString logicUrl = imports.value(QStringLiteral("mitcad/logic.js")).toString();
  CHECK(QByteArray::fromBase64(logicUrl.mid(prefix.size()).toLatin1()) == logic);

  // The viewer inlined as a module, its strings meaning the same.
  const QByteArray module = between(html, "<script type=\"module\">");
  CHECK(module.contains("import * as THREE from 'three';"));
  CHECK(module.contains("const tag = '<\\/script><\\!--MITCAD:DATA-->';"));

  // The data reads back as it was.
  const QJsonDocument back = QJsonDocument::fromJson(between(html, "<script id=\"mitcad-data\" type=\"application/json\">"));
  CHECK(back.isObject());
  CHECK(back.object() == data);

  // A missing module, a missing template and a template without a
  // placeholder: no page, and the reason names them.
  CHECK(QFile::remove(viewer + QStringLiteral("/logic.js")));
  error.clear();
  CHECK(web::pageHtmlFrom(data, root.path(), &error).isEmpty());
  CHECK(error.contains(QLatin1String("app/web/viewer/logic.js")));
  CHECK(writeFile(viewer + QStringLiteral("/logic.js"), logic));
  CHECK(writeFile(viewer + QStringLiteral("/template.html"), "<html><!--MITCAD:IMPORTMAP--><!--MITCAD:VIEWER--></html>"));
  error.clear();
  CHECK(web::pageHtmlFrom(data, root.path(), &error).isEmpty());
  CHECK(error.contains(QLatin1String("<!--MITCAD:DATA-->")));
  CHECK(QFile::remove(viewer + QStringLiteral("/template.html")));
  error.clear();
  CHECK(web::pageHtmlFrom(data, root.path(), &error).isEmpty());
  CHECK(error.contains(QLatin1String("app/web/viewer/template.html")));
  // The unit tests' build has no viewer resources: pageHtml says which
  // file it lacks.
  error.clear();
  CHECK(web::pageHtml(data, &error).isEmpty());
  CHECK(error.contains(QLatin1String("template.html")));
}

// A square in z = 0 as two faces of a triangle each.
std::shared_ptr<const render::MeshData> squareMesh() {
  auto mesh = std::make_shared<render::MeshData>();
  mesh->positions = {0, 0, 0, 10, 0, 0, 10, 10, 0, 0, 10, 0};
  mesh->normals = {0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1};
  mesh->indices = {0, 1, 2, 0, 2, 3};
  mesh->faceTriangles = {1, 1};
  return mesh;
}

std::uint32_t uint32At(const QByteArray& bytes, qsizetype at) {
  std::uint32_t value = 0;
  if (at + 4 <= bytes.size()) {
    std::memcpy(&value, bytes.constData() + at, sizeof value);
  }
  return value;
}

// The glb's JSON chunk.
QJsonObject glbJson(const QByteArray& glb) {
  if (glb.size() < 20 || uint32At(glb, 0) != 0x46546c67 || uint32At(glb, 4) != 2 ||
      uint32At(glb, 8) != static_cast<std::uint32_t>(glb.size()) || uint32At(glb, 16) != 0x4e4f534a) {
    return {};
  }
  return QJsonDocument::fromJson(glb.mid(20, static_cast<qsizetype>(uint32At(glb, 12)))).object();
}

void collectNodes(const QJsonArray& items, QStringList& nodes) {
  for (const QJsonValue& value : items) {
    const QJsonObject item = value.toObject();
    for (const QJsonValue& node : item.value(QStringLiteral("nodes")).toArray()) {
      nodes << node.toString();
    }
    collectNodes(item.value(QStringLiteral("children")).toArray(), nodes);
  }
}

void glbTests() {
  QTemporaryDir folder;
  CHECK(folder.isValid());
  const QString image = folder.filePath(QStringLiteral("wood.png"));
  QImage wood(4, 4, QImage::Format_RGB32);
  wood.fill(qRgb(150, 100, 50));
  CHECK(wood.save(image, "PNG"));

  const std::shared_ptr<const render::MeshData> square = squareMesh();
  std::vector<web::PageBody> bodies(5);
  // The root's body in the default look.
  bodies[0].uid = QStringLiteral("F2.b0");
  bodies[0].name = QStringLiteral("Plate");
  bodies[0].mesh = square;
  // The same mesh in an occurrence, moved 10 mm along X, its second face red.
  bodies[1].uid = QStringLiteral("F5.b0");
  bodies[1].name = QStringLiteral("Pin");
  bodies[1].occurrence = QStringLiteral("O1");
  bodies[1].mesh = square;
  bodies[1].transform = {1, 0, 0, 10, 0, 1, 0, 0, 0, 0, 1, 0};
  render::FaceMaterial red;
  red.faces = {1};
  red.material.baseColor = {1.0f, 0.0f, 0.0f};
  bodies[1].faces = {red};
  // Textured, in an occurrence inside it.
  bodies[2].uid = QStringLiteral("F7.b0");
  bodies[2].name = QStringLiteral("Board");
  bodies[2].occurrence = QStringLiteral("O1/O2");
  bodies[2].mesh = square;
  bodies[2].material.texture = image.toStdString();
  bodies[2].material.texturePlanar = true;
  // No triangles: no node.
  bodies[3].uid = QStringLiteral("F9.b0");
  bodies[3].mesh = std::make_shared<render::MeshData>();
  // An occurrence the tree does not have, its image missing.
  bodies[4].uid = QStringLiteral("F11.b0");
  bodies[4].occurrence = QStringLiteral("O9");
  bodies[4].mesh = square;
  bodies[4].material.texture = folder.filePath(QStringLiteral("missing.png")).toStdString();

  // Centimetres: 0.1 model units per mm.
  const web::PageGlb glb = web::pageGlb(bodies, 0.1);
  CHECK((glb.nodes == QStringList{QStringLiteral("n1"), QStringLiteral("n2"), QStringLiteral("n3"), QString(),
                                  QStringLiteral("n4")}));
  CHECK(glb.warnings.size() == 1 && glb.warnings.constFirst().contains(QLatin1String("missing.png")));
  const QJsonObject gltf = glbJson(glb.glb);
  CHECK(!gltf.isEmpty());
  CHECK(gltf.value(QStringLiteral("asset")).toObject().value(QStringLiteral("version")).toString() ==
        QLatin1String("2.0"));
  const QJsonArray nodes = gltf.value(QStringLiteral("nodes")).toArray();
  QStringList names;
  for (const QJsonValue& node : nodes) {
    names << node.toObject().value(QStringLiteral("name")).toString();
  }
  CHECK((names == QStringList{QStringLiteral("n1"), QStringLiteral("n2"), QStringLiteral("n3"), QStringLiteral("n4")}));
  CHECK(gltf.value(QStringLiteral("scenes")).toArray().at(0).toObject().value(QStringLiteral("nodes")).toArray().size() ==
        4);
  // The body's name in the node's extras, the placement in centimetres.
  const QJsonObject pin = nodes.at(1).toObject();
  CHECK(pin.value(QStringLiteral("extras")).toObject().value(QStringLiteral("body_name")).toString() == QLatin1String("Pin"));
  CHECK(!pin.value(QStringLiteral("extras")).toObject().contains(QStringLiteral("name")));
  CHECK(pin.value(QStringLiteral("extras")).toObject().value(QStringLiteral("occurrence")).toString() ==
        QLatin1String("O1"));
  const QJsonArray matrix = pin.value(QStringLiteral("matrix")).toArray();
  CHECK(matrix.size() == 16 && std::abs(matrix.at(12).toDouble() - 1.0) < 1e-9 && matrix.at(15).toDouble() == 1.0);
  CHECK(!nodes.at(0).toObject().contains(QStringLiteral("matrix")));
  // Plain, two faces' looks, textured; the missing image's body shares
  // the plain mesh's look (its texture left out).
  const QJsonArray meshes = gltf.value(QStringLiteral("meshes")).toArray();
  CHECK(meshes.size() == 3);
  CHECK(nodes.at(3).toObject().value(QStringLiteral("mesh")).toInt() == nodes.at(0).toObject().value(QStringLiteral("mesh")).toInt());
  const int pinMesh = pin.value(QStringLiteral("mesh")).toInt();
  CHECK(meshes.at(pinMesh).toObject().value(QStringLiteral("primitives")).toArray().size() == 2);
  const QJsonObject textured = meshes.at(nodes.at(2).toObject().value(QStringLiteral("mesh")).toInt())
                                   .toObject()
                                   .value(QStringLiteral("primitives"))
                                   .toArray()
                                   .at(0)
                                   .toObject();
  CHECK(textured.value(QStringLiteral("attributes")).toObject().contains(QStringLiteral("TEXCOORD_0")));
  const QJsonArray images = gltf.value(QStringLiteral("images")).toArray();
  CHECK(images.size() == 1 &&
        images.at(0).toObject().value(QStringLiteral("mimeType")).toString() == QLatin1String("image/png"));
  const QJsonObject board =
      gltf.value(QStringLiteral("materials")).toArray().at(textured.value(QStringLiteral("material")).toInt()).toObject();
  CHECK(board.value(QStringLiteral("pbrMetallicRoughness")).toObject().contains(QStringLiteral("baseColorTexture")));

  // The tree: the root's body, O1's and O2's inside it; O4 without bodies
  // left out; O9's body, which the tree does not have, with the root's.
  const QJsonArray occurrences{
      QJsonObject{{QStringLiteral("uid"), QStringLiteral("O1")},
                  {QStringLiteral("name"), QStringLiteral("Arm:1")},
                  {QStringLiteral("children"),
                   QJsonArray{QJsonObject{{QStringLiteral("uid"), QStringLiteral("O2")},
                                          {QStringLiteral("name"), QStringLiteral("Board:1")},
                                          {QStringLiteral("children"), QJsonArray{}}}}}},
      QJsonObject{{QStringLiteral("uid"), QStringLiteral("O4")},
                  {QStringLiteral("name"), QStringLiteral("Empty:1")},
                  {QStringLiteral("children"), QJsonArray{}}}};
  const QJsonArray tree = web::pageTree(QStringLiteral("Bracket"), occurrences, bodies, glb.nodes);
  CHECK(tree.size() == 1);
  const QJsonObject top = tree.at(0).toObject();
  CHECK(top.value(QStringLiteral("id")).toString() == QLatin1String("root"));
  CHECK(top.value(QStringLiteral("name")).toString() == QLatin1String("Bracket"));
  CHECK((top.value(QStringLiteral("nodes")).toArray() == QJsonArray{QStringLiteral("n1"), QStringLiteral("n4")}));
  const QJsonArray children = top.value(QStringLiteral("children")).toArray();
  CHECK(children.size() == 1);
  const QJsonObject arm = children.at(0).toObject();
  CHECK(arm.value(QStringLiteral("id")).toString() == QLatin1String("O1"));
  CHECK(arm.value(QStringLiteral("name")).toString() == QLatin1String("Arm:1"));
  CHECK((arm.value(QStringLiteral("nodes")).toArray() == QJsonArray{QStringLiteral("n2")}));
  const QJsonObject inner = arm.value(QStringLiteral("children")).toArray().at(0).toObject();
  CHECK(inner.value(QStringLiteral("id")).toString() == QLatin1String("O1/O2"));
  CHECK((inner.value(QStringLiteral("nodes")).toArray() == QJsonArray{QStringLiteral("n3")}));
  // Every node of the glb is in the tree once, and the tree names no other.
  QStringList inTree;
  collectNodes(tree, inTree);
  inTree.sort();
  names.sort();
  CHECK(inTree == names);
}

} // namespace

int webPageTests() {
  failures = 0;
  pageHtmlTests();
  glbTests();
  return failures;
}
