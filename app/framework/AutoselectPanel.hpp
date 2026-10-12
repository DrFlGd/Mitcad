// SPDX-License-Identifier: MIT
#pragma once

#include <array>

#include <QGroupBox>
#include <QJsonArray>
#include <QJsonObject>

#include "Selection.hpp"

class QPushButton;

namespace mitcad {

// The model classifies the last unused sketch; the view supplies its placement.
struct AutoselectProfiles {
  // Projections, Drawings, Hybrid, All profiles, in display order.
  std::array<Selection, 4> groups;
  static AutoselectProfiles read(const QJsonObject& answer, const QString& occurrence);
  bool available() const { return !groups.back().isEmpty(); }
};

// The same XOR as a viewport Ctrl-click on every distinct candidate. Exact
// selection identity includes the occurrence and keeps unrelated items intact.
Selection toggleSelectionGroup(const Selection& selected, const Selection& candidates);

// Idle selections must use current region keys, not the model's historical
// aliases for feature definitions. Other selection kinds are kept unchanged.
Selection retainCurrentProfiles(const Selection& selected, const QJsonArray& profiles);

class AutoselectPanel : public QGroupBox {
  Q_OBJECT

public:
  explicit AutoselectPanel(QWidget* parent = nullptr);
  void setProfiles(const AutoselectProfiles& profiles);
  void setActive(bool active);
  bool available() const { return m_active && m_profiles.available(); }

signals:
  void toggleRequested(const mitcad::Selection& candidates);
  void previewChanged(const mitcad::Selection& candidates);
  void clearRequested();

protected:
  bool eventFilter(QObject* watched, QEvent* event) override;
  void hideEvent(QHideEvent* event) override;

private:
  void clearPreview();
  AutoselectProfiles m_profiles;
  std::array<QPushButton*, 4> m_buttons;
  bool m_active = false;
  int m_preview = -1;
};

} // namespace mitcad
