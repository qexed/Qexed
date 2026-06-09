# Creature Game

`creature_game` implements the 浅屿闲游 creature minigame.

- Server display name: 浅屿闲游
- QQ group: 722632621
- Command: `/creature menu | /creature start | /creature lobby`
- Each player gets a deterministic private arena instance.
- The menu should bind two command actions: `/creature start` and `/creature lobby`.

Use `menu-example.toml` as the Qexed server menu snippet. The plugin cannot register server
menus by itself; it opens the configured `creature_game` menu id.

Returning to lobby uses `PlayerAction::ProxyConnect` with `lobby_server = "lobby_1"` by default.
If your Velocity backend name changes, update `lobby_server` in `config.toml`.
