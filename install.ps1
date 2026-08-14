<#
.SYNOPSIS
    Suture Git Merge Driver 安装器(install-merge-driver.sh 的 PowerShell 移植版)

.DESCRIPTION
    自动将 Git 配置为对结构化文件(JSON,YAML,TOML,XML,CSV,Markdown,DOCX,XLSX,PPTX)
    使用 Suture 的语义合并.

    本脚本的功能:
      1. 定位 suture 可执行文件(优先已安装 > 下载官方二进制 > 编译安装)
      2. 交互式引导选择安装位置与 PATH 写入位置
         (也可用 -InstallDir / -UserPath 参数直接指定,用于非交互/脚本调用)
      3. 为每种格式注册一个 Git merge driver,并显式指定 --driver <格式>
         (绝不使用 --driver auto:git 传给 driver 的是无扩展名的临时文件,
         自动检测无法工作)
      4. 将对应模式追加到 git attributes 文件
         (默认全局,或使用 -Local 追加到当前仓库的 .gitattributes)
      5. 二进制格式(docx/xlsx/pptx)自动设置 recursive=binary
      6. -Test 只验证当前配置,不做任何修改

    幂等性:已有扩展名对应的 attributes 条目不会被覆盖
    (打印警告提示并保留,而不是覆盖).

.PARAMETER Local
    仅配置当前仓库(默认:全局).

.PARAMETER Uninstall
    移除本脚本添加的所有 Suture merge driver 配置,
    并从用户级/系统级 PATH 中移除 suture 安装目录(含 suture.exe 的目录).

.PARAMETER Test
    仅验证当前配置,不做修改.

.PARAMETER Help
    显示用法说明.

.PARAMETER InstallDir
    自定义安装目录(放置 suture.exe 的目录).
    默认:C:\Program Files\suture\bin(需管理员权限).
    未指定时改为交互式引导询问.

.PARAMETER UserPath
    将安装目录加入用户级 PATH 而非系统级 PATH(无需管理员权限).
    未指定时改为交互式引导询问.

.EXAMPLE
    .\install-merge-driver.ps1                        # 交互式引导安装(询问位置与 PATH)
    .\install-merge-driver.ps1 -InstallDir "D:\tools\suture" -UserPath  # 参数直装(非交互)
    .\install-merge-driver.ps1 -Local                  # 仅配置当前仓库
    .\install-merge-driver.ps1 -Test                   # 仅验证不修改
    .\install-merge-driver.ps1 -Uninstall              # 移除配置
#>
param(
    [switch]$Local,
    [switch]$Uninstall,
    [switch]$Test,
    [switch]$Help,
    [string]$InstallDir = "",
    [switch]$UserPath
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# 颜色 / 日志
# ---------------------------------------------------------------------------
$Script:NC = [char]27 + "[0m"
$Script:RED = [char]27 + "[0;31m"
$Script:GREEN = [char]27 + "[0;32m"
$Script:YELLOW = [char]27 + "[1;33m"
$Script:BLUE = [char]27 + "[0;34m"

function Info  { Write-Host ("{0}[INFO]{1} {2}" -f $Script:BLUE, $Script:NC, $args[0]) }
function Ok    { Write-Host ("{0}[OK]{1} {2}" -f $Script:GREEN, $Script:NC, $args[0]) }
function Warn  { Write-Host ("{0}[WARN]{1} {2}" -f $Script:YELLOW, $Script:NC, $args[0]) }
function Fail  { Write-Host ("{0}[FAIL]{1} {2}" -f $Script:RED, $Script:NC, $args[0]); exit 1 }

# ---------------------------------------------------------------------------
# 配置
# ---------------------------------------------------------------------------
$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoLocalBinary = Join-Path $ScriptRoot "target\release\suture.exe"
$LocalExe = Join-Path $ScriptRoot "suture.exe"

# driver 名称 -> (显示名称, suture --driver 参数值, 是否二进制格式)
$Script:Drivers = [ordered]@{
    json  = @{ Name = "Suture JSON merge driver";     Driver = "json";     Binary = $false }
    yaml  = @{ Name = "Suture YAML merge driver";     Driver = "yaml";     Binary = $false }
    toml  = @{ Name = "Suture TOML merge driver";     Driver = "toml";     Binary = $false }
    xml   = @{ Name = "Suture XML merge driver";      Driver = "xml";      Binary = $false }
    ui    = @{ Name = "Suture UI merge driver";       Driver = "ui";       Binary = $false }
    csv   = @{ Name = "Suture CSV merge driver";      Driver = "csv";      Binary = $false }
    md    = @{ Name = "Suture Markdown merge driver"; Driver = "markdown"; Binary = $false }
    docx  = @{ Name = "Suture DOCX merge driver";     Driver = "docx";     Binary = $true }
    xlsx  = @{ Name = "Suture XLSX merge driver";     Driver = "xlsx";     Binary = $true }
    pptx  = @{ Name = "Suture PPTX merge driver";     Driver = "pptx";     Binary = $true }
}

# attributes 匹配模式: 模式 -> driver 名称
$Script:Patterns = @(
    "*.json", "*.jsonl", "*.yaml", "*.yml", "*.toml",
    "*.xml", "*.xsl", "*.svg", "*.ui", "*.csv", "*.tsv",
    "*.md", "*.markdown", "*.docx", "*.docm",
    "*.xlsx", "*.xlsm", "*.pptx", "*.pptm"
)

# 附加 attributes 属性条目: 模式 -> 属性(与上面的 merge= 条目共存于 attributes 文件)
# *.ui(Actions IDE 界面文件)本质是 XML,但禁止 git 行尾转换(-text),
# 避免 CRLF/LF 规范化破坏文件;合并由 ui driver处理(两者互不冲突).
$Script:ExtraAttributes = @(
    @{ Pattern = "*.ui"; Attr = "-text" }
)

function Get-DriverForPattern {
    param([string]$Pattern)
    $ext = $Pattern.Substring(2) # 去掉 "*. 前缀"
    switch ($ext) {
        "json"  { return "json" }
        "jsonl" { return "json" }
        "yaml"  { return "yaml" }
        "yml"   { return "yaml" }
        "toml"  { return "toml" }
        "xml"   { return "xml" }
        "xsl"   { return "xml" }
        "svg"   { return "xml" }
        "ui"    { return "ui" }
        "csv"   { return "csv" }
        "tsv"   { return "csv" }
        "md"    { return "md" }
        "markdown" { return "md" }
        "docx"  { return "docx" }
        "docm"  { return "docx" }
        "xlsx"  { return "xlsx" }
        "xlsm"  { return "xlsx" }
        "pptx"  { return "pptx" }
        "pptm"  { return "pptx" }
        default { return "" }
    }
}

# ---------------------------------------------------------------------------
# Git 辅助函数(兼容 stderr,基于退出码)
# ---------------------------------------------------------------------------
function Invoke-GitConfig {
    param([string[]]$Arguments)
    $oldEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $out = & git $Arguments 2>&1
    }
    finally {
        $ErrorActionPreference = $oldEAP
    }
    $code = $LASTEXITCODE
    return [pscustomobject]@{ Output = @($out | ForEach-Object { "$_" }); Code = $code }
}

# ---------------------------------------------------------------------------
# 查找 suture 可执行文件(已安装目录 / PATH / 仓库构建产物)
# ---------------------------------------------------------------------------
function Find-SutureBinary {
    # 仓库内构建产物优先(开发环境快捷方式)
    foreach ($cand in @($RepoLocalBinary)) {
        if (Test-Path $cand) { return $cand }
    }
    # 手工遍历 PATH,避免 Get-Command 在交互控制台找不到命令时
    # 输出 "Suggestion: 找不到命令 suture" 噪音
    foreach ($dir in @($env:PATH -split ';' | Where-Object { $_ })) {
        $cand = Join-Path $dir "suture.exe"
        if (Test-Path $cand) { return $cand }
    }
    return ""
}

# 检测当前进程是否以管理员身份运行
function Test-IsAdmin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($id)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# 解析安装目录:参数 > 默认 C:\Program Files\suture\bin
function Get-InstallDir {
    if ($InstallDir) { return $InstallDir }
    return "C:\Program Files\suture\bin"
}

# ---------------------------------------------------------------------------
# 引导式对话(仅在未显式指定对应参数时触发)
# ---------------------------------------------------------------------------
# 读取用户输入;在非交互环境(管道/CI)抛异常时给出明确提示.
function Read-Choice {
    param([string]$Prompt)
    try {
        return Read-Host $Prompt
    } catch {
        Fail "当前环境不支持交互输入.请改用参数指定:-InstallDir <路径> [-UserPath]"
    }
}

# 探测目录写入权限:写入测试文件再清理,不留任何副作用.
# 目录不存在时先尝试创建探测(成功则说明可写,探测后删除该目录).
function Test-DirWritable {
    param([string]$Dir)
    $created = $false
    if (-not (Test-Path -LiteralPath $Dir)) {
        try {
            New-Item -ItemType Directory -Force -Path $Dir -ErrorAction Stop | Out-Null
            $created = $true
        } catch {
            return $false
        }
    }
    try {
        $probe = Join-Path $Dir (".write-probe-" + [guid]::NewGuid().ToString("N") + ".tmp")
        Set-Content -LiteralPath $probe -Value "x" -ErrorAction Stop | Out-Null
        Remove-Item -LiteralPath $probe -Force -ErrorAction SilentlyContinue
        return $true
    } catch {
        return $false
    } finally {
        if ($created) {
            Remove-Item -LiteralPath $Dir -Force -ErrorAction SilentlyContinue
        }
    }
}

# 引导式询问安装位置,返回目录路径
function Prompt-InstallDir {
    $defaultDir = "C:\Program Files\suture\bin"
    $userDir = Join-Path $HOME "suture\bin"
    # 动态检测目录是否实际可写,输出准确的权限提示
    $defaultNote = if (Test-DirWritable $defaultDir) { "(无需管理员权限)" } else { "(需管理员权限)" }
    $userNote = if (Test-DirWritable $userDir) { "(无需管理员权限)" } else { "(需管理员权限)" }
    Write-Host ""
    Write-Host "请选择 Suture 安装位置:"
    Write-Host ("  [1] 默认目录 : {0}{1}" -f $defaultDir, $defaultNote)
    Write-Host ("  [2] 用户目录 : {0}{1}" -f $userDir, $userNote)
    Write-Host "  [3] 自定义目录..."
    $choice = Read-Choice "请选择(回车默认 2)"
    switch ($choice.Trim()) {
        "" { return $userDir }
        "1" { return $defaultDir }
        "2" { return $userDir }
        "3" {
            $custom = Read-Choice "请输入自定义安装目录"
            if ([string]::IsNullOrWhiteSpace($custom)) {
                Warn "输入为空,使用用户目录"
                return $userDir
            }
            return $custom.Trim()
        }
        default {
            Warn "无效选择,使用用户目录"
            return $userDir
        }
    }
}

# 引导式询问 PATH 写入位置,返回 $true 表示用户级
function Prompt-PathScope {
    Write-Host ""
    Write-Host "请选择 PATH 写入位置:"
    Write-Host "  [1] 系统级 PATH(推荐,需管理员权限)"
    Write-Host "  [2] 用户级 PATH(无需管理员权限)"
    $choice = Read-Choice "请选择(回车默认 2)"
    switch ($choice.Trim()) {
        "" { return $true }
        "1" { return $false }
        "2" { return $true }
        default {
            Warn "无效选择,使用用户级 PATH"
            return $true
        }
    }
}

# 从 GitHub Releases 下载官方二进制并解压到指定目录.返回 $true 表示成功.
function Download-SutureBinary {
    param([string]$DestDir)
    # 官方资产命名格式: suture-<arch>-<os>.[zip|tar.gz](如 suture-x86_64-windows.zip)
    $arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { "aarch64" }
    $os = if ($env:OS -match "Windows") { "windows" } elseif ($IsLinux) { "linux" } else { "macos" }
    $base = "https://github.com/Gocql022/suture/releases/latest/download"
    $candidates = @(
        @{ Name = "suture-${arch}-${os}.zip";    Kind = "zip" },
        @{ Name = "suture-${arch}-${os}.tar.gz"; Kind = "tar" }
    )
    $tmp = Join-Path $env:TEMP ("suture-dl-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    try {
        foreach ($cand in $candidates) {
            $url = "$base/$($cand.Name)"
            $dest = Join-Path $tmp $cand.Name
            Info "  尝试下载: $url"
            try {
                Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing -TimeoutSec 60
            } catch {
                continue
            }
            try {
                if ($cand.Kind -eq "tar") {
                    & tar -xzf $dest -C $DestDir 2>$null
                } else {
                    Expand-Archive -Path $dest -DestinationPath $DestDir -Force
                }
            } catch {
                continue
            }
            # 解压后查找 suture.exe(可能在子目录)
            $exe = Get-ChildItem -Path $DestDir -Recurse -Filter "suture.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($exe) {
                # 规范化路径比较(避免 8.3 短路径 vs 长路径导致误判)
                $destFull = [System.IO.Path]::GetFullPath($DestDir)
                $exeDirFull = [System.IO.Path]::GetFullPath($exe.DirectoryName)
                if ($exeDirFull -ne $destFull) {
                    Copy-Item $exe.FullName (Join-Path $DestDir "suture.exe") -Force
                }
                return $true
            }
        }
        return $false
    }
    finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

# 通过 cargo 编译安装到指定目录.返回 $true 表示成功.
function Compile-SutureBinary {
    param([string]$DestDir)
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        Warn "  cargo 未找到,无法编译安装."
        return $false
    }
    $root = Split-Path -Parent $DestDir   # cargo --root 会创建 bin 子目录
    Info "  正在编译安装 (cargo install --root $root)..."
    $oldEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & cargo install --root $root --locked suture-cli 2>&1 | Out-Host
        if ($LASTEXITCODE -ne 0) {
            & cargo install --root $root --locked suture-merge-driver 2>&1 | Out-Host
        }
    }
    finally {
        $ErrorActionPreference = $oldEAP
    }
    $built = Join-Path $root "bin\suture.exe"
    if (Test-Path $built) {
        if ((Split-Path $built -Parent) -ne $DestDir) {
            Copy-Item $built $DestDir -Force
        }
        return $true
    }
    return $false
}

# 将安装目录加入 PATH(系统级或用户级)
function Add-ToPath {
    param([string]$Dir, [bool]$UseUserScope)
    $scopeName = if ($UseUserScope) { "用户" } else { "系统" }
    $target = if ($UseUserScope) { [EnvironmentVariableTarget]::User } else { [EnvironmentVariableTarget]::Machine }

    if (-not $UseUserScope -and -not (Test-IsAdmin)) {
        Fail "修改系统 PATH 需要管理员权限.请以管理员身份运行 PowerShell,或改用 -UserPath(用户级 PATH,无需管理员)."
    }

    $cur = [Environment]::GetEnvironmentVariable("PATH", $target)
    if ($cur -and (@($cur -split ';') -contains $Dir)) {
        Ok "  $scopeName PATH 已包含 $Dir"
        return
    }
    $new = if ($cur) { "$Dir;$cur" } else { $Dir }
    [Environment]::SetEnvironmentVariable("PATH", $new, $target)
    Ok "  已添加 $scopeName PATH: $Dir"
}

# 从 PATH(系统级或用户级)中移除存在 suture.exe 的目录
# 返回值: $true=已移除 / $false=未找到无需修改 / $null=因权限不足跳过
function Remove-FromPath {
    param([bool]$UseUserScope)
    $scopeName = if ($UseUserScope) { "用户" } else { "系统" }
    $target = if ($UseUserScope) { [EnvironmentVariableTarget]::User } else { [EnvironmentVariableTarget]::Machine }

    if (-not $UseUserScope -and -not (Test-IsAdmin)) {
        Warn "  移除系统 PATH 需要管理员权限.请以管理员身份运行 PowerShell 后重试 -Uninstall."
        return $null
    }

    $cur = [Environment]::GetEnvironmentVariable("PATH", $target)
    if (-not $cur) { return $false }

    $kept = New-Object System.Collections.Generic.List[string]
    $removed = New-Object System.Collections.Generic.List[string]
    foreach ($e in @($cur -split ';' | Where-Object { $_.Trim() })) {
        $clean = $e.Trim().Trim('"')
        $exe = Join-Path $clean "suture.exe"
        if ($clean -and [System.IO.Path]::IsPathRooted($clean) -and (Test-Path -LiteralPath $exe -ErrorAction SilentlyContinue)) {
            $removed.Add($e)
        } else {
            $kept.Add($e)
        }
    }

    if ($removed.Count -eq 0) { return $false }

    [Environment]::SetEnvironmentVariable("PATH", ($kept -join ';'), $target)
    Ok "  已从 $scopeName PATH 移除: $($removed -join '; ')"
    return $true
}

# 安装(或复用)suture 二进制,返回其完整路径.
# $ForceDir: 用户显式指定了 -InstallDir,此时必须安装到该目录(不复用 PATH)
function Install-SutureBinary {
    param([string]$DestDir, [bool]$ForceDir)

    # 1) 目标目录已有 suture.exe → 直接复用
    $inDir = Join-Path $DestDir "suture.exe"
    if (Test-Path $inDir) {
        Ok "Suture found: $inDir"
        return $inDir
    }

    # 2) 未显式指定目录时,PATH 中已有可用 suture → 复用
    #    (提示用户可指定 -InstallDir 重新安装)
    if (-not $ForceDir) {
        $onPath = Find-SutureBinary
        if ($onPath) {
            Ok "Suture found on PATH: $onPath"
            return $onPath
        }
    }

    # 3) 创建安装目录(权限不足时给出明确指引)
    Info "安装 Suture 到 $DestDir ..."
    try {
        New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
    } catch {
        Fail "无法创建安装目录 $DestDir(可能需要管理员权限).请以管理员身份运行 PowerShell,或使用 -InstallDir 指定可写目录(如 $HOME\suture\bin)."
    }

    # 4) 优先下载官方二进制,失败则尝试脚本同目录的 suture.exe,再编译安装
    if (Download-SutureBinary $DestDir) {
        Ok "  已通过官方二进制安装: $inDir"
        return $inDir
    }
    Warn "  官方二进制下载失败,尝试脚本同目录的 suture.exe..."
    if (Test-Path $LocalExe) {
        Copy-Item $LocalExe $inDir -Force
        Ok "  已从脚本同目录复制: $LocalExe"
        return $inDir
    }
    Warn "  官方二进制下载失败,尝试编译安装..."
    if (Compile-SutureBinary $DestDir) {
        Ok "  已通过编译安装: $inDir"
        return $inDir
    }
    Fail "Suture 安装失败.请手动安装:cargo install suture-cli,或从 https://github.com/WyattAu/suture/releases 下载."
}

# ---------------------------------------------------------------------------
# driver 注册
# ---------------------------------------------------------------------------
function Get-GitScope {
    param([bool]$RepoOnly)
    return $(if ($RepoOnly) { "--local" } else { "--global" })
}

function Configure-Drivers {
    param([string]$Suture, [bool]$RepoOnly)
    $scope = Get-GitScope $RepoOnly
    $scopeLabel = if ($RepoOnly) { "local (current repo)" } else { "global" }
    Info "Configuring Git merge drivers ($scopeLabel)..."

    # 使用相对命令名 suture(依赖 PATH):安装流程已先把安装目录加入 PATH,
    # 且合并时 git 用 sh 执行 driver 命令,相对命令名最可靠.
    # 不要用带引号的绝对路径:git config --get 读取时会剥离引号,路径含空格时
    # sh 按空格拆词,报 "No such file or directory",driver 静默失败.
    foreach ($name in $Script:Drivers.Keys) {
        $d = $Script:Drivers[$name]
        $cmd = "suture merge-file --driver $($d.Driver) %O %A %B -o %A"
        Invoke-GitConfig @("config", $scope, "merge.$name.name", $d.Name) | Out-Null
        Invoke-GitConfig @("config", $scope, "merge.$name.driver", $cmd) | Out-Null
        if ($d.Binary) {
            Invoke-GitConfig @("config", $scope, "merge.$name.recursive", "binary") | Out-Null
        }
        Ok "  $name driver: $cmd"
    }
}

# ---------------------------------------------------------------------------
# attributes 配置
# ---------------------------------------------------------------------------
function Get-AttributesFile {
    param([bool]$RepoOnly)
    if ($RepoOnly) {
        return (Join-Path (Get-Location) ".gitattributes")
    }
    $cfg = Invoke-GitConfig @("config", "--global", "--get", "core.attributesfile")
    # 必须检查退出码:key 不存在时 git 退出非 0 且 stderr 输出 "fatal: ...",
    # 若不检查会被误当成路径返回
    if ($cfg.Code -eq 0 -and $cfg.Output -and $cfg.Output[0]) {
        return $cfg.Output[0].Trim()
    }
    # git 的默认全局 attributes 位置
    return (Join-Path $HOME ".config\git\attributes")
}

function Ensure-Attributes {
    param([string]$AttrFile, [bool]$RepoOnly)
    $dir = Split-Path -Parent $AttrFile
    if ($dir -and -not (Test-Path $dir)) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    $existing = @()
    if (Test-Path $AttrFile) {
        $existing = @(Get-Content $AttrFile | ForEach-Object { $_.Trim() })
    }

    $lines = @($existing)
    $skipped = 0
    $added = 0
    foreach ($pat in $Script:Patterns) {
        $driver = Get-DriverForPattern $pat
        $entry = "$pat merge=$driver"
        # 若该条目已存在则跳过
        if ($lines -contains $entry) {
            continue
        }
        # 若该扩展名已有非注释的 merge 条目则跳过
        # (不覆盖用户配置;注释行不算)
        $patPrefix = $pat.Replace(".", '\.').Replace("*", '\*')
        $already = $lines | Where-Object { $_ -match "^${patPrefix}\s+merge=" }
        if ($already) {
            Warn "  $pat already has an entry ('$($already[0])') - keeping it"
            $skipped++
            continue
        }
        $lines += $entry
        $added++
    }

    # 附加属性条目(如 *.ui -text):与 merge= 条目独立,同样保持幂等
    foreach ($extra in $Script:ExtraAttributes) {
        $entry = "$($extra.Pattern) $($extra.Attr)"
        if ($lines -contains $entry) {
            continue
        }
        # 已有该模式的其他 text 相关条目(text / -text / binary)则跳过
        # (不覆盖用户配置)
        $patPrefix = $extra.Pattern.Replace(".", '\.').Replace("*", '\*')
        $already = $lines | Where-Object { $_ -match "^${patPrefix}\s+(text|-text|binary)(\s|$)" }
        if ($already) {
            Warn "  $($extra.Pattern) already has a text/binary entry ('$($already[0])') - keeping it"
            $skipped++
            continue
        }
        $lines += $entry
        $added++
    }

    if ($added -gt 0) {
        $lines += "" # 末尾追加空行
        Set-Content -Path $AttrFile -Value $lines -Encoding UTF8
        Ok "  $AttrFile updated (+$added patterns)"
    } else {
        Ok "  $AttrFile already up to date"
    }
    if ($skipped -gt 0) {
        Warn "  $skipped pattern(s) left untouched (existing entries preserved)"
    }

    # 确保 git 确实读取该文件作为全局 attributes.
    # git 的默认位置遵循 XDG 规范($XDG_CONFIG_HOME 或
    # $HOME/.config/git/attributes);显式设置 core.attributesfile
    # 可使配置不受 XDG / HOME 变化的影响.
    if (-not $RepoOnly) {
        Invoke-GitConfig @("config", "--global", "core.attributesfile", $AttrFile) | Out-Null
        Ok "  core.attributesfile -> $AttrFile"
    }
}

# ---------------------------------------------------------------------------
# 配置验证
# ---------------------------------------------------------------------------
# $Suture: 已知的 suture 路径(安装流程传入,避免重新查找);
#          为空时回退到 Find-SutureBinary(适用于 -Test 独立验证).
function Test-Configuration {
    param([bool]$RepoOnly, [string]$Suture = "")
    Info "Testing configuration..."
    $errors = 0

    $suture = if ($Suture) { $Suture } else { Find-SutureBinary }
    if (-not $suture -or -not (Test-Path $suture)) {
        Warn "  suture not found on PATH - driver commands will fail at merge time"
        $errors++
    } else {
        $ver = & $suture version 2>&1 | Select-Object -First 1
        Ok "  suture binary: $suture ($ver)"
    }

    foreach ($name in $Script:Drivers.Keys) {
        $scope = Get-GitScope $RepoOnly
        $r = Invoke-GitConfig @("config", $scope, "--get", "merge.$name.driver")
        if ($r.Code -eq 0 -and $r.Output -and $r.Output[0]) {
            Ok "  $name driver: $($r.Output[0])"
        } else {
            Warn "  $name driver not configured"
            $errors++
        }
    }

    $attrFile = Get-AttributesFile $RepoOnly
    if (Test-Path $attrFile) {
        $attrLines = @(Get-Content $attrFile)
        $count = @($attrLines | Where-Object { $_ -match "merge=" }).Count
        Ok "  attributes ($attrFile): $count merge= pattern(s)"
        # 验证附加属性条目(如 *.ui -text);等价形式(text/binary)也算通过
        foreach ($extra in $Script:ExtraAttributes) {
            $entry = "$($extra.Pattern) $($extra.Attr)"
            if ($attrLines -contains $entry) {
                Ok "  $entry present"
            } else {
                $patPrefix = $extra.Pattern.Replace(".", '\.').Replace("*", '\*')
                $hasText = $attrLines | Where-Object { $_ -match "^${patPrefix}\s+(text|-text|binary)(\s|$)" }
                if ($hasText) {
                    Ok "  $($extra.Pattern) has equivalent text/binary entry: $($hasText[0])"
                } else {
                    Warn "  $entry missing - re-run the installer to add it"
                    $errors++
                }
            }
        }
    } else {
        Warn "  attributes file not found: $attrFile"
        $errors++
    }

    # 全局模式:core.attributesfile 必须指向实际文件
    if (-not $RepoOnly) {
        $cfg = Invoke-GitConfig @("config", "--global", "--get", "core.attributesfile")
        if ($cfg.Code -ne 0 -or -not $cfg.Output -or -not $cfg.Output[0]) {
            Warn "  core.attributesfile not set - re-run the installer to pin the global attributes file"
            $errors++
        } elseif ($cfg.Output[0].Trim() -ne $attrFile) {
            Warn "  core.attributesfile ($($cfg.Output[0].Trim())) differs from expected ($attrFile)"
            $errors++
        } else {
            Ok "  core.attributesfile -> $attrFile"
        }
    }

    if ($errors -gt 0) {
        Fail "$errors problem(s) found - re-run the installer without -Test"
    }
    Ok "Configuration OK"
}

# ---------------------------------------------------------------------------
# 卸载
# ---------------------------------------------------------------------------
function Uninstall-Configuration {
    Info "Removing Suture merge driver configuration..."

    # 删除 driver 段(全局和本地都删,忽略 -Local 参数)
    foreach ($scope in @("--global", "--local")) {
        foreach ($name in $Script:Drivers.Keys) {
            Invoke-GitConfig @("config", $scope, "--unset", "merge.$name.name") | Out-Null
            Invoke-GitConfig @("config", $scope, "--unset", "merge.$name.driver") | Out-Null
            Invoke-GitConfig @("config", $scope, "--unset", "merge.$name.recursive") | Out-Null
        }
    }
    Ok "  git config cleaned"

    # 从两个候选 attributes 文件中移除本脚本添加的条目
    # (merge= 行 + 附加属性行,如 *.ui -text)
    foreach ($attrFile in @((Get-AttributesFile $true), (Get-AttributesFile $false))) {
        if (-not (Test-Path $attrFile)) { continue }
        $excludeRx = @()
        foreach ($pat in $Script:Patterns) {
            $excludeRx += "^\s*$($pat.Replace(".", '\.').Replace("*", '\*'))\s+merge="
        }
        foreach ($extra in $Script:ExtraAttributes) {
            $excludeRx += "^\s*$($extra.Pattern.Replace(".", '\.').Replace("*", '\*'))\s+$([regex]::Escape($extra.Attr))\s*$"
        }
        $allLines = @(Get-Content $attrFile)
        $kept = @($allLines | Where-Object {
            $line = $_
            $hit = $false
            foreach ($rx in $excludeRx) {
                if ($line -match $rx) { $hit = $true; break }
            }
            -not $hit
        })
        if ($kept.Count -ne $allLines.Count) {
            Set-Content -Path $attrFile -Value $kept -Encoding UTF8
            Ok "  cleaned $attrFile"
        }
    }
    Ok "Suture merge driver uninstalled"

    # 从 PATH 中移除 suture 安装目录(用户级与系统级)
    $u = Remove-FromPath $true
    $m = Remove-FromPath $false
    if (($u -eq $false) -and ($m -eq $false)) {
        Info "  PATH 中未找到 suture.exe 所在目录,无需修改"
    }
}

# ---------------------------------------------------------------------------
# 用法说明
# ---------------------------------------------------------------------------
function Show-Usage {
    @"
Suture Git Merge Driver Installer (PowerShell)

用法:
  .\install-merge-driver.ps1 [选项]

选项(不指定时以交互式引导询问):
  -Local          仅配置当前仓库(默认:全局)
  -Uninstall      移除本脚本添加的所有 Suture merge driver 配置,并清理 PATH 中的 suture 目录
  -Test           仅验证当前配置,不做修改
  -Help           显示本帮助
  -InstallDir <路径>  自定义安装目录(默认引导询问,如 C:\Program Files\suture\bin)
  -UserPath       加入用户级 PATH(默认引导询问,否则系统级 PATH 需管理员)

示例:
  .\install-merge-driver.ps1                    # 交互式引导安装
  .\install-merge-driver.ps1 -InstallDir "D:\tools\suture" -UserPath
  .\install-merge-driver.ps1 -Local
  .\install-merge-driver.ps1 -Test
"@ | Write-Host
}

# ---------------------------------------------------------------------------
# 主流程
# ---------------------------------------------------------------------------
if ($Help) {
    Show-Usage
    exit 0
}

if ($Test) {
    Test-Configuration $Local
    exit 0
}

if ($Uninstall) {
    Uninstall-Configuration
    exit 0
}

Write-Host ""
Write-Host ("{0}============================================{1}" -f $Script:BLUE, $Script:NC)
Write-Host ("{0}    Suture Git Merge Driver Installer      {1}" -f $Script:BLUE, $Script:NC)
Write-Host ("{0}============================================{1}" -f $Script:BLUE, $Script:NC)
Write-Host ""

# 安装(或复用)suture 二进制,并将其目录加入 PATH
# 注意:PowerShell 变量名不区分大小写,$InstallDir 与 $installDir 是同一变量,
# 因此必须在赋值前记录用户是否显式指定了 -InstallDir.
$userSpecifiedInstallDir = -not [string]::IsNullOrEmpty($InstallDir)
$useUserPath = [bool]$UserPath

# 引导式收集未显式指定的选项
if (-not $userSpecifiedInstallDir) {
    $installDir = Prompt-InstallDir
} else {
    $installDir = Get-InstallDir
}
if (-not $useUserPath) {
    $useUserPath = Prompt-PathScope
}

# 权限检查:系统级 PATH 需要管理员权限
if (-not $useUserPath -and -not (Test-IsAdmin)) {
    Write-Host ""
    Warn "系统级 PATH 需要管理员权限,当前会话不是管理员."
    if (-not $userSpecifiedInstallDir) {
        # 引导模式下:给用户一次降级选择的机会
        $c = Read-Choice "是否改用用户级 PATH 继续?(y/n,回车默认 y)"
        if ($c.Trim() -match '^[nN]') {
            Fail "请以管理员身份重新运行本脚本,或改用用户级 PATH."
        }
        $useUserPath = $true
    } else {
        Fail "请以管理员身份重新运行本脚本,或改用 -UserPath(用户级 PATH)."
    }
}

if ($useUserPath) {
    Info "将使用用户级 PATH(安装目录: $installDir)"
} else {
    Info "将使用系统级 PATH(安装目录: $installDir)"
}

$suture = Install-SutureBinary $installDir $userSpecifiedInstallDir
Add-ToPath (Split-Path $suture -Parent) $useUserPath
Configure-Drivers $suture $Local
Ensure-Attributes (Get-AttributesFile $Local) $Local

Write-Host ""
# 传入刚安装的 exe 路径验证:新写入的用户 PATH 在当前进程不生效,
# 重新查找会误报 "suture not found".
Test-Configuration $Local $suture

Write-Host ""
Ok "Suture 已成功安装并配置完成!"
Write-Host ""
Write-Host "  Git现在会对以下文件类型使用Suture进行合并:"
Write-Host "    UI  JSON  YAML  TOML  XML  CSV  Markdown  DOCX  XLSX  PPTX"
Write-Host ""
Write-Host ("  suture 位置: {0}" -f $suture)
if ($useUserPath) {
    Write-Host "  PATH: 用户级(新开终端生效)"
} else {
    Write-Host "  PATH: 系统级(新开终端生效)"
}
Write-Host ""
if ($Local) {
    Write-Host "  Commit the generated .gitattributes:"
    Write-Host "    git add .gitattributes"
    Write-Host "    git commit -m 'Configure suture merge driver'"
} else {
    Write-Host ("  全局attributes位置: {0}" -f (Get-AttributesFile $false))
}
Write-Host ""
Write-Host "  如需卸载,请运行:"
Write-Host "    .\install.ps1 -Uninstall"
Write-Host ""
