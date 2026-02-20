param(
  [string]$Version = "latest",
  [string]$InstallDir = "$env:LOCALAPPDATA\Programs\otto",
  [string]$Repo = "mcmanussliam/otto"
)

$ErrorActionPreference = "Stop"

function Get-LatestTag {
  param([string]$Repository)
  $api = "https://api.github.com/repos/$Repository/releases/latest"
  $release = Invoke-RestMethod -Uri $api
  if (-not $release.tag_name) {
    throw "Failed to determine latest release tag from GitHub API."
  }
  return $release.tag_name
}

$arch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
  "X64" { "amd64" }
  "Arm64" { "arm64" }
  default { throw "Unsupported architecture: $([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)" }
}

if ($Version -eq "latest") {
  $Version = Get-LatestTag -Repository $Repo
}

$asset = "otto_${Version}_windows_${arch}.zip"
$url = "https://github.com/$Repo/releases/download/$Version/$asset"

$tmp = Join-Path $env:TEMP ("otto-install-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
  $zipPath = Join-Path $tmp $asset
  Write-Host "Downloading $url"
  Invoke-WebRequest -Uri $url -OutFile $zipPath

  Expand-Archive -LiteralPath $zipPath -DestinationPath $tmp -Force

  $binInArchive = "otto_${Version}_windows_${arch}.exe"
  $sourceExe = Join-Path $tmp $binInArchive
  if (-not (Test-Path $sourceExe)) {
    throw "Archive did not contain expected binary: $binInArchive"
  }

  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  $targetExe = Join-Path $InstallDir "otto.exe"
  Copy-Item -LiteralPath $sourceExe -Destination $targetExe -Force

  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  $hasPath = $false
  foreach ($entry in ($userPath -split ';')) {
    if ($entry.TrimEnd('\') -ieq $InstallDir.TrimEnd('\')) {
      $hasPath = $true
      break
    }
  }

  if (-not $hasPath) {
    $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $InstallDir } else { "$userPath;$InstallDir" }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "Added $InstallDir to user PATH. Open a new terminal to use 'otto'."
  }

  Write-Host "Installed otto $Version to $targetExe"
  Write-Host "Run: otto version"
}
finally {
  if (Test-Path $tmp) {
    Remove-Item -LiteralPath $tmp -Recurse -Force
  }
}

