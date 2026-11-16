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
  WriteRegStr HKCU "Software\SongPlayer\Capabilities\FileAssociations" ".${EXT}" "${FILECLASS}"
!macroend

!macroundef APP_UNASSOCIATE
!macro APP_UNASSOCIATE EXT FILECLASS
  DeleteRegValue HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${FILECLASS}"
  DeleteRegKey /ifempty HKCU "Software\Classes\.${EXT}\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "Software\Classes\.${EXT}"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Register the app for Windows' Default Apps page (per user).
  WriteRegStr HKCU "Software\SongPlayer\Capabilities" "ApplicationName" "Song Player"
  WriteRegStr HKCU "Software\SongPlayer\Capabilities" "ApplicationDescription" "Song Player"
  WriteRegStr HKCU "Software\SongPlayer\Capabilities" "ApplicationIcon" '$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0'
  WriteRegStr HKCU "Software\RegisteredApplications" "SongPlayer" "Software\SongPlayer\Capabilities"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Delete our registration only; never rewrite extension defaults/UserChoice.
  DeleteRegValue HKCU "Software\RegisteredApplications" "SongPlayer"
  DeleteRegKey HKCU "Software\SongPlayer\Capabilities"
  DeleteRegKey /ifempty HKCU "Software\SongPlayer"
  DeleteRegKey HKCU "Software\Classes\SongPlayer.Audio"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
