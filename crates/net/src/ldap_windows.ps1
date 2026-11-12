# Persistent Windows LDAP worker: one process and one signed/sealed SSPI connection
# per active MailCraft account worker, rather than spawning PowerShell on each
# recipient keystroke. Protocol: one UTF-8 JSON request per line, exactly one
# UTF-8 JSON response per line. No user string is executed as PowerShell.
$ErrorActionPreference = "Stop"
$utf8 = [System.Text.UTF8Encoding]::new($false, $true)
[Console]::InputEncoding = $utf8
[Console]::OutputEncoding = $utf8
Add-Type -AssemblyName System.DirectoryServices.Protocols
$ldap = $null
$endpoint = ""
function First-Attr($entry, [string[]]$names) {
    foreach ($key in $names) {
        if ($entry.Attributes.Contains($key)) {
            $values = $entry.Attributes[$key].GetValues([string])
            if ($values.Count -gt 0 -and $values[0]) { return [string]$values[0] }
        }
    }
    return ""
}
function Search-Gal($connection, [string]$baseDn, [string]$filter, [int]$limit) {
    $attributes = [string[]]@("cn","displayName","mail","rfc822Mailbox","proxyAddresses")
    $request = [System.DirectoryServices.Protocols.SearchRequest]::new(
        $baseDn, $filter, [System.DirectoryServices.Protocols.SearchScope]::Subtree, $attributes
    )
    $request.SizeLimit = $limit
    $request.TimeLimit = [TimeSpan]::FromSeconds(8)
    try {
        return $connection.SendRequest($request)
    } catch [System.DirectoryServices.Protocols.DirectoryOperationException] {
        if ($_.Exception.Response -and $_.Exception.Response.ResultCode -eq [System.DirectoryServices.Protocols.ResultCode]::SizeLimitExceeded) {
            return $_.Exception.Response
        }
        throw
    }
}
try {
    while ($null -ne ($line = [Console]::ReadLine())) {
        try {
            $p = $line | ConvertFrom-Json
            $server = [string]$p.host
            $baseDn = [string]$p.base_dn
            $filter = [string]$p.filter
            $port = [int]$p.port
            $limit = [int]$p.limit
            if ([string]::IsNullOrWhiteSpace($server) -or [string]::IsNullOrWhiteSpace($baseDn) -or
                $port -lt 1 -or $port -gt 65535 -or $limit -lt 1 -or $limit -gt 50 -or
                [string]::IsNullOrWhiteSpace($filter)) {
                throw "invalid LDAP search options"
            }
            $target = $server + ":" + $port
            if ($null -eq $ldap -or $target -ne $endpoint) {
                if ($null -ne $ldap) { $ldap.Dispose(); $ldap = $null }
                $identifier = [System.DirectoryServices.Protocols.LdapDirectoryIdentifier]::new($server, $port)
                $ldap = [System.DirectoryServices.Protocols.LdapConnection]::new($identifier)
                $ldap.Timeout = [TimeSpan]::FromSeconds(8)
                $ldap.AuthType = [System.DirectoryServices.Protocols.AuthType]::Negotiate
                $ldap.SessionOptions.ProtocolVersion = 3
                $ldap.SessionOptions.Signing = $true
                $ldap.SessionOptions.Sealing = $true
                $ldap.Credential = [System.Net.CredentialCache]::DefaultNetworkCredentials
                $ldap.Bind()
                $endpoint = $target
            }
            $response = Search-Gal $ldap $baseDn $filter $limit
            # One substring fallback per request only on a successful empty
            # prefix search; do not scan AD again on errors or full result sets.
            if ($response.Entries.Count -eq 0 -and $response.ResultCode -eq [System.DirectoryServices.Protocols.ResultCode]::Success -and
                -not [string]::IsNullOrWhiteSpace([string]$p.fallback_filter)) {
                $response = Search-Gal $ldap $baseDn ([string]$p.fallback_filter) $limit
            }
            $people = @()
            foreach ($entry in $response.Entries) {
                $name = First-Attr $entry @("displayName","cn")
                $email = if ([string]$p.schema -eq "exchange55") {
                    First-Attr $entry @("rfc822Mailbox","mail")
                } else { First-Attr $entry @("mail") }
                if (-not $email -and [string]$p.schema -eq "ad" -and $entry.Attributes.Contains("proxyAddresses")) {
                    $proxies = $entry.Attributes["proxyAddresses"].GetValues([string])
                    $primary = $proxies | Where-Object { $_.StartsWith("SMTP:", [StringComparison]::Ordinal) } | Select-Object -First 1
                    if (-not $primary) {
                        $primary = $proxies | Where-Object { $_.StartsWith("smtp:", [StringComparison]::Ordinal) } | Select-Object -First 1
                    }
                    if ($primary) { $email = [string]$primary.Substring(5) }
                }
                if ($email) { $people += @{name=$name; email=$email} }
            }
            [Console]::Out.WriteLine((ConvertTo-Json -InputObject @{
                people = @($people); error = $null
            } -Depth 5 -Compress))
            [Console]::Out.Flush()
        } catch {
            $category = "Unexpected"
            if ($_.Exception -is [System.DirectoryServices.Protocols.DirectoryOperationException]) {
                $category = "DirectoryOperation"
                if ($_.Exception.Response) {
                    $code = [string]$_.Exception.Response.ResultCode
                    if ($code.Length -gt 0 -and $code -notmatch '[^A-Za-z0-9_]') { $category += ":" + $code }
                }
            } elseif ($_.Exception -is [System.DirectoryServices.Protocols.LdapException]) {
                $category = "LdapTransport"
                $code = [int]$_.Exception.ErrorCode
                if ($code -ge 0 -and $code -le 200) { $category += ":" + $code }
            } elseif ($_.Exception -is [System.Management.Automation.RuntimeException]) {
                $category = "HelperRuntime"
            }
            # Drop a failed connection. Next query will bind again, without
            # silently downgrading signing or sealing.
            if ($null -ne $ldap) { $ldap.Dispose(); $ldap = $null; $endpoint = "" }
            [Console]::Out.WriteLine((ConvertTo-Json -InputObject @{
                people = @(); error = $category
            } -Depth 3 -Compress))
            [Console]::Out.Flush()
        }
    }
} finally {
    if ($null -ne $ldap) { $ldap.Dispose() }
}
