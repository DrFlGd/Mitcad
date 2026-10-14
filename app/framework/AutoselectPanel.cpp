// SPDX-License-Identifier: MIT
#include "AutoselectPanel.hpp"

#include <QEvent>
#include <QHash>
#include <QHideEvent>
#include <QPushButton>
#include <QSet>
#include <QVBoxLayout>

namespace mitcad {

AutoselectProfiles AutoselectProfiles::read(const QJsonObject& answer, const QString& occurrence) {
  AutoselectProfiles result;
  const QString sketch = answer.value(QStringLiteral("sketch")).toString();
  if (!answer.value(QStringLiteral("eligible")).toBool() || sketch.isEmpty()) {
    return result;
  }
  const std::array<const char*, 4> keys = {"projected", "drawn", "hybrid", "all"};
  for (std::size_t i = 0; i < keys.size(); ++i) {
    for (const QJsonValue& value : answer.value(QString::fromLatin1(keys[i])).toArray()) {
      const QString region = value.toString();
      const SelectionItem item{SelectKind::Profile, sketch, region, QStringLiteral("plane"), occurrence};
      if (!region.isEmpty() && !result.groups[i].contains(item)) {
        result.groups[i].append(item);
      }
    }
  }
  return result;
}

Selection toggleSelectionGroup(const Selection& selected, const Selection& candidates) {
  Selection result = selected;
  Selection handled;
  for (const SelectionItem& item : candidates) {
    if (!item.isValid() || handled.contains(item)) {
      continue;
    }
    handled.append(item);
    if (result.contains(item)) {
      result.removeAll(item);
    } else {
      result.append(item);
    }
  }
  return result;
}

Selection retainCurrentProfiles(const Selection& selected, const QJsonArray& profiles) {
  QHash<QString, QSet<QString>> current;
  for (const QJsonValue& value : profiles) {
    const QJsonObject profile = value.toObject();
    current[profile.value(QStringLiteral("sketch")).toString()].insert(
        profile.value(QStringLiteral("region")).toString());
  }
  Selection result;
  for (const SelectionItem& item : selected) {
    if (item.kind != SelectKind::Profile || current.value(item.owner).contains(item.name)) {
      result.append(item);
    }
  }
  return result;
}

AutoselectPanel::AutoselectPanel(QWidget* parent) : QGroupBox(tr("Autoselect"), parent) {
  setObjectName(QStringLiteral("autoselectPanel"));
  auto* layout = new QVBoxLayout(this);
  const std::array<const char*, 4> names = {"projected", "drawn", "hybrid", "all"};
  for (std::size_t i = 0; i < m_buttons.size(); ++i) {
    auto* button = new QPushButton(this);
    button->setObjectName(QStringLiteral("autoselect_") + QString::fromLatin1(names[i]));
    button->setProperty("nativeHoverEvents", true);
    button->setToolTip(tr("Toggle each profile in this group; keep other selections."));
    button->installEventFilter(this);
    connect(button, &QPushButton::clicked, this, [this, i] {
      if (available()) {
        emit toggleRequested(m_profiles.groups[i]);
      }
    });
    layout->addWidget(button);
    m_buttons[i] = button;
  }
  auto* clear = new QPushButton(tr("Clear selection"), this);
  clear->setObjectName(QStringLiteral("autoselect_clear"));
  connect(clear, &QPushButton::clicked, this, [this] {
    if (available()) {
      emit clearRequested();
    }
  });
  layout->addWidget(clear);
  setProfiles({});
}

void AutoselectPanel::setProfiles(const AutoselectProfiles& profiles) {
  clearPreview();
  m_profiles = profiles;
  const std::array<QString, 4> labels = {tr("Projections"), tr("Drawings"), tr("Hybrid"), tr("All profiles")};
  for (std::size_t i = 0; i < m_buttons.size(); ++i) {
    m_buttons[i]->setText(tr("%1 (%2)").arg(labels[i]).arg(m_profiles.groups[i].size()));
    m_buttons[i]->setVisible(!m_profiles.groups[i].isEmpty());
  }
  setVisible(available());
}

void AutoselectPanel::setSectionTitle(QWidget* title) {
  if (title == nullptr) {
    return;
  }
  setAccessibleName(title->accessibleName().isEmpty() ? this->title() : title->accessibleName());
  // No frame and no title of the group box's own: it paints nothing
  // (paintEvent). Not a style sheet, which would fix the palette of the
  // panel and its buttons at the theme it was set in.
  m_section = true;
  setTitle(QString());
  setFlat(true);
  auto* box = qobject_cast<QVBoxLayout*>(layout());
  if (box != nullptr) {
    box->setContentsMargins(0, 6, 0, 0);
    box->setSpacing(4);
    box->insertWidget(0, title);
  }
}

void AutoselectPanel::setActive(bool active) {
  m_active = active;
  if (!available()) {
    clearPreview();
  }
  setVisible(available());
}

bool AutoselectPanel::eventFilter(QObject* watched, QEvent* event) {
  for (std::size_t i = 0; i < m_buttons.size(); ++i) {
    if (watched == m_buttons[i]) {
      if (event->type() == QEvent::Enter && available() && !m_profiles.groups[i].isEmpty()) {
        m_preview = static_cast<int>(i);
        emit previewChanged(m_profiles.groups[i]);
      } else if (event->type() == QEvent::Leave && m_preview == static_cast<int>(i)) {
        clearPreview();
      }
      break;
    }
  }
  return QGroupBox::eventFilter(watched, event);
}

void AutoselectPanel::paintEvent(QPaintEvent* event) {
  if (m_section) {
    return; // the section title over the buttons is the panel's look
  }
  QGroupBox::paintEvent(event);
}

void AutoselectPanel::hideEvent(QHideEvent* event) {
  clearPreview();
  QGroupBox::hideEvent(event);
}

void AutoselectPanel::clearPreview() {
  if (m_preview != -1) {
    m_preview = -1;
    emit previewChanged({});
  }
}

} // namespace mitcad
