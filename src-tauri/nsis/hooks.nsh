; Routa 安装程序钩子（由 Tauri 的 installer.nsi 引入）：安装后注册进程分流服务，卸载前删除它。
;
; 安装程序按 currentUser 模式运行、本身不提权，所以经 UAC 单独以管理员身份运行
; routa-procsvc.exe。用户拒绝 UAC 或命令失败都不阻断安装 / 卸载，只写进详细日志：
; 之后可以在“进程分流”页面里启用或卸载组件。
; 不在这里删除 Program Files 下的任何目录，由 routa-procsvc.exe --uninstall 负责。

!define ROUTA_PROCSVC_EXE "routa-procsvc.exe"
!define ROUTA_PROCSVC_SERVICE "RoutaProcSvc"
!define ROUTA_PROCSVC_DIR "$PROGRAMFILES64\Routa\ProcessRouting"

; 以管理员身份运行 $INSTDIR\routa-procsvc.exe，参数取自 $R9，等它退出并把结果写进日志。
; 寄存器经 System::Store 保存和恢复。
!macro ROUTA_PROCSVC_RUN_ELEVATED
  System::Store S
  StrCpy $2 "$INSTDIR\${ROUTA_PROCSVC_EXE}"
  ; SHELLEXECUTEINFOW；fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC，nShow = SW_HIDE
  System::Call '*(&l4, i 0x140, p $HWNDPARENT, w "runas", w r2, w R9, p 0, i 0, p, p, p, p, i, p, p) p.r1'
  System::Call 'shell32::ShellExecuteExW(p r1) i.r0 ?e'
  Pop $3
  ${If} $0 = 0
    ; 1223 = ERROR_CANCELLED：用户拒绝了 UAC
    DetailPrint "进程分流组件：无法以管理员身份运行 ${ROUTA_PROCSVC_EXE}（错误 $3），已跳过"
  ${Else}
    System::Call '*$1(i, i, p, p, p, p, p, i, p, p, p, p, i, p, p.r4)'
    ${If} $4 = 0
      ; 没拿到进程句柄（例如由已有进程处理了请求），无法得知结果
      DetailPrint "进程分流组件：${ROUTA_PROCSVC_EXE} 已启动，但无法得知执行结果"
    ${Else}
      System::Call 'kernel32::WaitForSingleObject(p r4, i -1)'
      System::Call 'kernel32::GetExitCodeProcess(p r4, *i.r5)'
      System::Call 'kernel32::CloseHandle(p r4)'
      ${If} $5 = 0
        DetailPrint "进程分流组件：${ROUTA_PROCSVC_EXE} 执行成功"
      ${Else}
        DetailPrint "进程分流组件：${ROUTA_PROCSVC_EXE} 执行失败（退出码 $5），已跳过"
      ${EndIf}
    ${EndIf}
  ${EndIf}
  System::Free $1
  System::Store L
!macroend

; 当前（安装程序所属）用户的 SID 放入 $R8；取不到或格式不对时为空。
; System::Store L 会恢复全部寄存器，所以结果先算在 $8，经栈带出来。
; 用 whoami 的完整路径，避免从安装程序所在目录（例如“下载”）运行同名程序。
!macro ROUTA_CURRENT_USER_SID
  System::Store S
  StrCpy $8 ""
  nsExec::ExecToStack '"$SYSDIR\whoami.exe" /user /fo csv /nh'
  Pop $0
  Pop $1
  ; 输出形如 "desktop\user","S-1-5-21-...-1001"。
  ; StrLoc 是宏，参数原样代入 Push "..."，所以引号要写成 $\"
  ${If} $0 = 0
    ${StrLoc} $2 $1 "$\"S-1-" ">"
    ${If} $2 != ""
      IntOp $2 $2 + 1
      StrCpy $1 $1 "" $2
      ${StrLoc} $2 $1 "$\"" ">"
      ${If} $2 != ""
        StrCpy $1 $1 $2
        ; "S-1-" 之后只能是数字和 "-"，且以数字结尾
        StrLen $3 $1
        StrCpy $4 4
        StrCpy $6 1
        ${If} $3 <= 4
          StrCpy $6 0
        ${EndIf}
        ${DoWhile} $4 < $3
          StrCpy $5 $1 1 $4
          ${If} $5 == "-"
            IntOp $7 $3 - 1
            ${IfThen} $4 = $7 ${|} StrCpy $6 0 ${|}
          ${Else}
            ${StrLoc} $7 "0123456789" $5 ">"
            ${IfThen} $7 == "" ${|} StrCpy $6 0 ${|}
          ${EndIf}
          IntOp $4 $4 + 1
        ${Loop}
        ${If} $6 = 1
          StrCpy $8 $1
        ${EndIf}
      ${EndIf}
    ${EndIf}
  ${EndIf}
  Push $8
  System::Store L
  Pop $R8
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Push $R8
  Push $R9
  !insertmacro ROUTA_CURRENT_USER_SID
  ${If} $R8 == ""
    DetailPrint "进程分流组件：无法取得当前用户的 SID，跳过注册服务"
  ${Else}
    DetailPrint "进程分流组件：注册服务（需要管理员权限）"
    StrCpy $R9 '--install --owner-sid $R8 --source "$INSTDIR\procsvc"'
    !insertmacro ROUTA_PROCSVC_RUN_ELEVATED
  ${EndIf}
  Pop $R9
  Pop $R8
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; 安装程序覆盖安装（重新安装 / 升级 / 降级页选了“先卸载”）时，Tauri 模板
  ; （installer.nsi 的 PageLeaveReinstall，reinst_uninstall 分支）用
  ;   ExecWait '"<旧目录>\uninstall.exe" [/UPDATE] [/P] _?=<旧目录>'
  ; 原地运行旧卸载程序，随后马上重新安装。这时不能删除服务：--uninstall 会删掉全部 Routa
  ; WFP 过滤器（包括持久阻断），到重新安装的 UAC 之前（拒绝则一直）“阻断”的程序会直连。
  ; 区分方法：用户正常卸载时 NSIS 卸载程序先把自己复制到 %TEMP%\~nsuX.tmp 再从那里运行，
  ; $EXEDIR 是临时目录；只有带 _?= 原地运行时 $EXEDIR 才等于 $INSTDIR（_?= 本身不出现在
  ; $CMDLINE 里，无法直接判断）。手工带 _?= 运行卸载程序同样会跳过，此时可在页面上“卸载组件”。
  ; /UPDATE（更新模式）同理跳过。
  ${If} $UpdateMode = 1
  ${OrIf} $EXEDIR == $INSTDIR
    DetailPrint "进程分流组件：覆盖安装，保留服务和防护规则"
  ${Else}
    Push $0
    Push $R9
    ; 服务和组件目录都不存在（例如安装时拒绝了 UAC）就不必再弹 UAC
    nsExec::Exec '"$SYSDIR\sc.exe" query ${ROUTA_PROCSVC_SERVICE}'
    Pop $0
    ; 1060 = ERROR_SERVICE_DOES_NOT_EXIST
    ${If} $0 = 1060
    ${AndIfNot} ${FileExists} "${ROUTA_PROCSVC_DIR}\*.*"
      DetailPrint "进程分流组件：未安装，跳过"
    ${Else}
      ; 先确认 Routa 已退出（模板的检查，用户取消则中止卸载），再删除组件：
      ; 否则取消卸载后组件已经没了
      !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
      DetailPrint "进程分流组件：删除服务和防护规则（需要管理员权限）"
      StrCpy $R9 "--uninstall"
      !insertmacro ROUTA_PROCSVC_RUN_ELEVATED
    ${EndIf}
    Pop $R9
    Pop $0
  ${EndIf}
!macroend
