# Sources of the materials and print profiles

The base library of materials and print profiles (mitcad#640): one item
file each in `materials/`, `print/printers/` and `print/processes/`, in
Mitcad's own format (`docs/libraries.md`, "Materials and print profiles").
They are part of the library, under its MIT licence.

## Materials

The generic filaments PLA, PETG, ABS, ASA, TPU and PA (nylon). Their
densities are common published figures for the polymers (PLA, ABS and nylon
the same as Mitcad's built-in materials of those ids); their temperatures,
bed temperatures and fan speeds are Mitcad's own starting points within the
ranges filament makers publish for generic grades. None is copied from a
slicer's system profiles. Each names one of Mitcad's library appearances.

## Printers

The common Bambu Lab and QIDI printers with a 0.4 mm nozzle. Each carries
only what Mitcad uses: the bed (from 0, 0, as the slicers place it), the
printable height and the filament slots, from the makers' published
specifications of the build volume and of the filament systems sold with
the printer (the Bambu Lab AMS and AMS lite give 4 slots); the printer
model and the slicers' printer preset names are the names Bambu Studio,
OrcaSlicer and QIDI Studio give those printers, so that a 3MF Mitcad writes
names a preset the slicer has. Machine settings (accelerations,
retraction, G-code) stay with the slicers.

## Processes

Four process profiles for a 0.4 mm nozzle (draft 0.28 mm, standard 0.20
mm, fine 0.12 mm and a strong 0.20 mm with more walls and infill): Mitcad's
own values.
