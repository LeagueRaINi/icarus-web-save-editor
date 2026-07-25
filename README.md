# Icarus Save Editor

A browser-based save editor for the game [ICARUS](https://store.steampowered.com/app/1149460/ICARUS/).
Everything runs client-side (Rust compiled to WebAssembly via [Leptos](https://leptos.dev)) —
your save files never leave your machine.

**⚠ Always back up your original save files before overwriting them with an
edited copy, and close the game before replacing files.**

## Features

- **Characters.json**: edit name, XP, death/abandoned state, talents and
  blueprints with dependency-aware unlocking — unlocking an item offers to
  unlock its prerequisite chain, locking one cascades to its dependents, and
  the `UnlockedFlags` granted by talents are kept in sync automatically.
- **Profile.json**: edit account currencies (clamped to the game's practical
  maximum). Orbital workshop research is out of scope for now.
- Talent cards show per-rank stat effects; ranks cycle by clicking the card.
- Level estimation from XP with legitimacy checks: talent, blueprint, and
  solo point pools are compared against what the character's level can
  legitimately earn (with an allowance for mission-reward talent points).
- Round-trip fidelity: output is **byte-identical** to what the game itself
  writes (tab indentation, CRLF/LF conventions, inline numeric arrays),
  verified by tests against real save files. Fields the editor doesn't model
  are preserved untouched.

Save files live at `%LOCALAPPDATA%\Icarus\Saved\PlayerData\<steam-id>\`.

## Building

**Prerequisite — generate the game data first.** `web/assets/` (talent and
blueprint definitions, icons) is extracted from the game's pak files and is
not committed to this repository. You need ICARUS installed, then:

```bash
cargo run --release -p data-builder -- "C:\Program Files (x86)\Steam\steamapps\common\Icarus"
```

This writes `web/assets/data/talents.json` and the icon PNGs (downscaled to
96 px automatically). Without this step the `web` crate does not compile —
it embeds `talents.json` at build time. Re-run it after game updates.

Then build the web app with the Rust `wasm32-unknown-unknown` target and
[trunk](https://trunkrs.dev):

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
```

```bash
cd web
trunk serve        # dev server at http://localhost:8080
trunk build --release   # static site in web/dist
```

## Tests

The round-trip tests verify byte-identical serialization against real saves.
Point them at copies of your own files (never committed):

```bash
ICARUS_CHARACTERS=path/to/Characters.json ICARUS_PROFILE=path/to/Profile.json cargo test -p shared
```

Without the env vars those tests skip. The level-table unit tests run with
`cargo test -p web`.

## Disclaimer

A fan-made tool. Not affiliated with or endorsed by RocketWerkz or the game
ICARUS. Use at your own risk — back up your saves.
