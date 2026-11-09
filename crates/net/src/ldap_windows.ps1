# Embedded, constant script called by MailCraft on Windows only.
# A JSON request arrives via standard input; no user string is ever interpolated
# into executable PowerShell. Never print LDAP names to stderr or diagnostic logs.
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.DirectoryServices.Protocols
try {
    $p = [Console]::In.ReadToEnd() | ConvertFrom-Json
    $hostName = [string]$p.host
    $baseDn = [string]$p.base_dn
    $filter = [string]$p.filter
    $port = [int]$p.port
    $limit = [int]$p.limit
    if ([string]::IsNullOrWhiteSpace($hostName) -or [string]::IsNullOrWhiteSpace($baseDn) -or
        $port -lt 1 -or $port -gt 65535 -or $limit -lt 1 -or $limit -gt 50) {
        throw "invalid LDAP search options"
    }
    # Windows SSPI uses the currently logged-in Windows identity. No password,
    # credential prompt, LDAP simple bind or anonymous fallback is permitted.
    $ident = [System.DirectoryServices.Protocols.LdapDirectoryIdentifier]::new($hostName,$port)
    $ldap = [System.DirectoryServices.Protocols.LdapConnection]::new($ident)
    try {
        $ldap.Timeout = [TimeSpan]::FromSeconds(8)
        $ldap.AuthType = [System.DirectoryServices.Protocols.AuthType]::Negotiate
        $ldap.SessionOptions.ProtocolVersion = 3
        $ldap.SessionOptions.Signing = $true
        $ldap.SessionOptions.Sealing = $true
        $ldap.Credential = [System.Net.CredentialCache]::DefaultNetworkCredentials
        $ldap.Bind()
        $attrs = [string[]]@("cn","displayName","mail","rfc822Mailbox")
        $req = [System.DirectoryServices.Protocols.SearchRequest]::new(
            $baseDn,$filter,[System.DirectoryServices.Protocols.SearchScope]::Subtree,$attrs
        )
        $req.SizeLimit = $limit
        $req.TimeLimit = [TimeSpan]::FromSeconds(8)
        $response = $null
        try {
            $response = $ldap.SendRequest($req)
        } catch [System.DirectoryServices.Protocols.DirectoryOperationException] {
            # Retain the bounded result prefix when the LDAP server imposes a limit.
            if ($_.Exception.Response -and $_.Exception.Response.ResultCode -eq [System.DirectoryServices.Protocols.ResultCode]::SizeLimitExceeded) {
                $response = $_.Exception.Response
            } else { throw }
        }
        if ($null -eq $response) { throw "LDAP search returned no response" }
        $people = @()
        foreach ($entry in $response.Entries) {
            $name = ""
            foreach ($key in @("displayName","cn")) {
                if ($entry.Attributes.Contains($key)) {
                    $vals = $entry.Attributes[$key].GetValues([string])
                    if ($vals.Count -gt 0 -and $vals[0]) { $name = [string]$vals[0]; break }
                }
            }
            $email = ""
            $mailAttributes = if ([string]$p.schema -eq "exchange55") { @("rfc822Mailbox","mail") } else { @("mail") }
            foreach ($key in $mailAttributes) {
                if ($entry.Attributes.Contains($key)) {
                    $vals = $entry.Attributes[$key].GetValues([string])
                    if ($vals.Count -gt 0 -and $vals[0]) { $email = [string]$vals[0]; break }
                }
            }
            if ($email) { $people += @{name=$name; email=$email} }
        }
        # -Compress and explicit array avoid single-item shape changes.
        [Console]::Out.WriteLine((ConvertTo-Json -InputObject @($people) -Depth 4 -Compress))
    } finally {
        if ($null -ne $ldap) { $ldap.Dispose() }
    }
} catch {
    # Never echo server records, user-controlled query strings or credentials.
    [Console]::Error.WriteLine("Windows integrated LDAP search failed; check domain sign-in, directory permissions and signing/sealing support.")
    exit 2
}
