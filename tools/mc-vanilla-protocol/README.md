# Qexed Minecraft Vanilla Protocol Probe

This small Gradle project is intentionally separate from the Rust workspace.
It uses Sponge VanillaGradle to compile against Minecraft 26.1.2 sources and
keeps packet-format assumptions checked against Mojang code.

Run from this directory with a local Gradle installation:

```powershell
gradle test
```

Useful runs:

```powershell
gradle runServer
gradle runClient
```

Authoritative protocol reports (packet IDs, registries, commands) are generated
with Mojang's own data generator — no wiki lookup:

```powershell
gradle generateVanillaReports
# -> build/vanilla-reports/reports/packets.json      (phase/dir -> name -> protocol_id)
#    build/vanilla-reports/reports/registries.json
```

Field-level packet structures (name, wire type, order) are extracted from the
STREAM_CODEC / write() bytecode of every packet class in the joined jar:

```powershell
gradle extractVanillaPacketStructs
# -> build/vanilla-reports/packet-structures.json
#    (also runs generateVanillaReports first)
```

Join the two files on the packet ResourceLocation
(e.g. `clientbound/minecraft:login`) to get id + fields together.

The client run uses `--quickPlayMultiplayer 127.0.0.1:25565`.
