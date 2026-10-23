# Developing Mitcad

## Isolated builds

Dependency build scripts (Cargo, vcpkg, pip) run third-party code, so
Mitcad is built and tested only in isolated environments. Source and git
stay on the host; the working tree is mirrored one way into the build
environment before each build.

- Windows host: a **WSL distro** (no interop, no Windows drives) for
  Linux, a **Hyper-V VM** over SSH for Windows.
- Linux host: any Ubuntu 24.04 container or VM; `install-packages.sh` and
  `setup-user.sh` work there.
- Mac host (Apple Silicon): a **macOS guest VM** over SSH for macOS
  ([macOS: the build VM](#macos-the-build-vm)).

## Linux: the WSL distro

One-time setup (`<repo>` is the checkout as seen from the distro):

```bat
wsl --install Ubuntu-24.04 --name mitcad-dev
wsl -d mitcad-dev -u root -e sh -c "cp '/mnt/c/<repo>/tools/dev-env/wsl.conf' /etc/wsl.conf"
wsl --terminate mitcad-dev
```

Then, from Git Bash:

```bash
tools/dev-env/sync-to-wsl.sh
wsl -d mitcad-dev --cd "~/src/mitcad" -- sudo tools/dev-env/install-packages.sh
wsl -d mitcad-dev --cd "~/src/mitcad" -- bash -lc "tools/dev-env/setup-user.sh"
wsl -d mitcad-dev --cd "~/src/mitcad" -- bash -lc "cmake --preset dev && cmake --build --preset dev && ctest --preset dev"
```

| Script (`tools/dev-env`) | Use |
|---|---|
| `sync-to-wsl.sh [distro] [dir]` | Mirrors tracked and non-ignored untracked files into the distro (default `mitcad-dev`, `~/src/mitcad`). Only files whose content differs are replaced and get the sync time, so switching branches rebuilds exactly what changed. Deletes removed files; keeps `build/` and `CMakeUserPresets.json`. |
| `install-packages.sh` | System packages (compilers, CMake, Ninja, ccache, rsync, X11/OpenGL headers, gdb, Xvfb, xdotool, rsvg-convert, ImageMagick; mosquitto for the live updates' tests, its service disabled; libsecret's headers for QtKeychain and GNOME's keyring for the keychain's test). Run as root. |
| `setup-user.sh` | Pinned per-user toolchain: Rust from `rust-toolchain.toml`, cargo-deny, vcpkg at `vcpkg.json`'s `builtin-baseline`, Qt via aqtinstall, the render worker's Cycles with its CUDA device and Open Image Denoise (`build-cycles.sh --cuda`, once; `MITCAD_RENDER_CUDA=0` without CUDA); writes `CMakeUserPresets.json` (`dev` = Linux debug). Under WSLg sets `GALLIUM_DRIVER=d3d12`; `MITCAD_GPU_ADAPTER=NVIDIA` picks a GPU. |
| `install-appimage-tools.sh` | AppImage tools into `~/appimage-tools` (`MITCAD_APPIMAGE_TOOLS`), see [below](#tools-downloaded-from-releases). |

Parallel checkouts: `sync-to-wsl.sh mitcad-dev src/mitcad-<name>` and copy
`CMakeUserPresets.json` from the first one.

Gotchas:

- Use a login shell (`bash -lc`, `bash -l -s`) so cargo is on `PATH`.
- `wsl.exe` expands `$` on the Windows side. In Git Bash set
  `MSYS_NO_PATHCONV=1` and pipe anything non-trivial as a script:
  `wsl -d mitcad-dev -- bash -l -s < script.sh`. From PowerShell use
  `cmd /c 'wsl ... < script.sh'` (a PowerShell pipe adds a BOM and `\r`).
- `/tmp` in WSL can be emptied mid-session; keep files in `~`.
- Keep `sudo` password-protected; install as root with
  `wsl -d mitcad-dev -u root ...`.

## Windows: the build VM

A Hyper-V VM with Visual Studio 2022+ (C++ desktop tools) and OpenSSH:

```bash
tools/dev-env/sync-to-vm.sh <user>@<vm-host>
ssh -i <key> <user>@<vm-host> "powershell -ExecutionPolicy Bypass -File C:\dev\src\mitcad\tools\dev-env\setup-windows.ps1"
ssh -i <key> <user>@<vm-host> "cd /d C:\dev\src\mitcad && tools\dev-env\msvc.cmd cmake --preset dev -DMITCAD_MESA_DIR=C:/dev/mesa/bin && tools\dev-env\msvc.cmd cmake --build --preset dev && tools\dev-env\msvc.cmd ctest --preset dev"
```

| Script | Use |
|---|---|
| `sync-to-vm.sh [user@host] [dir]` | Like `sync-to-wsl.sh`, over SSH. Defaults: `MITCAD_VM`, `MITCAD_VM_DIR` (`C:/dev/src/mitcad`), `MITCAD_VM_KEY` (`~/.ssh/mitcad_vm`). |
| `setup-windows.ps1 [-Repo <dir>]` | Installs Git, CMake, Ninja, Rustup, Python, NSIS (winget), ccache, the pinned Rust toolchain, cargo-deny, vcpkg and Qt under `C:\dev`, the render worker's libraries (`build-cycles.ps1 -Cuda`, once; `MITCAD_RENDER_CUDA=0` without CUDA); writes `CMakeUserPresets.json` (`dev` = Windows release). Run as administrator. |
| `setup-ccache-windows.ps1 [-Dir] [-MaxSize]` | ccache into `C:\dev\ccache`, on the machine `PATH`, cache limit 4G (a build with the render worker; 2G was cleaned over and over). |
| `setup-mesa-windows.ps1 [-Out] [-Vcpkg] [-Jobs]` | Builds Mesa's llvmpipe (~20 min) into `<Out>\bin` (default `C:\dev\mesa`). Needed in a VM without a GPU (Windows offers only OpenGL 1.1). |
| `msvc.cmd <command>` | Runs a command in the VS x64 environment, rebuilding `PATH` from the registry first (SSH sessions keep a stale one). |

`-DMITCAD_MESA_DIR=<dir>/bin` copies Mesa's DLLs next to `mitcad.exe`
(development only, never in a release) and enables the Windows UI tests.
A second checkout in the VM needs its own directory and preset file.

## macOS: the build VM

An arm64 macOS guest on Apple's Virtualization.framework, e.g. with
[lume](https://github.com/trycua/lume) (MIT) or UTM; nothing is built on
the host:

```bash
brew install lume
lume config telemetry disable
lume create mitcad-mac --cpu 8 --memory 16 --disk-size 150 \
    --display 2880x1800 --ipsw latest --unattended tahoe
lume run mitcad-mac --detach --display none --vnc disabled
lume get mitcad-mac                      # the guest's IP address
```

No shared folders or clipboard; turn on Remote Login and install the Xcode
Command Line Tools with the macOS 27 SDK or newer, including the Metal SDK
headers and frameworks. The deployment target remains macOS 14.4; newer
AppKit APIs use runtime availability checks. The
CPU and Metal dependency builds use those tools. Open Image Denoise's
Metal device additionally needs Apple's offline `metal` and `metallib`
tools, supplied by Xcode or a separately installed Metal toolchain. Check
`xcrun --sdk macosx metal --version` in the build environment; the dependency
builder stops with setup instructions if the tools are missing. Cycles
compiles its own Metal kernels through the system's runtime compiler. Metal rendering still
needs a device and driver that can compile and execute those kernels;
see [rendering.md](rendering.md#macos). The UI tests need a user
logged in at the console (the unattended setup logs in `lume`) and a
screen of at least 2880 x 1800, or macOS shrinks their 1280 x 800 window.
Without a GPU, OpenGL runs on Apple's software renderer: enough for the
tests, but slow.

```bash
export MITCAD_MAC=<user>@<guest>
tools/dev-env/sync-to-mac.sh
ssh -i ~/.ssh/mitcad_mac $MITCAD_MAC "~/src/mitcad/tools/dev-env/setup-macos.sh"
ssh -i ~/.ssh/mitcad_mac $MITCAD_MAC "cd ~/src/mitcad && cmake --preset dev && cmake --build --preset dev && ctest --preset dev"
```

| Script | Use |
|---|---|
| `sync-to-mac.sh [user@host] [dir]` | Like `sync-to-vm.sh`. Defaults: `MITCAD_MAC`, `MITCAD_MAC_DIR` (`~/src/mitcad`), `MITCAD_MAC_KEY` (`~/.ssh/mitcad_mac`). Clears the target except `build` and `CMakeUserPresets.json` first; no `._*` files or extended attributes. |
| `setup-macos.sh [--compiler-caches-only]` | CMake, Ninja, ccache (universal) and sccache (arm64) release archives into `~/.local` (not Homebrew), the pinned Rust toolchain, cargo-deny, vcpkg at `vcpkg.json`'s baseline, Qt via aqtinstall (`~/Qt/<version>/macos`), the render worker's libraries (`build-cycles-macos.sh`, once); writes `CMakeUserPresets.json` (`dev` = `macos-debug`). Downloads are checked against the SHA-256 pins in `versions.sh`; an empty pin stops it unless `MITCAD_SKIP_HASH=1`. The optional flag installs only the two compiler caches, leaving other dependencies, shell files and presets alone. |

`tools/dev-env/versions.sh` holds the pins `setup-user.sh` and
`setup-macos.sh` share; the Windows script mirrors them by hand. The
presets `macos-debug` and `macos-release` build for arm64 with deployment
target 14.4 (the Qt 6.12 minimum) and the overlay triplet
`triplets/arm64-osx-dynamic.cmake` (OCCT as dylibs, the same deployment
target for vcpkg's libraries).

Compiler caches persist outside checkouts and build folders. ccache uses
`~/Library/Caches/ccache` by default (`ccache --show-config` shows any user
override); CMake enables its launcher for both C++ and Objective-C++,
including the macOS sources built with `-fobjc-arc`, unless that language
already has a launcher. `MITCAD_COMPILER_CACHE=OFF` disables these automatic
launchers. Linking, bundle assembly and hardened-runtime signing still run
normally; no cache executable is installed into the application bundle.

`MITCAD_RUST_COMPILER_CACHE` defaults to `ON` on macOS and `OFF` elsewhere.
When sccache is found, both Corrosion's Rust targets and the Cargo test-build
fixtures (debug and release, plus doctests) use it through `RUSTC_WRAPPER`,
with `CARGO_INCREMENTAL=0` and the cache in
`~/Library/Caches/Mozilla.sccache`. A missing tool is reported at configure;
a wrapper/server failure fails the build, even if the shell sets
`SCCACHE_IGNORE_SERVER_IO_ERROR=1`. A different explicit `RUSTC_WRAPPER`
requires disabling the option. Direct `cargo` commands retain the shell's
configuration. After changing a running sccache server's cache directory,
stop it once with `sccache --stop-server` before the next build.

Use `ccache --show-stats` and `sccache --show-stats` to inspect cache use.
sccache caches eligible library compilations; binaries, test harnesses,
procedural macros and other compilations that call the linker still run.
The pinned sccache's Rust keys also include the working directory and
Cargo's manifest paths: workspace crates in a different checkout may miss
even when registry dependencies hit. Its `SCCACHE_BASEDIRS` setting does
not normalize those Rust keys. Cache hits across checkouts therefore need
actual validation and are not guaranteed for every crate; see upstream's
[Rust limitations](https://github.com/mozilla/sccache/blob/v0.18.0/docs/Rust.md)
and [Rust key implementation](https://github.com/mozilla/sccache/blob/v0.18.0/src/compiler/rust.rs).

## macOS: bundle and native UI

- Configure `cmake --preset dev -DMITCAD_MACOS_NATIVE_UI=ON` to build
  the AppKit frontend. It starts by default in that build; `--ui=qt`
  runs the retained frontend and `--ui=native` explicitly selects AppKit.
  The native sources are confined to `app/platform/macos/` and compiled
  with ARC. Shared command/session changes also build without this option.
- `app.ui-macos-native` drives real native menu items, fields, buttons,
  history and command search in process, verifies model volumes and saves
  a composed screenshot. The older `app.ui-macos` and
  `app.ui-macos-workflow` explicitly use `--ui=qt` to cover retained
  workflows. Run GUI tests sequentially in the isolated VM. See
  [native-macos-ui.md](native-macos-ui.md) for rollout and manual checks.
- The app is a `MACOSX_BUNDLE` (`mitcad.app`) with
  `app/packaging/macos/Info.plist.in`. `MITCAD_BUNDLE_ID` (default
  `fi.nocodo.mitcad`) is a placeholder: fix it before the first release,
  since preferences and signatures are keyed on it. `Mitcad.icns` is built
  from `app/packaging/icon/mitcad.svg` by `mitcad-iconset` and `iconutil`.
- `cmake --install <build> --prefix <dir>` deploys Qt with `macdeployqt`,
  copies the vcpkg dylibs into `Contents/Frameworks` and the licence texts
  into `Contents/Resources/licenses`.
  It also installs the render worker `mitcad-render` (every build has it)
  in `Contents/MacOS`, its dependencies and Open Image Denoise's
  runtime-loaded CPU/BNNS and Metal modules in `Contents/Frameworks`, and their licence
  texts. The complete Cycles Metal runtime source tree is installed in
  `Contents/Resources/cycles/source`. `setup-macos.sh` prepares the pinned
  CPU and Metal rendering and denoising dependencies using
  `tools/dev-env/build-cycles-macos.sh` in the guest;
  see [rendering.md](rendering.md#macos).
- `tools/package-macos.sh [build dir]` installs, signs ad hoc, checks and
  makes a `.dmg`. Developer ID signing (`MITCAD_SIGN_IDENTITY`) and
  notarisation (`MITCAD_NOTARY_PROFILE`) need an Apple Developer account.
- `tools/check-bundle-macos.sh <Mitcad.app>`: only system and bundled
  libraries linked, relative rpaths, arm64 everywhere, and `Info.plist`,
  icon, licence texts, the complete Metal source manifest and the
  `libqmacstyle` style plug-in present
  (without it Qt falls back to its own cross-platform style).
- `app/platform/macos/*.mm` is Objective-C++ with ARC, linked with AppKit,
  as warning-free as C++ (`mitcad_warnings`); AppKit calls newer than 14.4
  are guarded with `@available`.
- `--chrome=docked|floating` and `--no-glass` try both layouts; the log's
  start-up lines say which was chosen. In the VM, look with the test
  driver (`MITCAD_TEST_INPUT`, `TestDriver.hpp`): `screenshot` and
  `screenshot-dialog` grab widgets without the card windows or glass;
  `screenshot-screen` saves the app's own windows as the window server
  shows them (no Screen Recording permission; software GL may be blank
  there, so glass is checked on hardware:
  [checklist](macos-trackpad-checklist.md)). `screencapture` and
  `osascript` do not work over SSH. Check both appearances
  (`defaults write -g AppleInterfaceStyle Dark`, `defaults delete -g
  AppleInterfaceStyle`) and restore light.

## Tools downloaded from releases

Build tools that are neither Ubuntu nor winget packages are downloaded
from a pinned GitHub release and checked against GitHub's published
SHA-256 digest. None is linked into Mitcad; only the AppImage runtime is
shipped.

| Tool | Script, folder | Release asset | SHA-256 |
|---|---|---|---|
| ccache 4.14.1 for Windows | `setup-ccache-windows.ps1`, `C:\dev\ccache` | https://github.com/ccache/ccache/releases/download/v4.14.1/ccache-4.14.1-windows-x86_64.zip | `6219f3865ca59aec41ee4b678df171d5d35855ecb2b6dbbbd20690b3a68af7b4` |
| ccache 4.14.2 for macOS (universal) | `setup-macos.sh`, `~/.local/bin` | https://github.com/ccache/ccache/releases/download/v4.14.2/ccache-4.14.2-darwin.tar.gz | `cf240009630b605b775c7c5c7b7105a3b196c79f31af322c1557969aa28003fc` |
| sccache 0.18.0 for Apple Silicon | `setup-macos.sh`, `~/.local/bin` | https://github.com/mozilla/sccache/releases/download/v0.18.0/sccache-v0.18.0-aarch64-apple-darwin.tar.gz | `308184519b646f5125289e8515b36f6ca65a13a041923994aebe702348674e8e` |
| linuxdeploy 1-alpha-20251107-1 | `install-appimage-tools.sh`, `~/appimage-tools` | https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage | `c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d` |
| linuxdeploy-plugin-qt, continuous build of 22 August 2026 | `install-appimage-tools.sh`, `~/appimage-tools` | https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-x86_64.AppImage | `cfc1055b2b9dbc08412b579f20990b7b41a17b61beaa5847dc9477c96c9e9617` |
| appimagetool 1.9.1 | `install-appimage-tools.sh`, `~/appimage-tools` | https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage | `ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0` |
| ISPC 1.31.0 (compiles Open Image Denoise) | `build-cycles.sh`, `<prefix>/src` | https://github.com/ispc/ispc/releases/download/v1.31.0/ispc-v1.31.0-linux.tar.gz | `d74089c835e10fd7e2c4b9225ced38b87d1fb53d35c7ceabd48cdf035da11b11` |
| ISPC 1.31.0 for Apple Silicon (compiles Open Image Denoise) | `build-cycles-macos.sh`, `<prefix>/src` | https://github.com/ispc/ispc/releases/download/v1.31.0/ispc-v1.31.0-macOS.arm64.tar.gz | `eac8009da38d41074d0adcf1fad4a3412fc9644a81ee5a49efeb07eac505b6ec` |

`build-cycles.sh --cuda` (mitcad#50; `setup-user.sh` passes it unless
`MITCAD_RENDER_CUDA=0`) fetches the CUDA compiler's
components (`cuda_nvcc` 12.9.86, `cuda_cudart` 12.9.79, `cuda_cccl`
12.9.27 of CUDA 12.9.1) from
https://developer.download.nvidia.com/compute/cuda/redist/ and checks
them against the SHA-256 digests NVIDIA lists in `redistrib_12.9.1.json`
there (pins in `versions.sh`); when no GCC of version 14 or older is
installed, it unpacks the distribution's `g++-14` packages
(`apt-get download`). `build-cycles.ps1 -Cuda` (mitcad#202;
`setup-windows.ps1` likewise) fetches the same components' Windows
archives (`CUDA_*_WINDOWS_SHA256`, from the same JSON) and uses MSVC as
the host compiler. All are build tools only; see
[rendering.md](rendering.md#building).

QtKeychain (BSD-3-Clause; the live updates' credentials in the system's
keychain, mitcad#89) is built by CMake on every platform from its release's
source archive (`cmake/Keychain.cmake`, FetchContent as for Corrosion),
pinned in `versions.sh` (`QTKEYCHAIN_VERSION` 0.17.0, `QTKEYCHAIN_SHA256` of
https://github.com/frankosterfeld/qtkeychain/archive/refs/tags/0.17.0.tar.gz),
as a static library against the build's Qt: the packages carry no library
of its own, only its licence text. On Linux it needs Qt's D-Bus module and
libsecret's headers (`libsecret-1-dev`); libsecret itself is loaded at run
time when the system has it, so Mitcad starts without it.

`build-cycles.sh` also builds the render worker's libraries from pinned
sources (`versions.sh`): Open Image Denoise 2.5.1's source release
(SHA-256 `e71fd043…c36`) and Cycles at tag v5.2.0 (commit `3b97e190…`);
see [rendering.md](rendering.md#building). Those are linked; ISPC is not.
On Windows `build-cycles.ps1` does the same with MSVC and reads the pins
of `versions.sh`; it takes ISPC's Windows release (SHA-256
`9a187938…140a`, `ISPC_WINDOWS_SHA256`); see
[rendering.md](rendering.md#windows).

linuxdeploy-plugin-qt publishes digests only for its continuous build, so
the check fails whenever it is rebuilt; update the digest in the script.
The tools are extracted (`--appimage-extract`, no FUSE). The AppImage
runtime is taken from the start of appimagetool's own AppImage (with its
update-information and signature sections cleared), so packing downloads
nothing.

## Windows installer

Built in the VM from a release build with CPack
([cmake/Packaging.cmake](../cmake/Packaging.cmake)):

```bat
tools\dev-env\msvc.cmd cmake --build --preset dev --target package
powershell -ExecutionPolicy Bypass -File tools\installer-windows-test.ps1 -MesaDir C:\dev\mesa\bin
```

`package` writes `mitcad-<version>-windows-x64.exe` (NSIS 3) and the same
files as a `.zip`. The installer script is our own,
[packaging/windows/NSIS.template.in](../packaging/windows/NSIS.template.in)
(CPack finds it through `CPACK_MODULE_PATH`). It runs as the user
(`RequestExecutionLevel user`) and installs either

- **just for me** (the default): into `%LOCALAPPDATA%\Programs\Mitcad`,
  with the Start menu shortcut, the `.mitcad` file type
  ([filetype.nsh](../packaging/windows/filetype.nsh)) and the uninstall
  entry under `HKCU`; no administrator rights, so no UAC prompt;
- **for all users:** into `Program Files\Mitcad`, registered under `HKLM`
  (32-bit view, as earlier releases). Only this choice asks for elevation:
  the installer starts itself again elevated (`runas`) with `/AllUsers
  /Elevated`.

The page *Choose Users* after the licence offers the choice; it starts on
*For all users* only when Mitcad is installed for all users and not for
the current user.
An earlier version in the same scope is uninstalled first (its own
uninstaller, silently, in place). Silent installs: `/S` installs just for
the current user, `/S /AllUsers` for all users (`/CurrentUser` says the
default explicitly), and `/D=<dir>` (last, without quotes) sets the folder
in both. A silent install whose `/D` is the folder of the installation
for all users, which is what the updater of an earlier release runs,
stays for all users. The installer writes the scope into
`<dir>\installation.ini` (`[Installation]`, `Scope=user` or `all`), which
the uninstaller and the [automatic updates](updates.md) read; an
installation without it is an earlier release's, which was for all users.
The uninstaller removes the files and the keys of its own scope only;
for all users it elevates as the installer does. Exit codes: 0 done, 1
cancelled, 2 aborted (also when the earlier version could not be
uninstalled), 740 elevated but still without administrator rights, 1223
elevation refused.

Contents: `bin` (executables, Qt, vcpkg DLLs,
VC++ runtime, `qt.conf`), `plugins` (via `windeployqt`), licences
(`LICENSE`, `THIRD-PARTY-NOTICES.txt`, `licenses\` with the font and every
vcpkg port's `copyright`), uninstaller. Never included: Mesa, Qt's
software OpenGL, PDBs. The render worker
`mitcad-render.exe` is installed with its DLLs (Open Image Denoise's too, its
CUDA device with `-Cuda`),
its CUDA kernels in `bin\cycles\lib` (a render prefix built with `-Cuda`)
and the licences of Cycles (with cuew's) and Open Image Denoise
([rendering.md](rendering.md#windows)).

`tools/installer-windows-test.ps1` (ctest `app.installer-windows` once
the installer exists) checks that the installer asks for no elevation,
then in each scope (`-Scope both|user|all`) installs silently into a
folder of the test's own and checks the scope's registration (uninstall
entry, `installation.ini`, file type, Start menu shortcut, nothing in the
other scope). Just for the current user it checks the files, runs
`mitcad-cli`, `mitcad.exe --version` and the render worker (a test render,
`--list-devices`; the build's GPU kernels installed) with a system-only `PATH` and,
with `-MesaDir`, renders a screenshot; for all users it reinstalls with
only `/D=` as an earlier release's updater does. With `-MesaDir` it tests
the automatic update in both scopes ([updates.md](updates.md#tests);
`-NoUpdate` skips it). Then it uninstalls and checks that no key of that
scope is left. It changes the `.mitcad` file type under `HKCU` and `HKLM`
and replaces an installation of either scope: run it only in the VM.
Session 0 cannot show a UAC prompt, so the test runs with administrator
rights and the elevation itself is not exercised.

Signing a release: [updates.md](updates.md#signing-a-release).

Icons: edit `app/branding/mitcad.svg` (64 px and up) and
`mitcad-small.svg` (below 64 px), then run `tools/make-icons.sh` in the
distro; the PNGs and `mitcad.ico` it writes are committed.

Not done yet: Authenticode signing (SmartScreen warns) and bundling the
licence texts of compiled-in Rust crates.

## Linux AppImage

`Mitcad-<version>-x86_64.AppImage`, built in the distro:

```bash
tools/dev-env/install-appimage-tools.sh            # once
cmake --build --preset dev --target appimage
```

The target ([cmake/AppImage.cmake](../cmake/AppImage.cmake),
[packaging/linux/make-appimage.sh](../packaging/linux/make-appimage.sh))
installs into `build/dev/appimage/AppDir` (executables, desktop file,
icons, licences incl. `packaging/linux/THIRD-PARTY-NOTICES.txt`), then:

- linuxdeploy copies needed libraries into `usr/lib` with `$ORIGIN`
  RUNPATH; its Qt plugin adds the X11/GLX platform, image formats, SVG
  icons and TLS backends. glibc, libstdc++, X11/xcb, OpenGL, fontconfig,
  FreeType and OpenSSL stay the system's.
- The render worker `mitcad-render` goes in
  with its libraries (and Open Image Denoise's CPU device module) and
  their licences ([rendering.md](rendering.md#appimage)).
- ELF files are stripped.
- appimagetool packs it with the pinned runtime. No AppImage update
  information and no GPG signature: Mitcad's own manifest signature is
  what counts.

Release AppImages come from a `linux-release`-based preset. Built on
Ubuntu 24.04 it needs glibc 2.39+. Without FUSE run it with
`--appimage-extract-and-run`.

At each start the AppImage writes its desktop file (`Exec` = the
AppImage, `StartupWMClass=Mitcad`) and icons into `$XDG_DATA_HOME` if they
differ, so docks show the icon
([app/framework/DesktopEntry.cpp](../app/framework/DesktopEntry.cpp)). No
GTK is bundled, so it uses the `xdgdesktopportal` platform theme unless
`QT_QPA_PLATFORMTHEME` is set.

`tools/appimage-test.sh` (ctest `app.appimage` once the AppImage exists)
checks that every ELF resolves its libraries inside the AppImage or from
system folders, runs it with an empty environment, renders with its
render worker when it has one, and updates it to itself from a locally
signed manifest.

## Tests

**Tolerance:** every geometric comparison in a test (lengths, positions,
distances, areas and volumes by their length scale) allows at least
0.000 001 mm (1e-6) unless the test says why it needs a tighter one. A
deviation within that, on any platform, is fixed by relaxing the test to
it; a larger one is a fault to fix or a case to agree on, not a reason to
relax silently. UI tests are not relaxed for one platform: the interface
differs by platform, and a failure there is a fault.

Run in the isolated environments:

- `ctest --preset dev`: geometry, bridge, model, solver, import,
  `mitcad-cli` scripts (`tools/cli/tests`), SPDX headers, dependency
  licences. `core.vcs.*` and `cli.remote` use a bare repository in a temp
  folder and a fake `ssh`; no network; skipped without git. `cli.library`
  makes a small parts library of fasteners with
  `tools/libraries/make-fastener-library.py` in the build tree (needs git
  and Python 3; skipped without them); `cli.profile-library` likewise one
  of BUILD parts with `tools/libraries/make-profile-library.py` from
  sections it makes; `cli.library-previews` renders two fasteners'
  previews with both generators and the render worker (skipped where it
  cannot render).
- The crates' tests (`core.model`, `core.vcs.*`, ...): the fixtures
  `core.cargo_build` and `core.cargo_build_release` build every crate's
  test binaries in one cargo run (`cargo test --no-run`, one lock on the
  target directory, the crates compiled in parallel), and the tests run
  the binaries directly, at the same time (`core/cargo-tests.cmake`;
  on Windows each run has `TMP` and `TEMP` in a folder of its own under
  `%LOCALAPPDATA%\mitcad-tests`, removed when it passes, as Storage Sense
  deletes the empty folders of git repositories just made in the
  system's temporary folder);
  `core.doctests` and `core.doctests_release` run the documentation tests.
  A large crate's tests run in parts by module
  (`mitcad_cargo_test(... PARTS <part> <filter>...)` in
  `core/CMakeLists.txt`): `core.vcs.remote`, `.sync`, `.library` and
  `.rest` (everything else, so a new module is tested before it gets a
  part), and `core.vcs.parts` checks with the binaries' `--list` that the
  parts hold each test exactly once.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` in
  `core/`.
- `tools/check-dependencies.sh`: `cargo deny` with `core/deny.toml`,
  including RustSec advisories, and the licence texts of the native
  libraries the build fetches from source and links (QtKeychain's
  BSD-3-Clause, in `build/dev/_deps`; `MITCAD_BUILD_DIR` for another
  build).
- Performance: `UI_APP=build/rel/app/mitcad tools/perf-measure.sh
  design.mitcad d3 21 20` on a release build;
  `mitcad-cli info design.mitcad --timings [--result-store <dir>
  --diagnostics]`. `cli.perf_recompute`, `cli.perf_join_recompute` and
  `cli.perf_holes_recompute` guard against large slowdowns.
  `mitcad-cli info design.mitcad --names` prints the names of every
  body's faces and edges, to compare the topology two builds (or one
  build with and without `MITCAD_NO_FAR_FEATURES=1`) give a design.
- `tools/repo-growth.sh <work dir> design.mitcad...` measures git
  repository growth of the design file formats over 50 saves.
- Live updates (mitcad#89): `core.vcs` runs the MQTT client's scenarios
  against a broker inside the test and against mosquitto when it is
  installed (`install-packages.sh`; `setup-macos.sh` with Homebrew,
  `setup-windows.ps1` with winget; skipped without it), each test with its
  own broker on free ports; the TLS test makes its certificate authority
  with `openssl` (skipped without it). Fuzz targets of the packet decoder
  and the message readers (cargo-fuzz, nightly Rust; by hand, not in the
  check script): in `core/vcs/fuzz`, `cargo +nightly fuzz run
  mqtt_packets` and `mqtt_messages` (its `Cargo.toml` has the steps); the
  normal tests run a seeded random sample of the same input. The
  application's side is `tools/ui-live-test.sh` (skipped without
  mosquitto): the trust question, Live and Live offline, two instances
  (who has a design open, a version arriving at once), Preferences'
  switch and Forget, signing in to a TLS broker of the test's own
  (`openssl`, `MITCAD_TEST_LIVE_CA`) without a keychain and, when GNOME's
  keyring and `dbus-daemon` are installed, with one in a D-Bus session of
  the test's own.

### Environment variables and options for tests

| Variable | Effect |
|---|---|
| `MITCAD_LOG_PICKS=1`, `MITCAD_LOG_VOLUMES=1`, `MITCAD_LOG_TIMING=1` | Log pick positions, body volumes after changes, step durations. |
| `MITCAD_SETTINGS_DIR` | Settings as an INI file in this folder. |
| `MITCAD_AUTOSAVE_DIR`, `MITCAD_TEST_AUTOSAVE_SECONDS` | Recovery folder and autosave interval. |
| `MITCAD_TEST_RECOMPUTE_DELAY_MS=n[,job=m...]` | Every feature computed in a job takes n ms longer (`400,undo=800`), to exercise the progress dialog. |
| `MITCAD_COMPUTE_INLINE=1` | Jobs on the UI thread, for a debugger. |
| `MITCAD_TEST_OCCT_CRASH=<operation>` | That kernel operation (e.g. `fillet`, or Section Analysis' `clip` and `section`) crashes like an access violation in OCCT; also in `mitcad-cli`. |
| `MITCAD_TEST_OCCT_OUT_OF_MEMORY=<operation>` | An allocation in that kernel operation (e.g. `extrude`) fails with `std::bad_alloc`: the operation fails "out of memory" (mitcad#80). |
| `MITCAD_TEST_OCCT_ALLOCATION_FAILS=<operation>` | An allocation of OCCT's collections fails in that kernel operation (e.g. `extrude`) or in the healing of a body built from B-rep data (`heal`), and is caught there as OCCT's checker catches its failures: only OCCT's count of failed allocations tells, and the operation or the body's build fails "out of memory" (mitcad#132). |
| `MITCAD_CHECK_INPUTS=1` | Report operations that modify their input shapes ([architecture](architecture.md#geometry-kernel)). |
| `MITCAD_NO_FAR_FEATURES=1` | Booleans on perforated bodies work on the whole body and cut a pattern's copies at once ([architecture](architecture.md#geometry-kernel)), for comparisons. |
| `MITCAD_NO_NEAR_COPIES=1` | A mirror's `combine` joins every image with its body whole, also a near copy of it ([commands.md](../core/model/src/api/commands.md#mirror)), for comparisons. |
| `MITCAD_RESULT_STORE` | Result store folder; `off` disables it. |
| `MITCAD_RESULT_STORE_MIN_MS` | Store results that took at least this long (default 100). |
| `MITCAD_PROJECTS_DIR` | Where New Project creates projects instead of Documents/Mitcad. |
| `MITCAD_GIT` | git program for remotes (Preferences sets it for the app). |
| `MITCAD_LIBRARIES_DIR` | Where fetched component libraries are kept (default: `libraries` in the user's local data folder). |
| `MITCAD_TEXTURES_DIR` | The texture cache of Edit Appearances' texture sources: thumbnails and fetched maps (default: `textures` in the user's local data folder; mitcad#315). |
| `MITCAD_TEXTURE_FIXTURES` | The online texture sources' requests answered by files of this folder (`<folder>/<host>/<path>`, the query left out; a missing file is HTTP 404) instead of the network: UI tests (`tools/ui-appearance-polyhaven-test.sh`, mitcad#313). |
| `MITCAD_TEST_REMOTE_RETRY_SECONDS` | First push retry delay instead of a minute. |
| `MITCAD_TEST_REMOTE_CHANGE_CHECK_SECONDS` | Age of the last fetch after which the first change refetches (default two minutes). |
| `MITCAD_TEST_LIVE_RETRY_MS` | The live updates' first delay before connecting again to a broker (default one second, doubling up to five minutes; with this, up to 20 times it). |
| `MITCAD_MOSQUITTO` | The mosquitto program for the live updates' tests, when it is not on `PATH` or in the usual places. |
| `MITCAD_TEST_LIVE_CA` | A PEM file of certificate authorities the app's live updates trust besides the system's (the test broker's, `tools/ui-live-test.sh`). |
| `MITCAD_NO_UPDATE_CHECK=1` | No update checks; update test variables: [updates.md](updates.md#tests). |
| `MITCAD_TEST_LOG_URLS=1` | Addresses the app would open in the browser (a report's issue form) are logged, not opened; the UI test library sets it. |
| `MITCAD_LOG_TYPED_TEXT=1` | Text fields are logged as they change: `File dialog <title>: <text>` (Qt's file dialog's file name), `Dialog field <title>: <text>` and `Panel field <input>: <text>` (typed into another dialog's field or a command panel's input; never a password); the UI test libraries set it (`app/framework/TypedTextLog.hpp`). |
| `MITCAD_TEST_CRASH=app\|model-worker\|import-worker\|render-worker` | That process (or the model's worker thread) crashes as an access violation would, for the crash reports' tests ([app/COMMANDS.md](../app/COMMANDS.md#feedback-and-error-reports)). |
| `MITCAD_CRASH_DIR` | Where crash reports go instead of `crashes` in the local app data (the app sets it for its workers, with `MITCAD_CRASH_PARENT`). |
| `MITCAD_CORE_DUMPS=1` | Linux: `mitcad-cli`, `mitcad_run_parallel` and the corpus tests may leave core dumps (see *Core dumps* below). |
| `MITCAD_RENDER_SAMPLES`, `MITCAD_RENDER_TEST_SCENE=1`, `MITCAD_RENDER_WORKER` | The rendered view's samples per pixel; the fixed test scene instead of the document; another render worker than `mitcad-render` next to the app ([rendering.md](rendering.md)). |

App options for tests: `--demo` (a ready block), `--screenshot <png>`,
`--no-recovery`, `--no-native-dialogs` (Qt's file dialogs, so paths can
be typed), `--open <file>`, `--set d3=35`.

### Linux UI tests

`tools/ui-*-test.sh` start the app on a hidden Xvfb display and drive it
with real input (xdotool), checking the app's log and screenshots
([tools/ui-test-lib.sh](../tools/ui-test-lib.sh)). Each test takes its own
display, so they run in parallel. `MITCAD_UI_VISIBLE=1` shows a run on the
current display, `UI_APP` picks another build. Never start the app
visibly on a desktop in use.

- **No fixed sleeps.** `ui_sync` sends Pause; the app (started with
  `MITCAD_TEST_SYNC=1`) answers `Sync <n>` once preceding input and
  delayed logs are handled
  ([app/COMMANDS.md](../app/COMMANDS.md#logs-that-ui-tests-read),
  `app/framework/TestSync.hpp`). Fixed pauses only where a test checks
  that nothing happens, lets a long operation start before cancelling,
  paces drags, or avoids a double-click (`ui_apart`).
- **Typed text is confirmed.** A path or a value typed into a dialog goes
  through `ui_type_path "<title>" <path>` (focus the dialog, select all,
  type, Enter) or `ui_type_field "<title>" <text>` / `ui_type_text <text>`
  (the dialog `ui_focus_dialog` focused last), a command panel's input
  through `ui_type_in`: they wait until the app has logged exactly the
  typed text for the field (`MITCAD_LOG_TYPED_TEXT`), and type once more
  if a character was lost or the field had not taken the keyboard
  (mitcad#102).
- `ui_step` waits until no job runs (`ui_wait_idle`), since the window
  takes no input while the progress dialog shows; a test that cancels a
  job sends Esc without a step.
- Isolation: each test has its own `XDG_CONFIG_HOME`, `XDG_DATA_HOME`
  (autosave folder) and git config (`GIT_CONFIG_GLOBAL`,
  `GIT_CONFIG_NOSYSTEM`). Tests start the app with `--no-recovery`;
  `UI_RECOVERY=1` re-enables the recovery prompt.
- The layout: `ui_start_app` starts the app Docked (`--chrome=docked`),
  which the tests were written for, although Floating is the default
  (mitcad#429), unless the test's arguments choose a layout (`--chrome…`)
  or `UI_DEFAULT_CHROME=1` leaves it to the preference and the default
  (`ui-chrome-layout-test.sh`, `ui-browser-rows-test.sh`). Tests that
  start the app themselves pass `--chrome=docked` too.
- The result store is off (`MITCAD_RESULT_STORE=off`) because stored
  results would change how many features later steps compute; tests of it
  set `MITCAD_RESULT_STORE=<dir>` and `MITCAD_RESULT_STORE_MIN_MS=0`.
- No network: update checks are off (`MITCAD_NO_UPDATE_CHECK=1`);
  `ui-update-test.sh` uses its own local HTTPS server.
- Xvfb runs with `-noreset` so a restarted app finds the display at once.
- The app runs without gdb: gdb loads the debug build's symbols (about
  1 GB) at every start, and the app's crash handler logs a crash's signal
  and stack itself. `UI_GDB=1` starts it under gdb for gdb's backtrace.

### Windows UI tests

ctest `app.ui-windows` and `app.ui-windows-workflow` (configured with
`MITCAD_MESA_DIR`), with their own `MITCAD_SETTINGS_DIR` and
`MITCAD_AUTOSAVE_DIR`. An SSH session runs in session 0 without an input
desktop: `SendInput` and UI Automation do not work, so the tests post
window messages, and popups (command search, menus) are unavailable, so
commands go through shortcuts. As on Linux, a step waits for the app's
answer to the sync key (`MITCAD_TEST_SYNC`; the library posts Pause), and
a key's modifiers stay in the keyboard state until the app logs that it
took the key (`MITCAD_LOG_KEYS`): while a model job runs, Qt keeps the
input for later. As on Linux, `Ui-StartApp` starts the app Docked unless
the arguments choose a layout or `-DefaultChrome` leaves it to the
preference and the default (Floating, mitcad#429). See
[tools/ui-windows-lib.ps1](../tools/ui-windows-lib.ps1).

Not a test: `tools/ui-windows-window-shots.ps1 -App <mitcad.exe> -Out <dir>`
saves screenshots of the whole main window in the Docked chrome on the light
and the dark theme (the demo block, a feature selected, Extrude's panel
open), through the test driver in the same session, for comparing the
window's look before and after a change.

### macOS UI tests

ctest `app.ui-macos` (`cmake/MacUiTests.cmake`, option
`MITCAD_MACOS_UI_TESTS`) runs the bundle's binary with `--demo`, `--open`
and `--screenshot` and checks the screenshots with
`tools/ui-image-stats.py --shot`, as `app.ui-windows` does. The app renders
offscreen and nothing is clicked, so no Accessibility or Screen Recording
permission is needed, but a window server is: without one the test exits
with 77 and is skipped (`MITCAD_UI_REQUIRE_GUI=1` fails instead). Settings,
recovery, projects and thumbnails go to a temp folder, and
`QT_FORCE_STDERR_LOGGING=1` brings Qt's log to the test's logs.
`MITCAD_UI_LLDB=1` runs the app under `lldb --batch` for a crash's stack.
`app.ui-macos-workflow` runs step scripts (`tools/ui-scripts/*.mitcad-ui`)
through the in-process test driver (`MITCAD_TEST_INPUT`), so it works over
SSH. Helpers: [tools/ui-macos-lib.sh](../tools/ui-macos-lib.sh) (bash 3.2,
BSD userland).

`app.ui-macos-dialogs` checks the default Qt command search and parameter
sheet with real keyboard input in both light and dark appearance. It saves
focused captures, checks filtering and native shortcut text, and exercises
parameter creation, inline editing, validation and deletion. The colour
scheme request applies only to the test process. Set
`MITCAD_UI_REQUIRE_RETINA=1` to require a backing scale of at least 2 for
Retina acceptance; ordinary runs also support a 1x display.

### .f3d corpus tests

Real `.f3d` designs are never committed. These tests read local data and
are skipped without it:

- `MITCAD_F3D_CORPUS` (default `~/f3d-corpus`): `.f3d`/`.f3z` files,
  searched recursively. ctests `f3d.corpus_occt`, `f3d.corpus_meshes`,
  `f3d.corpus_import`, `f3d.corpus_timeline` (debug builds sample every
  n-th file), `core/f3d/tests/corpus.rs`, `design_corpus.rs`, and the
  `.f3d` part of `ui-import-test.sh` (`MITCAD_F3D_UI_PART`, else the
  smallest part).
- `MITCAD_F3D_DESIGN_REF` (default `~/f3d-design-ref`): an independent
  decoder's JSON per design, compared field by field in
  `design_corpus.rs`.
- `MITCAD_F3D_MODELS` (default `~/f3d-models`): reference models
  `<id>/<id>.f3d` with external dumps `<id>/<id>.json`
  ([SCHEMA.md](../core/import/SCHEMA.md)). ctest `f3d.models_loft` and
  `core/f3d/tests/design_models.rs`.

Full corpus runs need a release build: `test_f3d_import --corpus [dir]
[--reports DIR] [--time-limit S]` ([core/import/README.md](../core/import/README.md))
and `test_brep_import --corpus` ([core/f3d/CORPUS_REPORT.md](../core/f3d/CORPUS_REPORT.md)).

The corpus runs (`test_f3d_import --corpus` and `--models`,
`test_brep_import --corpus`, `test_exchange --corpus`, `freecad.corpus`,
`ipt.corpus`)
import each file in a child process of its own, several at once, the
largest files first ([core/tests/parallel_runs.hpp](../core/tests/parallel_runs.hpp)).
OCCT's global state stays per file, and a crash, a hang or too much memory
ends only that file, which is reported and counted as failed. The lines
come in the files' order and the totals are those of a run in one
process, so outputs compare line by line (apart from the times, and a line
on standard error that sums up the children).

| Option | Environment | Default |
|---|---|---|
| `--jobs N` | `MITCAD_CORPUS_JOBS` | the cores / 4, at most the memory available / the limit; `1`: every file in the one process, as before |
| `--memory SIZE` | `MITCAD_CORPUS_MEMORY` | `4G` per child: what it may commit (`RLIMIT_DATA` on Linux, inherited by its children; a job object's process memory limit on Windows; not enforced on macOS); `none` |
| `--file-timeout S` | `MITCAD_CORPUS_TIMEOUT` | `3600` s per child; `0`: none |

A child that cannot allocate fails its file, or hangs in a thread that
could not allocate until its time is up: keep the limit well above what a
file needs (the line on standard error gives the largest peak).

#### Core dumps

On Linux, `mitcad-cli` (every command), `mitcad_run_parallel` and the
corpus tests that start themselves per file (`test_f3d_import`,
`test_brep_import`, `test_exchange`) make themselves non-dumpable at
start (`prctl(PR_SET_DUMPABLE, 0)`), unless `MITCAD_CORE_DUMPS=1`: a
child that runs out of its memory limit aborts (a Rust allocation that
fails ends the process with `SIGABRT`), and WSL captures core dumps
through a pipe in `core_pattern`, which `ulimit -c 0` does not stop. Each
dump was the size of the process (gigabytes), was written to the host,
and took the WSL service down with every distribution in it. The flag is
per process and `execve` sets it again, so each program clears it itself;
the crash and its report are as before. A process that is not dumpable
cannot be attached to with `gdb -p` either: start it under `gdb`, or set
`MITCAD_CORE_DUMPS=1` (only where core dumps go to a file).

### .ipt corpus

Real `.ipt` part files are never committed. `MITCAD_IPT_CORPUS` (a
`PATH`-like list of folders) points at them; without it the tests skip:
the ctest `ipt.corpus` (`tools/cli/ipt-corpus.cmake`: every file imported
with `mitcad-cli import-ipt`, compared with a STEP file of the same part
next to it, `<name>.stp`, within 1e-6 or the limit the folder's
`references.tsv` gives the file), `core/ipt/tests/corpus.rs` and a step
of `ui-import-test.sh` (the smallest file). Files run as the other corpus
tests do (the options above). See
[core/import/README.md](../core/import/README.md#ipt-import).

### .dwg corpus

Real `.dwg` drawings are never committed. `MITCAD_DWG_CORPUS` points at a
folder; every `index.tsv` under it lists drawings (a header row naming the
columns: `file`, relative to the index, and data sheet values to check:
`length_mm`, `chord_distance_mm`, `tube_diameter_mm`, `tube_wall_mm`,
`mass_kg`, `density_kg_m3`, `volume_mm3`, `solids`, `components`; other
columns are notes). The ctest `dwg.corpus` (`tools/cli/dwg-corpus.cmake`)
imports each with `mitcad-cli import-dwg --check`: every body built and
valid, every 3D entity taken, the bodies within the drawing's extents,
and the row's values (see
[core/dwg/README.md](../core/dwg/README.md)); it skips without the
variable.

### FreeCAD reference models and corpus

FreeCAD documents are never committed (they contain B-rep files, and
FreeCAD's examples are LGPL). Reference data is made with FreeCAD itself,
in the distro only:

- `tools/freecad-export/install-freecad.sh`: FreeCAD 0.21.2, 1.0.2 and
  1.1.4 AppImages, checksum-verified, extracted to `~/freecad/<version>/`.
- `tools/freecad-export/run_all.sh [--no-examples] [version...]` runs
  FreeCAD headless (`xvfb-run`): each script in
  `tools/freecad-export/models/` is saved as
  `~/fcstd-models/<version>/<id>.FCStd` with a dump `<id>.json`
  (`dump.py`: objects, enumerations, placements, stored shapes'
  properties, sketches as FreeCAD reads them, expression values). Model
  families: `sketch_*`, `pd_*`, `part_*`, `expr_*`. Script directives:
  `# Mitcad expects: no fallback` (the corpus test fails if any feature
  falls back) and `# Mitcad change: Width = 60 mm` with a FreeCAD-made
  variant `<id>_changed.FCStd` (`variant()` in `_prelude.py`). FreeCAD's
  bundled examples go to `~/fcstd-examples/<version>/`. `enums.py` and
  `merge_enums.py` produce `~/fcstd-models/enums.json`, the source of
  `core/freecad/data/enums.json`.
- `MITCAD_FCSTD_CORPUS` (a `PATH`-like list): ctest `freecad.corpus`
  (`tools/cli/fcstd-corpus.cmake`) imports every file with `mitcad-cli
  import-fcstd`, compares with the dump (`--reference`) and replays
  variants (`--set`), each file in a child process of its own
  (`mitcad_run_parallel`, the options above through the environment;
  `MITCAD_CORPUS_JOBS=1` checks them in the script's process).
  `core/freecad/tests/sketch_corpus.rs` compares every
  sketch with FreeCAD's reading. Example:
  `MITCAD_FCSTD_CORPUS=~/fcstd-models:~/fcstd-examples ctest -R freecad`.

### SolidCraft designs

SolidCraft designs are never committed except the import's own test
designs, written by hand (`core/solidcraft/tests/designs`, with their
expected values worked out from SolidCraft's definitions; SolidCraft is
not built or run for them). `MITCAD_SOLIDCRAFT_CORPUS` (a `PATH`-like
list of folders) points at real ones: ctest `solidcraft.corpus`
(`tools/cli/solidcraft-corpus.cmake`) imports every `.solidcraft` with
`mitcad-cli import-solidcraft` and checks one with `<name>.expected.json`
next to it (SolidCraft's `solidcraft-cli eval` of it) against it; a folder's
`known.txt` (file name, a tab, the reason) lists files allowed to fail
their check. `core/solidcraft/tests/corpus.rs` reads every file. Skipped
without the variable.

### Designs of earlier versions

`MITCAD_DESIGN_CORPUS` (a `PATH`-like list) names folders of Mitcad
projects saved by earlier versions (each design with its project's
`.mitcad/brep` store), never committed. ctest `designs.corpus`
(`tools/cli/designs-corpus.cmake`) opens every `.mitcad` with `mitcad-cli
info` and fails when a sketch of one fails, such as a linked projection
whose source changed shape since (mitcad#249); skipped without the corpus.

### USDZ files

USDZ export (mitcad#123, [commands.md](../core/model/src/api/commands.md#usdz))
writes the binary USD layer with Mitcad's own code; the tests read it back
with `mitcad-usdz`'s reader (the package's 64-byte alignment, the layer's
header and metadata, prims, points, indices, normals, materials, subsets,
texture coordinates and transforms). The format was checked once with
OpenUSD's own validators when the export was written (mitcad#123); no USD
library is needed to build or test.

## Before a commit

For development and merges to main the check is scoped and fast; an
occasional miss is accepted:

1. On Linux: `tools/check-all.sh` in its default mode (add `--fresh`
   when CMake files changed), e.g. in the distro after `sync-to-wsl.sh`:
   `wsl -d mitcad-dev --cd "~/src/mitcad" -- bash -lc tools/check-all.sh`.
   It runs only the ctests, UI tests and corpus the change affects.
2. Windows or macOS only when the change touches that platform's own
   code (`app/platform/`, `Q_OS_*` branches, packaging, its scripts):
   a warning-free build and `ctest` there.
3. Commit in steps that build and pass the tests.

Before a release build, the full check: `tools/check-all.sh --full` on
Linux, and warning-free builds with `ctest` in the Windows VM (with
`MITCAD_MESA_DIR`) and the macOS VM (with `app.ui-macos`).

### The check script

`tools/check-all.sh [--full] [--ui-jobs N] [--ui auto|all] [--ctest auto|all] [--ctest-jobs N] [--corpus auto|always|never] [--corpus-tests N] [--corpus-jobs N] [--memory SIZE] [--fresh] [--only STEP,...]`

It checks at two depths:

- **By default** (development, and before a merge to `main` on a Linux
  machine; a Windows or macOS machine checks on its own platform) the check is
  scoped to the change and fast: a test runs only when what it depends on
  differs from every run in which it passed on the machine, a few long
  ctests that seldom catch anything are left out, and a UI test depends
  only on the areas it exercises (below). Now and then a fault slips
  through to the full check; that is accepted.
- **`--full`** (or `--release`), the check before a release build, runs
  everything: every ctest (`--ctest all`, with the release builds'
  doctests and `app.appimage`), the corpus (`--corpus always`) and every
  UI test (`--ui all`). Release builds are checked only this way.

| Step | |
|---|---|
| configure, build | Compiler warnings fail the check (an incremental build only shows those of what it compiled). |
| fmt, clippy, deps | clippy's target dir is `build/dev/clippy`, kept across syncs. |
| ctest | The tests whose sources changed (below), a quarter of the cores, without the corpus group; `cli.perf_*` run serially. |
| corpus | `f3d.corpus*`, `f3d.models_loft`, `freecad.corpus`, `ipt.corpus`, several at once (`--corpus-tests`), each importing several files at once (`--corpus-jobs`, `MITCAD_CORPUS_JOBS`). By default all their children together fit the run's memory limit less 1G at `MITCAD_CORPUS_MEMORY` (1536M) each, and the cores of a test slot; about the square root of that many tests run at once (12G, 32 cores and three slots: two tests, three files each), each child within 300 s (`MITCAD_CORPUS_TIMEOUT`). Every corpus test is a row of the table. `auto` runs them only when `CORPUS_SOURCES` or the corpus files differ from every passing run on the machine (`~/.cache/mitcad/corpus-passed`). `CORPUS_SOURCES`: the readers and the import, the bridge's kernel and import files (`core/ffi/src/kernel`, `*_import.rs`, `ipt_history.rs`, `exchange.rs`, `memory.rs`, `core/cpp`; not `lib.rs` or other families' files), the geometry, `mitcad-cli`'s imports (`tools/cli/import.cpp`, not `main.cpp`), the build files, the OCCT port, and the resolved dependencies of the import's crates (`cargo tree` of `CORPUS_CRATES`, instead of `Cargo.toml` and `Cargo.lock`, so a crate added for something else does not count). Changes in `core/model` alone don't trigger it: use `--corpus always` when they may affect imports. |
| `ui-<name>` | The `tools/ui-*-test.sh` whose sources changed (below), four at a time, longest first, 30 min limit each (`UI_TIMEOUT`); `ui-compute` runs alone at the end (flaky under load). |

ctests by what changed (`--ctest auto`, the default; mitcad#134): each
ctest has a fingerprint of what it depends on, and is skipped (an
indented `ctest:<name>` row, `skipped`) when it passed with that
fingerprint on the machine before (`~/.cache/mitcad/ctest-passed/<name>`,
shared by the checkouts). `tools/ctest-fingerprints.py` finds the
dependencies from the build itself, not from a list kept by hand: ctest's
own listing (`--show-only=json-v1`), CMake's file API (check-all asks for
it before configuring; the targets, their folders, sources and
dependencies) and `cargo metadata` (the crates' dependencies). A test's
fingerprint covers:

- its registration (command, properties), without the checkout's paths,
  so that the checkouts share the passes;
- the files its command names: a cli test's JSON script or design, a
  `cmake -P` script, a Python or shell script and the files beside it;
- each program its command runs or names (the test executable,
  `mitcad-cli` as `-DCLI=...`): the CMake targets it is built from,
  through their dependencies. For each target its folder (less the
  folders of other CMake directories and of Rust crates, and less the
  files tests name, so that a cli test's JSON counts only for that test),
  its listed sources, and for a crate Corrosion builds (`mitcad_ffi`,
  `mitcad-release`) that crate's Rust closure; with them the build files
  (the top-level CMake files, `cmake/`, `triplets/`, `third_party/` with
  the OCCT port, `vcpkg.json`, `rust-toolchain.toml`), the compiler and
  the build's options;
- for a crate's tests (`core.vcs.*`, `core.model`, ...; the fixtures
  `core.cargo_build*`; the doctests) the Rust closure of the package: the
  workspace crates it depends on (with its dev-dependencies), the files
  they `include_str!` (so `core.mcp` follows `commands.md`), the external
  crates with their versions and features, `core/Cargo.toml` and
  `rust-toolchain.toml`;
- `core.dependencies` (cargo-deny): the manifests, `Cargo.lock` and
  `deny.toml`; `repo.spdx`: every file but documents;
- the fingerprints of the fixtures it requires (`cli.export` follows
  `cli.exchange`'s), but not of the fixtures that build the crates' test
  binaries.

A test whose program none of this recognises depends on the whole tree,
as does every test when the file API's reply is missing. Markdown
documents count only through an `include_str!`. So a documentation change
runs no ctest, one in `core/vcs` the vcs tests and what links the bridge
(`cli.*`, `core.bridge`, ...), one in `geometry/` the geometry tests and
what links it, and one in a cli test's JSON only that test (and the tests
whose fixtures it sets up). `FULL_ONLY` tests (`app.appimage`,
`core.doctests_release`) run only with `--ctest all`. A test named in
`--only` (`--only ctest:core.model`) always runs; `--only ctest:<name>`
without `ctest` runs only the tests named. ctest still brings in the
fixtures a test needs (`core.cargo_build` before a crate's test), and
the tests run in one ctest call, in parallel. `check-logs/ctest-plan.tsv`
lists each test's decision; `tools/ctest-fingerprints.py plan ...
--explain <test>` prints what a fingerprint is made of, and
`tools/check-all.sh --dry-run` lists the ctests, the corpus and the UI
tests that would run for the working tree, building and running nothing.

UI tests by what changed (`--ui auto`, the default): a UI test is skipped
(a `skipped` row) when its fingerprint is one it passed with on the
machine, kept in `~/.cache/mitcad/ui-passed/<name>` and shared by the
checkouts as the corpus's are. The fingerprint is of the files' contents
(Markdown documents left out, here and in the corpus's, since no UI test
or corpus test reads them):

- the areas the test exercises, which it declares in its header,
  `# check-all sources: app/sketch core/solver tools/cli` (several such
  lines add up), which must exist: the parts of `app/` it drives
  (`app/sketch`, `app/commands`, `app/files`, `app/render`, ...), the
  crates and bridge files of its family (`core/vcs`, the import's crates),
  `tools/cli` when it makes its designs with `mitcad-cli`;
- the build files (`UI_BASE`: `third_party/`, the top-level CMake files
  and `cmake/`, `triplets/`, `rust-toolchain.toml`, `vcpkg.json`) and the
  build's options (`MITCAD_*` switches such as `MITCAD_COMPILER_CACHE`,
  the build type);
- its script and the test library (`UI_LIBRARY`: `tools/ui-test-lib.sh`,
  `ui-image-stats.py`, `xwd2png.py`);
- the main window's layout as the built application lays it out:
  `tools/ui-layout.sh` starts Mitcad with the demo design on a hidden Xvfb
  display once per check (also in a dry run, with the last build) and
  takes its `Window layout:` line, the window's size and minimum size and
  where the menu bar, the toolbars, the docks, the status bar and the 3D
  view are (kept as `check-logs/ui-layout.txt`). Every UI test clicks and
  takes screenshots in that window, so a change that moves or resizes any
  of it reruns every UI test, whichever files it was in (mitcad#412: a row
  added to the sketch palette made the window 9 px taller, and
  `ui-report`, which declares only `app/report`, was skipped until main
  failed). When the layout cannot be read, no UI test is skipped and no
  pass is kept;
- for the smoke set (`UI_SMOKE`: `ui-smoke`, `ui-workflow`, `ui-command`,
  `ui-file`) also everything the application is built from (`UI_COMMON`:
  `app/`, `geometry/`, `core/`).

So a change in `app/sketch` reruns the sketch tests and the smoke set, one
in `app/MainWindow.cpp`, `geometry/` or `core/model` only the smoke set
(the full check runs the rest), and one only in `core/vcs` the tests that
declare it and the smoke set; a change anywhere that alters the window's
layout reruns them all. A test that comes to exercise another area
must declare it, else changes there skip it until the full check. The
rule for what to declare: the areas whose behaviour the test checks (the
dialogs, panels and commands it drives, the crates behind them, the tools
that make its designs); what every test shares, the window and its
layout, is covered by the layout line, so a test does not declare
`app/MainWindow.cpp` only because it runs in the window. A panel or page
that is longer than its dock scrolls (command panels, the docked sketch
palette) rather than raising the window's minimum size, so adding rows to
it leaves the layout, and the other tests, as they were.
`--ui all` runs every UI test; a test named in `--only` (`--only
ui-sketch`) always runs. Passes of the ctests, the UI tests and the corpus
are kept only from runs that built (the build step ran), so a run on an
older build does not count for newer sources.

Results: `build/dev/check-timings.tsv` (plus ctest tests ≥ 10 s as
`ctest:<name>`, and the ctests skipped), history in `build/dev/check-timings-history.tsv`, logs in
`build/dev/check-logs/<step>.log`. Exit 0 only if all steps pass.
`--only ui-compute` or `--only build,ctest` reruns single steps. A UI
test that fails keeps the application's whole log there too
(`ui-<name>.app.log`, `MITCAD_UI_LOG_DIR`), and a rerun of a failed UI
test first renames its logs to `ui-<name>.failed.log` and
`ui-<name>.failed.app.log`, so the failure's logs stay for a look. A
UI test that crashed runs once more under gdb (`UI_GDB=1`), its log
`ui-<name>.gdb.log` and the app's in `ui-<name>.gdb/`; the test stays
failed whatever that run gives. UI tests run `--ui-jobs` at once, by
default `MITCAD_UI_JOBS`, else 4 (each takes about 0.5 G without gdb).

Test steps hold one of two machine-wide slots (`flock` on
`MITCAD_TEST_LOCK`, default `~/.mitcad-test.lock` and `.2`;
`MITCAD_TEST_SLOTS`), so at most two checks test concurrently. Each run
has a memory limit (`--memory`, `MITCAD_CHECK_MEMORY`, default 12G,
`none` to disable) enforced with `systemd-run --user --scope`. Build and
lint use half the cores unless `CMAKE_BUILD_PARALLEL_LEVEL` /
`CARGO_BUILD_JOBS` are set.

### Faster builds

- The Linux `dev` preset builds Mitcad as Debug but links OCCT's release
  libraries (`MITCAD_OCCT_RELEASE_IN_DEBUG`, default on; not on Windows,
  where debug and release runtimes don't mix). Set it `OFF` to step into
  OCCT. Cargo's dev profile optimises dependencies at `opt-level = 2` and
  Mitcad crates at 1 (`core/Cargo.toml`).
- C++ and macOS Objective-C++ go through ccache when installed (`MITCAD_COMPILER_CACHE`, default
  on). Paths are hashed relative to the checkout, so side-by-side
  checkouts share the cache. MSVC uses `/Z7` (CMP0141) because ccache
  can't cache shared PDBs. Rust can use sccache with
  `MITCAD_RUST_COMPILER_CACHE` (default on for macOS, off elsewhere),
  disabling incremental compilation only while that cache is active;
  see the [macOS cache setup and limitations](#macos-the-build-vm).
- OCCT comes from the overlay port in `third_party/vcpkg-ports` (patched:
  speed-ups of booleans and the checker, and the fillet's edge overflow,
  mitcad#121, which changes results; see its README), so vcpkg builds it
  from source, debug and release, on
  the first configure of a machine and after every change to the port:
  about 15 min in the WSL distro and 19 min in the Windows VM (both at
  once, vcpkg limited to 12 jobs with `VCPKG_MAX_CONCURRENCY`), with build
  trees of about 5 GB in the distro and 14 GB in the VM. They go to
  `$VCPKG_ROOT/buildtrees` and `packages` unless
  `-DVCPKG_INSTALL_OPTIONS=--x-buildtrees-root=<dir>;--x-packages-root=<dir>`
  moves them (in the VM, under the checkout's `build` folder, which the
  sync keeps), and can be deleted afterwards: every later configure, in
  any checkout, restores OCCT from vcpkg's binary cache (an archive of
  0.3 GB on Linux, 0.55 GB on Windows). macOS builds it the same way.

## Code conventions

- Code, comments, UI texts, docs and commit messages in English.
- C++17, warning-free with MSVC `/W4` and GCC `-Wall -Wextra -Wpedantic`,
  no compiler-specific constructs, no Windows path assumptions; builds and
  runs on Windows, Linux and macOS.
- Every source file starts with an SPDX line (`// SPDX-License-Identifier:
  MIT` or `# ...`), checked by `tools/check-spdx.sh`.
- Narrow strings (`std::string`, `char*`, paths) are UTF-8 on every
  platform. On Windows a new executable gets the UTF-8 code page with
  `mitcad_utf8_code_page(<target>)` (`cmake/Utf8CodePage.cmake`), so that
  its `argv` and file names are UTF-8 too.
- Match the surrounding style and comment density.
- A tooltip says in one short line what the control is or does: a
  command's (`CommandDef::tooltip`, the ribbon and the menus) at most 80
  characters, an input's or any other control's at most 100. No usage
  instructions, no parenthesised asides, no lists of exceptions; a short
  shortcut hint ("Place timber members (Enter places)") is allowed and
  counts. The detail goes into the [user guide](user-guide.md). The ctest
  `repo.tooltips` (`tools/tooltip-check`) checks the texts in `app/`.
- Metric defaults everywhere ([architecture](architecture.md#units)).
- New geometry operation: a `Kernel` method with a default
  `Err(Unsupported)` in `core/model/src/kernel.rs`, grouped by family;
  bridges per family (`core/ffi/src/kernel/<family>.rs`,
  `core/cpp/bridge/<family>.{hpp,cpp}`, `geometry/src/<family>.cpp`).
- New feature: a module in `core/model/src/features/`, registered in
  `features/mod.rs`, documented in `core/model/src/api/commands.md`. UI
  commands: [app/COMMANDS.md](../app/COMMANDS.md).

## Licence policy

- Mitcad's code is MIT with an SPDX identifier in every file.
- Third-party components are used under their most permissive offered
  licence (an MIT/Apache crate as MIT, Qt as LGPL-3.0).
- Not allowed: GPL-2.0-only, non-commercial or otherwise restricted
  licences, statically linked LGPL. LGPL components (OCCT, Qt) are linked
  dynamically.
- Accepted exception: the AppImage runtime (type2-runtime, MIT) is a
  separate program statically linked with libfuse (LGPL-2.1), shipped
  unchanged except for its cleared update information;
  `packaging/linux/THIRD-PARTY-NOTICES.txt` names it with the sources.
- Copied or adapted code keeps its licence and headers in `third_party/`.
  OCCT is built with patches of Mitcad's (under OCCT's licence, as
  `SPDX-License-Identifier` lines in them say) through the vcpkg overlay
  port `third_party/vcpkg-ports/opencascade`, which `vcpkg.json` names, so
  every platform builds the same OCCT, still as shared libraries; the
  patches are published with the source, the third-party notices name them,
  and [its README](../third_party/vcpkg-ports/README.md) gives their origin
  and licences.
- If GPL components are ever included, the distribution becomes
  GPL-3.0-or-later; keep such components behind replaceable interfaces.
- Rust dependencies are checked by cargo-deny (`core/deny.toml`).
- ccache (GPL-3.0-or-later) and sccache (MPL-2.0) are development tools
  invoked as separate processes. Neither is linked into or shipped with
  Mitcad, and neither adds a Cargo or native library dependency.
- Native libraries built from source and linked statically: QtKeychain
  (BSD-3-Clause, `cmake/Keychain.cmake`); its licence text goes into the
  packages and the third-party notices name it. `tools/check-dependencies.sh`
  checks the fetched source's licence text. libsecret (LGPL-2.1), which it
  uses on Linux, is loaded at run time from the system, not linked or
  shipped.
- GPU SDKs (the render worker's GPU devices, mitcad#50,
  [rendering.md](rendering.md#licences)) are build tools only: nothing of
  NVIDIA's CUDA toolkit (CUDA EULA), AMD's HIP SDK or Intel's oneAPI
  compilers is linked statically or shipped; the GPU drivers are loaded
  at run time (Cycles' cuew and hipew, Apache-2.0), and the kernels
  shipped are Cycles' code compiled by them (the AppImage's
  `usr/lib/mitcad/cycles/lib`, the Windows installer's `bin\cycles\lib`;
  the notices name cuew). Open Image Denoise's CUDA device (Windows,
  `build-cycles.ps1 -Cuda`) contains CUTLASS (BSD-3-Clause) and CCCL
  header code and calls the driver through its own runtime; it links only
  the driver's import library. NVIDIA's OptiX SDK headers
  (a proprietary licence: binary-only, NVIDIA hardware only, recipients
  bound to its terms) would be compiled in, so Mitcad does not use OptiX
  (README, FAQ).
- This is not a legal review; licence combinations are checked before a
  release.

## MCP validation

The headless server is implemented in `core/mcp` (stdio protocol and
catalog), `core/ffi/src/mcp.rs` (native CAD tools and workspace paths),
and `tools/cli/mcp.cpp` (command-line entry). Live GUI access adds
`app/mcp/LiveServer.*`, the shared private endpoint discovery in
`app/mcp/LocalEndpoint.*`, `app/MainWindowMcp.cpp`, the application's own
tools in `app/MainWindowMcpUi.cpp` (mitcad#265) and the CLI's
`tools/cli/mcp-connect.*` stdio bridge. Which commands and queries MCP
allows follows their class in `core/model/src/api/classes.rs`; its unit
tests (in `core.model`) fail for a command or query without one. The CLI dispatches MCP
before configuring global component libraries, and suppresses OCCT's
stdout message printers. User setup and modeling examples:
[mcp.md](mcp.md).

In an approved isolated build environment, after mirroring the source
and building with the usual preset, run:

```bash
ctest --preset dev -R '^(cli\.mcp|core\.mcp)$' --output-on-failure
# In the isolated macOS VM with a logged-in window server:
ctest --preset dev -R '^app\.ui-macos-mcp$' --output-on-failure
# In the Windows VM, configured with MITCAD_MESA_DIR (session 0):
ctest --preset dev -R '^app\.ui-windows-mcp$' --output-on-failure
# In the Linux environment, on Xvfb:
tools/check-all.sh --only ui-mcp
```

`core.mcp` uses the Cargo build fixture that CTest includes automatically.
`cli.mcp` is registered when Python 3 is available and runs
`tools/cli/mcp-test.py` against the built native executable. Its subprocess
reads and shutdowns have timeouts, and it creates temporary workspaces
under the build directory, cleaning them after each test. It checks
native box geometry, file round trips and STEP export in addition to
protocol and path refusals. On Windows the test receives Qt's DLL
directory on `PATH`, like the other CLI tests (`mitcad-cli render` loads
Qt).

`app.ui-macos-mcp` runs the actual application and CLI with a private
`MITCAD_MCP_DISCOVERY_DIR`, settings and recovery directories under the
build tree. The in-process TestDriver verifies model volumes, native
history and viewport screenshots after live MCP edits. The Python harness
checks authentication, stale and insecure records, ambiguous instances,
notification and file-operation refusals, geometry errors with applied
mutations, undo, full-width integer IDs, malformed/oversized frames,
reconnect, EOF retention and GUI New invalidating old handles. It runs
serially with the other Mac GUI tests and needs no Accessibility permission.

The live debugging flow (mitcad#265,
[tools/ui-mcp-live.py](../tools/ui-mcp-live.py)) runs in `app.ui-macos-mcp`
and in the Linux UI test `ui-mcp` (`tools/ui-mcp-test.sh`) against a design
whose sketch holds a projection that no longer follows: queries and
`sketch.add_line` with the sketch open, `ui_state`, `ui_select`,
`ui_screenshot`, `ui_command` (Clean Up Stale Projections, Finish Sketch),
`document_save` and `ui_log`. `app.ui-windows-mcp`
(`tools/ui-windows-mcp-test.ps1`) drives the same flow from PowerShell
through `mitcad-cli.exe mcp --connect` in the Windows VM's session 0.
The Linux UI test `ui-mcp-files-camera` (`tools/ui-mcp-files-camera-test.sh`)
drives the bridge from the shell (jq, two FIFOs) for Save As and Open into
the folders agents may use and for the camera (mitcad#402, mitcad#405).

Follow the focused checks with `tools/check-all.sh` in the Linux isolated
environment and the documented Windows and macOS builds with CTest.
Builds, tests, formatters, package installation and runtime checks belong
in those isolated environments, never on the host. Commit messages must
identify any platforms whose checks were not run.
