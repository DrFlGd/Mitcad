// SPDX-License-Identifier: MIT
#pragma once

#include <QColor>
#include <QString>
#include <QVector>

#include "DocumentSnapshot.hpp"

namespace mitcad {

// The colours of the components as the timeline shows them (the band under
// each of a component's features); the root has none. Shared by the timeline
// and the viewer's section caps (mitcad#382), so that they agree.
inline QColor componentColor(int index) {
  static const QColor colors[] = {QColor(0xe0, 0x89, 0x2a), QColor(0x3a, 0xa3, 0x5b), QColor(0x8e, 0x5b, 0xd0),
                                  QColor(0x2a, 0xa7, 0xa7), QColor(0xc9, 0x48, 0x3f), QColor(0x9a, 0x9a, 0x2a),
                                  QColor(0xc4, 0x5a, 0x9a), QColor(0x3d, 0x8b, 0xd9)};
  return colors[index % 8];
}

// A component's index into the palette: its position in the snapshot's
// components minus one (the root is first); -1 for the root or an unknown
// component.
inline int componentColorIndex(const QVector<DocumentSnapshot::Component>& components, const QString& uid) {
  for (int i = 1; i < components.size(); ++i) {
    if (components[i].uid == uid) {
      return i - 1;
    }
  }
  return -1;
}

} // namespace mitcad
