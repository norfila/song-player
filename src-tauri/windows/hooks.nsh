; Advertise supported formats without assigning defaults. The stock Tauri
; association macros write extension defaults; replace them with OpenWith-only
; registration. Windows' Default Apps UI is the sole place the user changes them.
!macroundef APP_ASSOCIATE
!macro APP_ASSOCIATE EXT FILECLASS DESCRIPTION ICON COMMANDTEXT COMMAND
  WriteRegStr HKCU "Software\Classes\${FILECLASS}" "" `${DESCRIPTION}`
  WriteRegStr HKCU "Software\Classes\${FILECLASS}\DefaultIcon" "" `${ICON}`
  WriteRegStr HKCU "Software\Classes\${FILECLASS}\shell\open" "" `${COMMANDTEXT}`
  WriteRegStr HKCU "Software\Classes\${FILECLASS}\shell\open\command" "" `${COMMAND}`
  WriteRegStr HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${FILECLASS}" ""
  WriteRegStr HKCU "Software\Sonora\Capabilities\FileAssociations" ".${EXT}" "${FILECLASS}"
!macroend

!macroundef APP_UNASSOCIATE
!macro APP_UNASSOCIATE EXT FILECLASS
  DeleteRegValue HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${FILECLASS}"
  DeleteRegKey /ifempty HKCU "Software\Classes\.${EXT}\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "Software\Classes\.${EXT}"
!macroend

!macro MIGRATE_LEGACY_PLAYER_SHORTCUT DIRECTORY LEGACYNAME
  Push $0
  Push $1
  Push $2
  Push $3
  !insertmacro IsShortcutTarget "${DIRECTORY}\${LEGACYNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    ${If} ${FileExists} "${DIRECTORY}\${PRODUCTNAME}.lnk"
      ; Keep both shortcuts when the new name belongs to another application.
      !insertmacro IsShortcutTarget "${DIRECTORY}\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      Pop $0
      ${If} $0 = 1
        Delete "${DIRECTORY}\${LEGACYNAME}.lnk"
      ${EndIf}
    ${Else}
      Rename "${DIRECTORY}\${LEGACYNAME}.lnk" "${DIRECTORY}\${PRODUCTNAME}.lnk"
    ${EndIf}
  ${EndIf}
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

!macro REMOVE_LEGACY_PLAYER_SHORTCUT DIRECTORY LEGACYNAME
  Push $0
  Push $1
  Push $2
  Push $3
  !insertmacro IsShortcutTarget "${DIRECTORY}\${LEGACYNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "${DIRECTORY}\${LEGACYNAME}.lnk"
    Delete "${DIRECTORY}\${LEGACYNAME}.lnk"
  ${EndIf}
  Pop $3
  Pop $2
  Pop $1
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Only migrate links targeting this installation. The stable uninstall and
  ; manufacturer keys live in installer.nsi and retain upgrade/directory lookup.
  !insertmacro MIGRATE_LEGACY_PLAYER_SHORTCUT "$SMPROGRAMS" "Sonora"
  !insertmacro MIGRATE_LEGACY_PLAYER_SHORTCUT "$SMPROGRAMS" "Audio Player"
  ${If} $AppStartMenuFolder != ""
    !insertmacro MIGRATE_LEGACY_PLAYER_SHORTCUT "$SMPROGRAMS\$AppStartMenuFolder" "Sonora"
    !insertmacro MIGRATE_LEGACY_PLAYER_SHORTCUT "$SMPROGRAMS\$AppStartMenuFolder" "Audio Player"
  ${EndIf}
  !insertmacro MIGRATE_LEGACY_PLAYER_SHORTCUT "$DESKTOP" "Sonora"
  !insertmacro MIGRATE_LEGACY_PLAYER_SHORTCUT "$DESKTOP" "Audio Player"
  WriteRegStr HKCU "Software\Sonora\Capabilities" "ApplicationName" "Song Player"
  WriteRegStr HKCU "Software\Sonora\Capabilities" "ApplicationDescription" "Song Player"
  WriteRegStr HKCU "Software\Sonora\Capabilities" "ApplicationIcon" '$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0'
  WriteRegStr HKCU "Software\RegisteredApplications" "Sonora" "Software\Sonora\Capabilities"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; An unrelated shortcut may have prevented migration. Remove only our legacy
  ; links on a full uninstall; the updater leaves them for the next installer.
  ${If} $UpdateMode <> 1
    !insertmacro REMOVE_LEGACY_PLAYER_SHORTCUT "$SMPROGRAMS" "Sonora"
    !insertmacro REMOVE_LEGACY_PLAYER_SHORTCUT "$SMPROGRAMS" "Audio Player"
    ${If} $AppStartMenuFolder != ""
      !insertmacro REMOVE_LEGACY_PLAYER_SHORTCUT "$SMPROGRAMS\$AppStartMenuFolder" "Sonora"
      !insertmacro REMOVE_LEGACY_PLAYER_SHORTCUT "$SMPROGRAMS\$AppStartMenuFolder" "Audio Player"
    ${EndIf}
    !insertmacro REMOVE_LEGACY_PLAYER_SHORTCUT "$DESKTOP" "Sonora"
    !insertmacro REMOVE_LEGACY_PLAYER_SHORTCUT "$DESKTOP" "Audio Player"
  ${EndIf}
  ; Delete our registration only; never rewrite extension defaults/UserChoice.
  DeleteRegValue HKCU "Software\RegisteredApplications" "Sonora"
  DeleteRegKey HKCU "Software\Sonora\Capabilities"
  DeleteRegKey /ifempty HKCU "Software\Sonora"
  DeleteRegKey HKCU "Software\Classes\Sonora.Audio"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
