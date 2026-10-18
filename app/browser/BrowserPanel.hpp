// SPDX-License-Identifier: MIT
#pragma once

#include <QHash>
#include <QPoint>
#include <QRegion>
#include <QSet>
#include <QString>
#include <QStringList>
#include <QVector>
#include <QWidget>

#include "../framework/Selection.hpp"
#include "DocumentSnapshot.hpp"

class QTreeWidgetItem;

namespace mitcad {

class BrowserTree;

// A row of the browser.
struct BrowserNode {
  enum class Type {
    None,
    Settings,   // Document Settings
    Units,      // the document's length unit
    NamedViews, // Named Views
    NamedView,  // uid: home, top, front, right; the document's: named:<name>
    Origin,     // the root's Origin folder
    OriginDatum, // uid: xy, xz, yz, x, y, z, origin
    Component,  // the root (occurrence empty) or an occurrence; uid: the component
    Bodies,     // folders of a component (in an occurrence)
    Sketches,
    Construction,
    Body,   // uid: the body
    Sketch, // uid: the sketch feature
    Datum,  // uid: the construction feature
    Analyses, // the Analysis folder (mitcad#41)
    Analysis, // uid: its name
    Joints,   // a component's joints (mitcad#55); joint origins are Datum rows in it
    Joint,    // uid: a joint, as-built joint or rigid group
    // A feature under a sketch it uses (mitcad#177): uid: the feature,
    // sketch: that sketch; one row under each sketch it uses.
    Feature,
    // Sheet metal (mitcad#210), under a sheet-metal body's row: its rules
    // (uid: the rules feature) and its Flat Pattern (uid: the body; the
    // light bulb shows it flat in place of the folded body).
    SheetRules,
    FlatPattern,
  };

  Type type = Type::None;
  QString uid;
  QString occurrence; // the occurrence path ("O1/O4"); empty in the root
  QString component;  // the component the row belongs to
  QString featureType; // sketches, datums and features: construction_plane, ...
  QString sketch;     // a feature row: the sketch it is under
  QString name;       // as shown
  QString path;       // the names from the root, "Root/Bodies/Body1"
  bool visible = true; // its light bulb
  bool hasEye = false;
  // A body: a mesh body (mitcad#399); its item's geometry is "mesh".
  bool mesh = false;

  bool isValid() const { return type != Type::None; }
  bool isFolder() const {
    return type == Type::Bodies || type == Type::Sketches || type == Type::Construction ||
           type == Type::Origin || type == Type::Analyses || type == Type::Joints;
  }
  // The selection item the row stands for; invalid for folders and settings.
  SelectionItem item() const;
};

// The browser, docked on the left: the document settings, named
// views, the origin and the component tree with each occurrence's bodies,
// sketches (each with the features that use it under it, mitcad#177),
// construction geometry and joints, and the analyses kept in the document.
// A row reads: expand arrow, light bulb (an eye; shows and hides), icon,
// name and, for a component, the radio button that activates it. Selecting
// rows selects in the view and the timeline, and the other way round; a
// slow second click on a selected row renames it in place, as F2 does.
// Ctrl+F (Cmd+F) opens the timeline's search (mitcad#179). What the rows
// do is the BrowserController's.
class BrowserPanel : public QWidget {
  Q_OBJECT

public:
  explicit BrowserPanel(QWidget* parent = nullptr);

  // Toolkit-independent row identities and hierarchy for other
  // presentations of the same browser actions.
  struct PresentationRow {
    QString key;
    QString parent;
    BrowserNode node;
    QString tooltip;
  };
  QVector<PresentationRow> presentationRows() const;

  // Rebuilds the rows, keeping what was expanded, selected and scrolled to.
  void rebuild(const DocumentSnapshot& snapshot, bool originShown);
  // Selects the rows of these items (from the view) and the feature rows
  // of these features (picked in the timeline or here); others are
  // deselected. Emits nothing. With scroll, it scrolls to the newest
  // selected row when none of them is in sight (mitcad#318); a selection
  // picked in the tree itself does not scroll it.
  void showSelection(const Selection& items, const QStringList& features = {}, bool scroll = true);
  // The rows of a feature: its sketch or datum, or the bodies it made.
  QVector<BrowserNode> nodesOfFeature(const QString& feature) const;
  // Expands to, selects and scrolls to the rows of a feature, its rows under
  // its sketches too; false when it has none (Find in Browser).
  bool reveal(const QString& feature);
  // The same for the row of a body (of a face, edge or vertex picked in
  // the view), a sketch or a datum.
  bool revealItem(const SelectionItem& item);
  // Collapses every row, the descendants of a folded row too, and forgets
  // what was open: the remembered states of all rows are "folded", so neither
  // an arrow click (which opens only that row) nor a rebuild reopens a
  // subtree (mitcad#312). The top row stays, to be opened again. Only the
  // presentation changes.
  void collapseAll();
  // Renames a row in place.
  void startRename(const BrowserNode& node);
  // Closes an open rename editor without renaming; false when none is open.
  bool cancelEditing();
  // Logs where the rows are, for UI tests (when it changed): "Browser <path>
  // at x,y" for a row whose middle is in sight (also "Browser eye ...",
  // "Browser radio ..."), and "<the same> out of sight" once, for a place
  // logged before whose row has left it (scrolled away, folded, gone), so
  // that a test never clicks a place that is not valid any more. Logged again
  // when the tree scrolls and when the panel moves in its window.
  void logLayout();
  // The height of all the rows in sight (a floating card is as high as
  // they, up to a share of the view).
  int contentHeight() const;
  // A compact width, which the dock opens with (mitcad#10).
  QSize sizeHint() const override;

  // The Rows background of the Floating layout (mitcad#419,
  // BrowserBackground.hpp): no card behind the tree; each row in sight on a
  // small box of the card's material, from its arrow to the end of its name
  // or radio button, the selection and the pointer shown on the box. A place
  // beside the rows is logged for UI tests ("Browser view beside the rows at
  // x,y", logLayout).
  void setRowsBackground(bool rows);
  bool rowsBackground() const { return m_rows; }
  // With the Rows background, where the panel takes the mouse, in its own
  // coordinates: the rows' boxes in sight, the scroll bars shown and an
  // open rename editor.
  QRegion rowsRegion() const;

signals:
  // With the Rows background, rowsRegion changed (the rows scrolled, opened,
  // folded, were rebuilt, renamed or selected; the panel was resized).
  void rowsRegionChanged();
  // The rows in sight changed in number.
  void contentHeightChanged();
  // Rows were selected: the items of the rows that stand for one, and the
  // features of the feature rows under sketches (mitcad#177).
  void rowsPicked(const mitcad::Selection& items, const QStringList& features);
  // The items of rows another presentation selected.
  void itemsPicked(const mitcad::Selection& items);
  void eyeClicked(const mitcad::BrowserNode& node);
  void activateClicked(const mitcad::BrowserNode& node);
  void renamed(const mitcad::BrowserNode& node, const QString& name);
  // A slow second click on a selected row that can be renamed (mitcad#177).
  void renameClicked(const mitcad::BrowserNode& node);
  void doubleClicked(const mitcad::BrowserNode& node);
  void deleteRequested(const mitcad::BrowserNode& node);
  void contextMenuRequested(const mitcad::BrowserNode& node, const QPoint& globalPosition);
  // Ctrl+F (Cmd+F on macOS) in the rows: the timeline's search (mitcad#179).
  void searchRequested();

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  friend class BrowserTree;

  // With the Rows background: rowsRegionChanged once the changes in hand
  // are laid out.
  void queueRowsRegion();

  // The features under each sketch (mitcad#177), by the sketch's uid, and
  // the components each feature's bodies are in, by the feature's uid.
  struct SketchUses {
    QHash<QString, QVector<const DocumentSnapshot::Feature*>> features;
    QHash<QString, QSet<QString>> resultComponents;
  };

  QTreeWidgetItem* addRow(QTreeWidgetItem* parent, const BrowserNode& node, const QString& icon,
                          const QString& tooltip = QString());
  void addComponentRows(QTreeWidgetItem* parent, const DocumentSnapshot& snapshot,
                        const QString& component, const QString& occurrence, bool shown,
                        const SketchUses& uses);
  // The rows of the features that use a sketch, under its row (mitcad#177).
  void addFeatureRows(QTreeWidgetItem* parent, const DocumentSnapshot& snapshot,
                      const DocumentSnapshot::Feature& sketch, const QString& occurrence, bool shown,
                      const SketchUses& uses);
  // A sheet-metal body's rules and Flat Pattern under its row (mitcad#210).
  void addSheetRows(QTreeWidgetItem* bodyRow, const DocumentSnapshot& snapshot, const BrowserNode& body,
                    bool shown);
  // A constraint set's rows under its row (mitcad#148).
  void addConstraintRows(const DocumentSnapshot& snapshot, const DocumentSnapshot::Feature& feature,
                         QTreeWidgetItem* parent, bool shown);
  BrowserNode nodeOf(const QTreeWidgetItem* item) const;
  QString keyOf(const BrowserNode& node) const;
  bool revealKeys(const QSet<QString>& keys);
  void selectionChanged();
  void itemEdited(QTreeWidgetItem* item, int column);
  void logSelection();

  BrowserTree* m_tree = nullptr;
  QHash<QTreeWidgetItem*, BrowserNode> m_nodes;
  QHash<QString, bool> m_expanded; // by row key
  QSet<QString> m_shownSelection;  // what showSelection last selected (items and features)
  QSet<QString> m_foldedSelection; // the selection Collapse All left; showSelection does not reveal it again
  bool m_building = false;
  QString m_logged;
  QSet<QString> m_loggedPlaces; // what logLayout logged a place for, and has not logged out of sight
  QString m_loggedSelection;
  bool m_rows = false; // the Rows background (mitcad#419)
  bool m_rowsQueued = false;
};

} // namespace mitcad
