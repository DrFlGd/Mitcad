# Automatic updates

Mitcad checks for newer releases. Where it can replace itself, Mitcad
installs a newer release with your consent. The user guide also gives
the user-level description, refer to [Updates](user-guide.md#updates).

## Behaviour

Mitcad checks for a newer release, shows a notice and, if the
installation permits it, installs the release. This section gives the
check, the notice and the installation on each platform. It also gives
the installations that only announce, a download that does not verify
and the report after an update.

**Check:** Mitcad checks for updates at start-up, at most once a day.
The check starts 3 s after the window opens. You can also start a check
from *Help › Check for Updates*. A silent automatic check reports
nothing on failure. It also reports nothing when Mitcad is up to date.
The manual check always reports.

**Notice:** when a newer release is available, Mitcad shows a bar under
the toolbar. The bar does not block your work. It has the buttons
*Release Notes*, *Install*, *Skip This Version* and *Later*. If you
skip a version, Mitcad does not announce that version again
automatically.

**Install on Windows (NSIS installation):** Mitcad downloads the
installer to `%TEMP%\mitcad-update` and verifies it. Then the window
closes. Mitcad asks you to save your work first. After Mitcad has quit,
the installer runs silently into the same folder and scope as the
current installation. Then Mitcad starts again.

The scope comes from the `installation.ini` of the installation (refer
to [development.md](development.md#windows-installer)). An installation
just for the current user updates as the user, without a UAC prompt. An
installation for all users (Program Files) updates elevated. UAC asks
for permission, unless you already have administrator rights.

An installation of an earlier release has no `installation.ini`. Such
an installation was for all users, thus it continues to update there.
The installer recognises the folder of the installation for all users
and elevates itself.

**Install on Linux (AppImage in a writable folder):** Mitcad downloads
the new AppImage next to the installed file, as the hidden file
`.<name>.update`. Mitcad verifies the download and makes it executable.
After Mitcad has quit, the new file replaces the old file in one atomic
step. Then the new Mitcad starts.

**Install on macOS (application bundle in a writable folder):** Mitcad
downloads the `macos-arm64` disk image to a private temporary folder and
verifies it. Then the window closes. Mitcad asks you to save your work
first. After the process has quit, a helper mounts the disk image
read-only, without a Finder window. The helper checks the identifier,
the version and the code signature of the new app. Then it copies
`Mitcad.app` beside the current installation, into a staging folder.

The helper exchanges the installed bundle and the staged bundle in one
atomic step on the same filesystem. The old bundle stays in the staging
folder until the helper detaches the disk image and macOS Launch
Services accepts the relaunch. If a step fails, the helper reverses the
exchange and starts the old version again. If the rollback itself fails,
the log gives the path of the old bundle that the helper kept. If the
filesystem does not support an atomic exchange, the update fails and the
installed bundle does not move.

**Announce only:** some installations get the notice without *Install*:

- The portable `.zip`
- Unbundled development builds
- System packages
- Read-only installations
- On macOS, an app that runs from a read-only disk image
- On macOS, an app on an App Translocation path
- On macOS, a quarantined bundle in Downloads

The startup log gives the reason. On macOS, copy the app to a writable
Applications folder first. Mitcad requests no elevation to replace a
bundle that the current user cannot write.

**A download that does not verify:** if the signature, the SHA-256 or
the size of a download does not match, Mitcad deletes the download and
reports the problem. Mitcad never runs such a download. Mitcad refuses a
manifest with a bad signature.

**Report:** after an update, the new Mitcad reports the result. The
report comes from the settings in
[Settings and turning checks off](#settings-and-turning-checks-off).

An older production macOS version, released before the bundle updater,
can announce a new release. This occurs when the manifest of the
release lists `macos-arm64`. But you must download and install that
release manually. The asset in the manifest cannot add installation
code to the older application. When a version with the bundle updater
is installed, that version can install subsequent releases
automatically with your consent.

The local UI test simulates an earlier version with the corrected
binary. Thus it does not prove an update from an older production
binary.

The macOS updater keeps the code signature, the resource forks and the
extended attributes of the bundle. It adds no quarantine attribute and
removes none. It does not sign the release again, and it does not change
the Gatekeeper policy. If a code signature is present but invalid, the
updater refuses the bundle before it replaces the old bundle. The
Ed25519 release signature authenticates the disk image. macOS code
signing and Gatekeeper apply independently.

An ad-hoc signature, as development builds use, does not establish a
notarized developer identity. Mitcad can verify an unsigned local bundle
as a download, but macOS can still refuse the bundle at launch. The
updater cannot change an unsigned or ad-hoc build into a notarized
application. It cannot give such a build a new Gatekeeper exception.

## Settings and turning checks off

The update settings are on the page *Tools › Preferences › Updates*. The
page has two settings:

- **Check for updates at start-up, once a day**: on by default
- The **channel**: *Releases* (stable, the default) or *Pre-releases
  too*

Mitcad stores the settings under these keys:

| Key | Content |
|---|---|
| `updates/automatic` | the check at start-up, on or off |
| `updates/channel` | `stable` or `prerelease` |
| `updates/skipped` | the version that *Skip This Version* selected |
| `updates/lastCheck` | the time of the last check (UTC) |
| `updates/hintShown` | |
| `updates/installing`, `updates/installingFrom`, `updates/installLog` | set during an update |
| `updates/lastResult` | the result after an update: `installed 0.1.0 -> 0.2.0`, or `failed ...: <reason>` |

With the setting off, Mitcad makes no request at start-up. *Check for
Updates* still asks when you select it.

An administrator can set update checks to off for every user, for
example for offline or managed installations. Then Mitcad makes no
request at all. *Check for Updates* makes none either. Preferences
shows the switch off and gives the reason. The administrator has two
switches:

- The environment variable `MITCAD_NO_UPDATE_CHECK` with any value but
  empty or `0`. This switch works on every platform, for example in
  `/etc/environment` or in the system's environment variables.
- On Windows, the registry value `DisableUpdateCheck` (DWORD, not 0)
  under `HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Mitcad\Mitcad`:

  ```bat
  reg add HKLM\SOFTWARE\Policies\Mitcad\Mitcad /v DisableUpdateCheck /t REG_DWORD /d 1 /f
  ```

## What is sent

A check is one HTTPS request for the manifest. On the pre-release
channel, a check is two requests: first GitHub's list of releases, then
the manifest. An installation adds the download. The requests carry the
User-Agent `Mitcad/<version> (<platform>; <channel>)`, for example
`Mitcad/0.2.0 (windows-x64; stable)`. The requests carry nothing that
identifies you or the machine:

- No cookies. Mitcad keeps none and sends none.
- No cache
- `Accept-Language: *` instead of your languages
- No telemetry

Mitcad uses HTTPS only. It follows a redirect only from HTTPS to HTTPS,
and at most five redirects. GitHub's downloads redirect to its CDN.

## Releases on GitHub

The project publishes releases on GitHub (`mitcad/Mitcad`). Each
release carries these assets:

| Asset | Content |
|---|---|
| `mitcad-<version>-windows-x64.exe` | the NSIS installer (`package`) |
| `mitcad-<version>-windows-x64.zip` | the portable `.zip` (`package`; announce only) |
| `Mitcad-<version>-x86_64.AppImage` | the Linux AppImage (`appimage`) |
| `Mitcad-<version>-macos-arm64.dmg` | the macOS application disk image (`dmg`) |
| `update-manifest.json` | the signed manifest (`update-manifest`) |

The tag is `v<version>`. The version is a semantic version, for example
`0.2.0` or `0.3.0-beta.1`. The two channels read different addresses:

- **stable:** Mitcad reads
  `https://github.com/mitcad/Mitcad/releases/latest/download/update-manifest.json`.
  GitHub's *latest* release is never a pre-release or a draft. Mitcad
  ignores a pre-release version in it anyway.
- **pre-release:** Mitcad reads
  `https://api.github.com/repos/mitcad/Mitcad/releases?per_page=20`.
  This is the REST API, unauthenticated. One request a day stays far
  below the rate limit. Mitcad takes the release with the highest
  version that is not a draft and has an `update-manifest.json`. Then
  Mitcad gets the manifest of that release and verifies it as on the
  stable channel.

Mitcad never offers a version that is not newer than the installed
version. Thus there are no downgrades. Build metadata (`+...`) does not
count in the comparison.

## For developers

The update code has three parts:

- [core/update](../core/update/src/lib.rs): the manifest, the
  verification, the versions and the `mitcad-release` tool. It uses
  `ed25519-dalek`, `sha2` and `semver`.
- The bridge `core/ffi/src/update.rs`
- `app/update/`: the requests, with Qt Network. Thus TLS is Qt's
  platform backend.

### The manifest

`update-manifest.json` has this form:

```json
{
"manifest": {
  "format": 1,
  "version": "0.2.0",
  "date": "2026-10-06",
  "notes": "https://github.com/mitcad/Mitcad/releases/tag/v0.2.0",
  "assets": {
    "windows-x64": {
      "url": "https://github.com/mitcad/Mitcad/releases/download/v0.2.0/mitcad-0.2.0-windows-x64.exe",
      "size": 42614672,
      "sha256": "<64 hex digits>",
      "signature": "<128 hex digits>"
    }
  }
},
"signature": "<128 hex digits>"
}
```

- `signature`: the Ed25519 signature of `mitcad-update-manifest\n`
  followed by the exact bytes of the `manifest` value as the file has
  them. No canonical JSON is necessary. If you edit the file in any way,
  the signature breaks. Mitcad checks the signature before it reads
  anything else of the manifest.
- `version`, `date` (`YYYY-MM-DD`) and `notes` (the release page,
  HTTPS).
- `assets`: one entry per platform. `windows-x64` is the installer,
  `linux-x64` is the AppImage and `macos-arm64` is the disk image. Each
  entry gives the HTTPS address of the download, its size, its SHA-256
  and the Ed25519 signature of its statement:

  ```text
  mitcad-update-file
  version 0.2.0
  platform windows-x64
  size 42614672
  sha256 <64 hex digits>
  ```

  If a platform has no asset, Mitcad shows the notice without
  *Install*.
- Keys, signatures and digests are lowercase hex. `format` is 1. Mitcad
  refuses a manifest of another format.

### Signing a release

The release key is an Ed25519 key. Its private half exists only in the
release environment, as a CI secret. The private half is never in the
repository. Its public half is built into Mitcad.

1. **Make the key** once and keep it secret. For example,
   `openssl rand -hex 32 > mitcad-release.key` writes the 32-byte seed
   in hex.
2. **Build the public key in.** The command
   `MITCAD_RELEASE_KEY_FILE=mitcad-release.key mitcad-release public-key`
   prints the public key. Put that line of 64 hex digits into
   [packaging/update-key.txt](../packaging/update-key.txt). As an
   alternative, point the CMake cache variable `MITCAD_UPDATE_KEY_FILE`
   at a file with the key. **Until then, the file holds a placeholder.
   A build without a key makes no update request at all.** Its *Check
   for Updates* says "This build of Mitcad has no release key, so it
   cannot verify updates". **The core refuses every manifest.** CMake
   reports this when it configures.
3. **Sign the release** after `package`, in the release environment.
   Give the key in `MITCAD_RELEASE_KEY` (the hex digits) or in
   `MITCAD_RELEASE_KEY_FILE` (a file with the hex digits):

   ```bat
   tools\dev-env\msvc.cmd cmake --build --preset dev --target package
   set MITCAD_RELEASE_KEY_FILE=D:\secrets\mitcad-release.key
   tools\dev-env\msvc.cmd cmake --build --preset dev --target update-manifest
   ```

   `update-manifest` runs the release tool `mitcad-release`, built from
   `core/update`. The tool signs the installer as the `windows-x64`
   download at `<MITCAD_RELEASE_URL>/download/v<version>/<file>`.
   `MITCAD_RELEASE_URL` is a CMake cache variable. Its default is
   `https://github.com/mitcad/Mitcad/releases`. The notes are at
   `.../tag/v<version>`.

   The tool makes sure that the checks of the application accept the
   manifest. Then it writes `update-manifest.json` next to the
   installer. The Linux build environment makes the Linux AppImage
   (`cmake --build --preset <release preset> --target appimage`). One
   run of the tool signs a release with several platforms:

   ```sh
   mitcad-release manifest --version 0.2.0 \
     --notes https://github.com/mitcad/Mitcad/releases/tag/v0.2.0 \
     --asset windows-x64 mitcad-0.2.0-windows-x64.exe \
             https://github.com/mitcad/Mitcad/releases/download/v0.2.0/mitcad-0.2.0-windows-x64.exe \
     --asset linux-x64 Mitcad-0.2.0-x86_64.AppImage \
             https://github.com/mitcad/Mitcad/releases/download/v0.2.0/Mitcad-0.2.0-x86_64.AppImage \
     --asset macos-arm64 Mitcad-0.2.0-macos-arm64.dmg \
             https://github.com/mitcad/Mitcad/releases/download/v0.2.0/Mitcad-0.2.0-macos-arm64.dmg \
     --out update-manifest.json
   mitcad-release verify --manifest update-manifest.json --key <public key> \
     --asset windows-x64 mitcad-0.2.0-windows-x64.exe
   ```

   Never give the key on the command line.
4. **Publish** the installers and `update-manifest.json` as the assets
   of the GitHub release.

There is no Authenticode signature yet. Thus Windows SmartScreen can
warn about a downloaded installer that you run by hand. Mitcad itself
checks the Ed25519 signature.

To change to a new release key, first build a release with both keys.
`packaging/update-key.txt` takes one key per line, and Mitcad accepts a
manifest signed with any of them. Sign that release with the old key.
Then installations can move to the new key.

### Tests

No test contacts any host but the machine itself. The tests are:

- `core.update` (ctest, `cargo test -p mitcad-update`): manifests and
  downloads signed with keys that the tests make. The test covers valid
  manifests and downloads, and tampered manifests and downloads. It
  covers the wrong key, no release key, older and equal versions and
  pre-releases on the stable channel. It also covers skipped versions,
  invalid manifests, GitHub's list of releases and the release tool.
- `app.ui-macos-update` (`tools/ui-macos-update-test.sh`, in the macOS
  guest): a local HTTPS manifest and test-owned, ad-hoc signed
  application bundles in a real disk image. The test checks the atomic
  exchange and the rollback. It checks announce only on the mounted
  read-only image and on a synthetic App Translocation path. It checks
  the rejection of a changed download and of a broken code signature,
  and the rollback on a Launch Services refusal. Then it checks the
  offer, the download, the verification and the installation on quit
  and relaunch, with the real version of the build. Only the version of
  the initial process is overridden, to simulate an earlier
  installation. The path fixture checks the translocation policy. It
  does not claim that the OS translocated the fixture. Real OS
  translocation, and a production signed release manifest offered to an
  installed previous release, are separate release acceptance checks.
- `tools/ui-update-test.sh` (Linux, `check-all.sh` runs it): the test
  runs against a local HTTPS server (`tools/update-test-server.py`, with
  a certificate made for the run) and a release key made for the run.
  It checks the three ways to set checks off: the setting, the
  administrator's switch and a build without a release key. It checks
  the notice, what the requests send, skip and later, refused manifests,
  an HTTP redirect and the pre-release channel. It checks a per-user
  AppImage (a script stands for one) that refuses a tampered download
  and then updates itself after it saves the changed design. It checks a
  read-only AppImage that only announces.
- `tools/installer-windows-test.ps1 -MesaDir ...`, and the ctest
  `app.installer-windows` (configured with `MITCAD_MESA_DIR`): the ctest
  runs once `package` has made the installer of the build, and is
  skipped otherwise. An installation just for the current user refuses
  a tampered download. Then it updates itself from a local manifest
  served over HTTPS, as an installation for all users also does. It
  saves the changed design, starts again as the new version and stays
  registered in its scope only.
- `tools/appimage-test.sh`, and the ctest `app.appimage`: the ctest runs
  once `appimage` has made the AppImage of the build, and is skipped
  otherwise. The real AppImage is signed as the `linux-x64` download
  with a key made for the test and served over HTTPS from 127.0.0.1.
  Mitcad offers it with *Install*. The AppImage replaces itself after
  Mitcad has quit and starts again as the release.

The tests point Mitcad at their server with environment variables. An
installed Mitcad reads these variables too. They open no new way in: a
person who can set the environment of a user can also change what Qt
loads.

| Variable | Content |
|---|---|
| `MITCAD_UPDATE_URL` | the stable channel's manifest instead of GitHub's (HTTPS) |
| `MITCAD_UPDATE_RELEASES_URL` | the pre-release channel's list of releases |
| `MITCAD_UPDATE_TEST_KEY` | a public key trusted besides the built-in one |
| `MITCAD_UPDATE_TEST_CA` | a PEM certificate the update requests trust besides the system's |
| `MITCAD_TEST_VERSION` | the version Mitcad compares with (the new Mitcad an update starts does not get it) |

The other UI tests set `MITCAD_NO_UPDATE_CHECK=1` (in `ui-test-lib.sh`
and `ui-windows-lib.ps1`). Thus they make no requests.
