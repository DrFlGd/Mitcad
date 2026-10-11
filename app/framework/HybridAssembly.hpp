// SPDX-License-Identifier: MIT
#pragma once

#include <functional>

#include <QJsonArray>
#include <QJsonObject>
#include <QString>

class QWidget;

namespace mitcad {

// Hybrid assemblies (mitcad#171): a component made in the root while the
// root has modelled bodies of its own. Before such a component is made,
// Mitcad offers to move the root's modelled features into a component
// first (the model's `move_to_component`), so that the root keeps only
// components; the choice can be remembered per design
// (`set_hybrid_assembly`).

// What is about to make a component.
struct ComponentRequest {
  enum class Kind {
    NewComponent, // an empty component (New Component)
    FromBodies,   // Create Components from Bodies
    Feature,      // a feature's new-component operation
  };
  Kind kind = Kind::NewComponent;
  // The component it is made in (C0: the root); FromBodies: unused.
  QString parent;
  // FromBodies: the bodies, by uid.
  QJsonArray bodies;
};

struct ComponentAnswer {
  enum class Outcome {
    Proceed, // make it as asked
    Done,    // the move made the components already (FromBodies)
    Cancel,  // make nothing
  };
  Outcome outcome = Outcome::Proceed;
  // The component the root's features went into, when they moved.
  QString moved;
};

// The model calls the question makes.
struct HybridModel {
  // A query's answer; throws when the model refuses it.
  std::function<QJsonObject(const QJsonObject&)> query;
  // Runs a command (its own undo step) and shows why when the model
  // refuses it.
  std::function<bool(const QJsonObject&, QJsonObject*)> run;
};

// Asks, when the request makes a component in a root with modelled bodies
// and the design does not allow that without asking: Move into a New
// Component (every modelled feature of the root goes into one; for
// Create Components from Bodies, each selected body with the features that
// made it into its own), Continue as Is, or Cancel, with "Don't ask again
// for this design".
ComponentAnswer askHybridAssembly(QWidget* parent, const HybridModel& model, const ComponentRequest& request);

// Move to Component: `selection` is the model's (`component`, `bodies`,
// `features`, `groups`; empty: every modelled feature of the root). Lists
// the features that tie bodies to others and offers to keep those
// together. True when the features moved.
bool moveToComponent(QWidget* parent, const HybridModel& model, const QJsonObject& selection);

} // namespace mitcad
