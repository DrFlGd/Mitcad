// SPDX-License-Identifier: MIT
//
// The web viewer's logic that needs no three.js: a uniform grid over the
// model's triangles, the person's walking (walls, steps, gravity, walking
// through walls) and the measure's snapping. Plain functions on arrays, so
// that logic.test.js tests them without a browser.
//
// Coordinates are the viewer's world: model units with +Z up. The person's
// settings are in metres; `unitScale` (metres per model unit) converts them,
// so a model in millimetres is a 1:1 building.

export const PERSON = Object.freeze({
  eyeHeight: 1.65, // metres above the feet
  radius: 0.25, // the body's radius against walls
  stepHeight: 0.25, // the highest step climbed
  headroom: 0.1, // the body's top above the eyes
  walkSpeed: 1.4, // metres per second
  runFactor: 2.5, // Shift
  gravity: 9.81, // metres per second squared
  lookSensitivity: 0.0025, // radians per pixel of mouse movement
  fallLimit: 100, // metres below the model before the person is put back
});

// --- The grid ----------------------------------------------------------------

// A uniform grid over X and Y: each cell lists the triangles whose bounding
// box covers it. `positions` holds nine numbers per triangle (three corners).
// Built once for a model (and again only when the visible bodies change).
export function buildGrid(positions, options = {}) {
  const triCount = Math.floor(positions.length / 9);
  const triMinZ = new Float32Array(triCount);
  const triMaxZ = new Float32Array(triCount);
  // |z| of the triangle's unit normal; -1 for a degenerate triangle.
  const normalZ = new Float32Array(triCount);
  let minX = Infinity;
  let minY = Infinity;
  let minZ = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  let maxZ = -Infinity;
  for (let t = 0; t < triCount; t++) {
    const o = t * 9;
    const ax = positions[o];
    const ay = positions[o + 1];
    const az = positions[o + 2];
    const bx = positions[o + 3];
    const by = positions[o + 4];
    const bz = positions[o + 5];
    const cx = positions[o + 6];
    const cy = positions[o + 7];
    const cz = positions[o + 8];
    minX = Math.min(minX, ax, bx, cx);
    maxX = Math.max(maxX, ax, bx, cx);
    minY = Math.min(minY, ay, by, cy);
    maxY = Math.max(maxY, ay, by, cy);
    triMinZ[t] = Math.min(az, bz, cz);
    triMaxZ[t] = Math.max(az, bz, cz);
    minZ = Math.min(minZ, triMinZ[t]);
    maxZ = Math.max(maxZ, triMaxZ[t]);
    const ux = bx - ax;
    const uy = by - ay;
    const uz = bz - az;
    const vx = cx - ax;
    const vy = cy - ay;
    const vz = cz - az;
    const nx = uy * vz - uz * vy;
    const ny = uz * vx - ux * vz;
    const nz = ux * vy - uy * vx;
    const length = Math.hypot(nx, ny, nz);
    normalZ[t] = length > 0 ? Math.abs(nz) / length : -1;
  }
  if (triCount === 0) {
    minX = minY = minZ = maxX = maxY = maxZ = 0;
  }
  const sizeX = Math.max(maxX - minX, 1e-9);
  const sizeY = Math.max(maxY - minY, 1e-9);
  let cell = options.cellSize;
  if (!(cell > 0)) {
    // About two triangles per cell, at most 512 cells a side.
    cell = Math.sqrt((sizeX * sizeY) / Math.max(1, triCount / 2));
    cell = Math.max(cell, Math.max(sizeX, sizeY) / 512);
  }
  const maxEntries = options.maxEntries > 0 ? options.maxEntries : 1 << 24;
  for (;;) {
    const nx = Math.max(1, Math.ceil(sizeX / cell));
    const ny = Math.max(1, Math.ceil(sizeY / cell));
    const index = (value, low, n) => Math.min(n - 1, Math.max(0, Math.floor((value - low) / cell)));
    const ranges = new Int32Array(triCount * 4);
    const start = new Uint32Array(nx * ny + 1);
    let total = 0;
    for (let t = 0; t < triCount && total <= maxEntries; t++) {
      if (normalZ[t] < 0) {
        continue;
      }
      const o = t * 9;
      const i0 = index(Math.min(positions[o], positions[o + 3], positions[o + 6]), minX, nx);
      const i1 = index(Math.max(positions[o], positions[o + 3], positions[o + 6]), minX, nx);
      const j0 = index(Math.min(positions[o + 1], positions[o + 4], positions[o + 7]), minY, ny);
      const j1 = index(Math.max(positions[o + 1], positions[o + 4], positions[o + 7]), minY, ny);
      ranges[t * 4] = i0;
      ranges[t * 4 + 1] = i1;
      ranges[t * 4 + 2] = j0;
      ranges[t * 4 + 3] = j1;
      total += (i1 - i0 + 1) * (j1 - j0 + 1);
      for (let j = j0; j <= j1; j++) {
        for (let i = i0; i <= i1; i++) {
          start[j * nx + i + 1]++;
        }
      }
    }
    if (total > maxEntries) {
      cell *= 2; // Large triangles over a fine grid: coarser cells.
      continue;
    }
    for (let k = 1; k < start.length; k++) {
      start[k] += start[k - 1];
    }
    const items = new Uint32Array(total);
    const cursor = start.slice(0, nx * ny);
    for (let t = 0; t < triCount; t++) {
      if (normalZ[t] < 0) {
        continue;
      }
      const r = t * 4;
      for (let j = ranges[r + 2]; j <= ranges[r + 3]; j++) {
        for (let i = ranges[r]; i <= ranges[r + 1]; i++) {
          items[cursor[j * nx + i]++] = t;
        }
      }
    }
    return {
      positions,
      triCount,
      minX,
      minY,
      minZ,
      maxX,
      maxY,
      maxZ,
      cell,
      nx,
      ny,
      start,
      items,
      triMinZ,
      triMaxZ,
      normalZ,
      eps: 1e-6 * Math.max(sizeX, sizeY, maxZ - minZ, 1e-9),
      stamps: new Uint32Array(triCount),
      query: 0,
    };
  }
}

function cellIndex(grid, value, low, n) {
  return Math.min(n - 1, Math.max(0, Math.floor((value - low) / grid.cell)));
}

// Calls visit(t) once for each triangle whose cells meet the box.
export function forEachTriangleNear(grid, x0, y0, x1, y1, visit) {
  if (x1 < grid.minX || y1 < grid.minY || x0 > grid.maxX || y0 > grid.maxY) {
    return;
  }
  if (++grid.query >= 0xffffffff) {
    grid.stamps.fill(0);
    grid.query = 1;
  }
  const stamp = grid.query;
  const i0 = cellIndex(grid, x0, grid.minX, grid.nx);
  const i1 = cellIndex(grid, x1, grid.minX, grid.nx);
  const j0 = cellIndex(grid, y0, grid.minY, grid.ny);
  const j1 = cellIndex(grid, y1, grid.minY, grid.ny);
  for (let j = j0; j <= j1; j++) {
    for (let i = i0; i <= i1; i++) {
      const c = j * grid.nx + i;
      for (let k = grid.start[c]; k < grid.start[c + 1]; k++) {
        const t = grid.items[k];
        if (grid.stamps[t] !== stamp) {
          grid.stamps[t] = stamp;
          visit(t);
        }
      }
    }
  }
}

// The height of the highest non-vertical surface at (x, y) that is not above
// zTop; -Infinity when there is none. Both sides of a triangle count, so
// the result does not depend on how the model's triangles are wound.
export function groundAt(grid, x, y, zTop) {
  if (x < grid.minX || y < grid.minY || x > grid.maxX || y > grid.maxY) {
    return -Infinity;
  }
  const P = grid.positions;
  const eps = grid.eps;
  const c = cellIndex(grid, y, grid.minY, grid.ny) * grid.nx + cellIndex(grid, x, grid.minX, grid.nx);
  let best = -Infinity;
  for (let k = grid.start[c]; k < grid.start[c + 1]; k++) {
    const t = grid.items[k];
    if (grid.normalZ[t] < 0.05 || grid.triMinZ[t] > zTop + eps || grid.triMaxZ[t] <= best) {
      continue;
    }
    const o = t * 9;
    const x1 = P[o];
    const y1 = P[o + 1];
    const x2 = P[o + 3];
    const y2 = P[o + 4];
    const x3 = P[o + 6];
    const y3 = P[o + 7];
    const d = (y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3);
    if (d === 0) {
      continue;
    }
    const l1 = ((y2 - y3) * (x - x3) + (x3 - x2) * (y - y3)) / d;
    const l2 = ((y3 - y1) * (x - x3) + (x1 - x3) * (y - y3)) / d;
    const l3 = 1 - l1 - l2;
    if (l1 < -1e-9 || l2 < -1e-9 || l3 < -1e-9) {
      continue;
    }
    const z = l1 * P[o + 2] + l2 * P[o + 5] + l3 * P[o + 8];
    if (z <= zTop + eps && z > best) {
      best = z;
    }
  }
  return best;
}

// The ground under a footprint: the highest of groundAt at the centre and at
// four points `footRadius` from it, so that a step is climbed as the front
// of the foot reaches it and a narrow gap is not fallen through.
export function groundBelow(grid, x, y, zTop, footRadius = 0) {
  let best = groundAt(grid, x, y, zTop);
  if (footRadius > 0) {
    best = Math.max(
      best,
      groundAt(grid, x + footRadius, y, zTop),
      groundAt(grid, x - footRadius, y, zTop),
      groundAt(grid, x, y + footRadius, zTop),
      groundAt(grid, x, y - footRadius, zTop),
    );
  }
  return best;
}

// The point of triangle t closest to p, into out[0..2].
export function closestPointOnTriangle(P, t, px, py, pz, out) {
  const o = t * 9;
  const ax = P[o];
  const ay = P[o + 1];
  const az = P[o + 2];
  const abx = P[o + 3] - ax;
  const aby = P[o + 4] - ay;
  const abz = P[o + 5] - az;
  const acx = P[o + 6] - ax;
  const acy = P[o + 7] - ay;
  const acz = P[o + 8] - az;
  const set = (x, y, z) => {
    out[0] = x;
    out[1] = y;
    out[2] = z;
    return out;
  };
  const apx = px - ax;
  const apy = py - ay;
  const apz = pz - az;
  const d1 = abx * apx + aby * apy + abz * apz;
  const d2 = acx * apx + acy * apy + acz * apz;
  if (d1 <= 0 && d2 <= 0) {
    return set(ax, ay, az);
  }
  const bpx = apx - abx;
  const bpy = apy - aby;
  const bpz = apz - abz;
  const d3 = abx * bpx + aby * bpy + abz * bpz;
  const d4 = acx * bpx + acy * bpy + acz * bpz;
  if (d3 >= 0 && d4 <= d3) {
    return set(ax + abx, ay + aby, az + abz);
  }
  const vc = d1 * d4 - d3 * d2;
  if (vc <= 0 && d1 >= 0 && d3 <= 0) {
    const v = d1 / (d1 - d3);
    return set(ax + v * abx, ay + v * aby, az + v * abz);
  }
  const cpx = apx - acx;
  const cpy = apy - acy;
  const cpz = apz - acz;
  const d5 = abx * cpx + aby * cpy + abz * cpz;
  const d6 = acx * cpx + acy * cpy + acz * cpz;
  if (d6 >= 0 && d5 <= d6) {
    return set(ax + acx, ay + acy, az + acz);
  }
  const vb = d5 * d2 - d1 * d6;
  if (vb <= 0 && d2 >= 0 && d6 <= 0) {
    const w = d2 / (d2 - d6);
    return set(ax + w * acx, ay + w * acy, az + w * acz);
  }
  const va = d3 * d6 - d5 * d4;
  if (va <= 0 && d4 - d3 >= 0 && d5 - d6 >= 0) {
    const w = (d4 - d3) / (d4 - d3 + (d5 - d6));
    const bx = ax + abx;
    const by = ay + aby;
    const bz = az + abz;
    return set(bx + w * (acx - abx), by + w * (acy - aby), bz + w * (acz - abz));
  }
  const denom = 1 / (va + vb + vc);
  const v = vb * denom;
  const w = vc * denom;
  return set(ax + abx * v + acx * w, ay + aby * v + acy * w, az + abz * v + acz * w);
}

// --- The person --------------------------------------------------------------

// A person standing at `feet` (world coordinates). `options` may override
// any of PERSON's values and give `unitScale` (metres per model unit) and
// `yaw` (radians from +X towards +Y).
export function createPerson(feet, options = {}) {
  const person = { ...PERSON };
  for (const key of Object.keys(PERSON)) {
    if (typeof options[key] === 'number' && Number.isFinite(options[key])) {
      person[key] = options[key];
    }
  }
  person.unitScale = options.unitScale > 0 ? options.unitScale : 1;
  person.x = feet[0];
  person.y = feet[1];
  person.z = feet[2];
  person.vz = 0;
  person.grounded = false;
  person.through = false;
  person.yaw = options.yaw || 0;
  person.pitch = 0;
  person.home = [feet[0], feet[1], feet[2]];
  return person;
}

export function eyePosition(person) {
  return [person.x, person.y, person.z + person.eyeHeight / person.unitScale];
}

export function lookDirection(person) {
  const cp = Math.cos(person.pitch);
  return [cp * Math.cos(person.yaw), cp * Math.sin(person.yaw), Math.sin(person.pitch)];
}

// Mouse look: dx, dy in pixels (a drag or the pointer lock's movement).
export function turnPerson(person, dx, dy, sensitivity = PERSON.lookSensitivity) {
  person.yaw -= dx * sensitivity;
  person.pitch -= dy * sensitivity;
  const limit = Math.PI / 2 - 0.01;
  person.pitch = Math.max(-limit, Math.min(limit, person.pitch));
  return person;
}

const _closest = [0, 0, 0];

// Pushes the person's body out of the walls it overlaps. The body is a column
// of spheres from a step's height above the feet to the top of the head, so
// that anything lower than a step is climbed and not a wall.
export function pushOutOfWalls(grid, person) {
  const u = 1 / person.unitScale;
  const r = person.radius * u;
  const r2 = r * r;
  const bottom = person.z + person.stepHeight * u + r;
  const top = Math.max(bottom, person.z + (person.eyeHeight + person.headroom) * u - r);
  const count = Math.max(1, Math.ceil((top - bottom) / (0.75 * r)) + 1);
  const P = grid.positions;
  for (let pass = 0; pass < 4; pass++) {
    let moved = false;
    for (let s = 0; s < count; s++) {
      const zc = count === 1 ? bottom : bottom + ((top - bottom) * s) / (count - 1);
      forEachTriangleNear(grid, person.x - r, person.y - r, person.x + r, person.y + r, (t) => {
        if (grid.triMaxZ[t] < zc - r || grid.triMinZ[t] > zc + r) {
          return;
        }
        closestPointOnTriangle(P, t, person.x, person.y, zc, _closest);
        const dx = person.x - _closest[0];
        const dy = person.y - _closest[1];
        const dz = zc - _closest[2];
        const h2 = dx * dx + dy * dy;
        if (h2 + dz * dz >= r2 || h2 < 1e-12 * r2) {
          return; // Clear, or a floor or ceiling straight above or below.
        }
        const h = Math.sqrt(h2);
        const need = Math.sqrt(r2 - dz * dz) - h;
        if (need > 0) {
          person.x += (dx / h) * need;
          person.y += (dy / h) * need;
          moved = true;
        }
      });
    }
    if (!moved) {
      break;
    }
  }
  return person;
}

// Advances the person by dt seconds. input: forward and right in -1..1 (an
// analog stick gives fractions), up in -1..1 (through walls only), run and
// through as booleans. Walking stays on the ground: steps up to stepHeight
// are climbed and down, walls stop the body, and off an edge the person
// falls. Through walls neither walls nor gravity act, and the person moves
// where they look (Q/E: down and up).
export function stepPerson(grid, person, input, dt) {
  dt = Math.min(Math.max(Number.isFinite(dt) ? dt : 0, 0), 0.1);
  const u = 1 / person.unitScale;
  const speed = person.walkSpeed * (input.run ? person.runFactor : 1) * u;
  const cy = Math.cos(person.yaw);
  const sy = Math.sin(person.yaw);
  let forward = input.forward || 0;
  let right = input.right || 0;
  if (input.through) {
    const cp = Math.cos(person.pitch);
    const sp = Math.sin(person.pitch);
    const dx = forward * cp * cy + right * sy;
    const dy = forward * cp * sy - right * cy;
    const dz = forward * sp + (input.up || 0);
    const length = Math.hypot(dx, dy, dz);
    if (length > 1e-9) {
      const k = (speed * dt * Math.min(length, 1)) / length;
      person.x += dx * k;
      person.y += dy * k;
      person.z += dz * k;
    }
    person.vz = 0;
    person.grounded = false;
    person.through = true;
    return person;
  }
  person.through = false;
  const radius = person.radius * u;
  const step = person.stepHeight * u;
  const foot = radius * 0.5;
  let length = Math.hypot(forward, right);
  if (length > 1) {
    forward /= length;
    right /= length;
    length = 1;
  }
  const distance = speed * dt * length;
  if (distance > 0) {
    // Substeps of at most half the radius, so that no wall is passed.
    const n = Math.max(1, Math.ceil(distance / (radius * 0.5)));
    const mx = ((forward * cy + right * sy) / length) * (distance / n);
    const my = ((forward * sy - right * cy) / length) * (distance / n);
    for (let i = 0; i < n; i++) {
      person.x += mx;
      person.y += my;
      pushOutOfWalls(grid, person);
      if (person.grounded) {
        const g = groundBelow(grid, person.x, person.y, person.z + step, foot);
        if (g >= person.z - step) {
          person.z = g; // Level, or a step up or down.
        } else {
          person.grounded = false; // Off an edge.
        }
      }
    }
  }
  if (person.grounded) {
    const g = groundBelow(grid, person.x, person.y, person.z + step, foot);
    if (g >= person.z - step) {
      person.z = g;
    } else {
      person.grounded = false;
    }
  }
  if (!person.grounded) {
    const zTop = person.z + step;
    person.vz -= person.gravity * u * dt;
    person.z += person.vz * dt;
    const g = groundBelow(grid, person.x, person.y, zTop, foot);
    if (person.z <= g) {
      person.z = g;
      person.vz = 0;
      person.grounded = true;
    } else if (person.z < grid.minZ - person.fallLimit * u) {
      // Nothing below: back where the person was put down.
      [person.x, person.y, person.z] = person.home;
      person.vz = 0;
      person.grounded = true;
    }
  }
  return person;
}

// --- Measure -----------------------------------------------------------------

// The point of segment a-b closest to p.
export function closestPointOnSegment(p, a, b) {
  const abx = b[0] - a[0];
  const aby = b[1] - a[1];
  const abz = b[2] - a[2];
  const ab2 = abx * abx + aby * aby + abz * abz;
  let t = ab2 > 0 ? ((p[0] - a[0]) * abx + (p[1] - a[1]) * aby + (p[2] - a[2]) * abz) / ab2 : 0;
  t = Math.max(0, Math.min(1, t));
  return [a[0] + t * abx, a[1] + t * aby, a[2] + t * abz];
}

export function distanceBetween(a, b) {
  return Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]);
}

// Snaps a picked point to the model's corners and edges: `segments` holds six
// numbers per edge segment (its two ends). An end within `tolerance` wins,
// then the nearest point of an edge within it; otherwise the point stays on
// the face. Returns { point, kind: 'vertex' | 'edge' | 'face' }.
export function snapPoint(point, segments, tolerance) {
  const limit = tolerance * tolerance;
  let best = limit;
  let found = null;
  const count = Math.floor(segments.length / 6);
  for (let s = 0; s < count; s++) {
    for (let end = 0; end < 2; end++) {
      const o = s * 6 + end * 3;
      const dx = segments[o] - point[0];
      const dy = segments[o + 1] - point[1];
      const dz = segments[o + 2] - point[2];
      const d2 = dx * dx + dy * dy + dz * dz;
      if (d2 <= best) {
        best = d2;
        found = [segments[o], segments[o + 1], segments[o + 2]];
      }
    }
  }
  if (found) {
    return { point: found, kind: 'vertex' };
  }
  best = limit;
  for (let s = 0; s < count; s++) {
    const o = s * 6;
    const q = closestPointOnSegment(
      point,
      [segments[o], segments[o + 1], segments[o + 2]],
      [segments[o + 3], segments[o + 4], segments[o + 5]],
    );
    const d2 = (q[0] - point[0]) ** 2 + (q[1] - point[1]) ** 2 + (q[2] - point[2]) ** 2;
    if (d2 <= best) {
      best = d2;
      found = q;
    }
  }
  if (found) {
    return { point: found, kind: 'edge' };
  }
  return { point: [point[0], point[1], point[2]], kind: 'face' };
}

const DECIMALS = { mm: 2, cm: 3, m: 4, in: 3, ft: 4 };

// A length in the design's units, e.g. "123.46 mm".
export function formatLength(value, units) {
  const decimals = DECIMALS[units] ?? 3;
  const rounded = Math.abs(value) < 0.5 * 10 ** -decimals ? 0 : value;
  return `${rounded.toFixed(decimals)} ${units}`;
}
