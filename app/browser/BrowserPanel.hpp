// SPDX-License-Identifier: MIT
#pragma once

#include <QHash>
#include <QPoint>
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
// What the rows do is the BrowserController's.
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
  // deselected. Emits nothing.
  void showSelection(const Selection& items, const QStringList& features = {});
  // The rows of a feature: its sketch or datum, or the bodies it made.
  QVector<BrowserNode> nodesOfFeature(const QString& feature) const;
  // Expands to, selects and scrolls to the rows of a feature, its rows under
  // its sketches too; false when it has none (Find in Browser).
  bool reveal(const QString& feature);
  // The same for the row of a body (of a face, edge or vertex picked in
  // the view), a sketch or a datum.
  bool revealItem(const SelectionItem& item);
  // Renames a row in place.
  void startRename(const BrowserNode& node);
  // Closes an open rename editor without renaming; false when none is open.
  bool cancelEditing();
  // Logs where the rows are, for UI tests (when it changed).
  void logLayout();
  // The height of all the rows in sight (a floating card is as high as
  // they, up to a share of the view).
  int contentHeight() const;
  // A compact width, which the dock opens with (mitcad#10).
  QSize sizeHint() const override;

signals:
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

private:
  friend class BrowserTree;

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
  bool m_building = false;
  QString m_logged;
  QString m_loggedSelection;
};

} // namespace mitcad
