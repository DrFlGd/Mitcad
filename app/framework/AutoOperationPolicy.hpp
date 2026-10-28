// SPDX-License-Identifier: MIT
#pragma once

namespace mitcad {

// Whether a command's Operation and Objects follow the model's automatic
// choice (mitcad#289, Extrude's CommandDef::autoOperation). A new feature
// follows it until the user picks Operation, Objects or Affected
// components. An edit never does: an edited feature keeps the operation
// stored with it, also when Flip, Start or a distance moves its tool into
// a body or out of it, and the user changes Operation by hand (the owner,
// mitcad#868, 28.10.2026; it replaces mitcad#575's edits that followed the
// geometry when the stored operation matched the automatic choice, which
// turned an edited boss's Join into a Cut when Flip pointed it into its
// arm).
class AutoOperationPolicy {
public:
  void start(bool enabled, bool editing) { m_active = enabled && !editing; }
  bool active() const { return m_active; }
  // The user's own choice stays for the rest of the command.
  void disable() { m_active = false; }

private:
  bool m_active = false;
};

} // namespace mitcad
