# 用 MSVC 编译并运行 third_party/proxybridge/tests 下的单元测试（参照上游 Windows/tests/run_tests.ps1 的引擎测试编译方式）。
# 测试 exe 与 src/ 全部源文件一起编译，只链接 WinDivert 导入库，不加载驱动，无需管理员。
# 前置：先运行 `pnpm fetch-windivert`。退出码 0 = 全部通过。
$ErrorActionPreference = 'Stop'

$Root = Resolve-Path (Join-Path $PSScriptRoot '..')
$WinDivertPath = Join-Path $Root 'third_party\windivert\WinDivert-2.2.2-A'
$PbRoot = Join-Path $Root 'third_party\proxybridge'
$SourcePath = Join-Path $PbRoot 'src'
$TestPath = Join-Path $PbRoot 'tests'
$Arch = 'x64'

# 与 build-proxybridge.ps1 相同的源文件列表
$SourceNames = 'ProxyBridge.c pb_util.c pb_process.c pb_rules.c pb_proxy.c pb_dns.c pb_socks5.c pb_http.c pb_conntrack.c pb_udp.c pb_relay.c pb_sockmap.c'.Split(' ')
$Tests = @('test_routa_patches', 'test_udp_policy', 'test_udp_relay')

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

# 中间文件与测试 exe 放临时目录，避免污染源码树
$OutDir = Join-Path ([System.IO.Path]::GetTempPath()) ("proxybridge-test-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $OutDir | Out-Null

$failed = 0
try {
    # 测试 exe 链接 WinDivert 导入库，启动时需要 WinDivert.dll（驱动不会被加载）
    Copy-Item (Join-Path $WinDivertPath "$Arch\WinDivert.dll") $OutDir

    $sources = ($SourceNames | ForEach-Object { "`"$(Join-Path $SourcePath $_)`"" }) -join ' '
    foreach ($name in $Tests) {
        Write-Host "=== $name"
        $objDir = Join-Path $OutDir $name
        New-Item -ItemType Directory -Path $objDir | Out-Null
        $exe = Join-Path $OutDir "$name.exe"
        $clArgs = '/nologo /W3 /wd4100 /wd4189 /wd4267 /wd4244 /wd4996 ' +
                  '/D_CRT_SECURE_NO_WARNINGS /D_WINSOCK_DEPRECATED_NO_WARNINGS /DPROXYBRIDGE_EXPORTS ' +
                  "/Fo`"$objDir/`" " +  # 结尾用正斜杠，避免 \" 被当作转义引号
                  "/I`"$SourcePath`" /I`"$TestPath`" /I`"$WinDivertPath\include`" " +
                  "`"$(Join-Path $TestPath "$name.c")`" $sources " +
                  "/Fe`"$exe`" " +
                  "/link /LIBPATH:`"$WinDivertPath\$Arch`" WinDivert.lib ws2_32.lib iphlpapi.lib advapi32.lib"

        # 用临时 .cmd 执行，避免 cmd /c 对嵌套引号的解析问题
        $batFile = Join-Path $OutDir "build_$name.cmd"
        $batLines = @(
            '@echo off',
            "set `"PATH=$(Split-Path $vsWhere);%PATH%`"",
            "call `"$vcvars`" $Arch >nul || exit /b 1",
            "cl.exe $clArgs"
        )
        Set-Content -Path $batFile -Value $batLines -Encoding ASCII
        $ErrorActionPreference = 'Continue'  # 原生命令的 stderr 输出不应终止脚本
        $log = cmd /c "`"$batFile`"" 2>&1
        $buildExit = $LASTEXITCODE
        $ErrorActionPreference = 'Stop'
        if ($buildExit -ne 0 -or -not (Test-Path $exe)) {
            Write-Host "BUILD FAILED"
            $log | Where-Object { $_ -match 'error|fatal|LNK' } | ForEach-Object { Write-Host "  $_" }
            $failed++
            continue
        }
        # 编译警告也显示出来（cl 会回显源文件名，过滤掉）
        $log | Where-Object { $_ -match 'warning' } | ForEach-Object { Write-Host "  $_" }

        & $exe | ForEach-Object { Write-Host "  $_" }
        if ($LASTEXITCODE -ne 0) { Write-Host "FAIL (exit $LASTEXITCODE)"; $failed++ } else { Write-Host 'PASS' }
    }
} finally {
    Remove-Item $OutDir -Recurse -Force -ErrorAction SilentlyContinue
}

if ($failed -ne 0) { Write-Error "$failed test program(s) failed"; exit 1 }
Write-Host 'All ProxyBridge tests passed'
