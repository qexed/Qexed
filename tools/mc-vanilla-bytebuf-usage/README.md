# Qexed MC Vanilla ByteBuf Usage Extractor

A small standalone Gradle project that extracts **every method definition and
every call site / method reference** involving `net.minecraft.network.FriendlyByteBuf`
(and its subtypes, e.g. `RegistryFriendlyByteBuf`) from the vanilla Minecraft jar.

Like its sibling project `tools/mc-vanilla-protocol`, it is intentionally separate
from the Rust workspace and uses Sponge VanillaGradle, so packet-format assumptions
are checked against real Mojang code.

## How it works

1. VanillaGradle resolves the Minecraft jar as a normal Gradle dependency
   (`minecraft` configuration, `net.minecraft:joined:<version>`). The tool never
   downloads or looks up any jar itself; it only receives `--jar <path>` arguments
   from the Gradle task.
2. Pass 1 indexes every class in every resolved jar: supertype edges + declared
   method keys (name + descriptor) with access flags.
3. Pass 2 scans all method bodies for `invokevirtual/special/static/interface` and
   `invokedynamic` instructions (including bootstrap method handles and handle
   arguments), resolving each invocation through the class hierarchy to the class
   that actually declares the method.
4. A call counts only when the resolved declaration lands in the receiver universe
   (the target type + all its subtypes). So `output.writeVarInt(id)` on a
   `RegistryFriendlyByteBuf` parameter is correctly attributed to
   `FriendlyByteBuf#writeVarInt`, matching how the user example in the task
   description writes the `ClientboundAddEntityPacket` fields.

## Usage

```powershell
cd tools/mc-vanilla-bytebuf-usage
gradle extractFriendlyByteBufUsage
# -> build/bytebuf-usage/bytebuf-usage.json   full report
# -> build/bytebuf-usage/bytebuf-usage.csv    flat call-site rows
# -> build/bytebuf-usage/bytebuf-usage.md     per-method counts
```

Default Minecraft version is 26.3. To target another version:

```powershell
gradle extractFriendlyByteBufUsage -PmcVersion=26.1.2
```

Any other type can be analyzed the same way:

```powershell
gradle extractFriendlyByteBufUsage -PbytebufTarget=net.minecraft.network.RegistryFriendlyByteBuf
```

## Report shape

```jsonc
{
  "mcVersion": "26.3",
  "targetType": "net.minecraft.network.FriendlyByteBuf",
  "receiverUniverse": ["net/minecraft/network/FriendlyByteBuf", "net/minecraft/network/RegistryFriendlyByteBuf"],
  "methods": {
    "net.minecraft.network.FriendlyByteBuf#writeVarInt(I)Lnet/minecraft/network/FriendlyByteBuf;": {
      "owner": "net.minecraft.network.FriendlyByteBuf",
      "name": "writeVarInt",
      "signature": "(int) net.minecraft.network.FriendlyByteBuf",
      "modifiers": "public",
      "callSites": [
        {
          "callerClass": "net.minecraft.network.protocol.game.ClientboundAddEntityPacket",
          "callerMethod": "write(net.minecraft.network.RegistryFriendlyByteBuf) void",
          "line": 110,
          "kind": "INVOKEVIRTUAL",
          "invokedOwner": "net.minecraft.network.RegistryFriendlyByteBuf",
          "invokedName": "writeVarInt"
        }
      ]
    }
  }
  "totalDistinctMethods": 362,
  "totalCallSites": 953
}
```

`declaredHere: false` on a method entry means the invocation resolved into the
universe although the declaration was not found in the indexed jars (should not
happen with the joined jar).

## Tests

```powershell
gradle test
```

The tests are self-contained: they generate synthetic bytecode fixtures with ASM
(base class declares `writeVarInt`, subtype inherits it, a packet class calls it
through the subtype) and assert the attribution, the invokedynamic handle capture,
and the dotted-name output invariants — no Minecraft download needed.

## Version upgrades

The extractor holds no Minecraft-version-specific logic. To move to a newer
version, change the default in `build.gradle` (or pass `-PmcVersion=...`);
VanillaGradle resolves the new jar and the same analysis runs unchanged.