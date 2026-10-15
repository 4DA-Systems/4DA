; 4DA NSIS installer hooks.
;
; The installer makes NO security-relevant changes to the machine. Version 1.0.3
; added Windows Defender exclusions here (process name fourda.exe plus the
; install directory) without telling the user; a process-NAME exclusion also
; exempted any binary called fourda.exe, wherever it lived. That was removed.
;
; Install: remove the two exclusions 1.0.3 may have added (it could only add
;   them when the installer ran elevated). Not elevated, or Defender absent:
;   the cmdlets fail, the failure is swallowed and setup continues unchanged.
; Uninstall: remove the same exclusions, and the per-user scheduled task
;   "4DA Background Refresh" (engine_scheduler.rs TASK_NAME) when it runs the
;   copy being uninstalled. Skipped in update mode, where the app stays.
;
; User data is NOT touched here: the database, settings.json and keychain
; entries stay unless the user ticks Tauri's own "Delete app data" box.
;
; Values reach PowerShell through environment variables, never spliced into
; the script text, so a path containing a quote cannot break out of the
; command. `$$` is NSIS's escape for a literal `$`. PowerShell is started by
; full path (64-bit through sysnative when this 32-bit installer runs on a
; 64-bit OS, where the Defender and ScheduledTasks modules live). $0 and $R9
; are saved and restored around each hook.

!ifndef FOURDA_REFRESH_TASK
  !define FOURDA_REFRESH_TASK "4DA Background Refresh"
!endif
!ifndef FOURDA_EXCLUDED_PROCESS
  !define FOURDA_EXCLUDED_PROCESS "fourda.exe"
!endif

!macro FOURDA_FIND_POWERSHELL
    ${If} ${FileExists} "$WINDIR\sysnative\WindowsPowerShell\v1.0\powershell.exe"
        StrCpy $R9 "$WINDIR\sysnative\WindowsPowerShell\v1.0\powershell.exe"
    ${Else}
        StrCpy $R9 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
    ${EndIf}
!macroend

!macro FOURDA_REMOVE_DEFENDER_EXCLUSIONS
    Push $0
    Push $R9
    !insertmacro FOURDA_FIND_POWERSHELL
    System::Call 'Kernel32::SetEnvironmentVariable(t "FOURDA_INSTDIR", t "$INSTDIR")i'
    System::Call 'Kernel32::SetEnvironmentVariable(t "FOURDA_EXCLUDED_PROCESS", t "${FOURDA_EXCLUDED_PROCESS}")i'
    nsExec::ExecToLog `"$R9" -NoProfile -NonInteractive -Command "try { Remove-MpPreference -ExclusionProcess $$env:FOURDA_EXCLUDED_PROCESS -ErrorAction Stop } catch {}; try { Remove-MpPreference -ExclusionPath $$env:FOURDA_INSTDIR -ErrorAction Stop } catch {}; exit 0"`
    Pop $0
    Pop $R9
    Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
    !insertmacro FOURDA_REMOVE_DEFENDER_EXCLUSIONS
!macroend

!macro NSIS_HOOK_PREUNINSTALL
    ${If} $UpdateMode <> 1
        Push $0
        Push $R9
        !insertmacro FOURDA_FIND_POWERSHELL
        System::Call 'Kernel32::SetEnvironmentVariable(t "FOURDA_INSTDIR", t "$INSTDIR")i'
        System::Call 'Kernel32::SetEnvironmentVariable(t "FOURDA_TASK", t "${FOURDA_REFRESH_TASK}")i'
        ; Only a task whose action runs <INSTDIR>\fourda.exe is ours to remove: a
        ; developer's task pointing at a source checkout is left alone.
        nsExec::ExecToLog `"$R9" -NoProfile -NonInteractive -Command "try { $$t = Get-ScheduledTask -TaskName $$env:FOURDA_TASK -ErrorAction Stop; $$exe = (Join-Path $$env:FOURDA_INSTDIR 'fourda.exe').ToLowerInvariant(); $$runs = (($$t.Actions | ForEach-Object { [string]$$_.Execute + ' ' + [string]$$_.Arguments }) -join ' ').ToLowerInvariant(); if ($$runs.Contains($$exe)) { schtasks.exe /Delete /TN $$env:FOURDA_TASK /F | Out-Null } } catch {}; exit 0"`
        Pop $0
        Pop $R9
        Pop $0
    ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
    !insertmacro FOURDA_REMOVE_DEFENDER_EXCLUSIONS
!macroend
