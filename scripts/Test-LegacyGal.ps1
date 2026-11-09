# Read-only LDAP GAL smoke test. Windows PowerShell 5.1+; no password prompts.
# Run from domain-joined Windows client under ordinary employee account.
# powershell -NoProfile -File .\scripts\Test-LegacyGal.ps1 -Server <internal-host> -BaseDn <base-dn> -Query "Alice"
[CmdletBinding()]
param(
  [string]$Server = "directory.example.com",
  [int]$Port = 389,
  [string]$BaseDn = "dc=example,dc=com",
  [string]$Query = "Alice",
  [int]$MaxResults = 5
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.DirectoryServices.Protocols
if ($Port -lt 1 -or $Port -gt 65535 -or $MaxResults -lt 1 -or $MaxResults -gt 20) { throw "invalid port/result limit" }
function New-Connection([bool]$integrated) {
  $ident = [System.DirectoryServices.Protocols.LdapDirectoryIdentifier]::new($Server,$Port)
  $c = [System.DirectoryServices.Protocols.LdapConnection]::new($ident)
  $c.Timeout = [TimeSpan]::FromSeconds(8)
  $c.SessionOptions.ProtocolVersion = 3
  if ($integrated) {
    $c.AuthType = [System.DirectoryServices.Protocols.AuthType]::Negotiate
    $c.Credential = [System.Net.CredentialCache]::DefaultNetworkCredentials
    $c.SessionOptions.Signing = $true
    $c.SessionOptions.Sealing = $true
  } else {
    $c.AuthType = [System.DirectoryServices.Protocols.AuthType]::Anonymous
  }
  return $c
}
function Escape-Filter([string]$s) {
  return $s.Replace('\', '\5c').Replace('*', '\2a').Replace('(', '\28').Replace(')', '\29').Replace([string][char]0, '\00')
}
function First-Attr($entry,[string]$name) {
  if (-not $entry.Attributes.Contains($name)) { return "" }
  $values = $entry.Attributes[$name].GetValues([string])
  if ($values.Count -eq 0) { return "" }
  return [string]$values[0]
}
$escaped = Escape-Filter $Query.Trim()
$filter = "(&(|(mail=*)(proxyAddresses=SMTP:*))(!(msExchHideFromAddressLists=TRUE))(|(objectCategory=person)(objectCategory=group))(|(cn=*$escaped*)(displayName=*$escaped*)(mail=*$escaped*)(mailNickname=*$escaped*)(sAMAccountName=*$escaped*)(givenName=*$escaped*)(sn=*$escaped*)(proxyAddresses=*$escaped*)))"
Write-Host ("Server: {0}:{1} Base: {2}" -f $Server,$Port,$BaseDn)
Write-Host "Using no password for anonymous mode and current Windows sign-in for Negotiate."
Write-Host "Run only on an isolated/trusted LAN; anonymous LDAP queries are not encrypted."
foreach ($mode in @("Anonymous", "WindowsIntegrated")) {
  $c = $null
  try {
    $c = New-Connection ($mode -eq "WindowsIntegrated")
    $c.Bind()
    $req = [System.DirectoryServices.Protocols.SearchRequest]::new($BaseDn,$filter,[System.DirectoryServices.Protocols.SearchScope]::Subtree,[string[]]@("displayName","cn","mail"))
    $req.SizeLimit = $MaxResults
    $req.TimeLimit = [TimeSpan]::FromSeconds(8)
    $response = $c.SendRequest($req)
    Write-Host ("{0}: result={1}, count={2}" -f $mode,$response.ResultCode,$response.Entries.Count)
    foreach ($entry in $response.Entries) {
      $name = First-Attr $entry "displayName"
      if (-not $name) { $name = First-Attr $entry "cn" }
      $mail = First-Attr $entry "mail"
      Write-Host ("  {0} <{1}>" -f $name,$mail)
    }
  } catch [System.DirectoryServices.Protocols.DirectoryOperationException] {
    Write-Warning ("{0}: LDAP rejected query: {1}" -f $mode,$_.Exception.Response.ResultCode)
  } catch {
    Write-Warning ("{0}: {1}" -f $mode,$_.Exception.Message)
  } finally {
    if ($c -ne $null) { $c.Dispose() }
  }
}
Write-Host "Share ONLY the mode, result code and returned count; redact names/emails."
