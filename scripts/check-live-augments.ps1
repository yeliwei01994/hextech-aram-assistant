$ErrorActionPreference = 'Stop'
$base = 'https://127.0.0.1:2999'

function Read-LocalJson([string]$path) {
    $json = & curl.exe --noproxy '*' --insecure --fail --silent --max-time 4 "$base$path"
    if ($LASTEXITCODE -ne 0) { throw "Local game endpoint unavailable (curl exit $LASTEXITCODE)." }
    ($json -join "`n") | ConvertFrom-Json
}

function Find-AugmentFields($node, [string]$path) {
    if ($null -eq $node) { return }
    if ($node -is [array]) {
        foreach ($entry in $node) { Find-AugmentFields $entry "$path[]" }
    } elseif ($node -is [pscustomobject]) {
        foreach ($property in $node.PSObject.Properties) {
            # Exclude arbitrary object keys from output, including player names.
            if ($property.Name -notmatch '^[a-zA-Z][a-zA-Z0-9_]*$') { continue }
            $childPath = "$path.$($property.Name)"
            if ($property.Name -match 'augment') {
                $value = $property.Value
                $shape = if ($null -eq $value) { 'null' }
                    elseif ($value -is [array]) {
                        $ids = @($value | ForEach-Object {
                            if ($_ -is [int] -or $_ -is [long]) { $_ }
                            elseif ($_.augmentId -is [int] -or $_.augmentId -is [long]) { $_.augmentId }
                            elseif ($_.id -is [int] -or $_.id -is [long]) { $_.id }
                        })
                        "array count=$($value.Count); numeric IDs=$($ids -join ',')"
                    } elseif ($value -is [int] -or $value -is [long]) { "integer=$value" }
                    else { $value.GetType().Name }
                "$childPath ($shape)"
            }
            Find-AugmentFields $property.Value $childPath
        }
    }
}

try {
    $data = Read-LocalJson '/liveclientdata/allgamedata'
} catch {
    Write-Output $_.Exception.Message
    Write-Output 'Run this during an active ARAM Mayhem match, after picking an augment.'
    exit 1
}

$self = $null
foreach ($key in 'riotId', 'summonerName') {
    $identity = $data.activePlayer.$key
    if (-not $identity) { continue }
    $matches = @($data.allPlayers | Where-Object { $_.$key -ceq $identity })
    if ($matches.Count -eq 1) { $self = $matches[0]; break }
}

$fields = @(Find-AugmentFields $data.activePlayer 'activePlayer')
if ($null -ne $self) { $fields += @(Find-AugmentFields $self 'localPlayer') }
else { Write-Output 'Could not match the local player in allPlayers.' }
if ($fields.Count) { $fields | Sort-Object -Unique }
else { Write-Output 'No augment-named fields in the local player payload.' }

$eventNames = @($data.events.Events | ForEach-Object { $_.EventName } |
    Where-Object { $_ -cmatch '^[a-zA-Z][a-zA-Z0-9_]*$' -and $_ -match 'augment' } | Sort-Object -Unique)
if ($eventNames.Count) { Write-Output ('Augment event types: ' + ($eventNames -join ', ')) }
else { Write-Output 'No augment-named event types in this sample.' }
