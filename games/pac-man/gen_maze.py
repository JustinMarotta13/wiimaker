#!/usr/bin/env python3
"""Generate the original 640x480 arcade-style Pac-Man maze scene.

Note: scenes/main.scene.json has been edited by hand / CLI since (Animator, prefabs,
visibility, ...). Re-running this overwrites those edits.
"""
import json
from pathlib import Path

CELL = 20.0
COLS, ROWS = 32, 24
CLEAR = [0, 0, 0, 255]
WALL_BLUE = [33, 33, 222, 255]
PAC_YELLOW = [255, 255, 0, 255]
GHOSTS = {
    "B": ("GhostBlinky", [255, 0, 0, 255]),
    "K": ("GhostPinky", [255, 184, 222, 255]),
    "I": ("GhostInky", [0, 255, 255, 255]),
    "C": ("GhostClyde", [255, 184, 81, 255]),
}
PELLET = [255, 183, 174, 255]
TAG_WALL, TAG_DOT, TAG_POWER, TAG_GHOST = 1, 2, 3, 4

# 32×24. # wall  . pellet  o power  space empty  P player  B/K/I/C ghosts
# Every row is length-checked.
MAZE = [
    "################################",
    "#o.............##............o.#",
    "#.####.#######.##.#######.####.#",
    "#.####.#######.##.#######.####.#",
    "#..............................#",
    "#.####.##.############.##.####.#",
    "#.####.##.############.##.####.#",
    "#......##......##......##......#",
    "######.#######.##.#######.######",
    "#......##..............##......#",
    "#.####.##.#####  #####.##.####.#",
    "#.####.##.#          #.##.####.#",
    "#......##.#  BKIC    #.##......#",
    "#.####.##.#          #.##.####.#",
    "#.####.##.############.##.####.#",
    "#......##..............##......#",
    "######.#######.##.#######.######",
    "#......##......##......##......#",
    "#.####.##.############.##.####.#",
    "#.####.##.############.##.####.#",
    "#o..##................##......o#",
    "###.##.#######.##.#######.##.###",
    "#P.............................#",
    "################################",
]


def check():
    assert len(MAZE) == ROWS, len(MAZE)
    for i, r in enumerate(MAZE):
        assert len(r) == COLS, (i, len(r), r)


def cell_center(c, r):
    return [c * CELL + CELL * 0.5, r * CELL + CELL * 0.5, 0.0]


def identity_xf(xyz):
    return {
        "translation": xyz,
        "rotation": [0.0, 0.0, 0.0, 1.0],
        "scale": [1.0, 1.0, 1.0],
    }


def disc_ent(name, xyz, radius, color, tag, z=1.0, parent=None):
    ent = {
        "name": name,
        "transform": identity_xf(xyz),
        "components": {"Disc": {"radius": radius, "color": color, "z": z}},
        "tag": tag,
    }
    if parent:
        ent["parent"] = parent
    return ent


def empty_go(name, xyz=(0.0, 0.0, 0.0)):
    """Unity Create Empty — Transform only."""
    return {
        "name": name,
        "transform": identity_xf(list(xyz) if len(xyz) == 3 else [xyz[0], xyz[1], 0.0]),
        "components": {},
        "tag": 0,
    }


def flood(walk, start):
    from collections import deque
    seen = {start}
    q = deque([start])
    while q:
        c, r = q.popleft()
        for dc, dr in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            n = (c + dc, r + dr)
            if n in walk and n not in seen:
                seen.add(n)
                q.append(n)
    return seen


def main():
    check()
    cells = []
    solid = []
    walk = set()
    pellets = []
    powers = []
    player = None
    ghosts = []
    for r, row in enumerate(MAZE):
        for c, ch in enumerate(row):
            if ch == "#":
                cells.append(1)
                solid.append(1)
            else:
                cells.append(0)
                solid.append(0)
                walk.add((c, r))
                if ch == ".":
                    pellets.append((c, r))
                elif ch == "o":
                    powers.append((c, r))
                elif ch == "P":
                    player = (c, r)
                elif ch in GHOSTS:
                    ghosts.append((ch, c, r))
    assert player, "no P"
    assert len(ghosts) == 4, ghosts
    reach = flood(walk, player)
    unreachable = walk - reach
    if unreachable:
        print("UNREACHABLE", sorted(unreachable)[:20], "count", len(unreachable))
        raise SystemExit("maze not connected from P")
    print(f"walkable {len(walk)}  pellets {len(pellets)}  power {len(powers)}  reach ok")

    # Maze at root. Dots is a Unity empty (Transform only); pellets stay in
    # world space because the parent sits at the origin.
    entities = [
        {
            "name": "Maze",
            "transform": identity_xf([0.0, 0.0, 0.0]),
            "components": {
                "Tilemap": {
                    "cell": CELL,
                    "origin": [0.0, 0.0],
                    "width": COLS,
                    "height": ROWS,
                    "cells": cells,
                    "solid": solid,
                    "palette": [{"id": 1, "color": WALL_BLUE}],
                    "z": -1.0,
                }
            },
            "tag": 0,
        },
        empty_go("Dots"),
    ]
    for i, (c, r) in enumerate(pellets):
        entities.append(
            disc_ent(f"Dot{i}", cell_center(c, r), 2.5, PELLET, TAG_DOT, z=0.5, parent="Dots")
        )
    for i, (c, r) in enumerate(powers):
        entities.append(
            disc_ent(f"Power{i}", cell_center(c, r), 6.0, PELLET, TAG_POWER, z=0.5, parent="Dots")
        )
    px, py, pz = cell_center(*player)
    entities.append(disc_ent("Player", [px, py, pz], 8.0, PAC_YELLOW, 0, z=2.0))
    for ch, c, r in ghosts:
        name, color = GHOSTS[ch]
        entities.append(disc_ent(name, cell_center(c, r), 8.0, color, TAG_GHOST, z=2.0))
    entities.append(
        {
            "name": "MainCamera",
            "transform": identity_xf([320.0, 240.0, 0.0]),
            "components": {"Camera": {"active": True}},
            "tag": 0,
        }
    )
    scene = {"name": "main", "clear_color": CLEAR, "entities": entities}
    out = Path(__file__).resolve().parent / "scenes" / "main.scene.json"
    out.write_text(json.dumps(scene, indent=2) + "\n")
    print("wrote", out, "entities", len(entities))


if __name__ == "__main__":
    main()
