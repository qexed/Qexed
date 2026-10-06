param(
    [Parameter(Position = 0)]
    [string]$ClassesDir = "E:/code/qexed-v6/target/mc-26.3/classes",
    [Parameter(Position = 1)]
    [string]$OutJson = "E:/code/qexed-v6/target/mc-26.3/packets-26.3.json"
)
$ErrorActionPreference = "Stop"

# 状态 -> (注册类, 方向)
# lambda$static$N 内是 addPacket 序列；每状态有 clientbound/serverbound 两组 lambda。
$targets = @(
    @{ cls = "net.minecraft.network.protocol.handshake.HandshakeProtocols"; state = "handshaking" },
    @{ cls = "net.minecraft.network.protocol.status.StatusProtocols";      state = "status" },
    @{ cls = "net.minecraft.network.protocol.login.LoginProtocols";        state = "login" },
    @{ cls = "net.minecraft.network.protocol.configuration.ConfigurationProtocols"; state = "configuration" },
    @{ cls = "net.minecraft.network.protocol.game.GameProtocols";          state = "play" }
)

$all = @()
Push-Location $ClassesDir
try {
    foreach ($t in $targets) {
        $text = javap -p -c $t.cls 2>&1 | Out-String
        # 按 lambda 方法分段，每段内按顺序找 addPacket 前最近的 PacketType 字段 + Packet 类名
        # PacketType 字段名如 CLIENTBOUND_ADD_ENTITY / SERVERBOUND_HELLO，方向从字段名前缀取。
        $matches2 = [regex]::Matches($text, "Field ([\w.]+)\.((?:CLIENTBOUND|SERVERBOUND)_[A-Z_]+):Lnet/minecraft/protocol/PacketType;")
        if ($matches2.Count -eq 0) {
            # 字段类型写法可能是 net/minecraft/network/protocol/PacketType
            $matches2 = [regex]::Matches($text, "Field ([\w.]+)\.((?:CLIENTBOUND|SERVERBOUND)_[A-Z_]+):Lnet/minecraft/network/protocol/PacketType;")
        }
        $seq = @()
        foreach ($m in $matches2) {
            $fieldName = $m.Groups[2].Value
            $direction = if ($fieldName.StartsWith("CLIENTBOUND")) { "to_client" } else { "to_server" }
            $seq += @{ direction = $direction; field = $fieldName }
        }
        # addPacket 的注册顺序 = id。同方向各自从 0 递增。
        $idByDir = @{ to_client = 0; to_server = 0 }
        foreach ($entry in $seq) {
            $id = $idByDir[$entry.direction]
            $all += [ordered]@{
                state     = $t.state
                direction = $entry.direction
                id        = $id
                field     = $entry.field
            }
            $idByDir[$entry.direction] = $id + 1
        }
        Write-Host "$($t.state): $($seq.Count) registrations"
    }
} finally {
    Pop-Location
}

$json = $all | ConvertTo-Json -Depth 4
if ($all.Count -eq 1) { $json = "[$json]" }
[System.IO.File]::WriteAllText($OutJson, $json)
Write-Host "wrote $OutJson ($($all.Count) packets)"
