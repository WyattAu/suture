<#
.SYNOPSIS
    测试 install-merge-driver.ps1（Suture Git Merge Driver 安装器）的 PowerShell 测试脚本。

.DESCRIPTION
    通过"备份-恢复"策略保证测试不污染真实环境：
      1. 备份真实 ~/.gitconfig、全局 attributes 文件、用户 PATH
      2. 清空 .gitconfig 与 attributes（模拟全新环境）后运行各测试场景
      3. 无论成功失败，finally 恢复全部备份
    注意：本机 git for windows 不识别 GIT_CONFIG_GLOBAL 环境变量，
    因此采用文件级备份/恢复而非环境变量隔离。

    测试场景：
      [1] 无配置时 -Test 应报告缺失（检测功能有效）
      [2] 完整安装（-InstallDir <临时目录> -UserPath）应：
            - 安装/复用 suture.exe
            - 把安装目录加入用户 PATH
            - 注册 10 个 merge driver 到 git config
            - 生成 attributes 条目 + 绑定 core.attributesfile
      [3] 安装后 -Test 应 Configuration OK
      [4] 幂等性：重复安装不产生重复条目

.EXAMPLE
    .\scripts\test-install-merge-driver.ps1
#>
param()

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
$Installer = Join-Path $ProjectDir "install.ps1"

# ---------------------------------------------------------------------------
# 备份状态（文件级备份/恢复隔离）
# 说明：本机 git for windows 不识别 GIT_CONFIG_GLOBAL 环境变量，
# 因此采用备份/恢复真实文件的方式保证测试不污染环境。
# ---------------------------------------------------------------------------
$GitConfigPath = Join-Path $env:USERPROFILE ".gitconfig"   # git for windows 全局配置
$RealAttributes = "C:\Users\yhc013044\.config\git\attributes"
$OrigUserPath = [Environment]::GetEnvironmentVariable("PATH", "User")

$TestRoot = Join-Path $env:TEMP ("suture-test-" + [guid]::NewGuid().ToString("N"))
$InstallBin = Join-Path $TestRoot "suture\bin"

$BackupGitConfig = Join-Path $TestRoot "backup.gitconfig"
$BackupAttributes = Join-Path $TestRoot "backup.attributes"

function Test-Cleanup {
    # 恢复 .gitconfig
    if (Test-Path $BackupGitConfig) {
        Copy-Item $BackupGitConfig $GitConfigPath -Force
        Write-Host "  已恢复 ~/.gitconfig"
    }
    # 恢复 attributes
    if (Test-Path $BackupAttributes) {
        Copy-Item $BackupAttributes $RealAttributes -Force
        Write-Host "  已恢复 attributes"
    }
    # 恢复用户 PATH
    if ($null -ne $OrigUserPath) {
        [Environment]::SetEnvironmentVariable("PATH", $OrigUserPath, "User")
        Write-Host "  已恢复用户 PATH"
    }
    # 删除临时目录
    Remove-Item -Recurse -Force $TestRoot -ErrorAction SilentlyContinue
}

# 统计与失败处理
$Script:PassCount = 0
$Script:FailCount = 0
function Assert {
    param([bool]$Condition, [string]$Name, [string]$Detail = "")
    if ($Condition) {
        $Script:PassCount++
        Write-Host ("  [PASS] {0}" -f $Name) -ForegroundColor Green
    } else {
        $Script:FailCount++
        Write-Host ("  [FAIL] {0}  {1}" -f $Name, $Detail) -ForegroundColor Red
    }
}

try {
    # ------------------------------------------------------------------
    # 备份真实配置，清空以模拟全新环境
    # ------------------------------------------------------------------
    New-Item -ItemType Directory -Force -Path $TestRoot, $InstallBin | Out-Null
    if (Test-Path $GitConfigPath) {
        Copy-Item $GitConfigPath $BackupGitConfig -Force
    }
    if (Test-Path $RealAttributes) {
        Copy-Item $RealAttributes $BackupAttributes -Force
    }

    Write-Host "========================================================"
    Write-Host "  Suture 安装脚本测试（备份/恢复隔离）"
    Write-Host "========================================================"
    Write-Host (".gitconfig : {0}" -f $GitConfigPath)
    Write-Host ("attributes : {0}" -f $RealAttributes)
    Write-Host ("安装目录   : {0}" -f $InstallBin)
    Write-Host ""

    # 模拟全新环境：删除 .gitconfig 与 attributes
    # 注意：不能用 Set-Content 清空（UTF8 BOM 会让 git 报 bad config line），
    # 直接删除文件，git 会重新创建。
    Remove-Item $GitConfigPath -Force -ErrorAction SilentlyContinue
    Remove-Item $RealAttributes -Force -ErrorAction SilentlyContinue

    # ------------------------------------------------------------------
    # [1] 无配置时 -Test 应报告缺失
    # ------------------------------------------------------------------
    Write-Host "[1/4] 无配置时 -Test 应报告缺失..."
    # 用 *>&1 合并所有流（Write-Host 在 PS5.1 写 information stream，2>&1 捕获不到）
    $r1 = & $Installer -Test *>&1 | Out-String
    $r1exit = $LASTEXITCODE
    Assert ($r1exit -ne 0) "无配置时 -Test 返回非零" "exit=$r1exit"
    Assert ($r1 -match "not configured") "-Test 输出含 'not configured' 提示"

    # ------------------------------------------------------------------
    # [2] 完整安装（临时目录 + 用户 PATH）
    # ------------------------------------------------------------------
    Write-Host ""
    Write-Host "[2/4] 完整安装（-InstallDir $InstallBin -UserPath）..."
    $r2 = & $Installer -InstallDir $InstallBin -UserPath *>&1 | Out-String
    $installExit = $LASTEXITCODE
    Assert ($installExit -eq 0) "安装脚本退出码为 0" "exit=$installExit"

    # 2a. suture.exe 已安装且可用
    $installedExe = Join-Path $InstallBin "suture.exe"
    Assert (Test-Path $installedExe) "suture.exe 存在于安装目录"
    if (Test-Path $installedExe) {
        $null = & $installedExe version 2>&1
        $verExit = $LASTEXITCODE
        $ver = & $installedExe version 2>&1 | Select-Object -First 1
        Assert ($verExit -eq 0) "suture.exe 可运行 (version)" "($ver)"
    }

    # 2b. 安装目录已加入用户 PATH
    $curUserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    Assert (@($curUserPath -split ';') -contains $InstallBin) "安装目录已加入用户 PATH"

    # 2c. git config 中注册了 10 个 driver（json/yaml/toml/xml/ui/csv/md/docx/xlsx/pptx）
    $driverOk = $true
    foreach ($d in @("json","yaml","toml","xml","ui","csv","md","docx","xlsx","pptx")) {
        $val = git config --global --get "merge.$d.driver" 2>$null
        if (-not $val) { $driverOk = $false; break }
    }
    Assert $driverOk "10 个 merge driver 已注册到 git config"

    # 2d. 二进制格式 recursive=binary
    $rec = git config --global --get "merge.docx.recursive" 2>$null
    Assert ($rec -eq "binary") "docx 设置了 recursive=binary" "($rec)"

    # 2e. attributes 文件生成且绑定 core.attributesfile
    $cfgAttr = git config --global --get core.attributesfile 2>$null
    Assert ($cfgAttr -and (Test-Path $cfgAttr)) "core.attributesfile 已配置并存在" "($cfgAttr)"
    if ($cfgAttr) {
        $attrContent = Get-Content $cfgAttr -Raw
        $jsonPat = $attrContent -match '^\*\.json\s+merge=json$' -or $attrContent -match "\*\.json\s+merge=json"
        $docxPat = $attrContent -match "\*\.docx\s+merge=docx"
        $uiPat = $attrContent -match "\*\.ui\s+merge=ui"
        $uiTextPat = $attrContent -match "\*\.ui\s+-text"
        Assert $jsonPat "attributes 含 *.json merge=json"
        Assert $docxPat "attributes 含 *.docx merge=docx"
        Assert $uiPat "attributes 含 *.ui merge=ui"
        Assert $uiTextPat "attributes 含 *.ui -text(禁止行尾转换)"
    }

    # ------------------------------------------------------------------
    # [3] 安装后 -Test 应 Configuration OK
    # ------------------------------------------------------------------
    Write-Host ""
    Write-Host "[3/4] 安装后 -Test 应 Configuration OK..."
    $r3 = & $Installer -Test *>&1 | Out-String
    $r3exit = $LASTEXITCODE
    Assert ($r3exit -eq 0) "安装后 -Test 返回 0" "exit=$r3exit"
    Assert ($r3 -match "Configuration OK") "-Test 输出含 'Configuration OK'"

    # ------------------------------------------------------------------
    # [4] 幂等性：重复安装不产生重复条目
    # ------------------------------------------------------------------
    Write-Host ""
    Write-Host "[4/4] 幂等性：重复安装不产生重复条目..."
    $r4 = & $Installer -InstallDir $InstallBin -UserPath *>&1 | Out-String
    $r4exit = $LASTEXITCODE
    Assert ($r4exit -eq 0) "重复安装退出码为 0" "exit=$r4exit"
    $cfgAttr2 = git config --global --get core.attributesfile 2>$null
    if ($cfgAttr2) {
        $lines = @(Get-Content $cfgAttr2 | Where-Object { $_ -match '^\*.*merge=' })
        $dups = @($lines | Group-Object | Where-Object { $_.Count -gt 1 })
        Assert (-not $dups) "attributes 无重复 merge 条目" "重复: $($dups.Name -join ',')"
    }
    $curUserPath2 = [Environment]::GetEnvironmentVariable("PATH", "User")
    $count = @($curUserPath2 -split ';' | Where-Object { $_ -eq $InstallBin }).Count
    Assert ($count -eq 1) "用户 PATH 无重复安装目录" "(出现 $count 次)"

    # ------------------------------------------------------------------
    # 汇总
    # ------------------------------------------------------------------
    Write-Host ""
    Write-Host "========================================================"
    Write-Host ("  通过: {0}   失败: {1}" -f $Script:PassCount, $Script:FailCount)
    Write-Host "========================================================"
    if ($Script:FailCount -gt 0) { exit 1 } else { exit 0 }
}
finally {
    Test-Cleanup
    Write-Host ""
    Write-Host "已清理测试环境（PATH/HOME/临时目录已恢复）"
}
