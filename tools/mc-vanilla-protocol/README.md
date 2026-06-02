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

The client run uses `--quickPlayMultiplayer 127.0.0.1:25565`.
