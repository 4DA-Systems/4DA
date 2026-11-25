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
;   Before this version's files are written, remove the files older
;   installers left in the install directory that nothing reads (below).
; Uninstall: remove the same exclusions, and the per-user scheduled task
;   "4DA Background Refresh" (engine_scheduler.rs TASK_NAME) when it runs the
;   copy being uninstalled. Skipped in update mode, where the app stays.
;   Then every file older installers left in the install directory, so an
;   uninstall after an upgrade leaves nothing behind.
;
; User data is NOT touched here: the database, settings.json and keychain
; entries stay unless the user ticks Tauri's own "Delete app data" box. They
; live under %APPDATA%\com.4da.app and %LOCALAPPDATA%\com.4da.app (the
; downloaded model too); the install directory is %LOCALAPPDATA%\4DA.
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

; Files an older installer put in the install directory that this one does
; not ship. NSIS upgrades in place and deletes nothing, and an uninstaller
; deletes only what its own version installed, so these outlive every upgrade
; and the uninstall. The paths are exact, from 7-Zip listings of the published
; 4DA_1.0.2_x64-setup.exe and 4DA_1.0.3_x64-setup.exe: no wildcard, no
; recursive delete, nothing outside $INSTDIR, and a folder goes only once it
; is empty (RMDir without /r), so a file not named here keeps its folder.
!ifndef FOURDA_ARCTIC
  !define FOURDA_ARCTIC "models\embeddings\models--Snowflake--snowflake-arctic-embed-m"
!endif
!ifndef FOURDA_ARCTIC_SNAPSHOT
  !define FOURDA_ARCTIC_SNAPSHOT "${FOURDA_ARCTIC}\snapshots\fc74610d18462d218e312aa986ec5c8a75a98152"
!endif
!ifndef FOURDA_NOMIC
  !define FOURDA_NOMIC "models\embeddings\nomic-embed-text-v1.5-fp16"
!endif

; Read by nothing, so removed at install as well as at uninstall. At install
; this runs before the new files are written: a later installer that ships
; one of these paths again still installs it.
;   1.0.2        Snowflake arctic-embed-m, the embedding model 1.0.3 replaced
;                (6 files, 110.8 MB)
;   1.0.2, 1.0.3 the two OCR models (12.2 MB); OCR is not compiled into the
;                release build
!macro FOURDA_REMOVE_UNREAD_LEFTOVERS
    Delete "$INSTDIR\models\text-detection.rten"
    Delete "$INSTDIR\models\text-recognition.rten"
    Delete "$INSTDIR\${FOURDA_ARCTIC}\refs\main"
    Delete "$INSTDIR\${FOURDA_ARCTIC_SNAPSHOT}\config.json"
    Delete "$INSTDIR\${FOURDA_ARCTIC_SNAPSHOT}\onnx\model_quantized.onnx"
    Delete "$INSTDIR\${FOURDA_ARCTIC_SNAPSHOT}\special_tokens_map.json"
    Delete "$INSTDIR\${FOURDA_ARCTIC_SNAPSHOT}\tokenizer.json"
    Delete "$INSTDIR\${FOURDA_ARCTIC_SNAPSHOT}\tokenizer_config.json"
    RMDir "$INSTDIR\${FOURDA_ARCTIC_SNAPSHOT}\onnx"
    RMDir "$INSTDIR\${FOURDA_ARCTIC_SNAPSHOT}"
    RMDir "$INSTDIR\${FOURDA_ARCTIC}\snapshots"
    RMDir "$INSTDIR\${FOURDA_ARCTIC}\refs"
    RMDir "$INSTDIR\${FOURDA_ARCTIC}"
    RMDir "$INSTDIR\models\embeddings"
    ClearErrors
!macroend

; Read, so removed at uninstall only.
;   1.0.3        nomic-embed-text v1.5 fp16 (5 files, 274.6 MB). From 1.0.4 the
;                installer no longer ships it, but the app loads this copy
;                before its own download cache (embeddings_providers/
;                model_store.rs), so an upgrade keeps it and spares the user a
;                274 MB download. A release that stops reading it moves these
;                lines to the macro above.
!macro FOURDA_REMOVE_READ_LEFTOVERS
    Delete "$INSTDIR\${FOURDA_NOMIC}\config.json"
    Delete "$INSTDIR\${FOURDA_NOMIC}\model_fp16.onnx"
    Delete "$INSTDIR\${FOURDA_NOMIC}\special_tokens_map.json"
    Delete "$INSTDIR\${FOURDA_NOMIC}\tokenizer.json"
    Delete "$INSTDIR\${FOURDA_NOMIC}\tokenizer_config.json"
    RMDir "$INSTDIR\${FOURDA_NOMIC}"
    ClearErrors
!macroend

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

!macro NSIS_HOOK_PREINSTALL
    !insertmacro FOURDA_REMOVE_UNREAD_LEFTOVERS
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
    ; After Tauri's own deletes, which leave $INSTDIR in place while an older
    ; release's files are in it. Not in update mode, where the app stays.
    ${If} $UpdateMode <> 1
        !insertmacro FOURDA_REMOVE_UNREAD_LEFTOVERS
        !insertmacro FOURDA_REMOVE_READ_LEFTOVERS
        RMDir "$INSTDIR\models\embeddings"
        RMDir "$INSTDIR\models"
        RMDir "$INSTDIR"
        ClearErrors
    ${EndIf}
    !insertmacro FOURDA_REMOVE_DEFENDER_EXCLUSIONS
!macroend
