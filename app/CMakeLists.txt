# SPDX-License-Identifier: MIT
find_package(Qt6 REQUIRED COMPONENTS Svg Network)

# Scheduling/activity support also serves the windowless render worker and
# CLI. Keep it independent of the QWidget-based native chrome.
if(APPLE)
  add_library(mitcad_mac_work STATIC platform/MacChrome.hpp platform/macos/MacWork.mm)
  target_include_directories(mitcad_mac_work PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
  target_link_libraries(mitcad_mac_work PUBLIC Qt6::Gui PRIVATE TKernel "-framework Foundation" mitcad_warnings)
  set_source_files_properties(platform/macos/MacWork.mm PROPERTIES COMPILE_OPTIONS -fobjc-arc)
endif()

qt_add_executable(mitcad
  main.cpp
  MainWindow.cpp
  MainWindow.hpp
  OcctViewer.cpp
  OcctViewer.hpp
  LayoutGrid.cpp
  LayoutGrid.hpp
  PickContext.cpp
  PickContext.hpp
  # The browser, the timeline and the parameters dialog (U3).
  browser/BrowserController.cpp
  browser/BrowserController.hpp
  browser/BrowserPanel.cpp
  browser/BrowserPanel.hpp
  # The Browser's background in the Floating layout (mitcad#419).
  browser/BrowserBackground.cpp
  browser/BrowserBackground.hpp
  browser/DocumentHost.hpp
  browser/DocumentSnapshot.cpp
  browser/DocumentSnapshot.hpp
  browser/AppearanceDialog.cpp
  browser/AppearanceDialog.hpp
  # Find in Design (mitcad#350).
  browser/FindDialog.cpp
  browser/FindDialog.hpp
  browser/FindInDesign.cpp
  browser/FindInDesign.hpp
  browser/ParametersDialog.cpp
  browser/ParametersDialog.hpp
  # Property sets of components, occurrences and bodies (mitcad#496).
  browser/PropertySetsDialog.cpp
  browser/PropertySetsDialog.hpp
  # Design policy of components (mitcad#220).
  browser/PolicyDialog.cpp
  browser/PolicyDialog.hpp
  browser/ReorientDialog.cpp
  browser/ReorientDialog.hpp
  browser/ComponentColors.hpp
  browser/TimelineWidget.cpp
  browser/TimelineWidget.hpp
  browser/TimelineSearch.cpp
  browser/TimelineSearch.hpp
  # The command framework (COMMANDS.md).
  framework/Appearances.cpp
  framework/Appearances.hpp
  framework/AutoselectPanel.cpp
  framework/AutoselectPanel.hpp
  framework/ChromeStyle.cpp
  framework/ChromeStyle.hpp
  # Restarting the application (mitcad#419: the layout preference).
  framework/Restart.cpp
  framework/Restart.hpp
  framework/Command.cpp
  framework/Command.hpp
  framework/CommandPresenter.hpp
  framework/CommandMessage.cpp
  framework/CommandMessage.hpp
  framework/CommandPanel.cpp
  framework/CommandPanel.hpp
  framework/CutTargetsPanel.cpp
  framework/CutTargetsPanel.hpp
  framework/CommandRegistry.cpp
  framework/CommandRegistry.hpp
  framework/CommandSearch.cpp
  framework/CommandSearch.hpp
  framework/CommandSession.cpp
  framework/CommandSession.hpp
  framework/DocumentSession.cpp
  framework/DocumentSession.hpp
  framework/DesktopEntry.cpp
  framework/DesktopEntry.hpp
  framework/Diagnostics.hpp
  framework/EventFilterGuard.hpp
  framework/Icons.cpp
  framework/Icons.hpp
  framework/Json.hpp
  framework/ManipulatorOverlay.cpp
  framework/ManipulatorOverlay.hpp
  framework/ModelShapes.cpp
  framework/ModelShapes.hpp
  framework/SettingsWindow.cpp
  framework/SettingsWindow.hpp
  framework/Ribbon.cpp
  framework/Ribbon.hpp
  framework/Selection.cpp
  framework/Selection.hpp
  framework/ShortcutDialog.cpp
  framework/ShortcutDialog.hpp
  framework/Dialogs.cpp
  framework/Dialogs.hpp
  framework/DialogFolders.cpp
  framework/DialogFolders.hpp
  framework/Theme.cpp
  framework/Theme.hpp
  framework/TitleBar.cpp
  framework/TitleBar.hpp
  # Command definitions: one registration per toolbar family (U4).
  commands/CommandFactories.hpp
  commands/Commands.hpp
  commands/CommandSupport.cpp
  commands/CommandSupport.hpp
  commands/ConstructCommands.cpp
  commands/CreateCommands.cpp
  commands/ExtrudeCommands.cpp
  commands/FaceCommands.cpp
  commands/FilletCommands.cpp
  commands/HoleCommands.cpp
  commands/InspectCommands.cpp
  commands/JointCommands.cpp
  commands/ModifyCommands.cpp
  # Dragging components the joints move, animating joints (mitcad#55).
  MainWindowJoints.cpp
  # Motion studies and contact sets (mitcad#454).
  MainWindowMotion.cpp
  motion/MotionStudyDialog.cpp
  motion/MotionStudyDialog.hpp
  commands/MoveCommands.cpp
  commands/PatternCommands.cpp
  commands/PointPatternCommands.cpp
  commands/PrimitiveCommands.cpp
  commands/SketchCommands.cpp
  commands/SweepCommands.cpp
  # Emboss (mitcad#458).
  commands/EmbossCommands.cpp
  # Gear (mitcad#472).
  commands/GearCommands.cpp
  # Equal spacing in sketches (mitcad#221).
  commands/EqualSpacingCommands.cpp
  # Surface modelling: the SURFACE group (mitcad#455).
  commands/SurfaceCommands.cpp
  # Sketch mode (U2).
  sketch/AutoConstraint.cpp
  sketch/AutoConstraint.hpp
  sketch/BodyGeometry.cpp
  sketch/BodyGeometry.hpp
  sketch/DrawTools.cpp
  sketch/EditTools.cpp
  sketch/SketchController.cpp
  sketch/SketchController.hpp
  sketch/SketchGeometry.cpp
  sketch/SketchGeometry.hpp
  sketch/SketchModel.cpp
  sketch/SketchModel.hpp
  sketch/SketchOverlay.cpp
  sketch/SketchOverlay.hpp
  sketch/SketchPalette.cpp
  sketch/SketchPalette.hpp
  sketch/SketchSnap.cpp
  sketch/SketchSnap.hpp
  sketch/SketchTool.hpp
  sketch/ToolSupport.hpp
  # The View menu, display settings and dialogs (U5).
  view/ViewController.cpp
  view/ViewController.hpp
  view/ViewNavigation.hpp
  # The File menu: import and export (U6).
  files/FileDialogs.cpp
  files/FileDialogs.hpp
  files/FileFormats.cpp
  files/FileFormats.hpp
  files/F3dImport.cpp
  files/F3dImport.hpp
  # An .iam assembly's parts with their design history (mitcad#507).
  files/IamImport.cpp
  files/IamImport.hpp
  files/ImportDeviations.cpp
  files/ImportDeviations.hpp
  files/MainWindowFiles.cpp
  # Background computation (P7): the worker thread, the progress dialog and
  # the main window's jobs.
  MainWindowJobs.cpp
  framework/ComputeProgress.cpp
  framework/ComputeProgress.hpp
  framework/ModelWorker.cpp
  framework/ModelWorker.hpp
  # Autosave to the recovery folder (P8) and the general settings.
  files/Autosave.cpp
  files/AutosaveFiles.cpp
  files/Autosave.hpp
  files/MainWindowAutosave.cpp
  framework/AppSettings.cpp
  framework/AppSettings.hpp
  # Recovery of autosaved work (P8c).
  files/Recovery.cpp
  files/Recovery.hpp
  # The result store on disk and its settings, the caches' diagnostics (P7d).
  framework/CacheDiagnostics.cpp
  framework/CacheDiagnostics.hpp
  framework/CachePreferences.cpp
  framework/CachePreferences.hpp
  framework/ResultCache.cpp
  framework/ResultCache.hpp
  # Saving versions in projects with version history (P12d).
  files/MainWindowVersions.cpp
  files/VersionDialogs.cpp
  files/VersionDialogs.hpp
  # The Version History window (P12e).
  files/VersionHistory.cpp
  files/VersionHistory.hpp
  # Help > About Mitcad.
  AboutDialog.cpp
  AboutDialog.hpp
  # Files the system asks to open (Finder, the Dock).
  files/FileOpenFilter.cpp
  files/FileOpenFilter.hpp
  # The in-process UI test driver (MITCAD_TEST_INPUT).
  framework/TestDriver.cpp
  framework/TestDriver.hpp
  # Floating chrome: the cards over the 3D view.
  framework/FloatingLayout.cpp
  framework/FloatingLayout.hpp
  framework/GlassCard.cpp
  framework/GlassCard.hpp
  # Numbers typed with a decimal comma or point (mitcad#2).
  framework/Numbers.cpp
  framework/Numbers.hpp
  # The sketch tools' precision cursor (mitcad#3).
  framework/Cursors.cpp
  framework/Cursors.hpp
  # The UI tests' sync key (mitcad#15).
  framework/TestSync.cpp
  framework/TestSync.hpp
  platform/WindowsTestSync.hpp
  # Typed text in the UI tests' log (mitcad#102).
  framework/TypedTextLog.cpp
  framework/TypedTextLog.hpp
  # Automatic updates (mitcad#9).
  update/UpdateClient.cpp
  update/UpdateClient.hpp
  update/UpdateController.cpp
  update/UpdateController.hpp
  update/UpdateInstaller.cpp
  update/UpdateInstaller.hpp
  update/UpdateNotice.cpp
  update/UpdateNotice.hpp
  update/UpdatePreferences.cpp
  update/UpdatePreferences.hpp
  update/UpdateSettings.cpp
  update/UpdateSettings.hpp
  # 3D Print: bodies to a slicer (mitcad#13).
  files/MainWindowPrint.cpp
  files/Print3d.cpp
  files/Print3d.hpp
  # Remote repositories: connect, open from a remote, sync (P12 remote).
  files/RemoteController.cpp
  files/RemoteController.hpp
  files/RemoteDialogs.cpp
  files/RemoteDialogs.hpp
  files/RemotePreferences.cpp
  files/RemotePreferences.hpp
  files/RemoteTask.cpp
  files/RemoteTask.hpp
  # Local and Cloud projects: New Project, Open Project, Open from Cloud,
  # Project Settings, the project indicator (mitcad#89).
  files/CloudAddress.cpp
  files/CloudAddress.hpp
  files/CloudSection.cpp
  files/CloudSection.hpp
  files/MainWindowProjects.cpp
  files/ProjectDialogs.cpp
  files/ProjectDialogs.hpp
  files/ProjectIndicator.cpp
  files/ProjectIndicator.hpp
  files/ProjectSettings.cpp
  files/ProjectSettings.hpp
  files/Projects.cpp
  files/Projects.hpp
  # Feedback and error reports: Help > Send Feedback, crash capture, the
  # preview and the issue form (mitcad#61, mitcad#62).
  report/CrashHandler.cpp
  report/CrashHandler.hpp
  report/ReportCenter.cpp
  report/ReportCenter.hpp
  report/ReportDialogs.cpp
  report/ReportDialogs.hpp
  report/ReportText.cpp
  report/ReportText.hpp
  # Component libraries and the community library (mitcad#64, mitcad#63).
  files/Libraries.cpp
  files/Libraries.hpp
  files/LibraryDialogs.cpp
  files/LibraryDialogs.hpp
  files/MainWindowLibraries.cpp
  # Edit locks of Cloud projects' designs and read-only windows (mitcad#89).
  files/LockController.cpp
  files/LockController.hpp
  files/MainWindowLocks.cpp
  # The Projects panel: projects, folders and designs (mitcad#145).
  projects/MainWindowProjectsPanel.cpp
  projects/ProjectsPanel.cpp
  projects/ProjectsPanel.hpp
  projects/ProjectsTree.cpp
  projects/ProjectsTree.hpp
  # The project picker: a place, a design or a project among the listed
  # projects instead of a file dialog (mitcad#174, mitcad#176).
  projects/ProjectPicker.cpp
  projects/ProjectPicker.hpp
  # Hybrid assemblies: moving the root's features into a component (mitcad#171).
  framework/HybridAssembly.cpp
  framework/MoveToComponentDialog.cpp
  framework/HybridAssembly.hpp
  # Sheet metal: its command group, browser rows and Flat Pattern export
  # (mitcad#210).
  commands/SheetCommands.cpp
  # Plastic part features (mitcad#459).
  commands/PlasticCommands.cpp
  MainWindowSheet.cpp
  # Move to... on a component occurrence: another parent (mitcad#219).
  framework/MoveOccurrenceDialog.cpp
  framework/MoveOccurrenceDialog.hpp)

# Toolbar and command icons (own drawings, MIT), rendered with Qt SVG.
file(GLOB MITCAD_ICONS RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}" CONFIGURE_DEPENDS icons/*.svg)
qt_add_resources(mitcad icons PREFIX / FILES ${MITCAD_ICONS})

# The application icon (branding/mitcad.svg, rendered by tools/make-icons.sh):
# the window's icon from the PNG files, Explorer's and the taskbar's from
# mitcad.ico in the executable's resources.
file(GLOB MITCAD_APP_ICONS RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}" CONFIGURE_DEPENDS branding/png/*.png)
qt_add_resources(mitcad branding PREFIX / FILES ${MITCAD_APP_ICONS})
if(WIN32)
  target_sources(mitcad PRIVATE branding/mitcad.rc)
endif()
# Narrow strings as UTF-8 on Windows too: the paths the application hands
# the bridge and the geometry as std::string (cmake/Utf8CodePage.cmake).
mitcad_utf8_code_page(mitcad)

# Sketch selection filters: three states per category, body edges in sight
# or behind faces, Shift's wider clicks, Project's sources (mitcad#217).
target_sources(mitcad PRIVATE
  framework/SelectionScope.cpp
  framework/SelectionScope.hpp
  framework/SelectionFilterPanel.cpp
  framework/SelectionFilterPanel.hpp
  MainWindowSelection.cpp)

# The view's context menu: the commands that fit the selection in Solid,
# BUILD and Construct, those it completes first and the others under More;
# Delete for whole things and Hide (mitcad#494, mitcad#347).
target_sources(mitcad PRIVATE MainWindowContextMenu.cpp)

# Parameter names picked from the view into a feature panel's value field:
# the visible sketches' dimensions and the earlier features' values as
# labels while the field has the keyboard (mitcad#250).
target_sources(mitcad PRIVATE
  framework/CommandSessionPicks.cpp
  framework/ParameterPicker.cpp
  framework/ParameterPicker.hpp
  MainWindowParameterPicks.cpp)

# The Docked chrome's window style: raised panels, their titles, one accent
# colour; on Windows 11 the frame through DWM (mitcad#251).
target_sources(mitcad PRIVATE
  framework/WindowStyle.cpp
  framework/WindowStyle.hpp
  platform/WindowsFrame.hpp)
if(WIN32)
  target_sources(mitcad PRIVATE platform/windows/WindowsFrame.cpp)
  # The UI tests' sync key on Windows (mitcad#271).
  target_sources(mitcad PRIVATE platform/windows/WindowsTestSync.cpp)
  # DwmSetWindowAttribute: dwmapi.dll is in every Windows since Vista; the
  # Windows 11 attributes are skipped at run time on older builds.
  target_link_libraries(mitcad PRIVATE dwmapi)
endif()

# Which bodies a feature's preview draws and where: hidden ones stay
# hidden, placed and linked components' bodies where their occurrences put
# them (mitcad#266).
target_sources(mitcad PRIVATE
  framework/PreviewBodies.cpp
  framework/PreviewBodies.hpp)

# The Sketch Palette's warnings as links that select what they are about,
# the view panned to them (mitcad#264).
target_sources(mitcad PRIVATE
  sketch/PaletteWarnings.cpp
  sketch/PaletteWarnings.hpp
  sketch/SketchReveal.cpp
  view/ViewReveal.cpp
  view/ViewReveal.hpp)

# The debug log: Preferences' switch, --debug-log, its files with their
# rotation, Help > Open Log Folder (mitcad#269).
target_sources(mitcad PRIVATE
  framework/DebugLog.cpp
  framework/DebugLog.hpp
  MainWindowDebugLog.cpp)
# The scoped wait cursor of the jobs the window waits for, its log lines
# and its idle recovery (mitcad#268).
target_sources(mitcad PRIVATE
  framework/WaitCursor.cpp
  framework/WaitCursor.hpp)

target_include_directories(mitcad PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_definitions(mitcad PRIVATE MITCAD_VERSION="${PROJECT_VERSION}")
target_link_libraries(mitcad PRIVATE
  Qt6::Widgets Qt6::OpenGL Qt6::OpenGLWidgets Qt6::Svg
  mitcad_ffi mitcad_geometry
  TKService TKV3d TKOpenGl TKMesh
  mitcad_warnings)
# The view's meshes of large curved faces (mitcad#398): mesh.hpp's
# bounded meshing, also on the model's worker thread at the end of jobs.
target_link_libraries(mitcad PRIVATE mitcad_geometry_io)
target_sources(mitcad PRIVATE
  framework/DisplayMeshes.cpp
  framework/DisplayMeshes.hpp)

if(WIN32)
  # wglGetCurrentDC() in OcctViewer.cpp
  target_link_libraries(mitcad PRIVATE opengl32)
  # Crash reports' stacks and function names (report/CrashHandler.cpp).
  target_link_libraries(mitcad PRIVATE dbghelp)
endif()
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  # The X keyboard mapping in framework/TestSync.cpp.
  find_package(X11 REQUIRED)
  if(NOT TARGET X11::xcb)
    message(FATAL_ERROR "libxcb (libxcb1-dev) is required")
  endif()
  target_link_libraries(mitcad PRIVATE X11::xcb)
endif()

if(APPLE)
  # Qt's and OCCT's OpenGL headers on macOS warn about the deprecated API.
  target_compile_definitions(mitcad PRIVATE GL_SILENCE_DEPRECATION)
endif()

if(APPLE)
  # Native macOS parts (SF Symbols, menu images) in Objective-C++.
  target_link_libraries(mitcad PRIVATE mitcad_mac_work)
  target_sources(mitcad PRIVATE platform/MacChrome.hpp platform/macos/MacChrome.mm)
  set_source_files_properties(platform/macos/MacChrome.mm PROPERTIES COMPILE_OPTIONS -fobjc-arc)
  target_link_libraries(mitcad PRIVATE "-framework AppKit")
  if(MITCAD_MACOS_NATIVE_UI)
    target_link_libraries(mitcad PRIVATE "-framework UniformTypeIdentifiers")
    target_compile_definitions(mitcad PRIVATE MITCAD_MACOS_NATIVE_UI=1)
    target_sources(mitcad PRIVATE
      MainWindowNative.cpp
      platform/NativeMacWindow.hpp platform/macos/NativeMacWindow.mm
      platform/NativeForms.hpp platform/macos/NativeForms.mm
      platform/NativeDialogs.hpp platform/macos/NativeDialogs.mm
      platform/NativeInlineEditor.hpp platform/macos/NativeInlineEditor.mm
      platform/NativeTestInput.hpp platform/macos/NativeTestInput.mm)
    set_source_files_properties(
      platform/macos/NativeMacWindow.mm platform/macos/NativeForms.mm
      platform/macos/NativeDialogs.mm platform/macos/NativeTestInput.mm
      platform/macos/NativeInlineEditor.mm
      PROPERTIES COMPILE_OPTIONS -fobjc-arc)
  endif()
endif()

set_target_properties(mitcad PROPERTIES WIN32_EXECUTABLE ON)

# Automatic updates (mitcad#9, docs/updates.md). The requests go through Qt
# Network (its TLS backends are Qt's plugins: no TLS library of Mitcad's
# own). The release keys the manifests and downloads are verified with: the
# file's lines of 64 hex digits (two while the key changes); while it has
# none (the placeholder), Mitcad makes no update request.
target_link_libraries(mitcad PRIVATE Qt6::Network)
# MCP connections target the open GUI document, using the shared protocol
# through the FFI rather than creating an invisible headless document.
target_sources(mitcad PRIVATE
  MainWindowMcp.cpp
  mcp/LocalEndpoint.cpp mcp/LocalEndpoint.hpp
  mcp/LiveServer.cpp mcp/LiveServer.hpp)
# Live MCP for debugging (mitcad#265): queries and sketch commands while a
# sketch is open, the UI's state, log and view, selections, UI commands by
# id, Save.
target_sources(mitcad PRIVATE
  MainWindowMcpUi.cpp
  framework/CommandPanelState.cpp
  framework/CommandSessionScript.cpp
  mcp/LiveErrors.hpp
  mcp/LogRing.cpp mcp/LogRing.hpp
  mcp/ModalGuard.cpp mcp/ModalGuard.hpp)
# Live MCP's Save As and Open into the folders the user allows agents
# (mitcad#402).
target_sources(mitcad PRIVATE mcp/AgentFolders.cpp mcp/AgentFolders.hpp)
if(WIN32)
  target_sources(mitcad PRIVATE mcp/WindowsEndpointSecurity.cpp mcp/WindowsEndpointSecurity.hpp)
  target_link_libraries(mitcad PRIVATE advapi32)
endif()
set(MITCAD_UPDATE_KEY_FILE "${PROJECT_SOURCE_DIR}/packaging/update-key.txt" CACHE FILEPATH
    "The public Ed25519 keys of Mitcad's releases (lines of 64 hex digits)")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${MITCAD_UPDATE_KEY_FILE}")
file(STRINGS "${MITCAD_UPDATE_KEY_FILE}" mitcad_update_key_lines REGEX "^[0-9a-fA-F]+$")
set(mitcad_update_keys "")
foreach(line IN LISTS mitcad_update_key_lines)
  string(LENGTH "${line}" length)
  if(NOT length EQUAL 64)
    message(FATAL_ERROR "${MITCAD_UPDATE_KEY_FILE}: '${line}' is not a key of 64 hex digits")
  endif()
  string(APPEND mitcad_update_keys " ${line}")
endforeach()
string(STRIP "${mitcad_update_keys}" mitcad_update_keys)
if(NOT mitcad_update_keys)
  message(STATUS "No release key in ${MITCAD_UPDATE_KEY_FILE}: no update checks")
endif()
target_compile_definitions(mitcad PRIVATE MITCAD_UPDATE_KEY="${mitcad_update_keys}")
# Feedback and error reports (mitcad#61, mitcad#62) go to the issue form of
# this tracker, filled in through its address and opened in the browser
# (report/ReportCenter.hpp): an issue list's address (".../issues") or a
# template with {title} and {body}. The setting reports/issueUrl overrides
# it. The default is the issue page README.md names.
set(MITCAD_ISSUE_URL "https://github.com/mitcad/Mitcad/issues" CACHE STRING
    "The issue tracker feedback and error reports open (an issue list's address, or a template with {title} and {body})")
target_compile_definitions(mitcad PRIVATE MITCAD_ISSUE_URL="${MITCAD_ISSUE_URL}")
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  # Mitcad's own functions by name in crash reports' stacks
  # (backtrace_symbols reads the dynamic symbol table): only the symbols
  # of its namespaces, so that nothing of a library linked in statically
  # takes the place of a shared library's.
  target_link_options(mitcad PRIVATE "LINKER:--dynamic-list=${CMAKE_CURRENT_SOURCE_DIR}/report/crash-symbols.list")
  set_property(TARGET mitcad APPEND PROPERTY LINK_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/report/crash-symbols.list")
endif()

# Live updates through an MQTT broker (mitcad#89, files/LiveController.hpp):
# the core's LiveHub, the trust and sign-in questions, the brokers' user
# names and passwords in the system's keychain through QtKeychain (linked
# statically, cmake/Keychain.cmake).
target_sources(mitcad PRIVATE
  files/Keychain.cpp
  files/Keychain.hpp
  files/LiveBrokers.cpp
  files/LiveBrokers.hpp
  files/LiveController.cpp
  files/LiveController.hpp
  files/LiveDialogs.cpp
  files/LiveDialogs.hpp
  files/LiveLink.hpp
  files/MainWindowLive.cpp)
target_link_libraries(mitcad PRIVATE qt6keychain)

if(WIN32)
  # Runs the installer once Mitcad has quit (update/UpdaterWindows.cpp): no
  # Qt, the C++ runtime linked in, so that it runs from the temp folder.
  add_executable(mitcad-updater WIN32 update/UpdaterWindows.cpp)
  set_target_properties(mitcad-updater PROPERTIES MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
  target_link_libraries(mitcad-updater PRIVATE shell32 advapi32 mitcad_warnings)
  add_dependencies(mitcad mitcad-updater)
endif()

if(APPLE)
  # The helper is copied out of the signed bundle before that bundle is replaced.
  add_executable(mitcad-updater update/UpdaterMacos.cpp)
  target_link_libraries(mitcad-updater PRIVATE mitcad_warnings)
  set_target_properties(mitcad-updater PROPERTIES RUNTIME_OUTPUT_DIRECTORY "$<TARGET_FILE_DIR:mitcad>")
  add_dependencies(mitcad mitcad-updater)
  qt_add_resources(mitcad macos_updater PREFIX /update BASE update FILES update/UpdaterMacos.sh)
endif()

if(BUILD_TESTING)
  # Unit tests of the application's own helpers, on Qt's offscreen platform
  # (no window); Qt's DLLs are not next to the test on Windows.
  qt_add_executable(test_app tests/test_app.cpp)
  # Numbers typed with a decimal comma or point (mitcad#2).
  target_sources(test_app PRIVATE framework/Numbers.cpp)
  # The sketch tools' precision cursor with its tool icons (mitcad#3); the
  # icons' dark variants ask the theme.
  target_sources(test_app PRIVATE framework/Cursors.cpp framework/Icons.cpp framework/Theme.cpp)
  qt_add_resources(test_app test_icons PREFIX / FILES ${MITCAD_ICONS})
  # 3D Print's slicers and file names (mitcad#13).
  target_sources(test_app PRIVATE files/Print3d.cpp files/Print3d.hpp)
  target_sources(test_app PRIVATE framework/Dialogs.cpp framework/Dialogs.hpp)
  # The AppImage's desktop file and icons in the user's data folder.
  target_sources(test_app PRIVATE framework/DesktopEntry.cpp framework/DesktopEntry.hpp)
  # The rendered view's exposure and view transforms (mitcad#47): plain Qt
  # Gui.
  target_sources(test_app PRIVATE render/RenderImage.cpp render/RenderImage.hpp)
  # The render device's choice and the CPU's fallback (mitcad#50): plain
  # C++.
  target_sources(test_app PRIVATE render/RenderDevices.cpp render/RenderDevices.hpp)
  # Feedback and error reports (mitcad#61, mitcad#62): masking, crash files,
  # duplicate keys, the issue form's address.
  target_sources(test_app PRIVATE report/ReportText.cpp report/ReportText.hpp)
  # The addresses of Cloud projects (mitcad#89): composed and read back.
  target_sources(test_app PRIVATE files/CloudAddress.cpp files/CloudAddress.hpp)
  # Atomic autosave generations through normal recovery (mitcad#110).
  target_sources(test_app PRIVATE tests/autosave.cpp files/AutosaveFiles.cpp files/Recovery.cpp framework/Dialogs.cpp)
  # The timeline search's words and filters (mitcad#179): plain Qt Core.
  target_sources(test_app PRIVATE browser/TimelineSearch.cpp browser/TimelineSearch.hpp)
  # The command panel's message within narrow panels (mitcad#198): plain Qt
  # Widgets.
  target_sources(test_app PRIVATE tests/command_message.cpp framework/CommandMessage.cpp framework/CommandMessage.hpp)
  # Explicit move destinations and cancellation (mitcad#201): modal Qt Widgets.
  target_sources(test_app PRIVATE tests/move_to_component.cpp framework/MoveToComponentDialog.cpp)
  # Persistent assembly-cut exclusions and preview-fed checklist (mitcad#200).
  target_sources(test_app PRIVATE tests/cut_targets.cpp framework/CutTargetsPanel.cpp framework/CutTargetsPanel.hpp)
  # The sketch's auto constraints while drawing (mitcad#189): plain Qt Core
  # and plane geometry.
  target_sources(test_app PRIVATE sketch/AutoConstraint.cpp sketch/AutoConstraint.hpp sketch/SketchGeometry.cpp)
  # Selection inputs' entries: a click toggles one per region (mitcad#199):
  # plain Qt Core.
  target_sources(test_app PRIVATE framework/Selection.cpp framework/Selection.hpp)
  # Solid-mode profile groups share the actual selection and an independent hover.
  target_sources(test_app PRIVATE tests/autoselect.cpp framework/AutoselectPanel.cpp framework/AutoselectPanel.hpp)
  # When a navigation of the view ends (mitcad#214): plain C++.
  target_sources(test_app PRIVATE view/ViewNavigation.hpp)
  # Where an orthographic render's camera stands (mitcad#215): plain C++.
  target_sources(test_app PRIVATE render/RenderCamera.hpp)
  # Event filters on the 3D view when the main window closes (mitcad#226):
  # plain Qt Widgets.
  target_sources(test_app PRIVATE tests/event_filter_guard.cpp framework/EventFilterGuard.hpp)
  # Move to... on an occurrence: the parent picker and cancellation
  # (mitcad#219): modal Qt Widgets.
  target_sources(test_app PRIVATE tests/move_occurrence.cpp framework/MoveOccurrenceDialog.cpp)
  # Sketch selection filters' three states, Shift and edge sight (mitcad#217):
  # plain Qt Core.
  target_sources(test_app PRIVATE tests/selection_scope.cpp framework/SelectionScope.cpp
                 framework/SelectionScope.hpp)
  # The icons' colour roles on both themes and the disabled rule (mitcad#230).
  target_sources(test_app PRIVATE tests/icon_colours.cpp)
  # A feature preview's bodies in placed, linked and hidden components
  # (mitcad#266): plain Qt Core.
  target_sources(test_app PRIVATE tests/preview_bodies.cpp framework/PreviewBodies.cpp framework/PreviewBodies.hpp)
  # The debug log's files: rotation, the folder's limits, the header, the
  # command line (mitcad#269): plain Qt Core and the log ring.
  target_sources(test_app PRIVATE tests/debug_log.cpp framework/DebugLog.cpp framework/DebugLog.hpp
                 mcp/LogRing.cpp mcp/LogRing.hpp)
  # The wait cursor: nested guards, the progress dialog's release, the
  # log lines, the idle recovery (mitcad#268): plain Qt Widgets.
  target_sources(test_app PRIVATE tests/wait_cursor.cpp framework/WaitCursor.cpp framework/WaitCursor.hpp)
  # The sketch snapper's body vertices and edge midpoints (mitcad#258): the
  # sketch model's data with OCCT's geometry types.
  target_sources(test_app PRIVATE tests/sketch_snap.cpp sketch/SketchSnap.cpp sketch/BodyGeometry.cpp)
  # The maps of a texture set and their files for the renderer (mitcad#314):
  # plain Qt Gui.
  target_sources(test_app PRIVATE tests/texture_maps.cpp framework/Appearances.cpp framework/Appearances.hpp)
  # The rendered view's meshes face out of their solids however the
  # triangulations wind (mitcad#319): OCCT's shapes and meshing.
  target_sources(test_app PRIVATE tests/render_mesh.cpp render/RenderMesh.cpp render/RenderMesh.hpp)
  target_link_libraries(test_app PRIVATE TKMesh)
  # The folders live MCP agents may save to and open from (mitcad#402):
  # plain Qt Core and Widgets.
  target_sources(test_app PRIVATE tests/agent_folders.cpp mcp/AgentFolders.cpp mcp/AgentFolders.hpp)
  # The file dialogs' folders per group (mitcad#498): plain Qt Core.
  target_sources(test_app PRIVATE tests/dialog_folders.cpp framework/DialogFolders.cpp framework/DialogFolders.hpp)
  # An .iam assembly's import choice, progress lines and parts' outcomes
  # (mitcad#507): plain Qt Core.
  target_sources(test_app PRIVATE tests/iam_import.cpp files/IamImport.cpp files/IamImport.hpp)
  # A renamed command's former id and saved shortcut (mitcad#503): the
  # registry with the command definitions it keeps.
  target_sources(test_app PRIVATE tests/command_alias.cpp framework/CommandRegistry.cpp framework/Command.cpp)
  target_link_libraries(test_app PRIVATE mitcad_geometry)
  target_include_directories(test_app PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
  target_link_libraries(test_app PRIVATE Qt6::Widgets Qt6::Svg mitcad_warnings)
  if(APPLE)
    target_link_libraries(test_app PRIVATE mitcad_mac_work)
  endif()
  add_test(NAME app.unit COMMAND test_app)
  add_executable(test_view_tessellation tests/view_tessellation.cpp)
  target_include_directories(test_view_tessellation PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
  target_link_libraries(test_view_tessellation PRIVATE mitcad_geometry_io TKV3d TKPrim TKBO mitcad_warnings)
  add_test(NAME app.view_tessellation COMMAND test_view_tessellation
    "${CMAKE_CURRENT_BINARY_DIR}/view-tessellation-files")
  # Check renderer device policy and fallback independently of UI helpers.
  add_test(NAME app.render_devices COMMAND test_app --render-devices)
  set_tests_properties(app.unit app.render_devices PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    ENVIRONMENT_MODIFICATION "PATH=path_list_prepend:$<TARGET_FILE_DIR:Qt6::Core>")
endif()

# View > Rendered and its render worker mitcad-render (mitcad#32, mitcad#45,
# docs/rendering.md), in every build. The application has the
# client's side and links none of the renderer's libraries; only the
# worker, an executable of its own next to it, links Cycles and the rest.
include("${PROJECT_SOURCE_DIR}/cmake/Render.cmake")
target_sources(mitcad PRIVATE
  render/FrameMemory.cpp
  render/FrameMemory.hpp
  render/RenderClient.cpp
  render/RenderClient.hpp
  render/RenderDevice.cpp
  render/RenderDevice.hpp
  render/RenderEnvironmentDialog.cpp
  render/RenderEnvironmentDialog.hpp
  render/RenderLights.cpp
  render/RenderLights.hpp
  render/RenderImage.cpp
  render/RenderImage.hpp
  render/RenderMesh.cpp
  render/RenderMesh.hpp
  render/RenderMode.cpp
  render/RenderMode.hpp
  render/RenderProtocol.hpp
  render/RenderScene.cpp
  render/RenderScene.hpp
  render/Renderer.hpp)
target_include_directories(mitcad SYSTEM PRIVATE "${MITCAD_CGLTF_INCLUDE_DIR}")
# File > Render Image (mitcad#48): the final render in the worker's batch
# mode; RenderBatch is shared with mitcad-cli render.
target_sources(mitcad PRIVATE
  render/RenderBatch.cpp
  render/RenderBatch.hpp
  render/RenderImageDialog.cpp
  render/RenderImageDialog.hpp)
# The browser's light bulbs in every mode, outside a command's undo range
# (mitcad#295), and Create Sketch's origin planes (mitcad#294).
target_sources(mitcad PRIVATE
  framework/CommandSessionVisibility.cpp
  MainWindowVisibility.cpp)

# The worker: no windows, Qt Core and Gui (QImage for --bench --output),
# OCCT for the test scene, the renderer.
add_executable(mitcad-render
  render/RenderWorker.cpp
  render/FrameMemory.cpp
  render/FrameMemory.hpp
  render/RenderDevices.cpp
  render/RenderDevices.hpp
  render/RenderImage.cpp
  render/RenderImage.hpp
  render/RenderMesh.cpp
  render/RenderMesh.hpp
  render/RenderProtocol.hpp
  render/RenderScene.cpp
  render/RenderScene.hpp
  render/Renderer.hpp)
# Crash reports (mitcad#62), with the worker's functions named in their
# stacks as the application's are.
target_sources(mitcad-render PRIVATE report/CrashHandler.cpp report/CrashHandler.hpp)
mitcad_utf8_code_page(mitcad-render)
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  target_link_options(mitcad-render PRIVATE
    "LINKER:--dynamic-list=${CMAKE_CURRENT_SOURCE_DIR}/report/crash-symbols.list")
endif()
if(WIN32)
  target_link_libraries(mitcad-render PRIVATE dbghelp)
endif()
# The final render's image files (mitcad#48): PNG and JPEG through Qt,
# OpenEXR through OpenImageIO.
target_sources(mitcad-render PRIVATE render/RenderOutput.cpp render/RenderOutput.hpp)
target_include_directories(mitcad-render PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_include_directories(mitcad-render SYSTEM PRIVATE "${MITCAD_CGLTF_INCLUDE_DIR}")
target_compile_definitions(mitcad-render PRIVATE MITCAD_VERSION="${PROJECT_VERSION}")
target_link_libraries(mitcad-render PRIVATE
  mitcad_cycles_renderer Qt6::Gui TKMesh TKFillet TKPrim TKTopAlgo TKGeomBase TKG3d TKBRep mitcad_warnings)
# Built with the application. On macOS the worker lives in Contents/MacOS
# in the build bundle too, where the application's ordinary lookup finds it.
if(APPLE)
  target_link_libraries(mitcad-render PRIVATE mitcad_mac_work)
  set_target_properties(mitcad-render PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "$<TARGET_FILE_DIR:mitcad>"
    INSTALL_RPATH "@executable_path/../Frameworks")
  target_link_options(mitcad-render PRIVATE LINKER:-headerpad_max_install_names)
endif()
add_dependencies(mitcad mitcad-render)
# The GPU devices' kernels (mitcad#50, build-cycles.sh --cuda and the
# like) where the worker looks for them in a build folder: cycles/lib
# next to it.
if(MITCAD_CYCLES_KERNELS)
  add_custom_command(TARGET mitcad-render POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E make_directory "$<TARGET_FILE_DIR:mitcad-render>/cycles/lib"
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different ${MITCAD_CYCLES_KERNELS}
            "$<TARGET_FILE_DIR:mitcad-render>/cycles/lib/"
    VERBATIM)
endif()
# Metal needs the complete source hierarchy at run time, not the flat
# compiled-kernel directory above. Keep the build bundle and installed
# bundle identical; the manifest also causes a relink/copy after a rebuild
# of the dependency prefix changes a runtime source file.
if(APPLE)
  set_property(TARGET mitcad-render APPEND PROPERTY LINK_DEPENDS "${MITCAD_CYCLES_RUNTIME}/source.sha256")
  add_custom_command(TARGET mitcad-render POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E rm -rf "$<TARGET_FILE_DIR:mitcad-render>/../Resources/cycles"
    COMMAND "${CMAKE_COMMAND}" -E copy_directory "${MITCAD_CYCLES_RUNTIME}"
            "$<TARGET_FILE_DIR:mitcad-render>/../Resources/cycles"
    VERBATIM)
endif()
# On Windows Open Image Denoise's DLLs next to it (vcpkg's come with
# vcpkg's applocal step).
if(MITCAD_OIDN_DLLS)
  add_custom_command(TARGET mitcad-render POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different ${MITCAD_OIDN_DLLS} "$<TARGET_FILE_DIR:mitcad-render>"
    VERBATIM)
endif()
if(BUILD_TESTING)
  # The worker renders the test scene headless (no display needed).
  add_test(NAME app.render_worker
           COMMAND mitcad-render --bench 160x120 --samples 4
                   --output "${CMAKE_CURRENT_BINARY_DIR}/render-worker-test.png")
  set_tests_properties(app.render_worker PROPERTIES PASS_REGULAR_EXPRESSION "render-bench 160x120 samples 4")
  # Scene updates change only what changed (mitcad#49).
  add_test(NAME app.render_updates COMMAND mitcad-render --update-test)
  # In an environment of the render settings (mitcad#47): the outdoor sky
  # as the background, a polished ground at a height.
  add_test(NAME app.render_worker_environment
           COMMAND mitcad-render --bench 160x120 --samples 4
                   --environment "{\"environment\": {\"preset\": \"outdoor\", \"sun_elevation\": 0.4}, \"background\": {\"mode\": \"environment\"}, \"ground\": {\"reflections\": true, \"height\": -5}}"
                   --output "${CMAKE_CURRENT_BINARY_DIR}/render-worker-environment-test.png")
  set_tests_properties(app.render_worker_environment PROPERTIES
    PASS_REGULAR_EXPRESSION "render-bench 160x120 samples 4" FAIL_REGULAR_EXPRESSION "render-environment")
  # Render devices (mitcad#50). A device that is not there: the CPU
  # renders, and says why.
  add_test(NAME app.render_worker_fallback
           COMMAND mitcad-render --bench 64x48 --samples 2 --device NO_SUCH_DEVICE)
  set_tests_properties(app.render_worker_fallback PROPERTIES PASS_REGULAR_EXPRESSION
    "render-fallback There is no render device \"NO_SUCH_DEVICE\"; rendering on the CPU.*render-bench 64x48 samples 2.*render-device CPU")
  # A device that fails while it renders (the test hook fails the first
  # one, also the CPU): the CPU renders the same scene and view again.
  add_test(NAME app.render_worker_device_failure COMMAND mitcad-render --bench 64x48 --samples 2)
  set_tests_properties(app.render_worker_device_failure PROPERTIES
    ENVIRONMENT "MITCAD_RENDER_TEST_DEVICE_FAILURE=simulated device failure"
    PASS_REGULAR_EXPRESSION "render-fallback Rendering on the CPU: .* failed \\(simulated device failure\\).*render-bench 64x48 samples 2.*render-device CPU")
  # Open Image Denoise's own view of the GPUs (mitcad#202): its devices,
  # then a line per GPU (none without one).
  add_test(NAME app.render_worker_probe_denoiser COMMAND mitcad-render --probe-denoiser)
  set_tests_properties(app.render_worker_probe_denoiser PROPERTIES
    PASS_REGULAR_EXPRESSION "render-denoiser-probe Open Image Denoise [0-9.]+ finds: [^\r\n]*CPU")
  # The best GPU against the CPU: the same test scene, images close to each
  # other (8-bit sRGB: mean difference, 99th percentile). Skipped without a
  # GPU device (a machine without one, or a build without GPU devices).
  add_test(NAME app.render_worker_gpu
           COMMAND mitcad-render --bench 320x240 --samples 64 --device auto --compare cpu --tolerance 2:24
                   --output "${CMAKE_CURRENT_BINARY_DIR}/render-worker-gpu-test.png")
  set_tests_properties(app.render_worker_gpu PROPERTIES SKIP_RETURN_CODE 77
    PASS_REGULAR_EXPRESSION "render-compare (CUDA|OPTIX|HIP|METAL|ONEAPI) ")
  add_test(NAME app.render_required_metal_contract
    COMMAND "${CMAKE_COMMAND}" "-DWORKER=$<TARGET_FILE:mitcad-render>"
            -P "${CMAKE_CURRENT_SOURCE_DIR}/tests/render-required-metal-test.cmake")
  set_tests_properties(app.render_required_metal_contract PROPERTIES TIMEOUT 60)
  # Check completed denoising independently of render-device capabilities.
  if(APPLE)
    foreach(mode cpu failure cpu_previews failure_previews metal previews)
      add_test(NAME app.render_denoiser_${mode}
        COMMAND "${CMAKE_COMMAND}" "-DWORKER=$<TARGET_FILE:mitcad-render>" "-DMODE=${mode}"
                -P "${CMAKE_CURRENT_SOURCE_DIR}/tests/render-denoiser-test.cmake")
      set_tests_properties(app.render_denoiser_${mode} PROPERTIES
        TIMEOUT 220 SKIP_REGULAR_EXPRESSION "SKIP: Metal OIDN unavailable")
    endforeach()
  endif()
  # Each takes seconds; a hung GPU (mitcad#146) fails them in two minutes,
  # not at ctest's default of 25.
  set_tests_properties(app.render_worker app.render_updates app.render_worker_environment
                       app.render_worker_fallback app.render_worker_device_failure app.render_worker_gpu
                       app.render_worker_probe_denoiser
    PROPERTIES TIMEOUT 120)
  # On Windows the worker finds Qt's DLLs through PATH, as app.unit does.
  if(WIN32)
    set_tests_properties(app.render_worker app.render_updates app.render_worker_environment
                         app.render_worker_fallback app.render_worker_device_failure app.render_worker_gpu
                         app.render_worker_probe_denoiser app.render_required_metal_contract
      PROPERTIES ENVIRONMENT_MODIFICATION "PATH=path_list_prepend:$<TARGET_FILE_DIR:Qt6::Core>")
  endif()
  # The maps of a texture set (mitcad#314): generated normal, roughness,
  # metalness and occlusion maps on the test block change its image, or
  # leave it as it is where they equal what they replace.
  add_test(NAME app.render_texture_maps
           COMMAND mitcad-render --maps-test "${CMAKE_CURRENT_BINARY_DIR}/render-texture-maps")
  set_tests_properties(app.render_texture_maps PROPERTIES TIMEOUT 120 PASS_REGULAR_EXPRESSION "render-maps ok")
  if(WIN32)
    set_tests_properties(app.render_texture_maps PROPERTIES
      ENVIRONMENT_MODIFICATION "PATH=path_list_prepend:$<TARGET_FILE_DIR:Qt6::Core>")
  endif()
  # Material swaps in a running session (mitcad#538): a body's material
  # changed to and from textured ones, back and forth, renders in it (not
  # black), as a renderer that starts with it; an image that cannot be read
  # any more when its material comes back is warned of.
  add_test(NAME app.render_material_swap
           COMMAND mitcad-render --swap-test "${CMAKE_CURRENT_BINARY_DIR}/render-material-swap")
  set_tests_properties(app.render_material_swap PROPERTIES TIMEOUT 180 PASS_REGULAR_EXPRESSION "render-swap ok")
  if(APPLE)
    add_test(NAME app.render_material_swap_metal
      COMMAND mitcad-render --swap-test "${CMAKE_CURRENT_BINARY_DIR}/render-material-swap-metal" --device metal)
    set_tests_properties(app.render_material_swap_metal PROPERTIES
      TIMEOUT 180 SKIP_RETURN_CODE 77 PASS_REGULAR_EXPRESSION "render-swap ok")
  endif()
  if(WIN32)
    set_tests_properties(app.render_material_swap PROPERTIES
      ENVIRONMENT_MODIFICATION "PATH=path_list_prepend:$<TARGET_FILE_DIR:Qt6::Core>")
  endif()
endif()

if(APPLE)
  # Configure the bundle after its render worker exists, so its
  # install rules include the worker, denoiser module and their libraries.
  include(packaging/macos/Bundle.cmake)
endif()

# The browser's and the timeline's places logged again when they move (mitcad#262).
target_sources(mitcad PRIVATE framework/PlaceWatcher.hpp)
# The side panels' trees' common indentation (mitcad#393).
target_sources(mitcad PRIVATE framework/PanelTree.hpp)
# The hover of a tangent chain in Fillet, Chamfer and the face tools (mitcad#290).
target_sources(mitcad PRIVATE framework/CommandSessionHover.cpp)
# A new feature's operation and objects follow what the model chooses from
# the preview until the user picks them: Extrude's automatic operation
# (mitcad#289).
target_sources(mitcad PRIVATE framework/CommandSessionAuto.cpp)
# Texture sources in Edit Appearances (mitcad#315, app/materials): the
# TextureSource interface and its registry, local texture folders, the
# browser's grid, Use's command, Preferences' Textures group.
target_sources(mitcad PRIVATE
  materials/FolderTextureSource.cpp
  materials/FolderTextureSource.hpp
  materials/TextureBrowser.cpp
  materials/TextureBrowser.hpp
  materials/TexturePreferences.cpp
  materials/TexturePreferences.hpp
  materials/TextureSource.cpp
  materials/TextureSource.hpp
  materials/TextureUse.cpp
  materials/TextureUse.hpp)
if(BUILD_TESTING)
  # The folder source's map detection, scan, thumbnails and fetch, the
  # registry, the cache's folders and Use's command: plain Qt Gui.
  target_sources(test_app PRIVATE tests/texture_sources.cpp
                 materials/FolderTextureSource.cpp materials/FolderTextureSource.hpp
                 materials/TextureSource.cpp materials/TextureSource.hpp
                 materials/TextureUse.cpp materials/TextureUse.hpp)
endif()
# Poly Haven as an online texture source (mitcad#313, app/materials): the
# requests of the texture sources on the internet (Qt Network; files of a
# folder for UI tests) and the source. The unit tests give it a fake
# fetcher and never use the network.
target_sources(mitcad PRIVATE
  materials/PolyHavenTextureSource.cpp
  materials/PolyHavenTextureSource.hpp
  materials/TextureFetcher.cpp
  materials/TextureFetcher.hpp)
if(BUILD_TESTING)
  target_sources(test_app PRIVATE tests/polyhaven_source.cpp
                 materials/PolyHavenTextureSource.cpp materials/PolyHavenTextureSource.hpp
                 materials/TextureFetcher.cpp materials/TextureFetcher.hpp)
  target_link_libraries(test_app PRIVATE Qt6::Network)
endif()
# The BUILD tab: frames of slot profiles with their connectors, panels and
# linear parts, the cut list and the parts list (mitcad#375, app/COMMANDS.md
# "BUILD"); QPdfWriter of Qt Gui writes the lists' PDFs.
target_sources(mitcad PRIVATE
  build/BuildCatalog.cpp
  build/BuildCatalog.hpp
  build/BuildLists.cpp
  build/BuildLists.hpp
  build/BuildModel.cpp
  build/BuildModel.hpp
  commands/BuildCommands.cpp)
# The part picker's data (mitcad#418, app/library): the user's favourite
# parts and groups in the settings, the picker's order, the library parts
# the design uses (the model's `library_usage` query) and the groups' tree
# (mitcad#527).
target_sources(mitcad PRIVATE
  library/PartFavourites.cpp
  library/PartFavourites.hpp
  library/PartGroups.cpp
  library/PartGroups.hpp
  library/UsedParts.cpp
  library/UsedParts.hpp)
if(BUILD_TESTING)
  # The order with stars, the stars' settings, the used parts read from a
  # model's answer and the groups' tree: plain Qt Core and the command
  # context's interface.
  target_sources(test_app PRIVATE tests/part_picker_data.cpp
                 library/PartFavourites.cpp library/PartFavourites.hpp
                 library/PartGroups.cpp library/PartGroups.hpp
                 library/UsedParts.cpp library/UsedParts.hpp)
endif()
# The part picker (mitcad#418, app/library): the library's parts as a grid
# of pictures with favourites and the design's parts, a floating window for
# the part-choosing commands and Insert from Library's results.
target_sources(mitcad PRIVATE
  library/PartPicker.cpp
  library/PartPicker.hpp)
# The web viewer's pages (mitcad#423, app/web, app/COMMANDS.md "Web
# page"): a design as one self-contained HTML file, File > Export > Web
# page. The viewer's files (app/web/viewer, its libraries in
# third_party/three) are resources under :/web-viewer/<repository path>,
# found by a glob that may find none (pageHtml then names the file the
# build lacks).
target_sources(mitcad PRIVATE
  web/WebGlb.cpp
  web/WebGlb.hpp
  web/WebPage.cpp
  web/WebPage.hpp
  web/WebPageHtml.cpp)
file(GLOB_RECURSE MITCAD_WEB_VIEWER_FILES CONFIGURE_DEPENDS
  "${PROJECT_SOURCE_DIR}/app/web/viewer/*" "${PROJECT_SOURCE_DIR}/third_party/three/*")
if(MITCAD_WEB_VIEWER_FILES)
  foreach(web_viewer_file IN LISTS MITCAD_WEB_VIEWER_FILES)
    file(RELATIVE_PATH web_viewer_alias "${PROJECT_SOURCE_DIR}" "${web_viewer_file}")
    set_source_files_properties("${web_viewer_file}" PROPERTIES QT_RESOURCE_ALIAS "${web_viewer_alias}")
  endforeach()
  qt_add_resources(mitcad web_viewer PREFIX /web-viewer FILES ${MITCAD_WEB_VIEWER_FILES})
endif()
if(BUILD_TESTING)
  # The page's HTML from fixture files of the viewer, and the glb's nodes
  # with the tree: Qt Core and Gui and the renderer's plain types.
  target_sources(test_app PRIVATE tests/web_page.cpp
                 web/WebGlb.cpp web/WebGlb.hpp web/WebPage.hpp web/WebPageHtml.cpp)
endif()
# A project's web viewer (mitcad#423, app/web): File > Project > Web Viewer
# and Update Web Viewer, the Save that updates a design's page, the web/
# folder's index and designs.json (the pages: web/WebPage.hpp).
target_sources(mitcad PRIVATE
  web/MainWindowWeb.cpp
  web/ProjectWeb.cpp
  web/ProjectWeb.hpp)
if(BUILD_TESTING)
  # The designs' slugs, designs.json and the index: plain Qt Core.
  target_sources(test_app PRIVATE tests/project_web.cpp web/ProjectWeb.cpp web/ProjectWeb.hpp)
endif()
# The orientation cube and its Home button in glass of their own
# (mitcad#430).
target_sources(mitcad PRIVATE
  view/ViewCube.cpp
  view/ViewCube.hpp)
# The card at the top of the view while Create Sketch waits for a plane
# (mitcad#474).
target_sources(mitcad PRIVATE
  view/ModeBanner.cpp
  view/ModeBanner.hpp)
# File > Export Drawing (mitcad#471, app/COMMANDS.md "Drawing"): its dialog
# and the choices it keeps (settings group drawing); the model writes the
# file (export_drawing).
target_sources(mitcad PRIVATE
  files/DrawingDialog.cpp
  files/DrawingDialog.hpp
  files/DrawingSettings.cpp
  files/DrawingSettings.hpp
  files/MainWindowDrawing.cpp)
if(BUILD_TESTING)
  # The settings and the command they make: plain Qt Core.
  target_sources(test_app PRIVATE tests/drawing_settings.cpp files/DrawingSettings.cpp files/DrawingSettings.hpp)
endif()
if(BUILD_TESTING)
  # The view's context menu (mitcad#494): which commands a selection
  # completes (first) and which need further picks (More), from the command
  # definitions.
  target_sources(test_app PRIVATE tests/context_menu.cpp framework/Command.cpp framework/Command.hpp)
endif()
# Sketch text from font files: Preferences' Fonts page and the font
# chooser of Edit Text (mitcad#473).
target_sources(mitcad PRIVATE
  sketch/FontFiles.cpp
  sketch/FontFiles.hpp)
# The sketch's origin and its x and y axes as picks for constraints and
# dimensions (mitcad#500).
target_sources(mitcad PRIVATE sketch/SketchAxes.cpp)
if(BUILD_TESTING)
  # Their picks on the sketch model's data: plain Qt Core and the sketch
  # geometry.
  target_sources(test_app PRIVATE tests/sketch_axes.cpp sketch/SketchAxes.cpp)
endif()
# Mount frames of any part and Place Part (mitcad#525, app/COMMANDS.md
# "Component libraries"): a library part placed by its frame on a face, a
# hole or a grid; the frames a manifest lists.
target_sources(mitcad PRIVATE
  commands/MountCommands.cpp
  commands/MountCommands.hpp
  library/MountFrames.cpp
  library/MountFrames.hpp)
if(BUILD_TESTING)
  # The frames of a manifest's block and their texts: plain Qt Core.
  target_sources(test_app PRIVATE tests/mount_frames.cpp library/MountFrames.cpp library/MountFrames.hpp)
endif()
# A timber frame's openings (mitcad#519, app/COMMANDS.md "Timber"): their
# rows, text form and checks, and the Openings window that edits them
# beside Wall's, Floor / Roof's and Timber Frame's panels.
target_sources(mitcad PRIVATE
  build/OpeningsWindow.cpp
  build/OpeningsWindow.hpp
  build/TimberOpenings.cpp
  build/TimberOpenings.hpp)
if(BUILD_TESTING)
  # Text to rows and back, the model's openings, the rows' checks: plain Qt
  # Core.
  target_sources(test_app PRIVATE tests/timber_openings.cpp build/TimberOpenings.cpp build/TimberOpenings.hpp)
endif()
# The joint highlight (mitcad#523): a selected joint's sides, frames and
# occurrences in the view, from the `joints` query.
target_sources(mitcad PRIVATE
  MainWindowJointHighlight.cpp
  framework/JointHighlight.cpp
  framework/JointHighlight.hpp)
if(BUILD_TESTING)
  # What each side shows, its log lines and the card's text: plain Qt Core.
  target_sources(test_app PRIVATE tests/joint_highlight.cpp framework/JointHighlight.cpp
                 framework/JointHighlight.hpp)
endif()
# Move Face (mitcad#531): its summary line ("MoveFace1: 2 faces of Body1,
# translate (...) mm"), for the panel, the log and the selection card.
target_sources(mitcad PRIVATE
  commands/MoveFaceText.cpp
  commands/MoveFaceText.hpp
  MainWindowMoveFace.cpp)
if(BUILD_TESTING)
  # Its motions in words: plain Qt Core.
  target_sources(test_app PRIVATE tests/move_face_text.cpp commands/MoveFaceText.cpp commands/MoveFaceText.hpp)
endif()
# The Projects panel's rows (mitcad#529): compact marks for their state, a
# name elided in the middle.
target_sources(mitcad PRIVATE
  projects/ProjectRowText.cpp
  projects/ProjectRowText.hpp)
if(BUILD_TESTING)
  # The marks and the elision: plain Qt Core and Gui.
  target_sources(test_app PRIVATE tests/project_rows.cpp projects/ProjectRowText.cpp projects/ProjectRowText.hpp)
endif()
# File > Open from Project (mitcad#530): the projects browsed as folders,
# their designs searched by name.
target_sources(mitcad PRIVATE
  projects/ProjectBrowser.cpp
  projects/ProjectBrowser.hpp
  projects/ProjectBrowserData.cpp
  projects/ProjectBrowserData.hpp)
if(BUILD_TESTING)
  # A place's contents and the search over a fixture project tree: plain Qt
  # Core (the marks' words from ProjectRowText).
  target_sources(test_app PRIVATE tests/project_browser.cpp projects/ProjectBrowserData.cpp
                 projects/ProjectBrowserData.hpp)
endif()
# The view's fit of a design's first bodies, once per design (mitcad#467;
# not again after every body was hidden and one shown).
target_sources(mitcad PRIVATE view/FirstBodiesFit.hpp)
if(BUILD_TESTING)
  # The decision: plain C++.
  target_sources(test_app PRIVATE tests/first_bodies_fit.cpp view/FirstBodiesFit.hpp)
endif()
# Section Analysis off the UI thread (mitcad#438): the section worker's
# rounds, and the window's clips, view and measurement through them.
target_sources(mitcad PRIVATE
  MainWindowSection.cpp
  framework/SectionWorker.cpp
  framework/SectionWorker.hpp)
if(BUILD_TESTING)
  # Rounds superseded, waiting, stopped, off the UI thread: plain Qt Core
  # and the geometry's cancel.
  target_sources(test_app PRIVATE tests/section_worker.cpp framework/SectionWorker.cpp
                 framework/SectionWorker.hpp)
endif()

# Native scheduling, pool initialization and exception-safe QoS restoration
# (mitcad#556). No windows or renderer performance campaign.
if(APPLE AND BUILD_TESTING)
  add_executable(test_mac_work tests/mac_work.cpp
    framework/ModelWorker.cpp framework/ModelWorker.hpp
    framework/SectionWorker.cpp framework/SectionWorker.hpp)
  set_target_properties(test_mac_work PROPERTIES AUTOMOC ON)
  target_link_libraries(test_mac_work PRIVATE mitcad_mac_work mitcad_ffi mitcad_geometry mitcad_warnings)
  add_test(NAME app.macos_work COMMAND test_mac_work)
  set_tests_properties(app.macos_work PROPERTIES TIMEOUT 60)
endif()
# Fillet's and Chamfer's handles (mitcad#534): where the arrows of a
# rounding's radius and of a bevel's distances stand on an edge.
target_sources(mitcad PRIVATE
  commands/ChamferHandles.cpp
  commands/ChamferHandles.hpp)
if(BUILD_TESTING)
  # Convex, concave, obtuse and placed edges, the measured face: plain OCCT
  # math.
  target_sources(test_app PRIVATE tests/chamfer_handles.cpp commands/ChamferHandles.cpp
                 commands/ChamferHandles.hpp)
endif()
# The patterns' Direction Type and Distance Type (mitcad#545): the panels'
# choices as the definitions' `symmetric` and `distance_type`, the length
# row's label, the options in the log.
target_sources(mitcad PRIVATE
  commands/PatternOptions.cpp
  commands/PatternOptions.hpp)
if(BUILD_TESTING)
  # The mapping and the words: plain Qt Core.
  target_sources(test_app PRIVATE tests/pattern_options.cpp commands/PatternOptions.cpp
                 commands/PatternOptions.hpp)
endif()

# The sketches a command shows for its time (mitcad#546): an edited
# feature's, as if their light bulbs were on, until its panel closes.
target_sources(mitcad PRIVATE view/CommandSketches.hpp)
if(BUILD_TESTING)
  # Which sketches, which only for the command, what each goes back to:
  # plain Qt Core.
  target_sources(test_app PRIVATE tests/command_sketches.cpp view/CommandSketches.hpp)
endif()
