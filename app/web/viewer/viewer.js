// SPDX-License-Identifier: MIT
//
// Mitcad's web viewer: shows the design inlined in the page (#mitcad-data:
// the glb, the component tree, the named views and the units) with a free
// camera, a person who walks through it, and a measure. The walking and
// snapping logic is in logic.js; this module is the page around it.
//
// The world is the design's coordinates turned so that its up is +Z (for a
// Mitcad design, which is +Z up, nothing turns), in model units.

import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';
import * as logic from 'mitcad/logic.js';

const $ = (id) => document.getElementById(id);
const statusBox = $('status');
const container = $('view');

function fail(message) {
  statusBox.textContent = message;
  statusBox.classList.add('error');
  statusBox.classList.remove('hidden');
}

let data;
try {
  data = JSON.parse($('mitcad-data').textContent);
} catch (error) {
  fail('This page has no design in it.');
  throw error;
}

const units = typeof data.units === 'string' && data.units ? data.units : 'mm';
const unitScale = data.unit_scale > 0 ? data.unit_scale : 0.001; // metres per model unit
const metre = 1 / unitScale; // model units per metre
$('title').textContent = data.title || document.title;

// --- Renderer, scene, cameras ------------------------------------------------

THREE.Object3D.DEFAULT_UP.set(0, 0, 1);

let renderer;
try {
  renderer = new THREE.WebGLRenderer({ antialias: true });
} catch (error) {
  fail('This browser cannot show 3D: WebGL is turned off or not supported.');
  throw error;
}
renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
container.appendChild(renderer.domElement);
const canvas = renderer.domElement;
canvas.tabIndex = 0;

const scene = new THREE.Scene();
scene.background = new THREE.Color(0xe9edf2);
const pmrem = new THREE.PMREMGenerator(renderer);
scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
scene.environmentRotation.set(Math.PI / 2, 0, 0); // The room's Y up to our Z up.
scene.add(new THREE.HemisphereLight(0xffffff, 0x8a8f99, 0.6));
const headlight = new THREE.DirectionalLight(0xffffff, 1.2);
scene.add(headlight, headlight.target);

// The design, turned so that its up is +Z.
const designUp = new THREE.Vector3().fromArray(Array.isArray(data.up) ? data.up : [0, 0, 1]);
if (designUp.lengthSq() === 0) {
  designUp.set(0, 0, 1);
}
const root = new THREE.Group();
root.quaternion.setFromUnitVectors(designUp.normalize(), new THREE.Vector3(0, 0, 1));
scene.add(root);
const toWorld = (array) => new THREE.Vector3().fromArray(array).applyQuaternion(root.quaternion);
const toModel = (vector) => vector.clone().applyQuaternion(root.quaternion.clone().invert());

const perspective = new THREE.PerspectiveCamera(45, 1, 0.1, 1000);
const orthographic = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000);
let camera = perspective;
const controls = new OrbitControls(perspective, canvas);
controls.mouseButtons = { LEFT: THREE.MOUSE.ROTATE, MIDDLE: THREE.MOUSE.PAN, RIGHT: THREE.MOUSE.PAN };
controls.touches = { ONE: THREE.TOUCH.ROTATE, TWO: THREE.TOUCH.DOLLY_PAN };
controls.zoomToCursor = true;
controls.screenSpacePanning = true;

let modelRadius = 1000;
const modelCenter = new THREE.Vector3();
let dirty = true;
const redraw = () => {
  dirty = true;
};
controls.addEventListener('change', redraw);

function setClipping(near, far) {
  for (const cam of [perspective, orthographic]) {
    cam.near = near;
    cam.far = far;
  }
  updateProjection();
}

function updateProjection() {
  const width = Math.max(1, container.clientWidth);
  const height = Math.max(1, container.clientHeight);
  perspective.aspect = width / height;
  perspective.updateProjectionMatrix();
  const halfHeight = (orthographic.top - orthographic.bottom) / 2;
  orthographic.left = -halfHeight * perspective.aspect;
  orthographic.right = halfHeight * perspective.aspect;
  orthographic.updateProjectionMatrix();
  redraw();
}

function resize() {
  renderer.setSize(Math.max(1, container.clientWidth), Math.max(1, container.clientHeight), false);
  updateProjection();
}
new ResizeObserver(resize).observe(container);
resize();

// Switches between the perspective and the orthographic camera, keeping the
// view: the orthographic frustum shows at the target what the perspective one
// does.
function useOrthographic(on) {
  const next = on ? orthographic : perspective;
  if (next === camera) {
    return;
  }
  next.position.copy(camera.position);
  next.quaternion.copy(camera.quaternion);
  next.up.copy(camera.up);
  const distance = camera.position.distanceTo(controls.target);
  if (on) {
    const halfHeight = (distance * Math.tan(THREE.MathUtils.degToRad(perspective.fov / 2))) / perspective.zoom;
    orthographic.top = halfHeight;
    orthographic.bottom = -halfHeight;
    orthographic.zoom = 1;
  } else {
    // Back to the perspective camera at the distance that shows as much.
    const halfHeight = orthographic.top / orthographic.zoom;
    const want = halfHeight / Math.tan(THREE.MathUtils.degToRad(perspective.fov / 2));
    const direction = next.position.clone().sub(controls.target).normalize();
    next.position.copy(controls.target).addScaledVector(direction, want);
    perspective.zoom = 1;
  }
  camera = next;
  controls.object = camera;
  updateProjection();
  controls.update();
}

// --- Views -------------------------------------------------------------------

function fitView() {
  const distance = (modelRadius / Math.sin(THREE.MathUtils.degToRad(perspective.fov / 2))) * 1.05;
  const direction = new THREE.Vector3(1, -1.2, 0.9).normalize();
  return {
    eye: modelCenter.clone().addScaledVector(direction, distance),
    target: modelCenter.clone(),
    up: new THREE.Vector3(0, 0, 1),
    orthographic: false,
  };
}

function viewFromData(view) {
  if (!view || !Array.isArray(view.eye) || !Array.isArray(view.target)) {
    return null;
  }
  const result = {
    eye: toWorld(view.eye),
    target: toWorld(view.target),
    up: Array.isArray(view.up) ? toWorld(view.up) : new THREE.Vector3(0, 0, 1),
    orthographic: view.orthographic === true,
  };
  if (result.eye.distanceToSquared(result.target) === 0 || result.up.lengthSq() === 0) {
    return null;
  }
  return result;
}

function applyView(view) {
  if (mode === 'person') {
    leavePerson();
  }
  useOrthographic(false);
  perspective.position.copy(view.eye);
  // Straight down or up: the orbit's +Z up cannot give the roll, so the eye
  // leans a little against the view's up, which keeps it on the screen's top.
  const direction = view.target.clone().sub(view.eye);
  const distance = direction.length();
  if (Math.abs(direction.z) > 0.9999 * distance) {
    perspective.position.addScaledVector(view.up.clone().normalize(), -1e-3 * distance * Math.sign(-direction.z));
  }
  perspective.up.copy(view.up).normalize();
  controls.target.copy(view.target);
  perspective.lookAt(view.target);
  // Orbiting keeps +Z up; a view whose up differs is turned to it.
  perspective.up.set(0, 0, 1);
  controls.update();
  useOrthographic(view.orthographic);
  redraw();
}

function home() {
  applyView(viewFromData(data.home) || fitView());
}

const viewSelect = $('views');
const namedViews = (Array.isArray(data.views) ? data.views : [])
  .map((view) => ({ name: String(view.name || 'View'), view: viewFromData(view) }))
  .filter((entry) => entry.view);
for (const [index, entry] of namedViews.entries()) {
  const option = document.createElement('option');
  option.value = String(index);
  option.textContent = entry.name;
  viewSelect.appendChild(option);
}
viewSelect.style.display = namedViews.length ? '' : 'none';
viewSelect.addEventListener('change', () => {
  const entry = namedViews[Number(viewSelect.value)];
  viewSelect.value = '';
  if (entry) {
    applyView(entry.view);
  }
  canvas.focus();
});
$('home-button').addEventListener('click', () => {
  home();
  canvas.focus();
});

// --- The model and the component tree ----------------------------------------

const nodesByName = new Map(); // glb node name -> objects
const itemOfNode = new Map(); // glb node name -> tree item
const meshes = [];
const treeItems = []; // { item, row, checkbox, parent, visible }
let collision = null; // logic.buildGrid of the visible triangles, built when needed

function base64ToBytes(text) {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

function visibleInWorld(object) {
  for (let o = object; o; o = o.parent) {
    if (!o.visible) {
      return false;
    }
  }
  return true;
}

function nodeOf(object) {
  for (let o = object; o && o !== root; o = o.parent) {
    if (o.userData && typeof o.userData.name === 'string') {
      return o;
    }
  }
  return null;
}

function buildTree() {
  let items = Array.isArray(data.tree) ? data.tree : [];
  if (!items.length) {
    // No tree: one item per glb node.
    items = [...nodesByName.keys()].map((name, i) => ({ id: `n${i}`, name, nodes: [name], children: [] }));
  }
  const panel = $('tree');
  const add = (item, parentList, parentEntry) => {
    const li = document.createElement('li');
    const row = document.createElement('div');
    const toggle = document.createElement('span');
    toggle.className = 'toggle';
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.checked = true;
    checkbox.title = 'Show or hide';
    const label = document.createElement('span');
    label.textContent = String(item.name || item.id || '');
    label.title = label.textContent;
    row.append(toggle, checkbox, label);
    li.appendChild(row);
    parentList.appendChild(li);
    const entry = { item, row, checkbox, parent: parentEntry, visible: true };
    treeItems.push(entry);
    for (const node of Array.isArray(item.nodes) ? item.nodes : []) {
      itemOfNode.set(String(node), entry);
    }
    checkbox.addEventListener('change', () => {
      entry.visible = checkbox.checked;
      applyVisibility();
    });
    label.addEventListener('click', () => {
      const objects = (item.nodes || []).flatMap((node) => nodesByName.get(String(node)) || []);
      showPicked(entry, objects[0] || null);
    });
    const children = Array.isArray(item.children) ? item.children : [];
    if (children.length) {
      const list = document.createElement('ul');
      li.appendChild(list);
      toggle.textContent = '▾';
      toggle.addEventListener('click', () => {
        li.classList.toggle('closed');
        toggle.textContent = li.classList.contains('closed') ? '▸' : '▾';
      });
      for (const child of children) {
        add(child, list, entry);
      }
    }
  };
  const list = document.createElement('ul');
  panel.appendChild(list);
  for (const item of items) {
    add(item, list, null);
  }
}

function applyVisibility() {
  const shown = (entry) => {
    for (let e = entry; e; e = e.parent) {
      if (!e.visible) {
        return false;
      }
    }
    return true;
  };
  for (const entry of treeItems) {
    const on = shown(entry);
    for (const node of Array.isArray(entry.item.nodes) ? entry.item.nodes : []) {
      for (const object of nodesByName.get(String(node)) || []) {
        object.visible = on;
      }
    }
  }
  collision = null;
  if (picked.object && !visibleInWorld(picked.object)) {
    clearPicked();
  }
  redraw();
}

function onModel(gltf) {
  const model = gltf.scene || (gltf.scenes && gltf.scenes[0]);
  if (!model) {
    fail('The design has no bodies to show.');
    return;
  }
  root.add(model);
  root.updateMatrixWorld(true);
  model.traverse((object) => {
    if (object.isMesh) {
      meshes.push(object);
    }
    if (object.userData && typeof object.userData.name === 'string') {
      const list = nodesByName.get(object.userData.name) || [];
      list.push(object);
      nodesByName.set(object.userData.name, list);
    }
  });
  const box = new THREE.Box3().setFromObject(root);
  if (box.isEmpty()) {
    box.set(new THREE.Vector3(-1, -1, -1), new THREE.Vector3(1, 1, 1));
  }
  const sphere = box.getBoundingSphere(new THREE.Sphere());
  modelCenter.copy(sphere.center);
  modelRadius = Math.max(sphere.radius, 1e-6);
  setClipping(modelRadius / 1000, modelRadius * 100);
  controls.minDistance = modelRadius / 10000;
  controls.maxDistance = modelRadius * 50;
  buildTree();
  home();
  statusBox.classList.add('hidden');
}

try {
  const bytes = base64ToBytes(String(data.glb || ''));
  new GLTFLoader().parse(bytes.buffer, '', onModel, (error) => {
    fail(`The design could not be read: ${error && error.message ? error.message : error}`);
  });
} catch (error) {
  fail(`The design could not be read: ${error.message}`);
}

// The visible triangles in world coordinates, nine numbers each.
function visibleTriangles() {
  root.updateMatrixWorld(true);
  const shown = meshes.filter(visibleInWorld);
  let count = 0;
  for (const mesh of shown) {
    const geometry = mesh.geometry;
    count += geometry.index ? geometry.index.count / 3 : geometry.attributes.position.count / 3;
  }
  const positions = new Float32Array(Math.floor(count) * 9);
  const v = new THREE.Vector3();
  let o = 0;
  for (const mesh of shown) {
    const position = mesh.geometry.attributes.position;
    const index = mesh.geometry.index;
    const n = index ? index.count - (index.count % 3) : position.count - (position.count % 3);
    for (let i = 0; i < n; i++) {
      v.fromBufferAttribute(position, index ? index.getX(i) : i).applyMatrix4(mesh.matrixWorld);
      positions[o++] = v.x;
      positions[o++] = v.y;
      positions[o++] = v.z;
    }
  }
  return positions.subarray(0, o);
}

function collisionGrid() {
  if (!collision) {
    collision = logic.buildGrid(visibleTriangles());
  }
  return collision;
}

// --- Picking -----------------------------------------------------------------

const raycaster = new THREE.Raycaster();
const pointerNdc = new THREE.Vector2();

function hitAt(clientX, clientY) {
  const rect = canvas.getBoundingClientRect();
  pointerNdc.set(((clientX - rect.left) / rect.width) * 2 - 1, -((clientY - rect.top) / rect.height) * 2 + 1);
  raycaster.setFromCamera(pointerNdc, camera);
  const hits = raycaster.intersectObjects(meshes, false);
  return hits.find((hit) => visibleInWorld(hit.object)) || null;
}

// World length of one pixel at a point.
function pixelSize(point) {
  const height = Math.max(1, container.clientHeight);
  if (camera.isOrthographicCamera) {
    return (camera.top - camera.bottom) / camera.zoom / height;
  }
  const distance = Math.max(camera.position.distanceTo(point), camera.near);
  return (2 * distance * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2))) / camera.zoom / height;
}

const picked = { entry: null, object: null, helper: null };

function clearPicked() {
  if (picked.helper) {
    scene.remove(picked.helper);
    picked.helper.dispose();
  }
  if (picked.entry) {
    picked.entry.row.classList.remove('picked');
  }
  picked.entry = picked.object = picked.helper = null;
  $('info').classList.remove('shown');
  redraw();
}

function showPicked(entry, object) {
  clearPicked();
  const names = [];
  for (let e = entry; e; e = e.parent) {
    names.unshift(String(e.item.name || e.item.id || ''));
  }
  const node = object ? nodeOf(object) : null;
  const nodeName = node ? node.userData.name : '';
  if (nodeName && (!entry || !names.includes(nodeName))) {
    names.push(nodeName);
  }
  if (!names.length) {
    return;
  }
  picked.entry = entry;
  picked.object = node || object;
  if (entry) {
    entry.row.classList.add('picked');
    entry.row.scrollIntoView({ block: 'nearest' });
  }
  if (picked.object) {
    picked.helper = new THREE.BoxHelper(picked.object, 0xff8800);
    picked.helper.material.depthTest = false;
    picked.helper.renderOrder = 10;
    scene.add(picked.helper);
  }
  const info = $('info');
  info.textContent = names.join(' › ');
  info.classList.add('shown');
  redraw();
}

function pickName(clientX, clientY) {
  const hit = hitAt(clientX, clientY);
  if (!hit) {
    clearPicked();
    return;
  }
  const node = nodeOf(hit.object);
  showPicked(node ? itemOfNode.get(node.userData.name) || null : null, hit.object);
}

// --- Modes -------------------------------------------------------------------

// 'free' (camera), 'place' (click a floor for the person), 'person', 'measure'
let mode = 'free';
const hint = $('hint');

function setHint(text, through = false) {
  hint.textContent = text;
  hint.classList.toggle('shown', Boolean(text));
  hint.classList.toggle('through', through);
}

function setMode(next) {
  mode = next;
  document.body.classList.toggle('place', mode === 'place');
  document.body.classList.toggle('person', mode === 'person');
  document.body.classList.toggle('measure', mode === 'measure');
  $('person-button').setAttribute('aria-pressed', String(mode === 'place' || mode === 'person'));
  $('measure-button').setAttribute('aria-pressed', String(mode === 'measure'));
  controls.enabled = mode !== 'person';
  if (mode === 'free') {
    setHint('');
  } else if (mode === 'place') {
    setHint('Click a floor to stand there. Esc cancels.');
  } else if (mode === 'measure') {
    setHint('Click two points to measure. Esc clears.');
  }
  redraw();
}

$('person-button').addEventListener('click', () => {
  if (mode === 'person' || mode === 'place') {
    if (mode === 'person') {
      leavePerson();
    } else {
      setMode('free');
    }
  } else {
    clearMeasure();
    setMode('place');
  }
  canvas.focus();
});

$('measure-button').addEventListener('click', () => {
  if (mode === 'measure') {
    clearMeasure();
    setMode('free');
  } else {
    if (mode === 'person') {
      leavePerson();
    }
    setMode('measure');
  }
  canvas.focus();
});

$('tree-button').addEventListener('click', () => {
  const open = document.body.classList.toggle('tree-open');
  $('tree-button').setAttribute('aria-pressed', String(open));
});

const help = $('help');
const toggleHelp = (show = !help.classList.contains('shown')) => help.classList.toggle('shown', show);
$('help-button').addEventListener('click', () => toggleHelp());
$('help-close').addEventListener('click', () => toggleHelp(false));
help.addEventListener('click', (event) => {
  if (event.target === help) {
    toggleHelp(false);
  }
});

// --- Keys --------------------------------------------------------------------

const keys = new Set();
const typing = (event) => event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement;
let lockLostAt = -Infinity;

window.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    onEscape();
    return;
  }
  if (typing(event)) {
    return;
  }
  if (event.key === 'h' || event.key === 'H' || event.key === '?') {
    toggleHelp();
    return;
  }
  if (event.key === 'Home') {
    home();
    return;
  }
  keys.add(event.code);
});
window.addEventListener('keyup', (event) => keys.delete(event.code));
window.addEventListener('blur', () => {
  keys.clear();
  through.mouse = through.touch = false;
});

function onEscape() {
  if (help.classList.contains('shown')) {
    toggleHelp(false);
  } else if (mode === 'person') {
    // The browser releases the pointer lock on Esc; only an Esc after that
    // (not the same key press) leaves the person.
    if (document.pointerLockElement !== canvas && performance.now() - lockLostAt > 300) {
      leavePerson();
    }
  } else if (mode === 'place') {
    setMode('free');
  } else if (mode === 'measure') {
    if (measure.points.length || measure.hover) {
      clearMeasure();
    } else {
      setMode('free');
    }
  } else {
    clearPicked();
  }
}

const pressed = (...codes) => codes.some((code) => keys.has(code));
const axis = (plus, minus) => (pressed(plus) ? 1 : 0) - (pressed(minus) ? 1 : 0);

// --- Free camera: flying with the keys ---------------------------------------

const _forward = new THREE.Vector3();
const _right = new THREE.Vector3();
const _move = new THREE.Vector3();

function fly(dt) {
  const forward = axis('KeyW', 'KeyS');
  const right = axis('KeyD', 'KeyA');
  const up = axis('KeyE', 'KeyQ');
  if (!forward && !right && !up) {
    return;
  }
  camera.getWorldDirection(_forward);
  _right.set(1, 0, 0).applyQuaternion(camera.quaternion);
  _move.set(0, 0, 0).addScaledVector(_forward, forward).addScaledVector(_right, right);
  _move.z += up;
  if (_move.lengthSq() === 0) {
    return;
  }
  const speed = Math.max(modelRadius * 0.5, 0.5 * metre) * (pressed('ShiftLeft', 'ShiftRight') ? 4 : 1);
  _move.normalize().multiplyScalar(speed * dt);
  camera.position.add(_move);
  controls.target.add(_move);
  controls.update();
  redraw();
}

// --- The person --------------------------------------------------------------

let person = null;
let savedCamera = null;
const through = { mouse: false, touch: false };
const stick = { x: 0, y: 0, pointer: null };
const eyeInput = $('eye-height');

function eyeHeight() {
  const value = Number(eyeInput.value);
  return Number.isFinite(value) && value > 0 ? value : logic.PERSON.eyeHeight;
}
eyeInput.addEventListener('change', () => {
  if (person) {
    person.eyeHeight = eyeHeight();
  }
});

function placePerson(hit, pointerType) {
  const grid = collisionGrid();
  if (!grid.triCount) {
    return;
  }
  camera.getWorldDirection(_forward);
  savedCamera = {
    position: camera.position.clone(),
    target: controls.target.clone(),
    orthographic: camera.isOrthographicCamera,
  };
  useOrthographic(false);
  // On a wall, stand in front of it, not in it.
  const feet = hit.point.clone();
  if (hit.face) {
    const normal = hit.face.normal.clone().transformDirection(hit.object.matrixWorld);
    if (normal.dot(raycaster.ray.direction) > 0) {
      normal.negate();
    }
    if (Math.abs(normal.z) < 0.7) {
      normal.z = 0;
      feet.addScaledVector(normal.normalize(), logic.PERSON.radius * metre * 1.05);
    }
  }
  person = logic.createPerson([feet.x, feet.y, feet.z], {
    unitScale,
    eyeHeight: eyeHeight(),
    yaw: Math.atan2(_forward.y, _forward.x),
  });
  setClipping(0.02 * metre, Math.max(modelRadius * 4, 200 * metre));
  setMode('person');
  updatePersonHint();
  if (pointerType !== 'touch') {
    lockPointer();
  }
}

function leavePerson() {
  if (document.pointerLockElement === canvas) {
    document.exitPointerLock();
  }
  person = null;
  through.mouse = through.touch = false;
  stick.x = stick.y = 0;
  stick.pointer = null;
  setClipping(modelRadius / 1000, modelRadius * 100);
  setMode('free');
  if (savedCamera) {
    perspective.position.copy(savedCamera.position);
    perspective.up.set(0, 0, 1);
    controls.target.copy(savedCamera.target);
    perspective.lookAt(savedCamera.target);
    controls.update();
    useOrthographic(savedCamera.orthographic);
    savedCamera = null;
  }
}

function lockPointer() {
  if (!canvas.requestPointerLock) {
    return;
  }
  try {
    const result = canvas.requestPointerLock();
    if (result && typeof result.catch === 'function') {
      result.catch(() => updatePersonHint());
    }
  } catch {
    updatePersonHint();
  }
}

function updatePersonHint() {
  if (mode !== 'person') {
    return;
  }
  const throughNow = through.mouse || through.touch;
  if (throughNow) {
    setHint('Walking through walls and floors (Q/E: down, up)', true);
  } else if (document.body.classList.contains('touch')) {
    setHint('The stick walks, a drag looks. Hold Through walls to pass walls.');
  } else if (document.pointerLockElement === canvas) {
    setHint('W A S D walk, Shift runs. Hold the right mouse button to walk through walls. Esc releases the mouse.');
  } else {
    setHint('Click to look around with the mouse. Esc leaves the person.');
  }
}

document.addEventListener('pointerlockchange', () => {
  if (document.pointerLockElement !== canvas) {
    lockLostAt = performance.now();
    through.mouse = false;
  }
  updatePersonHint();
});

document.addEventListener('mousemove', (event) => {
  if (mode === 'person' && document.pointerLockElement === canvas) {
    logic.turnPerson(person, event.movementX || 0, event.movementY || 0);
  }
});

// The right button walks through walls while held (the pointer may be locked:
// the events then come to the document).
document.addEventListener('mousedown', (event) => {
  if (mode === 'person' && event.button === 2) {
    through.mouse = true;
    updatePersonHint();
  }
});
document.addEventListener('mouseup', (event) => {
  if (event.button === 2 && through.mouse) {
    through.mouse = false;
    updatePersonHint();
  }
});
document.addEventListener('contextmenu', (event) => {
  if (mode === 'person' || event.target === canvas) {
    event.preventDefault();
  }
});

function walk(dt) {
  const grid = collisionGrid();
  const stickOn = stick.pointer !== null;
  const input = {
    forward: stickOn ? -stick.y : axis('KeyW', 'KeyS'),
    right: stickOn ? stick.x : axis('KeyD', 'KeyA'),
    up: axis('KeyE', 'KeyQ'),
    run: pressed('ShiftLeft', 'ShiftRight'),
    through: through.mouse || through.touch,
  };
  logic.stepPerson(grid, person, input, dt);
  const eye = logic.eyePosition(person);
  const look = logic.lookDirection(person);
  perspective.position.set(eye[0], eye[1], eye[2]);
  perspective.up.set(0, 0, 1);
  perspective.lookAt(eye[0] + look[0], eye[1] + look[1], eye[2] + look[2]);
  redraw();
}

// The on-screen stick and the through-walls button (touch).
const stickBox = $('stick');
const knob = $('stick-knob');
function moveStick(event) {
  const rect = stickBox.getBoundingClientRect();
  const radius = rect.width / 2;
  let x = (event.clientX - rect.left - radius) / radius;
  let y = (event.clientY - rect.top - radius) / radius;
  const length = Math.hypot(x, y);
  if (length > 1) {
    x /= length;
    y /= length;
  }
  stick.x = x;
  stick.y = y;
  knob.style.transform = `translate(${x * radius * 0.6}px, ${y * radius * 0.6}px)`;
}
stickBox.addEventListener('pointerdown', (event) => {
  stick.pointer = event.pointerId;
  stickBox.setPointerCapture(event.pointerId);
  moveStick(event);
  event.preventDefault();
});
stickBox.addEventListener('pointermove', (event) => {
  if (event.pointerId === stick.pointer) {
    moveStick(event);
  }
});
const releaseStick = (event) => {
  if (event.pointerId === stick.pointer) {
    stick.pointer = null;
    stick.x = stick.y = 0;
    knob.style.transform = '';
  }
};
stickBox.addEventListener('pointerup', releaseStick);
stickBox.addEventListener('pointercancel', releaseStick);
const throughButton = $('through');
const setTouchThrough = (on) => {
  through.touch = on;
  updatePersonHint();
};
throughButton.addEventListener('pointerdown', (event) => {
  throughButton.setPointerCapture(event.pointerId);
  setTouchThrough(true);
  event.preventDefault();
});
throughButton.addEventListener('pointerup', () => setTouchThrough(false));
throughButton.addEventListener('pointercancel', () => setTouchThrough(false));

// --- Measure -----------------------------------------------------------------

const measure = { points: [], hover: null, markers: [], line: null };
const markerGeometry = new THREE.SphereGeometry(1, 16, 12);
const markerColors = { vertex: 0xff6a00, edge: 0x1f6feb, face: 0x6b7280 };
const measureLabel = $('measure-label');

function marker(kind) {
  const material = new THREE.MeshBasicMaterial({ color: markerColors[kind], depthTest: false });
  const mesh = new THREE.Mesh(markerGeometry, material);
  mesh.renderOrder = 20;
  scene.add(mesh);
  return mesh;
}

function removeMarker(mesh) {
  if (mesh) {
    scene.remove(mesh);
    mesh.material.dispose();
  }
}

function clearMeasure() {
  for (const mesh of measure.markers) {
    removeMarker(mesh);
  }
  removeMarker(measure.hover);
  measure.markers = [];
  measure.points = [];
  measure.hover = null;
  if (measure.line) {
    scene.remove(measure.line);
    measure.line.geometry.dispose();
    measure.line.material.dispose();
    measure.line = null;
  }
  measureLabel.classList.remove('shown');
  redraw();
}

// The feature edges of a mesh (where faces meet at an angle, and open
// edges), in the mesh's own coordinates, computed once per mesh.
function featureEdges(mesh) {
  if (!mesh.userData.mitcadEdges) {
    const edges = new THREE.EdgesGeometry(mesh.geometry, 20);
    mesh.userData.mitcadEdges = edges.attributes.position.array;
    edges.dispose();
  }
  return mesh.userData.mitcadEdges;
}

// The picked point snapped to a corner or an edge near the cursor.
function snappedAt(clientX, clientY) {
  const hit = hitAt(clientX, clientY);
  if (!hit) {
    return null;
  }
  const mesh = hit.object;
  const local = mesh.worldToLocal(hit.point.clone());
  const scale = mesh.getWorldScale(new THREE.Vector3());
  const tolerance = (pixelSize(hit.point) * 10) / Math.max(scale.x, 1e-12);
  const snap = logic.snapPoint([local.x, local.y, local.z], featureEdges(mesh), tolerance);
  const point = mesh.localToWorld(new THREE.Vector3().fromArray(snap.point));
  return { point, kind: snap.kind };
}

function hoverMeasure(clientX, clientY) {
  const snap = snappedAt(clientX, clientY);
  removeMarker(measure.hover);
  measure.hover = null;
  if (snap) {
    measure.hover = marker(snap.kind);
    measure.hover.position.copy(snap.point);
    measure.hover.material.opacity = 0.7;
    measure.hover.material.transparent = true;
  }
  redraw();
}

function clickMeasure(clientX, clientY) {
  const snap = snappedAt(clientX, clientY);
  if (!snap) {
    return;
  }
  if (measure.points.length >= 2) {
    clearMeasure();
  }
  measure.points.push(snap.point);
  const mesh = marker(snap.kind);
  mesh.position.copy(snap.point);
  measure.markers.push(mesh);
  if (measure.points.length === 2) {
    const [a, b] = measure.points;
    const geometry = new THREE.BufferGeometry().setFromPoints([a, b]);
    measure.line = new THREE.Line(geometry, new THREE.LineBasicMaterial({ color: 0xff6a00, depthTest: false }));
    measure.line.renderOrder = 19;
    scene.add(measure.line);
    const delta = toModel(b.clone().sub(a));
    const distance = logic.distanceBetween(a.toArray(), b.toArray());
    measureLabel.textContent =
      `${logic.formatLength(distance, units)}  ` +
      `(ΔX ${logic.formatLength(delta.x, units)}, ΔY ${logic.formatLength(delta.y, units)}, ` +
      `ΔZ ${logic.formatLength(delta.z, units)})`;
    measureLabel.classList.add('shown');
  }
  redraw();
}

const _projected = new THREE.Vector3();

function updateMeasureOverlay() {
  for (const mesh of [...measure.markers, measure.hover]) {
    if (mesh) {
      mesh.scale.setScalar(pixelSize(mesh.position) * 5);
    }
  }
  if (measure.points.length === 2) {
    _projected.copy(measure.points[0]).add(measure.points[1]).multiplyScalar(0.5).project(camera);
    const behind = _projected.z > 1;
    measureLabel.style.left = `${((_projected.x + 1) / 2) * container.clientWidth}px`;
    measureLabel.style.top = `${((1 - _projected.y) / 2) * container.clientHeight}px`;
    measureLabel.classList.toggle('shown', !behind);
  }
}

// --- Pointer on the canvas: clicks, hover, person's drag look ----------------

let down = null; // { x, y, id, type }
let dragLook = null; // { id, x, y }
let hoverPending = null;

canvas.addEventListener('pointerdown', (event) => {
  document.body.classList.toggle('touch', event.pointerType === 'touch');
  down = { x: event.clientX, y: event.clientY, id: event.pointerId, type: event.pointerType };
  if (mode === 'person' && document.pointerLockElement !== canvas && event.button === 0) {
    dragLook = { id: event.pointerId, x: event.clientX, y: event.clientY };
  }
  if (mode === 'person') {
    updatePersonHint();
  }
});

canvas.addEventListener('pointermove', (event) => {
  if (dragLook && event.pointerId === dragLook.id) {
    logic.turnPerson(person, event.clientX - dragLook.x, event.clientY - dragLook.y);
    dragLook.x = event.clientX;
    dragLook.y = event.clientY;
    return;
  }
  if (mode === 'measure' && event.pointerType !== 'touch' && !event.buttons) {
    if (!hoverPending) {
      requestAnimationFrame(() => {
        if (mode === 'measure') {
          hoverMeasure(hoverPending.x, hoverPending.y);
        }
        hoverPending = null;
      });
    }
    hoverPending = { x: event.clientX, y: event.clientY };
  }
});

const endDragLook = (event) => {
  if (dragLook && event.pointerId === dragLook.id) {
    dragLook = null;
  }
};
canvas.addEventListener('pointerup', endDragLook);
canvas.addEventListener('pointercancel', endDragLook);

canvas.addEventListener('click', (event) => {
  if (!down || Math.hypot(event.clientX - down.x, event.clientY - down.y) > 5) {
    return; // A drag, not a click.
  }
  const type = down.type;
  if (mode === 'place') {
    const hit = hitAt(event.clientX, event.clientY);
    if (hit) {
      placePerson(hit, type);
    }
  } else if (mode === 'person') {
    if (type !== 'touch' && document.pointerLockElement !== canvas) {
      lockPointer();
    }
  } else if (mode === 'measure') {
    clickMeasure(event.clientX, event.clientY);
  } else {
    pickName(event.clientX, event.clientY);
  }
});

// --- The frame loop ----------------------------------------------------------

const clock = new THREE.Clock();

renderer.setAnimationLoop(() => {
  const dt = Math.min(clock.getDelta(), 0.1);
  if (mode === 'person' && person) {
    walk(dt);
  } else if (!help.classList.contains('shown')) {
    fly(dt);
  }
  if (!dirty) {
    return;
  }
  dirty = false;
  headlight.position.copy(camera.position);
  camera.getWorldDirection(_forward);
  headlight.target.position.copy(camera.position).add(_forward);
  if (picked.helper) {
    picked.helper.update();
  }
  updateMeasureOverlay();
  renderer.render(scene, camera);
});

canvas.focus();
