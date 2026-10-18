# three.js

The 3D library of the web viewer (`app/web/viewer/`, the pages that
Export › Web page and the project's `web/` folder write). Every page
inlines these files, so the viewer runs offline from `file://`.

- Version: three.js r170 (released 31.10.2024).
- Source: the release tarball
  `https://github.com/mrdoob/three.js/archive/refs/tags/r170.tar.gz`,
  SHA-256 `bafff4b591c56f8f5e7e687f2a68d605506e479baad28813342748fb858b8c93`.
- Licence: MIT (`LICENSE`, copied from the tarball), Copyright © 2010-2024
  three.js authors. `three.module.js` keeps its own licence header.
- No npm and no build step: the files are the tarball's, copied as they are
  but for the one change below.

## Files

| Here | In the tarball | SHA-256 of the tarball's file |
|---|---|---|
| `three.module.js` | `build/three.module.js` | `ce1fa418de16a19495a9f72495580e3015d7745c296d3ce0485897f902ddedfb` |
| `addons/loaders/GLTFLoader.js` | `examples/jsm/loaders/GLTFLoader.js` | `45139faddd5aaf48ed2d62203d976e5cbd703db1a592de40527f9f6cf58abd44` |
| `addons/controls/OrbitControls.js` | `examples/jsm/controls/OrbitControls.js` | `80efaadea4f8a636a65fb0bd08bfef62f3d93a0bb94e2e7500f23176c5c07f4e` |
| `addons/utils/BufferGeometryUtils.js` | `examples/jsm/utils/BufferGeometryUtils.js` | `c25b7930e570e9ec56173cd3b866ec8d2e10016630db3937efb439daf1cedbf6` |
| `addons/environments/RoomEnvironment.js` | `examples/jsm/environments/RoomEnvironment.js` | `85869bdd22f7cf0b22e68dcefe6cb710480af0f5c901a76f7a7764fd07fc8014` |
| `LICENSE` | `LICENSE` | `4c40a1ef62450b857c3b2aaf294936304cd552d965fbcd9d32d4c5bcf4ba4454` |

## Changes

- `addons/loaders/GLTFLoader.js`: its one relative import,
  `'../utils/BufferGeometryUtils.js'`, now names the module by its
  specifier, `'three/addons/utils/BufferGeometryUtils.js'`. The page writer
  turns every module into a `data:` URL in an import map
  (`app/web/viewer/modules.txt`), where relative imports cannot resolve.
  The other files import only `'three'` and are unchanged.

## Updating

Download the new release's tarball in a build environment (not on a
workstation's host), check its SHA-256 against the release, copy the same
files, redo the change above and update the version, URL and checksums
here and in the third-party notices (`packaging/*/THIRD-PARTY-NOTICES.txt`,
`app/packaging/macos/Bundle.cmake`).
