// SPDX-License-Identifier: MIT
#pragma once

// 3D Print (mitcad#13): bodies to a slicer as one 3MF file (an object with
// a part per body) or as an STL file per body, written into a folder of
// their own and given to the slicer in one start. Here: the slicers found
// on the system, the settings, file and folder names, the panel's rows and the
// Preferences group; MainWindowPrint.cpp defines and runs the command.

#include <QGroupBox>
#include <QJsonArray>
#include <QPair>
#include <QString>
#include <QStringList>
#include <QVector>

#include "../framework/Selection.hpp"

class QComboBox;
class QWidget;

namespace mitcad {

// A slicer that takes the files on its command line: `program` with
// `arguments`, the files, then `argumentsAfter` (a Flatpak: flatpak run
// --file-forwarding <id> @@ <files> @@, so that the sandbox sees them).
struct Slicer {
  QString name;
  QString program;
  QStringList arguments;
  QStringList argumentsAfter;

  bool isValid() const { return !program.isEmpty(); }
  bool operator==(const Slicer& other) const {
    return program == other.program && arguments == other.arguments && argumentsAfter == other.argumentsAfter;
  }
  bool operator!=(const Slicer& other) const { return !(*this == other); }
  // "Bambu Studio (C:\...\bambu-studio.exe)", for lists and logs.
  QString describe() const;
  // The program, or the application bundle `open -a` starts (macOS).
  QString location() const;
};

// A slicer for a program the user chose: a macOS application bundle
// (Name.app) starts with `open -a`, which hands it the files; any other
// program takes them on its command line.
Slicer slicerForProgram(const QString& name, const QString& program);

// Where findSlicers looks; system() gives the system's places, tests their
// own folders.
struct SlicerSearch {
  // Windows: Program Files and the user's Programs folder, where the
  // slicers' installers put them (Bambu Studio\bambu-studio.exe, ...).
  QStringList programDirectories;
  // Folders of executables (PATH).
  QStringList path;
  // Folders of .desktop files (Linux; an AppImage integrated into the
  // desktop has one).
  QStringList applicationDirectories;
  // Folders of installed Flatpaks (Linux: /var/lib/flatpak/app,
  // ~/.local/share/flatpak/app); found ones need `flatpak` on `path`.
  QStringList flatpakDirectories;
  // Windows: the installed programs listed in the registry (read only).
  bool registry = false;
  // Folders of application bundles (macOS: /Applications and
  // ~/Applications), where the slicers are Name.app.
  QStringList bundleDirectories;

  static SlicerSearch system();
};

// The slicers found, each once: Bambu Studio, OrcaSlicer, PrusaSlicer and
// UltiMaker Cura in their usual install folders and the registry
// (Windows), as application bundles (macOS), on PATH, as Flatpaks and in
// desktop files (Linux).
QVector<Slicer> findSlicers(const SlicerSearch& search = SlicerSearch::system());

// The settings of 3D Print (group "print"), remembered between sends.
struct PrintSettings {
  Slicer slicer;              // none: the first one found
  QString format = "stl";     // "stl": a file per body; "3mf": one file
  QString refinement = "high"; // low, medium, high or custom
  double deviation = 0.01;     // custom: mm
  double angle = 8.0;          // custom: degrees

  static PrintSettings load();
  void save() const;
};

// A name a file can take on every system: characters that are not allowed
// (or are path separators) become "_", and so does a name Windows reserves
// (CON, NUL, COM1, ...); never empty.
QString safeFileName(const QString& name);

// File names for bodies: safe, with the extension, and unique ignoring
// case ("Body1.stl", "Body1_2.stl").
QStringList printFileNames(const QStringList& bodies, const QString& extension);

// What goes into one STL file of 3D Print (mitcad#17): a body where the
// design shows it, by one occurrence when it shows the body more than once.
struct PrintPiece {
  QString body;       // uid
  QString occurrence; // its path of occurrence uids ("O1/O4"); empty: every placement
  QString name;       // of the file, before the extension: "Pin", "Pin (Arm:2)"
};

// The STL files of 3D Print: one per body (`names` are theirs), and one per
// placement of a body the design places more than once, named with the
// occurrence. The placements are those of the `instances` query (with
// hidden ones): the shown ones, all when none is shown, as the model's
// export places bodies; of a body placed more than once only those
// `chosen` (body items with their occurrences, mitcad#234), all when none
// of its placements is.
QVector<PrintPiece> printPieces(const QStringList& bodies, const QStringList& names, const QJsonArray& instances,
                                const Selection& chosen = {});

// The folder the files go into, <root>/mitcad-print/<design>: made when
// missing, and emptied, so that no file of an earlier send reaches the
// slicer. Empty, with the reason, when it cannot be.
QString preparePrintFolder(const QString& root, const QString& design, QString* error);

// The 3D Print panel (mitcad#234, MainWindowPrint.cpp).
//
// The bodies it starts with: what the selection made before stands for (a
// face, edge or vertex its body, an occurrence the bodies shown in it), or
// with none of that every body shown (`instances`, the query's answer),
// each placement a body item with its occurrence, once; `selected` tells
// which.
Selection printStartBodies(const Selection& selection, const QJsonArray& instances, bool* selected);

// The Slicer row's choices: none (open the files' folder), `current` (the
// settings' slicer) and `other` (one chosen by hand) unless found, the
// slicers found, and Other program... (otherSlicerChoice), which asks for
// a program. A slicer's choice is slicerChoice's text of it.
QVector<QPair<QString, QString>> slicerChoices(const QVector<Slicer>& found, const Slicer& current,
                                               const Slicer& other);
QString slicerChoice(const Slicer& slicer);
Slicer slicerOfChoice(const QString& choice);
QString otherSlicerChoice();

// Preferences' 3D Print group: the slicer the command starts.
class SlicerPreferencesBox : public QGroupBox {
  Q_OBJECT

public:
  explicit SlicerPreferencesBox(QWidget* parent = nullptr);

  // The slicer as chosen (none: the first one found); saved when
  // Preferences is accepted.
  Slicer chosen() const;

private:
  QComboBox* m_slicers = nullptr;
};

// A combo box of slicers: none ("first found" or "open the folder"), the
// found ones and `current`, and Browse... (another program); for Preferences.
QComboBox* slicerComboBox(const QVector<Slicer>& found, const Slicer& current, const QString& noneLabel,
                          QWidget* parent);
Slicer slicerOf(const QComboBox* combo);

} // namespace mitcad
