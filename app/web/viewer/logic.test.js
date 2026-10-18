// SPDX-License-Identifier: MIT
//
// Unit tests of the web viewer's logic (logic.js), without three.js. They run
// in tests.html (a browser) and with `node app/web/viewer/logic.test.js`
// (Node.js 22 or newer, which takes the file for the ES module it is; with
// an older one, from a copy beside a package.json of {"type": "module"}).
// The fixtures are synthetic: boxes of a building in millimetres, so that
// the metre settings of the person go through the unit scale.

const logic = await import(
  typeof window === 'undefined' ? new URL('./logic.js', import.meta.url).href : 'mitcad/logic.js'
);

const MM = 1000; // model units (mm) per metre
const UNIT_SCALE = 0.001; // metres per model unit
const DT = 1 / 60;

// The twelve triangles of a box, in metres scaled to millimetres, wound
// outwards.
export function box(x0, y0, z0, x1, y1, z1) {
  const c = (i) => [i & 1 ? x1 : x0, i & 2 ? y1 : y0, i & 4 ? z1 : z0].map((v) => v * MM);
  const quads = [
    [0, 2, 3, 1], // bottom (-Z)
    [4, 5, 7, 6], // top (+Z)
    [0, 1, 5, 4], // -Y
    [2, 6, 7, 3], // +Y
    [0, 4, 6, 2], // -X
    [1, 3, 7, 5], // +X
  ];
  const out = [];
  for (const [a, b, d, e] of quads) {
    out.push(...c(a), ...c(b), ...c(d), ...c(a), ...c(d), ...c(e));
  }
  return out;
}

function scene(...boxes) {
  return new Float32Array(boxes.flat());
}

// A floor 15 m by 10 m with its top at z = 0.
const floor = () => box(-5, -5, -0.2, 10, 5, 0);
// A wall across the way, 0.2 m thick at x = 2, 3 m high.
const wall = () => box(2, -5, 0, 2.2, 5, 3);
// Five steps of 0.18 m with 0.28 m treads from x = 1, the top landing to x = 6.
const stairs = () => {
  const out = [];
  for (let i = 1; i <= 5; i++) {
    out.push(...box(1 + (i - 1) * 0.28, -1, 0, 6, 1, i * 0.18));
  }
  return out;
};
// A slab whose top is at z = 3 m, over x < 0.
const upperSlab = () => box(-5, -5, 2.8, 0, 5, 3);

export function stairPositions() {
  return scene(floor(), stairs());
}

function walk(grid, person, input, seconds) {
  for (let t = 0; t < seconds - 1e-9; t += DT) {
    logic.stepPerson(grid, person, input, DT);
  }
  return person;
}

function person(feetMetres, yaw = 0) {
  return logic.createPerson(
    feetMetres.map((v) => v * MM),
    { unitScale: UNIT_SCALE, yaw },
  );
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function near(actual, expected, tolerance, what) {
  assert(
    Math.abs(actual - expected) <= tolerance,
    `${what}: ${actual} is not within ${tolerance} of ${expected}`,
  );
}

const tests = [];
const test = (name, fn) => tests.push([name, fn]);

test('the grid finds the highest floor below a height', () => {
  const grid = logic.buildGrid(scene(floor(), upperSlab()));
  near(logic.groundAt(grid, -2 * MM, 0, 3.25 * MM), 3 * MM, 1e-6, 'on the slab');
  near(logic.groundAt(grid, -2 * MM, 0, 2.5 * MM), 0, 1e-6, 'under the slab');
  near(logic.groundAt(grid, 2 * MM, 0, 3.25 * MM), 0, 1e-6, 'beside the slab');
  assert(logic.groundAt(grid, 20 * MM, 0, 1e9) === -Infinity, 'outside the model there is no ground');
});

test('a wall stops the person', () => {
  const grid = logic.buildGrid(scene(floor(), wall()));
  const p = walk(grid, person([0, 0, 0]), { forward: 1 }, 3);
  assert(p.x < (2 - 0.25 + 0.001) * MM, `the person passed into the wall: x = ${p.x}`);
  assert(p.x > 1.7 * MM, `the person stopped too early: x = ${p.x}`);
  near(p.z, 0, 1e-6, 'feet on the floor');
});

test('a stair of 0.18 m steps is climbed', () => {
  const grid = logic.buildGrid(stairPositions());
  const p = walk(grid, person([0, 0, 0]), { forward: 1 }, 3.5);
  near(p.z, 0.9 * MM, 1e-3, 'feet on the top landing');
  assert(p.x > 3 * MM, `the person did not get up the stair: x = ${p.x}`);
});

test('a step of 0.4 m is refused', () => {
  const grid = logic.buildGrid(scene(floor(), box(1, -1, 0, 3, 1, 0.4)));
  const p = walk(grid, person([0, 0, 0]), { forward: 1 }, 3);
  assert(p.x < 1 * MM, `the person got onto the step: x = ${p.x}`);
  assert(p.x > 0.7 * MM, `the person stopped too early: x = ${p.x}`);
  near(p.z, 0, 1e-6, 'feet on the floor');
});

test('gravity takes the person onto the floor below', () => {
  const grid = logic.buildGrid(scene(floor(), upperSlab()));
  const p = walk(grid, person([-1, 0, 3]), { forward: 1 }, 0.5);
  near(p.z, 3 * MM, 1e-6, 'still on the slab');
  walk(grid, p, { forward: 1 }, 2);
  near(p.z, 0, 1e-6, 'feet on the lower floor');
  assert(p.grounded, 'standing after the fall');
  const dropped = walk(grid, person([5, 0, 2]), {}, 1);
  near(dropped.z, 0, 1e-6, 'a person put down in the air lands');
});

test('walking down a stair keeps the feet on the steps', () => {
  const grid = logic.buildGrid(stairPositions());
  const p = walk(grid, person([4, 0, 0.9], Math.PI), {}, 0.1);
  near(p.z, 0.9 * MM, 1e-6, 'on the landing');
  walk(grid, p, { forward: 1 }, 4);
  near(p.z, 0, 1e-6, 'down on the floor');
  assert(p.x < 0.9 * MM, `the person did not get down: x = ${p.x}`);
});

test('holding the right button walks through walls without gravity', () => {
  const grid = logic.buildGrid(scene(floor(), wall()));
  const p = walk(grid, person([0, 0, 1]), { forward: 1, through: true }, 3);
  assert(p.x > 2.2 * MM, `the person did not pass the wall: x = ${p.x}`);
  near(p.z, 1 * MM, 1e-6, 'no gravity while held');
  walk(grid, p, {}, 1);
  near(p.z, 0, 1e-6, 'gravity again when released');
  const q = walk(logic.buildGrid(scene(floor(), upperSlab())), person([-2, 0, 3]), { through: true, up: -1 }, 1);
  assert(q.z < 2.8 * MM, `the person did not pass the floor: z = ${q.z}`);
});

test('Shift runs at 2.5 times the walking speed', () => {
  const grid = logic.buildGrid(scene(floor()));
  const walked = walk(grid, person([0, 0, 0]), { forward: 1 }, 1);
  const ran = walk(grid, person([0, 0, 0]), { forward: 1, run: true }, 1);
  near(walked.x, 1.4 * MM, 1e-3, 'walked one second');
  near(ran.x, 3.5 * MM, 1e-3, 'ran one second');
  const half = walk(grid, person([0, 0, 0], Math.PI / 2), { forward: 0.5 }, 1);
  near(half.y, 0.7 * MM, 1e-3, 'half a stick walks at half speed along +Y');
  near(half.x, 0, 1e-6, 'and straight');
  const sideways = walk(grid, person([0, 0, 0]), { right: 1 }, 1);
  near(sideways.y, -1.4 * MM, 1e-3, 'right of +X is -Y');
});

test('the eyes follow the eye height in metres', () => {
  const p = person([1, 2, 0]);
  near(logic.eyePosition(p)[2], 1.65 * MM, 1e-6, 'default eye height');
  p.eyeHeight = 1.2;
  near(logic.eyePosition(p)[2], 1.2 * MM, 1e-6, 'adjusted eye height');
  logic.turnPerson(p, 0, -1e6);
  assert(p.pitch < Math.PI / 2 && p.pitch > 1.5, `looking up is clamped: ${p.pitch}`);
  const [, , dz] = logic.lookDirection(p);
  assert(dz > 0.99, 'looking up');
});

test('measure snaps to corners, then edges', () => {
  // A square of 100 mm.
  const segments = [0, 0, 0, 100, 0, 0, 100, 0, 0, 100, 100, 0, 100, 100, 0, 0, 100, 0, 0, 100, 0, 0, 0, 0];
  const corner = logic.snapPoint([99, 1, 0], segments, 3);
  assert(corner.kind === 'vertex', `a corner: ${corner.kind}`);
  near(corner.point[0], 100, 0, 'corner x');
  near(corner.point[1], 0, 0, 'corner y');
  const cornerFirst = logic.snapPoint([97.5, 0.1, 0], segments, 3);
  assert(cornerFirst.kind === 'vertex', 'a corner within reach wins over a nearer edge');
  const edge = logic.snapPoint([50, 2, 0], segments, 3);
  assert(edge.kind === 'edge', `an edge: ${edge.kind}`);
  near(edge.point[0], 50, 1e-9, 'edge x');
  near(edge.point[1], 0, 1e-9, 'edge y');
  const face = logic.snapPoint([50, 50, 0], segments, 3);
  assert(face.kind === 'face' && face.point[0] === 50 && face.point[1] === 50, 'a face point stays');
  near(logic.distanceBetween(corner.point, [100, 100, 0]), 100, 1e-9, 'distance');
  assert(logic.formatLength(123.456, 'mm') === '123.46 mm', logic.formatLength(123.456, 'mm'));
  assert(logic.formatLength(-0.0001, 'mm') === '0.00 mm', 'no negative zero');
  assert(logic.formatLength(2.5, 'm') === '2.5000 m', logic.formatLength(2.5, 'm'));
});

test('a floor of 100 000 triangles builds a grid and walks', () => {
  const n = 224; // 224 * 224 * 2 triangles
  const size = 40 * MM;
  const positions = new Float32Array(n * n * 18);
  let o = 0;
  for (let j = 0; j < n; j++) {
    for (let i = 0; i < n; i++) {
      const x0 = (i / n) * size;
      const x1 = ((i + 1) / n) * size;
      const y0 = (j / n) * size;
      const y1 = ((j + 1) / n) * size;
      positions.set([x0, y0, 0, x1, y0, 0, x1, y1, 0, x0, y0, 0, x1, y1, 0, x0, y1, 0], o);
      o += 18;
    }
  }
  const grid = logic.buildGrid(positions);
  assert(grid.triCount === n * n * 2, 'every triangle');
  const p = walk(grid, person([20, 20, 0]), { forward: 1, run: true }, 2);
  near(p.z, 0, 1e-6, 'on the floor');
  near(p.x, 27 * MM, 1e-2, 'ran 7 m');
});

test('a glb fixture is a valid binary glTF', () => {
  const glb = new DataView(fixtureGlb(stairPositions()));
  assert(glb.getUint32(0, true) === 0x46546c67, 'magic');
  assert(glb.getUint32(4, true) === 2, 'version');
  assert(glb.getUint32(8, true) === glb.byteLength, 'length');
  assert(glb.getUint32(16, true) === 0x4e4f534a, 'a JSON chunk first');
});

// A binary glTF of one node ("Fixture") holding the triangles, without
// indices: for tests.html, which loads it with three.js's loader.
export function fixtureGlb(positions, name = 'Fixture') {
  const floats = new Float32Array(positions);
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < floats.length; i++) {
    min[i % 3] = Math.min(min[i % 3], floats[i]);
    max[i % 3] = Math.max(max[i % 3], floats[i]);
  }
  const json = {
    asset: { version: '2.0', generator: 'Mitcad viewer tests' },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [{ name, mesh: 0 }],
    meshes: [{ name, primitives: [{ attributes: { POSITION: 0 } }] }],
    accessors: [{ bufferView: 0, componentType: 5126, count: floats.length / 3, type: 'VEC3', min, max }],
    bufferViews: [{ buffer: 0, byteOffset: 0, byteLength: floats.byteLength }],
    buffers: [{ byteLength: floats.byteLength }],
  };
  let text = JSON.stringify(json);
  while (text.length % 4 !== 0) {
    text += ' ';
  }
  const jsonBytes = new TextEncoder().encode(text);
  const total = 12 + 8 + jsonBytes.length + 8 + floats.byteLength;
  const out = new ArrayBuffer(total);
  const view = new DataView(out);
  view.setUint32(0, 0x46546c67, true); // "glTF"
  view.setUint32(4, 2, true);
  view.setUint32(8, total, true);
  view.setUint32(12, jsonBytes.length, true);
  view.setUint32(16, 0x4e4f534a, true); // "JSON"
  new Uint8Array(out, 20, jsonBytes.length).set(jsonBytes);
  const binAt = 20 + jsonBytes.length;
  view.setUint32(binAt, floats.byteLength, true);
  view.setUint32(binAt + 4, 0x004e4942, true); // "BIN"
  const bin = new Uint8Array(floats.buffer, floats.byteOffset, floats.byteLength);
  new Uint8Array(out, binAt + 8, floats.byteLength).set(bin);
  return out;
}

// Runs every test; log receives one line per test and a summary.
export async function runTests(log = console.log) {
  let passed = 0;
  let failed = 0;
  for (const [name, fn] of tests) {
    try {
      await fn();
      passed++;
      log(`ok   ${name}`);
    } catch (error) {
      failed++;
      log(`FAIL ${name}: ${error.message}`);
    }
  }
  log(`${passed} passed, ${failed} failed`);
  return { passed, failed };
}

export { logic };

if (typeof window === 'undefined' && typeof process !== 'undefined') {
  const { failed } = await runTests();
  process.exitCode = failed ? 1 : 0;
}
