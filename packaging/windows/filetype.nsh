; SPDX-License-Identifier: MIT
; The .mitcad file type for the NSIS installer (cmake/Packaging.cmake). The
; commands live in a file of their own: CPack mangles double quotes in its
; CPACK_NSIS_EXTRA_*_COMMANDS variables.
;
; In the scope of the installation: SHCTX is HKCU\Software\Classes for an
; installation just for the current user and HKLM\Software\Classes for one
; for all users, as the installer and the uninstaller set the shell context
; (packaging/windows/NSIS.template.in). A registration of the current user
; takes precedence over the one for all users, so a per-user installation
; opens .mitcad files for its user even when another one is installed for
; all users.

!macro MitcadFileTypeRegister
  WriteRegStr SHCTX "Software\Classes\.mitcad" "" "Mitcad.Project"
  WriteRegStr SHCTX "Software\Classes\Mitcad.Project" "" "Mitcad design"
  WriteRegStr SHCTX "Software\Classes\Mitcad.Project\shell\open\command" "" \
    '"$INSTDIR\bin\mitcad.exe" --open "%1"'
  ; Tell the shell that file associations changed.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro MitcadFileTypeUnregister
  DeleteRegKey SHCTX "Software\Classes\.mitcad"
  DeleteRegKey SHCTX "Software\Classes\Mitcad.Project"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
