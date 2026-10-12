// SPDX-License-Identifier: MIT
#pragma once

#include <QHash>
#include <QStringList>
#include <QWidget>

class QCheckBox;
class QLabel;
class QRadioButton;

namespace mitcad {

class CommandRegistry;

namespace sketch {

class SketchController;

// The sketch palette, in the command panel's place while sketching:
// the sketch's degrees of freedom, display and snap options, the
// constraints, the drawing tool's auto constraints and Finish Sketch.
class SketchPalette : public QWidget {
  Q_OBJECT

public:
  // `constraints` are the ids of the constraint commands, in palette order.
  SketchPalette(SketchController& controller, CommandRegistry& registry, const QStringList& constraints,
                QWidget* parent = nullptr);

  // Logs where the options are, for UI tests.
  void logLayout() const;

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;

private:
  void refresh();
  // The Auto constraint section (mitcad#189): shown once the drawing tool
  // has drawn an entity, with the choices of its session.
  void refreshAuto();
  void logAutoLayout();

  SketchController& m_c;
  QLabel* m_title = nullptr;
  QLabel* m_status = nullptr;
  QLabel* m_tool = nullptr;
  QCheckBox* m_construction = nullptr;
  QCheckBox* m_grid = nullptr;
  QCheckBox* m_dimensions = nullptr;
  QCheckBox* m_constraints = nullptr;
  QCheckBox* m_profiles = nullptr;
  QCheckBox* m_hideAbove = nullptr;
  QWidget* m_autoSection = nullptr;
  QRadioButton* m_autoFirst = nullptr;
  QRadioButton* m_autoLast = nullptr;
  QHash<QString, QCheckBox*> m_autoChecks; // by constraint type
  QString m_autoLogged;                    // the section as last logged
};

} // namespace sketch
} // namespace mitcad
