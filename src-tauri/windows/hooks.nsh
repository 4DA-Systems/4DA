; 4DA NSIS Installer Hooks
; Adds/removes Windows Defender exclusions for the 4DA process and install directory.
; Runs silently — if the user lacks admin privileges or Defender is absent, the
; commands fail gracefully and installation continues normally.

!macro NSIS_HOOK_POSTINSTALL
    ; Add Windows Defender exclusion for the 4DA process name
    nsExec::ExecToLog 'powershell -NoProfile -NonInteractive -Command "try { Add-MpPreference -ExclusionProcess ''fourda.exe'' -ErrorAction Stop } catch { exit 0 }"'
    ; Add Windows Defender exclusion for the install directory
    nsExec::ExecToLog 'powershell -NoProfile -NonInteractive -Command "try { Add-MpPreference -ExclusionPath ''$INSTDIR'' -ErrorAction Stop } catch { exit 0 }"'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
    ; Remove Windows Defender exclusion for the 4DA process name
    nsExec::ExecToLog 'powershell -NoProfile -NonInteractive -Command "try { Remove-MpPreference -ExclusionProcess ''fourda.exe'' -ErrorAction Stop } catch { exit 0 }"'
    ; Remove Windows Defender exclusion for the install directory
    nsExec::ExecToLog 'powershell -NoProfile -NonInteractive -Command "try { Remove-MpPreference -ExclusionPath ''$INSTDIR'' -ErrorAction Stop } catch { exit 0 }"'
!macroend
