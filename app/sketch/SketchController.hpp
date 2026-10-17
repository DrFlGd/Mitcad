// SPDX-License-Identifier: MIT
#pragma once

// Sketch mode (U2): the sketch being edited, its drawing tools, snapping,
// the constraints and dimensions drawn over the view, dragging with the
// solver, and the values typed in the view. The model does the geometry
// (`sketch.*` commands); this class turns mouse and keyboard input into
// those commands, one undo step per user action.

#include <functional>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

#include <QHash>
#include <QJsonObject>
#include <QMouseEvent>
#include <QObject>
#include <QPointF>
#include <QPointer>
#include <QRectF>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QVector>

#include "../framework/CommandSession.hpp"
#include "AutoConstraint.hpp"
#include "SketchModel.hpp"
#include "SketchSnap.hpp"

class QLabel;
class QLineEdit;

namespace mitcad {

class OcctViewer;
class CommandRegistry;

// What sketch mode needs from the main window besides a command host.
class SketchHost : public CommandHost {
public:
  virtual const Selection& selection() const = 0;
  virtual void setSelection(const Selection& items) = 0;
  // A click on items, with the selection rules (Ctrl and Shift add or
  // remove).
  virtual void pick(const Selection& items, Qt::KeyboardModifiers modifiers) = 0;
  virtual void showError(const QString& message) = 0;
  // Starts a command of the registry (an edit panel opened by a
  // double-click).
  virtual void trigger(const QString& id) = 0;
  // Whether sketch mode's selection filter (mitcad#217) lets a click take
  // an item of the edited sketch that sketch mode itself picks: its
  // dimensions and constraints, and its curves and points a press would
  // drag. `expanded`: Shift is held.
  virtual bool sketchClickable(const SelectionItem& /*item*/, bool /*expanded*/) const { return true; }
};

namespace sketch {

class SketchOverlay;
class SketchTool;

// A value typed in the view while drawing (heads-up input).
struct FieldSpec {
  enum class Kind { Length, Angle, Count, Text };
  QString id;
  QString label;
  Kind kind = Kind::Length;
};

// Entities that a picking tool or a drag accepts.
enum EntityFilter : unsigned {
  PickPoints = 1u << 0,
  PickCurves = 1u << 1,
  PickAll = PickPoints | PickCurves,
  // Also the body edges and vertices in the sketch plane (mitcad#256), by
  // their BodyGeometry ids; the sketch's own entities win ties.
  PickBody = 1u << 2,
};

class SketchController;

// One user action: several model commands that undo as one step, with
// the constraints that keep new points where they snapped.
class SketchOp {
public:
  explicit SketchOp(SketchController& controller);
  ~SketchOp();
  SketchOp(const SketchOp&) = delete;
  SketchOp& operator=(const SketchOp&) = delete;

  // A `sketch.*` command of the edited sketch.
  QJsonObject command(const QString& name) const;
  // Runs a command; false, keeping the model's reason, when it is refused.
  bool run(const QJsonObject& command, QJsonObject* result = nullptr);
  // A command that may be refused, such as a constraint a snap or an
  // inference adds: the action goes on without it, and the reason is shown
  // as a hint when the action ends (mitcad#287).
  bool attempt(const QJsonObject& command);
  // An existing point where the cursor snapped to one (made at the origin
  // when it snapped there; a body vertex's projected point, mitcad#258), else
  // the position.
  QJsonValue pointInput(const Snap& snap);
  // The constraints that keep `point` where it snapped (on a body vertex or
  // at a body edge's middle: on the projected vertex or edge, projected
  // first).
  void constrain(const QString& point, const Snap& snap);
  void addConstraint(const QJsonObject& constraint);
  // A dimension with a typed value; refused ones are left out.
  void addDimension(const QJsonObject& dimension, const QString& value);
  // A body edge or vertex in the sketch plane projected into the sketch,
  // linked, for the dimension or point the action makes from it; or the
  // entities of an existing projection of the same source that still match
  // it, and within one action the projection made before (mitcad#256,
  // mitcad#258). Empty when the model refused the projection (error()).
  ProjectedItem projectBody(const BodyItem& item);
  // The sketch after the commands so far.
  const SketchModel& model();
  const QString& error() const { return m_error; }
  // Ends the action as one undo step and shows the result; or takes it
  // all back.
  bool commit();
  void rollback();

private:
  // Logs the constraints a new one replaced (mitcad#287) and keeps them
  // for the hint at the end.
  void noteImplied(const QJsonObject& command, const QJsonObject& answer);

  SketchController& m_c;
  int m_depth = 0;
  QString m_label;
  QString m_error;
  // What the action is to tell when it ends: refused attempts, constraints
  // a new one replaced (mitcad#287).
  QStringList m_notices;
  bool m_finished = false;
  bool m_stale = false;
  QHash<QString, ProjectedItem> m_projected; // by source, body and occurrence
};

class SketchController : public QObject {
  Q_OBJECT

public:
  SketchController(SketchHost& host, OcctViewer& viewer, CommandRegistry& registry, QObject* parent = nullptr);
  ~SketchController() override;

  // --- Sketch mode
  void enter(const QString& uid);
  void leave();
  bool isActive() const { return !m_uid.isEmpty(); }
  const QString& uid() const { return m_uid; }
  const SketchModel& model() const { return m_model; }
  // Reads the sketch again after the model changed.
  void reload();
  // While a command's panel runs in the sketch, its picks are the view's:
  // no tools, drags or picking of constraints and dimensions.
  void setPaused(bool paused) {
    m_paused = paused;
    if (!paused) {
      setSpacingPreview({});
      setStalePreview({});
    }
  }
  // Turns the view to look at the sketch plane.
  void lookAt();
  // Logs where the sketch origin and axes are in the window, when that
  // changed ("Sketch view x,y x,y x,y" for (0, 0), (100, 0) and (0, 100)).
  void logView();

  // --- Tools
  void startTool(std::unique_ptr<SketchTool> tool);
  void stopTool();
  // The icon of the tool command named `toolName` (as its tool names
  // itself), which the tool's cursor shows (mitcad#3).
  void setToolIcon(const QString& toolName, const QString& icon) { m_toolIcons.insert(toolName, icon); }
  SketchTool* tool() const { return m_tool.get(); }
  // Enter and Esc; false when sketch mode had nothing to do with them.
  bool confirm();
  bool cancel();
  // Takes back what the tool has half done (before an undo); the tool
  // stays.
  void resetTool();
  // Commands on the selection.
  void deleteSelection();
  void toggleConstruction();
  void toggleCenterline();
  // Convert to Sketch Geometry (mitcad#257): the selected projected curves
  // and points into ordinary free sketch geometry; whether the selection
  // holds any (the command is enabled only then).
  void convertProjected();
  bool selectionHasProjected() const;
  void updateConvertAction();
  void toggleFixed();
  void editDimension(const QString& id);

  // --- Options of the sketch palette
  bool construction() const { return m_construction; }
  void setConstruction(bool on);
  bool gridSnap() const { return m_gridSnap; }
  void setGridSnap(bool on);
  // The grid's fixed spacing (mm), or 0: snap steps follow the zoom (U5).
  void setGridStep(double step) { m_gridStep = step; }
  bool showDimensions() const { return m_showDimensions; }
  void setShowDimensions(bool on);
  bool showConstraints() const { return m_showConstraints; }
  void setShowConstraints(bool on);
  bool showProfiles() const { return m_showProfiles; }
  void setShowProfiles(bool on);
  // The bodies shown cut at the sketch plane, without what is in front of
  // it (on its normal's side), so that the sketch is not hidden.
  bool hideAbove() const { return m_hideAbove; }
  void setHideAbove(bool on);
  // What a dimension shows (mitcad#231): its value, its parameter's name or
  // both; a UI preference (QSettings sketch/dimensionText), not in the design.
  enum class DimensionText { Value, Name, NameValue };
  DimensionText dimensionTextMode() const { return m_dimensionText; }
  void setDimensionTextMode(DimensionText mode);

  // --- Auto constraints while drawing (mitcad#189): the session of the
  // active drawing tool, which the palette's Auto constraint section shows.
  const AutoConstraintSession& autoConstraints() const { return m_auto; }
  void setAutoLink(AutoConstraintSession::Link link);
  void setAutoConstraint(const QString& type, bool on);
  // The session's reference among the entities that still exist; empty
  // before the first entity (or when undo took them all back).
  QString autoReference() const;
  // Whether the palette shows the Auto constraint section: a tool that
  // takes auto constraints has drawn an entity that still exists.
  bool autoConstraintsShown() const;
  // The reference and the checked types, when the next entity gets any.
  std::optional<AutoPlan> autoPlan() const;
  // The pointer is over the palette's section: the reference is
  // highlighted.
  void setAutoHover(bool on);
  bool autoHover() const { return m_autoHover; }
  // Adds the checked constraints between the reference and the entity
  // `made` in `op`. When some are refused, asks whether to keep the entity
  // with the others: false when the user cancels (the caller then rolls
  // back `op`).
  bool applyAutoConstraints(SketchOp& op, const QString& made, const QString& what);
  // A drawing tool made `made` (after its op's commit): the session's new
  // last entity.
  void recordAutoEntity(const QString& made);

  // --- For tools and the overlay
  SketchHost& host() { return m_host; }
  const SketchHost& host() const { return m_host; }
  OcctViewer& viewer() { return m_viewer; }
  QPointF toScreen(const V2& p) const;
  // The sketch plane point under a view position.
  std::optional<V2> sketchAt(const QPointF& screen) const;
  // Millimetres per pixel near the sketch origin.
  double pixel() const;
  const Snap& snapped() const { return m_snap; }
  const QPointF& cursor() const { return m_cursor; }
  bool cursorInView() const { return m_cursorValid; }
  // The pointer's place in sketch coordinates and the document's length
  // unit ("x 12.00, y -29.00 mm"; where a tool snaps it), while the pointer
  // is over the sketch plane; empty otherwise (mitcad#254).
  const QString& pointerText() const { return m_pointerText; }
  // The point or curve under a view position.
  QString entityAt(const QPointF& screen, unsigned filter = PickAll) const;
  // Highlights for picking tools.
  void setHover(const QString& entity);
  const QString& hover() const { return m_hover; }
  void setPicked(const QStringList& entities);
  const QStringList& picked() const { return m_picked; }
  QString hoverAnnotation() const { return m_hoverAnnotation; }
  // The dimensions and constraints a window holds (view pixels; with
  // `crossing` also those it touches), as selection items (mitcad#217).
  Selection annotationsIn(const QRectF& area, bool crossing) const;
  // Whether a dimension or a constraint's symbol is at a place in the view
  // (view pixels): a right click there is for it, not for the geometry
  // under it.
  bool annotationAt(const QPointF& position) const;
  // A dimension's text being dragged.
  std::optional<std::pair<QString, V2>> textOverride() const { return m_textOverride; }
  void refreshView();
  void hint(const QString& message);
  // A hint about what an action did (a constraint refused or replaced,
  // mitcad#287), logged, and kept in front of the next prompt hint so a
  // tool's prompt does not hide it at once.
  void notice(const QString& message);
  void error(const QString& message);

  // Values typed in the view.
  void setFields(const QVector<FieldSpec>& fields);
  void clearFields();
  bool typed(const QString& id) const;
  // The typed value (mm, radians, a count) when it is valid.
  std::optional<double> fieldValue(const QString& id) const;
  // The typed expression as the model takes it ("20" gives "20 mm").
  QString fieldExpression(const QString& id) const;
  QString fieldText(const QString& id) const;
  // Shows what the cursor gives, while nothing is typed.
  void setLive(const QString& id, double value);
  // A value editor at a view position (a dimension's value).
  void openValueEditor(const QPointF& at, const QString& text, FieldSpec::Kind kind,
                       std::function<void(const QString&)> accept,
                       const QString& name = QString());
  // Inserts a parameter's name at the open value editor's cursor.
  void insertIntoEditor(const QString& name);
  bool valueEditorOpen() const { return m_editor != nullptr; }

  // Evaluates an expression; the expression the model takes ("20 mm") and
  // its value, or the reason it is invalid.
  bool evaluate(const QString& text, FieldSpec::Kind kind, QString& expression, double& value,
                QString& problem) const;
  QString formatLength(double mm) const;
  QString formatAngle(double radians) const;
  const QString& lengthUnit() const { return m_unit; }
  // Millimetres per document length unit.
  double lengthScale() const { return m_unitScale; }

  // Model access for SketchOp: throws the model's error.
  QJsonObject runModel(const QJsonObject& command);
  int undoDepth() const;
  QString undoLabel() const;

  // Equal spacing (mitcad#221): the objects an Equal Spacing panel has
  // (its preview's chain is drawn numbered, in the order the model keeps).
  void setSpacingPreview(const QStringList& anchors);
  const QStringList& spacingPreview() const { return m_spacingPreview; }

  // Clean Up Stale Projections (mitcad#252): what its preview removes and
  // trims, from the command's answer (an empty object: nothing). The
  // overlay draws the removed curves as they were and the pieces cut off in
  // the remove colour, and highlights the trimmed lines, until the panel
  // ends.
  struct StalePreview {
    std::vector<Curve2> removed;
    QStringList trimmed;
    std::vector<std::pair<V2, V2>> cut;
    bool empty() const { return removed.empty() && trimmed.isEmpty() && cut.empty(); }
  };
  void setStalePreview(const QJsonObject& answer);
  const StalePreview& stalePreview() const { return m_stalePreview; }

  // Selects curves and points of the edited sketch by id, in place of the
  // selection, as clicks would (Delete and the other selection commands
  // take them); ids the sketch lacks are left out. With `reveal` the view
  // pans to them when none is in sight, its zoom kept unless they do not
  // fit; `fit` zooms to them anyway (SketchReveal.cpp: the Sketch Palette's
  // warnings, mitcad#264; live MCP's ui_select, mitcad#265). Returns how
  // many it selected.
  int selectEntities(const QStringList& ids, bool reveal, bool fit = false);
  // Body edges and vertices in the sketch plane (mitcad#256, mitcad#258), for
  // the tool that picks or snaps to them: the model's plane_geometry query,
  // read once per tool session and again after the document changed.
  const BodyGeometry& bodyGeometry();
  // As last read, for drawing (no query).
  const BodyGeometry& bodyGeometryShown() const { return m_body; }
  // Changes whenever the sketch is read again (a tool's view of it is
  // stale then).
  int modelRevision() const { return m_modelRevision; }

signals:
  // The sketch or its status changed (the palette shows the DOF).
  void changed();
  // The option toggles changed.
  void optionsChanged();
  void toolChanged(const QString& name);
  // The auto constraint session changed: a new tool, a choice, an entity.
  void autoConstraintsChanged();

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  struct Field {
    FieldSpec spec;
    QPointer<QLineEdit> edit;
    bool typed = false;
    bool valid = false;
    double value = 0.0;
    QString expression;
  };

  bool viewerEvent(QEvent* event);
  bool fieldEvent(QLineEdit* edit, QEvent* event);
  void updateCursor(const QPointF& position);
  void placeFields();
  void fieldEdited(int index);
  void acceptEditor();
  void closeEditor();
  void selectAnnotation(const QString& kind, const QString& id, Qt::KeyboardModifiers modifiers);
  void startDrag();
  void applyDrag();
  void endDrag();
  // A modal dialog took the input mid-drag (a long computation's progress,
  // P7), and with it the release: the drag ends when the model is free.
  void interruptDrag();
  void forwardClick(const QMouseEvent* release);
  int fieldIndex(const QString& id) const;
  QString describeStatus() const;

  CommandRegistry& m_registry;
  SketchHost& m_host;
  OcctViewer& m_viewer;
  QString m_uid;
  SketchModel m_model;
  QString m_unit = QStringLiteral("mm");
  double m_unitScale = 1.0; // mm per unit
  QString m_status;         // last logged status
  QString m_loggedProjections; // the linked projections last logged
  QString m_loggedView;     // last logged view placement
  SketchOverlay* m_overlay = nullptr;
  std::unique_ptr<SketchTool> m_tool;
  QHash<QString, QString> m_toolIcons; // by tool name

  bool m_paused = false;
  bool m_construction = false;
  bool m_gridSnap = true;
  double m_gridStep = 0.0;
  bool m_showDimensions = true;
  bool m_showConstraints = true;
  bool m_showProfiles = true;
  bool m_hideAbove = false;

  AutoConstraintSession m_auto;
  bool m_autoHover = false;
  mutable QString m_autoTarget; // the reference last logged as a target

  Snap m_snap;
  QPointF m_cursor;
  bool m_cursorValid = false;
  // The last notice(), until the next prompt hint has shown it.
  QString m_notice;
  // The pointer's place (pointerText()), the last one shown, and the one
  // last logged ("Sketch pointer x 12.00, y -29.00 mm", once the pointer
  // rests: logPointer()).
  QString m_pointerText;
  QString m_movedPointer;
  QString m_loggedPointer;
  QTimer m_pointerTimer;
  QString pointerTextAt(const V2& at) const;
  void setPointerText(const QString& text);
  void logPointer();
  QString m_hover;
  QStringList m_picked;
  QString m_hoverAnnotation;

  // A press on geometry: a click (selection) or the start of a drag.
  bool m_pressed = false;
  bool m_dragging = false;
  bool m_forwarding = false;
  QPointF m_pressPosition;
  QPointF m_rightPressPosition;
  Qt::KeyboardModifiers m_pressModifiers;
  QString m_dragEntity;
  QString m_dragDimension; // a dimension's text
  V2 m_dragStart;
  V2 m_dragLast;
  V2 m_dragTarget;
  int m_dragDepth = 0;
  int m_dragSteps = 0;
  bool m_dragPending = false;
  std::optional<std::pair<QString, V2>> m_textOverride;
  QTimer m_dragTimer;

  QVector<Field> m_fields;
  QPointer<QLineEdit> m_editor;
  FieldSpec::Kind m_editorKind = FieldSpec::Kind::Length;
  std::function<void(const QString&)> m_editorAccept;
  std::optional<int> m_editorToolSession; // a callback belonging to this tool session
  QPointer<QLabel> m_editorLabel; // "d3 =" before the editor (mitcad#231)
  QString m_editorDimension;       // the dimension it edits
  QString m_editorText;            // the text it opened with
  QString m_tooltipDimension;      // the dimension whose tooltip is shown
  // Logs what each dimension shows ("Dimension texts: k5 d1 = 60.00, ...").
  void logDimensionTexts() const;
  // Shows the tooltip of the dimension under the pointer (empty: none).
  void showDimensionTooltip(const QString& dimension, const QPointF& at);
  DimensionText m_dimensionText = DimensionText::Value;

  // Equal spacing (mitcad#221): a running panel's objects, and the chains
  // last logged (by constraint).
  QStringList m_spacingPreview;
  QHash<QString, QString> m_loggedSpacings;
  void logSpacings();

  // Clean Up Stale Projections' preview (mitcad#252).
  StalePreview m_stalePreview;

  // Body edges and vertices in the sketch plane (mitcad#256): what was read,
  // for which sketch, document revision and tool session.
  BodyGeometry m_body;
  QString m_bodyKey;
  QString m_revision; // the document's, when the sketch was read
  int m_toolSession = 0;
  int m_modelRevision = 0;
};

} // namespace sketch
} // namespace mitcad
