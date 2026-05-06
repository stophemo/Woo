# 扫描 Python 残留目录
$paths = @(
  'C:\Users\Administrator\AppData\Local\Programs\Python\Python38',
  'C:\Users\Administrator\AppData\Local\Programs\Python\Python38-32',
  'C:\Users\Administrator\AppData\Local\Programs\Python\Python39',
  'C:\Users\Administrator\AppData\Local\Programs\Python\Python39-32',
  'C:\Users\Administrator\AppData\Local\Programs\Python\Python310',
  'C:\Users\Administrator\AppData\Local\Programs\Python\Python310-32',
  'C:\Users\Administrator\AppData\Local\Programs\Python\Python311',
  'C:\Program Files\Python38',
  'C:\Program Files\Python38-32',
  'C:\Program Files\Python39',
  'C:\Program Files\Python39-32',
  'C:\Program Files\Python310',
  'C:\Program Files\Python310-32',
  'C:\Program Files\Python311',
  'C:\Program Files (x86)\Python38-32',
  'C:\Program Files (x86)\Python39-32',
  'C:\Program Files (x86)\Python310-32',
  'D:\envir\Python311',
  'D:\envir\Python310',
  'D:\envir\python'
)

Write-Host '=== 1. Python 目录扫描 ===' -ForegroundColor Cyan
foreach ($p in $paths) {
  if (Test-Path $p) {
    $exe = Join-Path $p 'python.exe'
    $hasExe = Test-Path $exe
    $size = 0
    try {
      $size = (Get-ChildItem $p -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    } catch {}
    $sizeMB = if ($size) { [math]::Round(($size / 1MB), 2) } else { 0 }
    $fileCount = (Get-ChildItem $p -Recurse -File -ErrorAction SilentlyContinue | Measure-Object).Count
    Write-Host ("  EXISTS  {0}" -f $p) -ForegroundColor Yellow
    Write-Host ("          python.exe={0}  fileCount={1}  size={2}MB" -f $hasExe, $fileCount, $sizeMB)
  }
}

Write-Host ''
Write-Host '=== 2. 注册表中已注册的 Python 安装 ===' -ForegroundColor Cyan
$uninstallKeys = @(
  'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
  'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
  'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
foreach ($k in $uninstallKeys) {
  Get-ItemProperty $k -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like 'Python *' -or $_.DisplayName -like '*Python*Launcher*' } |
    ForEach-Object {
      Write-Host ("  [{0}]" -f $_.DisplayName) -ForegroundColor Yellow
      Write-Host ("     InstallLocation   : {0}" -f $_.InstallLocation)
      Write-Host ("     UninstallString   : {0}" -f $_.UninstallString)
      Write-Host ("     QuietUninstall    : {0}" -f $_.QuietUninstallString)
    }
}

Write-Host ''
Write-Host '=== 3. PATH 环境变量里的 Python 相关条目 ===' -ForegroundColor Cyan
$machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
Write-Host '--- Machine PATH (系统级) ---'
($machinePath -split ';') | Where-Object { $_ -match 'python|Python' } | ForEach-Object { Write-Host ('  ' + $_) -ForegroundColor Yellow }
Write-Host '--- User PATH (用户级) ---'
($userPath -split ';') | Where-Object { $_ -match 'python|Python' } | ForEach-Object { Write-Host ('  ' + $_) -ForegroundColor Yellow }

Write-Host ''
Write-Host '=== 4. where.exe python 当前能解析到的 ==='  -ForegroundColor Cyan
& where.exe python 2>$null
& where.exe python3 2>$null
