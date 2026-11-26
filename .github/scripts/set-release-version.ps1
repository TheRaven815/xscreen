param(
    [Parameter(Mandatory)]
    [string]$Tag
)

$ErrorActionPreference = 'Stop'

if ($Tag -cnotmatch '^v(?<version>(?:0|[1-9][0-9]{0,4})\.(?:0|[1-9][0-9]{0,4})\.(?:0|[1-9][0-9]{0,4}))$') {
    throw "Expected a stable version tag such as v0.2.0, got '$Tag'."
}
$version = $Matches['version']
if ($version.Split('.') | Where-Object { [int]$_ -gt [uint16]::MaxValue }) {
    throw "Tag '$Tag' exceeds Windows version resource limits (65535 per component)."
}

# Stamp only this package and its lockfile entry in the disposable release checkout.
# This keeps --locked usable without resolving any new dependency versions.
$root = Join-Path $PSScriptRoot '../..'
$manifest = Join-Path $root 'Cargo.toml'
$lockfile = Join-Path $root 'Cargo.lock'
$manifestText = [IO.File]::ReadAllText($manifest)
$lockText = [IO.File]::ReadAllText($lockfile)
$manifestEntry = [regex]'(?m)(^\[package\]\r?\nname = "xscreen"\r?\nversion = ")([^"\r\n]+)("\r?$)'
$lockEntry = [regex]'(?m)(^\[\[package\]\]\r?\nname = "xscreen"\r?\nversion = ")([^"\r\n]+)("\r?$)'
$manifestMatches = $manifestEntry.Matches($manifestText)
$lockMatches = $lockEntry.Matches($lockText)
if ($manifestMatches.Count -ne 1 -or $lockMatches.Count -ne 1 -or
    $manifestMatches[0].Groups[2].Value -ne $lockMatches[0].Groups[2].Value) {
    throw 'Expected one matching xscreen version in Cargo.toml and Cargo.lock.'
}

$replacement = '${1}' + $version + '${3}'
[IO.File]::WriteAllText($manifest, $manifestEntry.Replace($manifestText, $replacement))
[IO.File]::WriteAllText($lockfile, $lockEntry.Replace($lockText, $replacement))
$version
