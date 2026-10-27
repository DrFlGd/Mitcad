// SPDX-License-Identifier: MIT
// The numbers each key takes (mitcad#746): the widest range the key's
// slicers define for it, in Mitcad's units, so that the editors refuse
// only what no slicer of the key takes. Taken from the slicers' option
// definitions (their minimum and maximum; for UltiMaker Cura its hard
// bounds and the warning bounds it flags), read as interface facts
// like the rest of the table (see the module documentation's sources).
//
// A numeric key not listed here takes 0 and more: a length, speed,
// acceleration, temperature, time, count or ratio, which no slicer
// takes negative. Every percentage (a `Percent` key or one in `%`) and
// every angle is listed, so that none falls back to that default
// unread (`limits_cover_every_percentage_and_angle`).
//
// Where one dialect takes less than the others, its own range is the
// dialect key's `range` (in the table): Bambu Studio's -1 for
// "automatic" is in the key's range below, and OrcaSlicer's or
// PrusaSlicer's narrower range is on their dialect keys.

const INF: f64 = f64::INFINITY;

/// `(key, min, max)`: the key's range where it is not 0 and more.
pub(super) static LIMITS: &[(&str, f64, f64)] = &[
    // Quality: seam, precision, ironing, wall generator.
    ("scarf_angle_threshold", 0.0, 180.0),
    // Compensations and offsets grow or shrink: either sign.
    ("xy_size_compensation", -INF, INF),
    ("xy_hole_compensation", -INF, INF),
    // Cura's first layer offset is the negative of the compensation.
    ("elefant_foot_compensation", -INF, INF),
    ("circle_compensation_manual_offset", -INF, INF),
    ("ironing_flowrate", 0.0, 100.0),
    // A direction turns either way.
    ("ironing_direction", -INF, INF),
    ("wall_transition_angle", 0.0, 90.0),
    ("bridge_density", 0.0, 100.0),
    // Cura's conical overhang angle may be negative (a cone inwards).
    ("make_overhang_printable_angle", -90.0, 90.0),
    ("monotonic_travel_into_wall", 0.0, 100.0),
    // Strength: walls, top and bottom shells, infill.
    ("top_area_threshold", 0.0, INF),
    ("top_surface_density", 0.0, 100.0),
    ("bottom_surface_density", 0.0, 100.0),
    ("fill_density", 0.0, 100.0),
    ("fill_angle", -INF, INF),
    ("skin_infill_density", 0.0, 100.0),
    ("skeleton_infill_density", 0.0, 100.0),
    ("infill_shift_step", -INF, INF),
    ("infill_rotate_step", -INF, INF),
    ("sparse_infill_lattice_angle_1", -INF, INF),
    ("sparse_infill_lattice_angle_2", -INF, INF),
    ("lightning_overhang_angle", 0.0, 90.0),
    ("lightning_prune_angle", 0.0, 90.0),
    ("lightning_straightening_angle", 0.0, 90.0),
    ("bridge_angle", -INF, INF),
    // Speed.
    ("accel_to_decel_factor", 0.0, 100.0),
    // Support: thresholds and inclinations from 0 to 90 degrees,
    // directions either way.
    ("support_material_threshold", 0.0, 90.0),
    ("raft_first_layer_density", 0.0, 100.0),
    // Bambu Studio's -1: automatic.
    ("raft_first_layer_expansion", -1.0, INF),
    ("support_material_angle", -INF, INF),
    ("support_tree_angle", 0.0, 90.0),
    ("support_tree_angle_slow", 0.0, 90.0),
    ("support_tree_branch_diameter_angle", 0.0, 90.0),
    ("support_tree_top_rate", 0.0, 100.0),
    // Bambu Studio's -1: automatic.
    ("tree_support_wall_count", -1.0, INF),
    ("support_ironing_flow", 0.0, 100.0),
    ("support_ironing_direction", -INF, INF),
    // Others: prime tower, multiple materials.
    // Bambu Studio's -1: automatic.
    ("wipe_tower_brim_width", -1.0, INF),
    ("wipe_tower_cone_angle", 0.0, 90.0),
    // Purge line spacing and flow: 100 % is the plain one.
    ("wipe_tower_extra_spacing", 100.0, 300.0),
    ("wipe_tower_extra_flow", 100.0, 300.0),
    // The gap of the tower's infill: 100 % is lines side by side.
    ("prime_tower_infill_gap", 100.0, INF),
    // The idle nozzle's temperature relative to the printing one.
    ("standby_temperature_delta", -INF, INF),
    ("interlocking_orientation", -INF, INF),
    // Filament: the slicers differ in what 100 % means (no shrinkage, or
    // a correction of that size).
    ("filament_shrink", 0.0, INF),
    ("filament_shrinkage_compensation_z", -INF, INF),
    // A preheat relative to the printing temperature.
    ("filament_preheat_temperature_delta", -INF, INF),
    // Fans run from 0 to 100 %.
    ("min_fan_speed", 0.0, 100.0),
    ("max_fan_speed", 0.0, 100.0),
    ("initial_layer_fan_speed", 0.0, 100.0),
    ("first_x_layer_part_fan_speed", 0.0, 100.0),
    ("overhang_fan_speed", 0.0, 100.0),
    ("bridge_fan_speed", 0.0, 100.0),
    ("ironing_fan_speed", 0.0, 100.0),
    ("additional_cooling_fan_speed", 0.0, 100.0),
    ("first_x_layer_fan_speed", 0.0, 100.0),
    ("during_print_exhaust_fan_speed", 0.0, 100.0),
    ("complete_print_exhaust_fan_speed", 0.0, 100.0),
    ("filament_retract_before_wipe", 0.0, 100.0),
    // Hole and contour compensation: coefficients and the bounds of the
    // correction take either sign.
    ("counter_coef_1", -INF, INF),
    ("counter_coef_2", -INF, INF),
    ("counter_coef_3", -INF, INF),
    ("hole_coef_1", -INF, INF),
    ("hole_coef_2", -INF, INF),
    ("hole_coef_3", -INF, INF),
    ("counter_limit_min", -INF, INF),
    ("counter_limit_max", -INF, INF),
    ("hole_limit_min", -INF, INF),
    ("hole_limit_max", -INF, INF),
];
