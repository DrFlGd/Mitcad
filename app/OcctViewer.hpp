// SPDX-License-Identifier: MIT
#pragma once

#include <array>
#include <cstddef>
#include <functional>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include <QColor>
#include <QElapsedTimer>
#include <QImage>
#include <QMargins>
#include <QOpenGLWidget>
#include <QPoint>
#include <QRectF>
#include <QString>
#include <QStringList>
#include <QSurfaceFormat>
#include <QtGlobal>

#include <AIS_InteractiveContext.hxx>
#include <AIS_Shape.hxx>
#include <AIS_ViewController.hxx>
#include <AIS_ViewCube.hxx>
#include <Bnd_Box.hxx>
#include <Graphic3d_Camera.hxx>
#include <Graphic3d_Texture2D.hxx>
#include <Graphic3d_ZLayerId.hxx>
#include <SelectMgr_EntityOwner.hxx>
#include <SelectMgr_Filter.hxx>
#include <TopLoc_Location.hxx>
#include <TopoDS_Edge.hxx>
#include <TopoDS_Shape.hxx>
#include <V3d_View.hxx>
#include <V3d_Viewer.hxx>
#include <gp_Ax3.hxx>
#include <gp_Pnt.hxx>
#include <gp_Pnt2d.hxx>
#include <gp_Trsf.hxx>

#include "LayoutGrid.hpp"
#include "framework/Appearances.hpp"
#include "framework/Selection.hpp"
#include "framework/SelectionScope.hpp"
#include "mitcad/geometry/shape.hpp"
#include "view/ViewMatch.hpp"
#include "view/ViewNavigation.hpp"

class QNativeGestureEvent;
class QTimer;
class QToolButton;
class QWheelEvent;

namespace mitcad {

class ViewCubeHomeButton;

#ifdef Q_OS_MACOS
// The OpenGL format of every context Qt creates on macOS, the viewport's and
// the window's shared one (main() sets it as the default format): a core
// profile, which macOS only offers as a forward-compatible 4.1 context.
// Contexts of different profiles cannot share resources there.
QSurfaceFormat macSurfaceFormat();
#endif

// Identifies a sketch profile: (sketch feature uid, region key).
using ProfileKey = std::pair<std::string, std::string>;

// Whether two transformations are the same by value (TopLoc_Location's
// IsEqual compares its items, which differ for every new location).
bool sameTransformation(const gp_Trsf& a, const gp_Trsf& b);

// A body to show; its face names name what is picked on it.
struct BodyDisplay {
  enum class Style {
    Normal,
    New, // a body the previewed command makes: see-through blue, not pickable
    // A body a command picks through (mitcad#147): see-through in its own
    // colour, not pickable. setGhosts() draws whole occurrences so;
    // setActiveScope() draws inactive components with the same look, but
    // pickable (mitcad#420).
    Ghost,
  };
  std::string uid;
  std::shared_ptr<geometry::Shape> shape;
  Style style = Style::Normal;
  // Where an occurrence of a component puts the body (F6); identity in the
  // design's own coordinates. Picks on a placed body name the body's own
  // faces, edges and vertices, with the occurrence (U3).
  TopLoc_Location placement;
  // The occurrence that places the body (SelectionItem::occurrence): tells
  // repeated or placed bodies apart; empty in the root component.
  std::string occurrence;
  // Its appearance's colour (U4); the default grey when invalid.
  QColor color = QColor();
  // Its appearance's parameters for the rendered view (mitcad#46); an
  // empty id for the default look.
  Appearance appearance = Appearance();
  // Faces with appearances of their own (mitcad#53): their indices in the
  // shape (geometry::Shape's order), their colour and appearance.
  struct FaceLook {
    std::vector<int> faces;
    QColor color;
    Appearance appearance;
  };
  std::vector<FaceLook> faceLooks = {};
  // Owning component, shared by all its bodies and occurrences.
  std::string component;
};

// A sketch curve or point in model space.
struct SketchEntityDisplay {
  QString sketch;
  QString id;       // c3, p4
  QString geometry; // line, circle, arc, ellipse, point, ...
  TopoDS_Shape shape;
  bool construction = false;
  // How sketch mode draws it (U2): fully constrained geometry darker (light
  // on a dark background, mitcad#253), centre lines dash-dotted, projected
  // geometry purple.
  bool constrained = false;
  bool centerline = false;
  bool reference = false;
  // The occurrence that places the sketch when it is a component's (the
  // shape is placed already); picks carry it.
  QString occurrence = QString();
  // What the shape is made from (the entity's solved geometry, the sketch
  // frame, the style and the placement): an entity shown with the same
  // signature keeps its display (setSketchEntities). Empty: always new.
  QString signature = QString();
};

// An origin or construction plane, axis or point.
struct DatumDisplay {
  QString uid;
  SelectKind kind = SelectKind::Plane;
  TopoDS_Shape shape;            // placed already
  QString occurrence = QString(); // as for sketch entities
};

// A selected item drawn in its input's colour.
struct HighlightDisplay {
  SelectionItem item;
  TopoDS_Shape shape;
  QColor color;
};

// How bodies are drawn (U5).
enum class VisualStyle {
  Shaded,               // faces only
  ShadedVisibleEdges,   // faces and the edges in sight (the default)
  ShadedHiddenEdges,    // also the edges behind faces, dashed
  Wireframe,            // edges only
  WireframeHiddenEdges, // edges in sight, those behind faces dashed
};

// Which mouse buttons pan, orbit and zoom (U5), named after what the mouse
// does. The wheel zooms in every scheme, and a trackpad's pinch (zoom),
// rotation (roll) and two-finger double tap (fit) work in every scheme too.
enum class NavigationScheme {
  MiddlePan,   // middle: pan; Shift + middle: orbit (the default)
  AltButtons,  // Alt + left: orbit; Alt + middle: pan; Alt + right: zoom
  MiddlePanF4, // middle: pan; Shift + middle or F4 + left: orbit
  MiddleOrbit, // middle: orbit; Ctrl + middle: pan; Shift + middle: zoom
  RightOrbit,  // right: orbit; middle or Shift + right: pan
  // For a trackpad (the default on macOS): two-finger scroll
  // pans, Alt + scroll orbits, Shift or Ctrl + scroll zooms; Alt + left
  // orbits, Shift + Alt + left, middle: pan; right: orbit. A mouse wheel
  // zooms as in the other schemes.
  Trackpad,
  // As Trackpad, but Shift + scroll orbits and Alt or Ctrl + scroll zooms.
  TrackpadShift,
};

struct NavigationSettings {
  NavigationScheme scheme = NavigationScheme::MiddlePan;
  bool zoomToCursor = true;    // the wheel zooms about the cursor, else the middle
  bool reverseZoom = false;    // the wheel zooms the other way
  bool constrainedOrbit = false; // orbits keep Z up
  bool animate = true;         // view changes (standard views, named views) move smoothly
};

// What a left drag without keys that starts on a body does (mitcad#150):
// the view's window selection, the owner's drag, or neither (refused: the
// owner says why).
enum class OccurrenceDragStart { Select, Drag, Refuse };

// Dragging components in the view (mitcad#55): a left drag without keys
// that starts on a body `accepts` takes is the owner's instead of a window
// selection (a click there still picks); one it refuses is neither, and
// `refused` is called once the pointer has moved. Points are in the
// design's coordinates: where the press hit the body, and where the
// pointer is on the plane through it that faces the viewer.
struct OccurrenceDrag {
  std::function<OccurrenceDragStart(const SelectionItem&)> accepts;
  std::function<void(const SelectionItem&)> refused;
  std::function<void(const SelectionItem&, const gp_Pnt&)> started;
  std::function<void(const gp_Pnt&)> moved;
  // `cancelled`: a dialog took the mouse (P7), or the pointer left the plane.
  std::function<void(const gp_Pnt&, bool cancelled)> finished;
};

// A camera that can be saved (named views): in design coordinates, mm.
struct CameraState {
  gp_Pnt eye;
  gp_Pnt target;
  gp_Dir up = gp_Dir(0, 0, 1);
  bool perspective = false;
  double height = 250.0; // of the view at the target
};

// 3D view: Qt owns the OpenGL context and the widget's framebuffer object,
// OCCT renders the scene into it.
//
// Navigation by default: middle drag pans, Shift + middle drag orbits and
// the wheel zooms about the cursor (setNavigation offers the other
// schemes). Left click picks, left drag picks with a window (left to right:
// what lies inside; right to left: what it touches too). Picks are reported
// as model names (SelectionItem); the view itself keeps no selection, the
// owner draws it with setHighlights().
//
// The orientation cube (upper right) turns the view when its faces, edges
// and corners are clicked and orbits when it is dragged; the house above
// it goes home, a right click on it opens the owner's cube menu.
class OcctViewer : public QOpenGLWidget, public AIS_ViewController {
  Q_OBJECT

public:
  // How a command preview's tool body is drawn.
  enum class PreviewStyle {
    Add,    // blue: a new body or material a Join adds
    Remove, // red: material a Cut removes
    Failed, // red outline: the feature fails
    Datum,  // blue, nearly opaque, over picked references: a construction
            // feature's plane, axis or point (mitcad#446)
  };

  explicit OcctViewer(QWidget* parent = nullptr);
  ~OcctViewer() override;

  // Display settings (U5).
  void setVisualStyle(VisualStyle style);
  void setComponentColors(bool on);
  VisualStyle visualStyle() const { return m_visualStyle; }
  void setNavigation(const NavigationSettings& settings);
  const NavigationSettings& navigation() const { return m_navigation; }
  // The background: a vertical gradient from top to bottom, or one colour.
  void setBackground(const QColor& top, const QColor& bottom, bool gradient);
  // The layout grid on the grid plane (XY, or the sketch plane): `step`
  // mm between lines, or a step that follows the zoom when 0.
  void setGrid(bool shown, double step);
  void setGridPlane(const gp_Ax3& plane);
  // The grid's minor step now (mm), shown or not: what sketch points snap to.
  double gridStep() const;
  // The faces where Section Analysis or Hide Above Sketch cuts the bodies
  // (caps), drawn over the cut and hatched along `plane`'s axes. One group
  // per colour (mitcad#382: the timeline colour of the bodies' component; an
  // invalid colour is the default one), each hatched in a darker shade of
  // its own; none clears them.
  struct SectionCap {
    TopoDS_Shape faces;
    QColor color = QColor();
  };
  void setSectionCaps(const std::vector<SectionCap>& caps, const gp_Ax3& plane);
  // Cosmetic threads: rings on the threaded faces, drawn over the bodies
  // and not pickable; null clears them.
  void setThreads(const TopoDS_Shape& rings);

  // The camera (named views, U5).
  CameraState camera() const;
  // Moves the camera there, smoothly when the navigation settings say so.
  void setCamera(const CameraState& camera);
  void setPerspective(bool perspective);
  bool isPerspective() const;
  // Looks along `direction` (from the eye to the model) with `up` upwards,
  // fitted, smoothly when the navigation settings say so (standard views,
  // the orientation cube's home).
  void turnTo(const gp_Dir& direction, const gp_Dir& up);
  // The home view: the document's (setHome) or the isometric one, fitted.
  void goHome();
  void setHome(const std::optional<CameraState>& home) { m_home = home; }
  // The orientation cube's corner of the view (widget pixels): the square
  // its arrows and the home button stay in, and its centre's distance from
  // the corner.
  static constexpr int kCubeArea = 190;
  static constexpr int kCubeOffset = 100;
  // The part of the view that is free for its own overlays (the orientation
  // cube, its arrows and the home button): the widget less these margins.
  // Floating chrome lays cards over the view and keeps these overlays out
  // from under them; the picking of the cube follows. Logs "View overlay
  // insets" when it changes.
  void setOverlayInsets(const QMargins& insets);
  // AppKit provides the general view controls in its native toolbar.
  void setNativeControls(bool native);
  QMargins overlayInsets() const { return m_overlayInsets; }
  // The width that the sketch's badge takes at the bottom left, margin
  // included (0: none), so that the floating status pill keeps clear of it.
  void setBadgeWidth(int width);
  int badgeWidth() const { return m_badgeWidth; }
  // Parts of the view that panels float over (widget pixels): the places
  // that logs and tests offer for clicking (picks, the datum planes) avoid
  // them, and the view itself is not picked there.
  void setCoveredRects(const std::vector<QRect>& rects);
  bool isCovered(const QPointF& position) const;
  // The cards floating at the top of the view (mitcad#425: the title card and
  // the toolbar's group cards) and the bottom of their rows (widget
  // pixels; 0: none): the free area starts below them, and the view is not
  // picked inside a card's own rectangle, but is between the cards.
  void setTopCards(const std::vector<QRect>& cards, int bottom);
  // The cube's area in widget pixels, with the insets applied.
  QRect overlayCubeArea() const { return cubeArea().toRect(); }
  // Whether a point of the widget is on the orientation cube.
  bool isOnOrientationCube(const QPointF& position);
  // A left press on the cube (or its drag) is in progress: sketch mode
  // leaves the mouse to the view.
  bool cubeInteraction() const { return m_cubePress; }
  // A frame is being drawn: the view handles its events (picks) meanwhile,
  // and nothing may compute the model then (P7, MainWindow::runJob).
  bool isPainting() const { return m_painting; }

  // Replaces the bodies, keyed by their uids (F2.b0).
  void setBodies(const std::vector<BodyDisplay>& bodies);
  // The bodies shown (setBodies), without command previews; bodies that
  // setGhosts() draws as ghosts are among them, as given.
  std::vector<BodyDisplay> shownBodies() const;
  // Occurrences (paths of uids from the root, `O1/O4`) whose bodies, and
  // the bodies of the occurrences inside them, are drawn as ghosts
  // (BodyDisplay::Style::Ghost) instead of normally, now and by later
  // setBodies() calls, until the list changes (mitcad#147: a command picks
  // through them). Highlights of items in them are not pickable either, so
  // a click goes to what lies behind. Logs "View ghosts: ..." when the
  // list changes.
  void setGhosts(const QStringList& occurrences);
  // What the view draws normally while a component other than the root is
  // active (mitcad#420): the bodies, sketches, profiles and construction
  // items of `occurrences` (the active component's occurrences, paths from
  // the root, `O1/O4`) and of the occurrences inside them, and the items
  // whose owners `normal` names (the sketch being edited, the origin's
  // datums, which are the active component's own). Everything else is an
  // inactive ghost: bodies see-through in their own colour (as setGhosts()
  // draws them), lines, points and planes pale. Unlike setGhosts()' ghosts
  // they stay pickable as references, with the hover and highlights of any
  // item (after the active component's own: setActivePickPriority()); a
  // body that a command ghosts too is drawn once, as the command's
  // ghost. Not `on` (the root active): everything is drawn normally.
  struct ActiveScope {
    bool on = false;
    QStringList occurrences;
    QStringList normal;
    QString name; // the active component's occurrences, for the log
    bool operator==(const ActiveScope& other) const {
      return on == other.on && occurrences == other.occurrences && normal == other.normal && name == other.name;
    }
  };
  // Logs "View inactive ghosts: <n> bodies (active <name>)" or "View
  // inactive ghosts: none" when that changes.
  void setActiveScope(const ActiveScope& scope);
  // Preferences' Ghost Inactive Components (`view/ghostInactive`, default
  // on): off draws everything normally whatever the scope.
  void setGhostInactive(bool on);
  // Preferences' Pick the Active Component First (`view/activePickPriority`,
  // default on; mitcad#420): while the scope is on, a click, a command's
  // pick and the hover take the nearest item of the active component under
  // the cursor before nearer items of other components (PickContext.hpp's
  // setPickPriority); where none of its items is under the cursor, the
  // others are taken as usual. Logs "Pick priority: active <name>" when a
  // click took such an item before a nearer other one.
  void setActivePickPriority(bool on);

  // The rendered view (docs/rendering.md): the image fills the view
  // (stretched; a frame that refines is smaller) over the bodies, which
  // keep their depth and stay pickable; their edges and the layout grid
  // are not drawn; the orientation cube, sketches, datums, highlights and
  // previews are drawn over it, hidden by the bodies as in the shaded view.
  // A null image draws the bodies again. While the rendered mode is on
  // (setRenderedMode), the bodies are in a layer of their own under the
  // image's. The image is of the camera and size of the view when it is
  // set: once either changes, the view draws the bodies again before its
  // next frame (no image of an old camera over a moved view, mitcad#214).
  void setRenderedImage(const QImage& image);
  void setRenderedMode(bool on);
  bool showsRenderedImage() const { return !m_renderedImage.isNull(); }
  // The camera the view draws with, for the renderer to follow.
  const occ::handle<Graphic3d_Camera>& viewCamera() const { return m_view->Camera(); }
  // Whether the user is navigating the view (a button held, a wheel burst
  // or gesture that has not kept still for a moment, a camera animation;
  // ViewNavigation), as of the last frame drawn; navigationFinished() when
  // it ends (mitcad#214).
  bool isNavigating() const { return m_navigationState.active(); }
  // The background's colours, top and bottom (the same for one colour).
  void backgroundColors(QColor& top, QColor& bottom) const;
  // Whether the background is dark (what is drawn over it needs light lines).
  bool backgroundDark() const { return m_backgroundDark; }
  // Replaces the visible sketch profiles (planar faces, placed already);
  // `occurrences` are those that place the sketches of components.
  void setProfiles(const std::vector<std::pair<ProfileKey, TopoDS_Shape>>& profiles,
                   const std::map<std::string, QString>& occurrences = {});
  void setSketchEntities(const std::vector<SketchEntityDisplay>& entities);
  void setDatums(const std::vector<DatumDisplay>& datums);
  void setHighlights(const std::vector<HighlightDisplay>& highlights);
  // Semi-transparent preview of the active command; null clears it.
  void setPreview(const TopoDS_Shape& shape, PreviewStyle style = PreviewStyle::Add);
  // Outline of the sketch shape being drawn; null clears it.
  void setSketchPreview(const TopoDS_Shape& shape);

  // What can be picked: kinds, and optionally a finer test.
  void setPickFilter(SelectFilter filter, std::function<bool(const SelectionItem&)> test = {});
  // A model job owns the document and its geometry. Keep drawing cached
  // presentations, but do not resolve owners or call command predicates.
  void setPickingSuspended(bool suspended);
  // A selection scope (mitcad#217, framework/SelectionScope.hpp): `kinds`
  // can be picked, `expanded` (which holds them) while Shift is held over a
  // click; `test` decides each item by how it is picked: a click or a
  // window, Shift, and for body edges which part of the edge was hit (in
  // sight or behind the faces of the shown bodies, seen from the camera
  // now). Shift pressed or released picks again under the pointer, so the
  // hover shows what the click would select.
  using PickTest = std::function<bool(const SelectionItem&, const PickCase&)>;
  void setPickScope(SelectFilter kinds, SelectFilter expanded, PickTest test);
  bool shiftHeld() const { return m_shiftHeld; }
  // The last window selection in widget pixels, and whether it also took
  // what it crosses (dragged to the left).
  QRectF lastWindow() const { return m_windowRect; }
  bool lastWindowCrossing() const { return m_windowCrossing; }
  // For UI tests (MITCAD_LOG_EDGE_PLACES): per edge of the shown bodies, a place
  // in the widget where a click hits that edge first among what the view
  // offers now, one where the edge is in sight and one where it is behind
  // faces (when it has such parts in the view).
  struct EdgePlace {
    SelectionItem item;
    QPoint at;
    EdgeSight sight = EdgeSight::Visible;
  };
  std::vector<EdgePlace> edgePlaces();
  // Dragging components (mitcad#55).
  void setOccurrenceDrag(OccurrenceDrag drag) { m_occurrenceDrag = std::move(drag); }
  // The move cursor over a body a drag would move (mitcad#150).
  void setDragCursor(bool shown);
  // The point under a widget position on the plane through `through` that
  // faces the viewer; none when the view looks along it.
  std::optional<gp_Pnt> viewPlanePoint(const QPointF& position, const gp_Pnt& through) const;
  SelectFilter pickFilter() const { return m_filter; }
  // Colour of the highlight under the mouse: see-through and unlit, with
  // the face's boundary, edges and vertices opaque, hidden by nearer
  // geometry (mitcad#149; as the selections of setHighlights()).
  void setHoverColor(const QColor& color);
  // More of the hover (mitcad#290): the rest of the tangent chain of the
  // edge or face under the mouse, drawn as the hover draws that one (edges
  // in a compound) and not pickable; null takes it away.
  void setHoverChain(const TopoDS_Shape& shape);

  // While a sketch tool draws, the left button is the tool's (sketch mode
  // handles it), not the view's: no picking or window selection. The
  // pointer is then the precision cursor (framework/Cursors.hpp) with the
  // tool's icon `toolIcon`; a changed device pixel ratio rebuilds it.
  void setSketchInput(bool enabled, const QString& toolIcon = QString());
  // While a sketch tool's pick snaps to a point: the cursor shows a square.
  void setSketchSnapping(bool snapping);
  // The sketch plane: sketchPlanePoint() gives points in its frame.
  void setSketchPlane(const gp_Ax3& plane) { m_sketchPlane = plane; }
  // The point of the sketch plane under a widget position (logical pixels).
  std::optional<gp_Pnt2d> sketchPlanePoint(const QPointF& position) const;

  // Turns to look at the sketch plane; restoreCamera() returns to the view
  // saved when `saveCamera` was set.
  void lookAtSketchPlane(bool saveCamera = true);
  void restoreCamera();
  void fitAll();
  // Fits the view to a box (model coordinates) as Fit fits everything:
  // along the current view direction, with Fit's margin, in the part of the
  // view no card covers; at once (mitcad#405).
  void frameBox(const Bnd_Box& box);
  // The box of the bodies shown as placed (not previews, ghosts, datums,
  // sketches or the grid); void when none is shown.
  Bnd_Box bodiesBox() const;
  // Ends a camera animation at its end and makes a pending fit now, so that
  // camera() and the next frame are where the view is going (live MCP's
  // replies and screenshots, mitcad#405).
  void settleCamera();
  // The perspective camera's vertical field of view, degrees.
  double fieldOfView() const;
  void setFieldOfView(double degrees);
  // Looks at a plane from the side its normal points to, `up` upwards, and
  // fits the view (Look At, named views; U3).
  void lookAlong(const gp_Dir& normal, const gp_Dir& up);
  // The isometric home view, fitted.
  void lookHome();

  // Position of a model point in the widget (logical pixels), for tests.
  QPoint toWidget(const gp_Pnt& point) const;
  // The same without rounding, for drawing over the view.
  QPointF toWidgetF(const gp_Pnt& point) const;
  // A point of a plane datum that faces the viewer, away from the others.
  gp_Pnt planePickPoint(const gp_Pnt& origin, const gp_Dir& normal, double size) const;
  // Points of a plane datum in the order a click is best tried at.
  std::vector<gp_Pnt> planePickCandidates(const gp_Pnt& origin, const gp_Dir& normal,
                                          double size) const;
  // The camera's up direction (manipulators measure the zoom along it).
  gp_Dir viewUp() const;
  // What a click at a widget position would pick now, if anything.
  std::optional<SelectionItem> itemAt(const QPointF& position);
  // Everything under a widget position that the filter takes, the item a
  // click picks first.
  std::vector<SelectionItem> itemsAt(const QPointF& position);
  // For UI tests: a point in the widget where a click picks each face,
  // edge or vertex of the shown bodies that the filter takes (only those
  // a click there would pick).
  std::vector<std::pair<SelectionItem, QPoint>> pickPoints(SelectFilter kinds);
  // The OpenGL version string (with the driver's version), once the view
  // is initialised; for error reports (mitcad#61).
  QString glVersion() const { return m_glVersion; }

  // A popup or native control can consume the pointer's release. End its
  // interaction without turning the interrupted drag into a selection.
  void cancelPointerInteraction();

signals:
  void badgeWidthChanged();
  void glInitialized(const QString& renderer);
  void glFailed(const QString& message);
  // A frame was drawn (the camera may have moved): drawings over the view
  // follow.
  void viewChanged();
  // A click (window false) or a window selection; empty when the click hit
  // nothing. Emitted while the view paints: connect queued.
  void picked(const mitcad::Selection& items, Qt::KeyboardModifiers modifiers, bool window);
  // What a click would pick changed as the mouse moved (invalid when
  // nothing), for drawings over the view that show it more boldly than
  // OCCT's highlight, whose lines are one pixel wide on macOS. Emitted while
  // the view paints: connect queued.
  void hovered(const mitcad::SelectionItem& item);
  // The point under the mouse moved on what a click would pick: the item
  // with `at` where the mouse is now (in its component's coordinates), or
  // an invalid item when the mouse left the view (mitcad#417: BUILD shows
  // an end click's part before the click). Emitted while the view paints:
  // connect queued.
  void hoverMoved(const mitcad::SelectionItem& item);
  // A right click; `item` is what lies under the mouse (invalid if nothing).
  void contextMenuRequested(const QPoint& globalPosition, const mitcad::SelectionItem& item);
  // A right click on the orientation cube.
  void orientationCubeMenuRequested(const QPoint& globalPosition);
  // The camera came to rest after it moved (logged as "Camera direction ...").
  void cameraRested();
  // A navigation of the view ended (isNavigating()): the camera is where the
  // user left it.
  void navigationFinished();
  // The application's palette changed (a dark or light mode): what depends
  // on it outside the view follows.
  void themeChanged();
  // setBodies() changed what is shown.
  void bodiesChanged();
  // Shift went down or up while a selection scope is set (setPickScope).
  void shiftChanged(bool held);

protected:
  // Shift anywhere in the window widens a scope's clicks (setPickScope).
  bool eventFilter(QObject* watched, QEvent* event) override;
  // A modal dialog that opens meanwhile (a long computation's progress, P7)
  // takes the release of a button held down: the view lets go of it then.
  bool event(QEvent* event) override;
  void initializeGL() override;
  void paintGL() override;
  void resizeGL(int width, int height) override;
  void mousePressEvent(QMouseEvent* event) override;
  void mouseReleaseEvent(QMouseEvent* event) override;
  void mouseMoveEvent(QMouseEvent* event) override;
  void wheelEvent(QWheelEvent* event) override;
  void keyPressEvent(QKeyEvent* event) override;
  void keyReleaseEvent(QKeyEvent* event) override;

  void handleViewRedraw(const occ::handle<AIS_InteractiveContext>& context,
                        const occ::handle<V3d_View>& view) override;
  void contextLazyMoveTo(const occ::handle<AIS_InteractiveContext>& context,
                         const occ::handle<V3d_View>& view, const NCollection_Vec2<int>& point) override;
  void OnSelectionChanged(const occ::handle<AIS_InteractiveContext>& context,
                          const occ::handle<V3d_View>& view) override;

private:
  // What an object displays, for naming its picks.
  struct PickInfo {
    SelectionItem item;                    // complete, except for bodies
    std::shared_ptr<geometry::Shape> body; // bodies: names sub-shapes
    bool highlight = false;
  };
  class Filter;
  // A shown body's faces for looking along a line (occluded()), made when
  // first needed.
  struct Occluder;

  struct BodyEntry {
    std::string uid;
    std::string component;
    std::string occurrence;
    occ::handle<AIS_Shape> object;
    std::shared_ptr<geometry::Shape> shape;
    BodyDisplay::Style style;
    TopLoc_Location placement;
    QColor color;
    Appearance appearance;
    std::vector<BodyDisplay::FaceLook> faceLooks;
    // A normal body drawn as a ghost by setGhosts() (`style` is Ghost).
    bool ghosted = false;
    // Outside the active component (setActiveScope): a normal body is drawn
    // as a ghost that stays pickable (`style` stays Normal).
    bool inactive = false;
    // The hidden-edge styles' edges: dashed through faces, solid in sight.
    occ::handle<AIS_Shape> hiddenEdges;
    occ::handle<AIS_Shape> visibleEdges;
    // Its faces placed, for occluded() (mitcad#217).
    mutable std::shared_ptr<Occluder> occluder;
  };

  Aspect_Drawable occtDrawable();
  NCollection_Vec2<int> toDevicePixels(const QPointF& point) const;
  Aspect_VKeyMouse viewButtons(Qt::MouseButtons buttons) const;
  Aspect_VKeyFlags viewFlags(Qt::KeyboardModifiers modifiers) const;
  void display(const occ::handle<AIS_Shape>& object, int displayMode, PickInfo info,
               bool pickable);
  // Shows a body in the visual style; the entry's objects are made here.
  void showBody(BodyEntry& entry);
  void removeBody(BodyEntry& entry);
  // Whether setGhosts() names an occurrence or one it lies in.
  bool isGhosted(const QString& occurrence) const;
  // Inactive ghosts (setActiveScope, mitcad#420): whether what an occurrence
  // places lies outside the active component, and whether an item (a
  // sketch entity, profile or datum) is drawn as one.
  bool isInactive(const QString& occurrence) const;
  bool isInactiveItem(const SelectionItem& item) const;
  // The same by the scope alone, whatever the setting (the pick priority).
  bool outsideActive(const QString& occurrence) const;
  bool outsideActive(const SelectionItem& item) const;
  // Draws the bodies, sketch entities, profiles and datums by the scope
  // and the setting again; `faded` restyles the pale ones too (their
  // colour follows the background).
  void applyInactiveGhosts(bool faded = false);
  // Marks an object as drawn pale or not; true when that changed.
  bool markInactive(const occ::handle<AIS_Shape>& object, bool inactive);
  bool drawnInactive(const occ::handle<AIS_Shape>& object) const {
    return m_inactiveObjects.count(object.get()) != 0;
  }
  // A profile's and a datum's look, pale when drawnInactive(); a datum's
  // display mode.
  void styleProfile(const occ::handle<AIS_Shape>& profile) const;
  int styleDatum(const occ::handle<AIS_Shape>& object, SelectKind kind) const;
  void logInactiveGhosts();
  // The rendered mode's layer for a body, and its edges hidden under a
  // rendered image.
  void applyRenderedVisibility(BodyEntry& entry);
  // The background colours again (after a rendered image).
  void applyBackground();
  void bindGestures();
  // Starts a smooth camera move to `end` (or jumps when not animating).
  void moveCamera(const occ::handle<Graphic3d_Camera>& end);
  void fitCamera(const occ::handle<Graphic3d_Camera>& camera) const;
  // The part of the view the floating cards leave free (the whole view when
  // there are no cards).
  QRectF freeArea() const;
  // Zooms out and pans a camera that has been fitted to the whole view so
  // that the fitted objects are inside freeArea() and centred in it.
  void fitToFreeArea(const occ::handle<Graphic3d_Camera>& camera) const;
  QRectF cubeArea() const;
  void placeHomeButton();
  // The buttons over the view take dark or light icons by the background.
  void updateOverlayIcons();
  // What a shown sketch entity's colour and line follow (setSketchEntities):
  // fully constrained geometry and centre lines are light on a dark
  // background (mitcad#253).
  struct SketchEntityLook {
    bool point = false;
    bool construction = false;
    bool centerline = false;
    bool constrained = false;
    bool reference = false;
  };
  void styleSketchEntity(const occ::handle<AIS_Shape>& object, const SketchEntityLook& look) const;
  // Whether the entity is drawn over a halo (mitcad#302): fully constrained
  // curves and centre lines, whose colour follows the background and would
  // otherwise vanish over a body's face of the same tone.
  static bool hasSketchHalo(const SketchEntityLook& look);
  // The halo: a wider line in the background's tone under the entity's.
  void styleSketchHalo(const occ::handle<AIS_Shape>& halo, const SketchEntityLook& look) const;
  // Logs "Sketch halos: <n> dark|light" when it changed (for UI tests).
  void logSketchHalos();
  // Recolours the sketch entities shown after the background turned dark
  // or light.
  void restyleSketchEntities();
  // The orientation cube and the pick tolerance are in device pixels; this
  // scales them by the widget's device pixel ratio (no change at 1).
  void applyDevicePixelRatio();
  // Sets the cube's size and place (device pixel ratio, insets) and shows it
  // again.
  void applyCubePlacement();
  // The orientation cube's arrows (P9): shown while the view looks
  // straight at a face; the triangles turn the view to the face beside it,
  // the curved arrows roll it 90 degrees about the line of sight.
  void placeCubeArrows();
  void updateCubeArrows();
  void turnCube(const QString& arrow);
  // Logs the camera, and with MITCAD_LOG_PICKS where the cube's faces,
  // edges and corners can be clicked, once the camera rests.
  void cameraMoved();
  void logCamera();
  void logOrientationCube();
  // Navigation (mitcad#214): the state evaluated now, and a step of input
  // without a release (wheel, pinch, roll, resize).
  void updateNavigation();
  void navigationStep();
  // The last button held on the view went up.
  void buttonsReleased();
  // The view's camera and size now; why the rendered image no longer
  // fits the view (its camera or size changed), or nothing.
  ViewPose viewPose() const;
  QString renderedImageStale() const;
  // Rebuilds the grid's lines when the zoom changes their step or the view
  // nears their edge (or always with `force`).
  void updateGrid(bool force = false);
  void remove(const occ::handle<AIS_Shape>& object);
  void activatePicking(const occ::handle<AIS_Shape>& object);
  void activateAllPicking();
  std::optional<SelectionItem> itemFor(const occ::handle<SelectMgr_EntityOwner>& owner) const;
  // Where the last click hit an owner, in its object's own coordinates.
  std::optional<std::array<double, 3>> pickedPoint(const occ::handle<SelectMgr_EntityOwner>& owner) const;
  // The same in the design's coordinates.
  std::optional<gp_Pnt> pickedWorldPoint(const occ::handle<SelectMgr_EntityOwner>& owner) const;
  // Ends a drag of a component (mitcad#55), cancelled or where it is.
  void endOccurrenceDrag(const QPointF& position, bool cancelled);
  bool accepts(const occ::handle<SelectMgr_EntityOwner>& owner) const;
  // Selection scopes (mitcad#217): the kinds pickable now (the expanded
  // ones while Shift is held), Shift's state (`hover`: pick again where the
  // pointer is), whether a point of the design is behind a face of a shown
  // body seen from the camera, and which part of an edge a click (at its
  // picked point) or the last window (its samples inside) hit.
  SelectFilter activeKinds() const;
  void setShiftHeld(bool held, bool hover);
  bool occluded(const gp_Pnt& point) const;
  EdgeSight edgeSight(const occ::handle<SelectMgr_EntityOwner>& owner) const;
  EdgeSight windowSight(const TopoDS_Edge& edge, const gp_Trsf& toWorld) const;
  void sceneChanged();
  // Asks for a frame (update()). The view handles its events (picks,
  // camera animations) while it draws a frame; where Qt never exposes the
  // window, so that update() paints nothing (Windows session 0, where the
  // UI tests run over SSH), the frame is drawn directly instead.
  void requestFrame();
  // Trackpad navigation: a two-finger scroll (wheelEvent), and the pinch,
  // rotation and smart zoom gestures (event, every scheme).
  void trackpadScroll(QWheelEvent* event);
  void nativeGesture(QNativeGestureEvent* event);
  // Ends the drag that a scroll is passed to the view as (pan or orbit).
  void endScrollDrag();
  // Rolls the view about its direction of sight (clockwise positive, as Qt's rotation
  // gesture counts).
  void rollView(double degrees);
  // The arrow, or the sketch tool's precision cursor for the screen's ratio;
  // `log` tells which (for UI tests), not each snap.
  void updateCursor(bool log);

#if !defined(_WIN32) && !defined(Q_OS_MACOS)
  // Qt 6 makes the widget's context current on an offscreen pbuffer, but
  // OCCT's GLX backend needs an X11 window to make the context current on.
  // This hidden window is created with the context's own format, so the two
  // are compatible.
  std::unique_ptr<QWindow> m_glxWindow;
#endif
  occ::handle<V3d_Viewer> m_viewer;
  occ::handle<V3d_View> m_view;
  occ::handle<AIS_InteractiveContext> m_context;
  occ::handle<AIS_ViewCube> m_orientationCube;
  occ::handle<SelectMgr_Filter> m_pickFilter;
  std::map<std::string, BodyEntry> m_bodies;
  QStringList m_ghosts; // setGhosts()
  // Inactive ghosts (mitcad#420): the scope, the setting, the sketch
  // entities, profiles and datums drawn pale, and the log line last written.
  ActiveScope m_activeScope;
  bool m_ghostInactive = true;
  bool m_activePickPriority = true; // setActivePickPriority()
  std::set<const AIS_InteractiveObject*> m_inactiveObjects;
  QString m_loggedInactiveGhosts = QStringLiteral("none");
  std::map<ProfileKey, occ::handle<AIS_Shape>> m_profiles;
  std::vector<occ::handle<AIS_Shape>> m_sketchEntities;
  // The shown sketch entities by "<sketch>/<id>@<occurrence>", with their
  // signatures.
  std::map<QString, std::pair<QString, occ::handle<AIS_Shape>>> m_sketchEntityKeys;
  // How each of them is drawn, for restyleSketchEntities().
  std::map<const AIS_InteractiveObject*, SketchEntityLook> m_sketchEntityLooks;
  // Their halos (mitcad#302), not pickable, drawn before the entities.
  std::map<const AIS_InteractiveObject*, occ::handle<AIS_Shape>> m_sketchEntityHalos;
  // The halos as last logged ("Sketch halos:").
  QString m_loggedSketchHalos;
  // The shown sketches' colours as last logged ("Sketch colours:", for UI
  // tests; mitcad#286).
  QString m_loggedSketchColours;
  std::vector<occ::handle<AIS_Shape>> m_datums;
  std::vector<occ::handle<AIS_Shape>> m_highlights;
  // The whole bodies highlighted, as last logged ("View body highlights:",
  // for UI tests: the bodies a command's selection input holds).
  QString m_loggedBodyHighlights = QStringLiteral("none");
  // The profiles highlighted with their faces, as last logged ("View
  // profile highlights:", for UI tests: every piece of a split region,
  // mitcad#410).
  QString m_loggedProfileHighlights = QStringLiteral("none");
  // Where faces, edges, vertices and datums are highlighted, as last logged
  // ("View highlight places:", with MITCAD_LOG_PICKS; mitcad#432).
  QString m_loggedHighlightPlaces = QStringLiteral("none");
  // What each pickable object shows.
  std::map<const AIS_InteractiveObject*, PickInfo> m_pickInfo;
  occ::handle<AIS_Shape> m_preview;
  occ::handle<AIS_Shape> m_sketchPreview;
  occ::handle<Graphic3d_Camera> m_savedCamera;
  SelectFilter m_filter;
  // setPickFilter's test, or setPickScope's (m_scoped).
  PickTest m_pickTest;
  bool m_pickingSuspended = false;
  SelectFilter m_expandedFilter; // the kinds while Shift is held
  bool m_scoped = false;
  bool m_shiftHeld = false;
  bool m_contextPick = false; // a right click finds what a Shift+click would take
  // A window selection is being applied (from its release until the
  // selection is reported): its rectangle and direction.
  bool m_windowSelecting = false;
  QRectF m_windowRect;
  bool m_windowCrossing = false;
  gp_Ax3 m_sketchPlane;
  bool m_sketchInput = false;
  // The sketch tool's cursor (mitcad#3): its icon, and whether it snaps.
  QString m_sketchToolIcon;
  bool m_sketchSnapping = false;
  bool m_fitPending = false;
  bool m_directFrameQueued = false; // requestFrame() in an unexposed window
  bool m_painting = false;          // in paintGL()
  QString m_glVersion;              // GL_VERSION (glVersion())
  // The left press that starts a pick, and whether its release was a drag.
  QPointF m_pressPosition;
  Qt::KeyboardModifiers m_pressModifiers;
  bool m_windowPick = false;
  bool m_cubeClick = false;
  bool m_cubePress = false; // the left button went down on the cube
  QPointF m_rightPressPosition;
  bool m_orbitKey = false; // F4 held (MiddlePanF4: the left button orbits)
  // Dragging components (mitcad#55): the body pressed on, where, and
  // whether the pointer has moved far enough to drag.
  OccurrenceDrag m_occurrenceDrag;
  std::optional<SelectionItem> m_dragItem;
  gp_Pnt m_dragGrab;
  bool m_dragging = false;
  bool m_dragRefused = false; // the press is neither a drag nor a window selection
  bool m_dragCursor = false;
  // What the scrolling of a trackpad does until it pauses; a pan or an orbit
  // is a drag of the left button that is passed to the view in steps, so
  // that it goes the way of a mouse drag (m_scrollPoint: device pixels).
  enum class ScrollMode { None, Pan, Orbit, Zoom };
  ScrollMode m_scrollMode = ScrollMode::None;
  bool m_scrollDragging = false;
  QPointF m_scrollPoint;
  QTimer* m_scrollTimer = nullptr;

  // Display settings (U5).
  VisualStyle m_visualStyle = VisualStyle::ShadedVisibleEdges;
  bool m_componentColors = false;
  NavigationSettings m_navigation;
  Graphic3d_ZLayerId m_hiddenLayer = Graphic3d_ZLayerId_UNKNOWN;
  Graphic3d_ZLayerId m_edgeLayer = Graphic3d_ZLayerId_UNKNOWN;
  // Sketch entities' halos, just before the top layer (mitcad#302).
  Graphic3d_ZLayerId m_sketchHaloLayer = Graphic3d_ZLayerId_UNKNOWN;
  std::vector<occ::handle<AIS_Shape>> m_sectionCaps;
  std::vector<occ::handle<AIS_Shape>> m_sectionHatches;
  QStringList m_sectionLook;      // per cap group: colour, hatch colour, direction, lines
  QString m_loggedSectionLook;
  void logSectionCaps();
  occ::handle<AIS_Shape> m_threads;
  QColor m_paper = QColor(0xd8, 0xdc, 0xe2); // the hidden-line style's faces
  bool m_gridShown = true;
  double m_gridStep = 0.0; // mm; 0: follows the zoom
  occ::handle<LayoutGrid> m_grid;
  gp_Ax3 m_gridPlane;
  Graphic3d_ZLayerId m_gridLayer = Graphic3d_ZLayerId_UNKNOWN;
  std::optional<CameraState> m_home;
  ViewCubeHomeButton* m_homeButton = nullptr;
  bool m_nativeControls = false;
  bool m_backgroundDark = false; // the view's background is dark
  QColor m_backgroundTop = QColor(237, 240, 245);
  QColor m_backgroundBottom = QColor(189, 196, 209);
  bool m_backgroundGradient = true;
  // The rendered view: the image, the pixels OCCT reads from it, and the
  // background texture that shows them.
  QImage m_renderedImage;
  occ::handle<Image_PixMap> m_renderedPixels;
  occ::handle<Graphic3d_Texture2D> m_renderedTexture;
  bool m_renderedMode = false;
  // The camera and the window's size when the image was set.
  ViewPose m_renderedPose;
  // Navigation (isNavigating()): its state, its clock, and the timer that
  // ends a step's quiet time.
  ViewNavigation m_navigationState;
  QElapsedTimer m_navigationClock;
  QTimer* m_navigationTimer = nullptr;
  class RenderedImageQuad;
  occ::handle<RenderedImageQuad> m_renderedQuad;
  Graphic3d_ZLayerId m_renderedBodyLayer = Graphic3d_ZLayerId_UNKNOWN;
  Graphic3d_ZLayerId m_renderedImageLayer = Graphic3d_ZLayerId_UNKNOWN;
  QMargins m_overlayInsets;
  int m_badgeWidth = 0;
  std::vector<QRect> m_covered;
  std::vector<QRect> m_topCards; // setTopCards
  int m_topCardsBottom = 0;
  qreal m_cubeRatio = 1.0;       // the device pixel ratio applied to the cube
  double m_cubeBaseSize = 0.0;   // the cube's size and font height at ratio 1
  double m_cubeBaseFont = 0.0;
  // up, down, left, right, cw, ccw
  std::vector<std::pair<QString, QToolButton*>> m_cubeArrows;
  QTimer* m_restTimer = nullptr;
  QString m_loggedCamera;
  double m_loggedGridStep = -1.0;
  QString m_restingCamera; // the camera when the rest timer started
  // What the mouse was last over, for hovered().
  occ::handle<SelectMgr_EntityOwner> m_hoveredOwner;
  // Where the mouse was last on it, for hoverMoved(), and whether it moved
  // over the view since the last frame.
  std::optional<std::array<double, 3>> m_hoveredPoint;
  bool m_hoverMoved = false;
  // The hover's colour, and the rest of the hovered tangent chain
  // (setHoverChain, mitcad#290).
  QColor m_hoverColor = QColor(0x2f, 0x9b, 0xff);
  occ::handle<AIS_Shape> m_hoverChain;
};

} // namespace mitcad
