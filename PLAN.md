# QUIVERFALL — game plan

A Hollow-Knight-flavoured 2D platformer. You are **Wren**, a hooded archer with a pale
mask, descending through a ruined underground kingdom. Each level is a gauntlet of
platforming, hazards and enemies that ends at an **Exit Gate**. You have no wall-jump
and no double jump — **your arrows are your climbing gear**.

## Core feel

| Thing | Value | Notes |
|---|---|---|
| Tile | 32 px | levels are ASCII grids (`src/levels/`) |
| Run | 270 px/s | instant-ish accel, low air control loss |
| Jump | ~3.8 tiles high, ~5.5 tiles long | variable height, coyote time 0.1 s, jump buffer 0.12 s |
| Dash | ~3 tiles, 1 per airtime | refreshes on ground / pegs / lifts |
| Bow | hold to draw, release to fire | full draw = fast + flat + 2 dmg; tap = weak lob |
| Air draw | drawing in mid-air slows your fall once per airtime | the "archer hang" |
| Melee | bow-swipe, down-swipe pogoes off enemies/spikes | for close calls |
| Health | 5 masks | spikes = 1 dmg + snap back to last safe ground |
| Charges | 3 quiver charges | special arrows cost 1; regen 1 / 2.5 s, enemy hits refill faster — never soft-locks |

## The arrows

| # | Arrow | What it does | Puzzle use |
|---|---|---|---|
| 1 | **Steel** (free) | Damage. Sticks into **wood** and becomes a **peg** you can stand on (max 3, oldest pops). | climb wooden walls by shooting a staircase of pegs |
| 2 | **Lift** (elevator) | Plants a glowing disc where it lands. Floor hit → a platform rises from the floor up to 7 tiles (stops at ceilings), holds, then sinks. Wall hit → a ledge sprouts and rides up the wall. | vertical shafts, carrying you past spikes |
| 3 | **Tether** | Hits an **anchor ring** or **wood** → you zip to it and hang there until you jump or fire the next one. | crossing spike pits ring to ring |
| 4 | **Ember** | Burns **thorn walls** (fire spreads through connected thorns), lights **braziers** that open gates, sets enemies on fire. | burn paths open, brazier gate puzzles |
| 5 | **Frost** | Freezes the surface of **water** into walkable ice; freezes enemies into **ice blocks** you can stand on. | water crossings, enemy stepping stones |
| 6 | **Blink** | Teleports you to where it lands. Hit an enemy → swap places. Flies through **grates** that block you. | getting past grates, escaping, reaching sealed rooms |

Future arrows (not in v1): Burst (breaks cracked walls, rocket-jump), Gust (updraft
column), Echo (ricochet that hits switches around corners), Vine (rope you climb).

## Obstacles

Spikes (floor/ceiling) · deadly black water · thorn walls · crumbling platforms ·
moving platforms · grates · targets (shoot to open linked gates) · braziers (Ember to open) ·
wooden walls (peg surfaces) · anchor rings.

## Enemies

| Enemy | Behaviour | Counter |
|---|---|---|
| **Crawler** | walks, turns at ledges | anything |
| **Hopper** | leaps toward you | shoot mid-air, pogo |
| **Moth** | floats in a sine wave, dives at you | full-draw shots |
| **Spitter** | stationary, lobs acid globs | Blink-swap, shoot from cover |
| **Shieldbearer** | blocks arrows from the front | hit from behind/above, Ember, Blink-swap |
| **The Warden** (boss) | guards the final Exit — leaps, slams, spawns moths | uses everything |

## Levels

1. **The Hollow Gate** — tutorial: run/jump/dash, Steel pegs on wood, targets & gates, crawlers.
2. **Lift Shafts** — Lift Arrow shrine; tall shafts, moths, crumbling ledges.
3. **Rootdeep** — Tether + Ember; anchor rings over spike pits, thorn walls, braziers.
4. **Frozen Cistern** — Frost + Blink; black-water pools, grates, shieldbearers.
5. **The Warden's Ascent** — everything combined, the Warden, then the final **Exit**.

Arrows are unlocked at **Arrow Shrines** inside levels and kept for later levels.
Checkpoints are **lanterns** — touching one heals you and sets the respawn.

## Controls

| Action | Keyboard + mouse | Gamepad |
|---|---|---|
| Move | A/D or ←/→ | left stick / d-pad |
| Jump | Space / W / ↑ | A |
| Dash | Shift / K | RB / B |
| Draw & fire | hold Left Mouse / J | hold RT / X |
| Aim | mouse (or WASD/arrows while drawing) | left/right stick |
| Melee | Right Mouse / F | Y |
| Switch arrow | 1–6, Q/E, mouse wheel | LB / d-pad up |
| Pause | Esc | Start |

## Art & assets

All art is generated with GPT-5.6 Sol via the Cursor agent CLI (`tools/gen_assets.py`,
prompts in `tools/assets.json`) on flat magenta backgrounds. `tools/assetproc` keys,
slices and normalises it into `assets/art/`. Characters come from one sprite sheet per
character, because a single generation stays consistent while separate generations drift.
The bow, bowstring, glows, rim lighting and moss are procedural.

## Tech

Rust + Bevy 0.20 (2D, UI, audio). Custom kinematic AABB-vs-tile physics, ASCII levels
embedded at compile time, HDR bloom, synthesised WAV sound effects. Debug env vars
(`QF_LEVEL`, `QF_SCRIPT`, `QF_SHOT`, ...) are listed in the README.
