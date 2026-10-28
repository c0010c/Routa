# 用 MSVC 编译 third_party/proxybridge/src 为 ProxyBridgeCore.dll（参照上游 Windows/compile.ps1 的 MSVC 分支，去掉签名与 gcc）。
# 前置：先运行 `pnpm fetch-windivert`。输出：src-tauri/binaries/procsvc/ProxyBridgeCore.dll
$ErrorActionPreference = 'Stop'

$Root = Resolve-Path (Join-Path $PSScriptRoot '..')
$WinDivertPath = Join-Path $Root 'third_party\windivert\WinDivert-2.2.2-A'
$SourcePath = Join-Path $Root 'third_party\proxybridge\src'
$OutDir = Join-Path $Root 'src-tauri\binaries\procsvc'
$OutputDLL = Join-Path $OutDir 'ProxyBridgeCore.dll'
$Arch = 'x64'

# 与上游一致的源文件列表（见 pb_internal.h）
$SourceNames = 'ProxyBridge.c pb_util.c pb_process.c pb_rules.c pb_proxy.c pb_dns.c pb_socks5.c pb_http.c pb_conntrack.c pb_relay.c pb_sockmap.c'.Split(' ')

if (-not (Test-Path (Join-Path $WinDivertPath 'include\windivert.h'))) {
    Write-Error "WinDivert not found at: $WinDivertPath (run: pnpm fetch-windivert)"
    exit 1
}

$vsWhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path $vsWhere)) { Write-Error "vswhere.exe not found: $vsWhere"; exit 1 }
$vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsPath) { Write-Error 'Visual Studio C++ tools (VC.Tools.x86.x64) not found'; exit 1 }
$vcvars = Join-Path $vsPath 'VC\Auxiliary\Build\vcvarsall.bat'
if (-not (Test-Path $vcvars)) { Write-Error "vcvarsall.bat not found: $vcvars"; exit 1 }

New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

# 中间文件放临时目录，避免污染源码树
$ObjDir = Join-Path ([System.IO.Path]::GetTempPath()) ("proxybridge-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $ObjDir | Out-Null
$ResFile = Join-Path $ObjDir 'core.res'

try {
    $sources = ($SourceNames | ForEach-Object { "`"$(Join-Path $SourcePath $_)`"" }) -join ' '
    $clArgs = '/nologo /O2 /Ot /GL /Gy /W4 /wd4100 /wd4189 /wd4267 /wd4244 /wd4996 ' +
              '/D_CRT_SECURE_NO_WARNINGS /D_WINSOCK_DEPRECATED_NO_WARNINGS /DPROXYBRIDGE_EXPORTS /DNDEBUG ' +
              '/arch:SSE2 /fp:fast /GS /guard:cf /Qpar ' +
              "/Fo`"$ObjDir/`" " +  # 结尾用正斜杠，避免 \" 被当作转义引号
              "/I`"$WinDivertPath\include`" " +
              "$sources " +
              "`"$ResFile`" " +
              '/LD ' +
              '/link /LTCG /OPT:REF /OPT:ICF /RELEASE /DYNAMICBASE /NXCOMPAT ' +
              "/LIBPATH:`"$WinDivertPath\$Arch`" " +
              'WinDivert.lib ws2_32.lib iphlpapi.lib ' +
              "/OUT:`"$OutputDLL`" /IMPLIB:`"$ObjDir\ProxyBridgeCore.lib`""

    # 用临时 .cmd 执行，避免 cmd /c 对嵌套引号的解析问题
    $batFile = Join-Path $ObjDir 'build.cmd'
    $batLines = @(
        '@echo off',
        "set `"PATH=$(Split-Path $vsWhere);%PATH%`"",
        "call `"$vcvars`" $Arch >nul || exit /b 1",
        "rc /nologo /fo `"$ResFile`" `"$(Join-Path $SourcePath 'core.rc')`" || exit /b 1",
        "cl.exe $clArgs"
    )
    Set-Content -Path $batFile -Value $batLines -Encoding ASCII
    Write-Host "Compiling ProxyBridgeCore.dll with MSVC ($vsPath)"
    $ErrorActionPreference = 'Continue'  # 原生命令的 stderr 输出不应终止脚本
    cmd /c "`"$batFile`"" 2>&1 | ForEach-Object { Write-Host $_ }
    $buildExit = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($buildExit -ne 0) { Write-Error "MSVC build failed (exit $buildExit)"; exit 1 }
    if (-not (Test-Path $OutputDLL)) { Write-Error "Output not produced: $OutputDLL"; exit 1 }
    Write-Host "Built: $OutputDLL"
} finally {
    Remove-Item $ObjDir -Recurse -Force -ErrorAction SilentlyContinue
}
