"""Unit tests for native Linux virtual desktop icons (add, shuffle, clear, merge, CLI)."""

from __future__ import annotations

import random

from deskland_linux.ctl import build_command_payload
from deskland_linux.virtual_icons import (
    DEFAULT_ICON_H,
    DEFAULT_ICON_W,
    add_virtual_icon,
    add_virtual_icons_batch,
    clear_virtual_icons,
    infer_icon_type,
    merge_host_and_virtual_icons,
    rename_virtual_icon,
    shuffle_virtual_icons,
    virtual_icons_to_physical,
)


def test_add_shuffle_clear_virtual_icons_stay_in_safe_bounds():
    rng = random.Random(42)
    safe = (36, 16, 48, 16)
    icons = add_virtual_icons_batch([], 6, 1920, 1080, safe_insets=safe, rng=rng)
    assert len(icons) == 6
    names = [ic["name"] for ic in icons]
    assert len(set(names)) == 6
    for ic in icons:
        assert ic["x"] >= 16
        assert ic["y"] >= 36
        assert ic["x"] + ic["w"] <= 1920 - 16
        assert ic["y"] + ic["h"] <= 1080 - 48
        assert ic["w"] == DEFAULT_ICON_W
        assert ic["h"] == DEFAULT_ICON_H

    # Adding more icons beyond preset length generates unique numbered names
    more = add_virtual_icons_batch(icons, 8, 1920, 1080, safe_insets=safe, rng=rng)
    assert len(more) == 14
    assert len({ic["name"] for ic in more}) == 14

    # Shuffle preserves names/types and repositions within bounds
    before_coords = [(ic["x"], ic["y"]) for ic in icons]
    shuffled = shuffle_virtual_icons(icons, 1920, 1080, safe_insets=safe, rng=random.Random(99))
    assert [ic["name"] for ic in shuffled] == names
    assert [(ic["x"], ic["y"]) for ic in shuffled] != before_coords
    for ic in shuffled:
        assert ic["x"] >= 16 and ic["y"] >= 36

    # Clear empties the list
    assert clear_virtual_icons() == []


def test_merge_host_and_virtual_icons_scales_to_physical_and_respects_mode():
    v_icons = add_virtual_icon([], 1280, 720, rng=random.Random(7))
    rects, labels = virtual_icons_to_physical(v_icons, scale=2.0, mode="auto")
    assert len(rects) == 1
    assert labels == [v_icons[0]["name"]]
    assert rects[0] == [
        v_icons[0]["x"] * 2,
        v_icons[0]["y"] * 2,
        (v_icons[0]["x"] + v_icons[0]["w"]) * 2,
        (v_icons[0]["y"] + v_icons[0]["h"]) * 2,
    ]

    # Mode 'off' suppresses virtual icons completely
    assert virtual_icons_to_physical(v_icons, scale=2.0, mode="off") == ([], [])

    # Iconless desktop (real_vis=False) with virtual icons -> vis=True with virtual rects
    vis, merged, m_labels = merge_host_and_virtual_icons(
        False, [], [], v_icons, scale=1.0, mode="auto"
    )
    assert vis is True
    assert len(merged) == 1
    assert m_labels == [v_icons[0]["name"]]

    # Iconless desktop with no virtual icons -> vis=False, []
    vis0, merged0, _ = merge_host_and_virtual_icons(
        False, [], [], [], scale=1.0, mode="auto"
    )
    assert vis0 is False and merged0 == []

    # Desktop with real icons + virtual icons -> concatenated cleanly
    vis2, merged2, _ = merge_host_and_virtual_icons(
        True, [[10, 10, 86, 96]], ["real.txt"], v_icons, scale=1.0, mode="always"
    )
    assert vis2 is True and len(merged2) == 2


def test_ctl_parses_vicons_subcommands():
    p1, _, _ = build_command_payload(["vicons", "add"])
    assert p1 == {"cmd": "vicons", "action": "add"}

    p2, _, _ = build_command_payload(["vicons", "add", "5"])
    assert p2 == {"cmd": "vicons", "action": "add", "count": 5}

    p3, _, _ = build_command_payload(["vicons", "shuffle"])
    assert p3 == {"cmd": "vicons", "action": "shuffle"}

    p4, _, _ = build_command_payload(["vicons", "clear"])
    assert p4 == {"cmd": "vicons", "action": "clear"}

    p5, _, _ = build_command_payload(["vicons", "mode", "off"])
    assert p5 == {"cmd": "vicons", "action": "mode", "mode": "off"}

    p6, _, _ = build_command_payload(["vicons", "rename", "0", "Berlin_Karte.pdf"])
    assert p6 == {"cmd": "vicons", "action": "rename", "index": 0, "name": "Berlin_Karte.pdf"}

    p7, _, _ = build_command_payload(["vicons", "rename", "2", "Ankara", "Cephesi"])
    assert p7 == {"cmd": "vicons", "action": "rename", "index": 2, "name": "Ankara Cephesi"}

    p8, _, _ = build_command_payload(["vicons", "list"])
    assert p8 == {"cmd": "vicons", "action": "list"}


def test_rename_virtual_icon_and_infer_type():
    rng = random.Random(11)
    icons = add_virtual_icons_batch([], 3, 1920, 1080, rng=rng)
    orig_pos = (icons[1]["x"], icons[1]["y"])

    # Rename updates name & infers type from extension while keeping coordinates intact
    renamed = rename_virtual_icon(icons, 1, "  Berlin_Karte.pdf  ")
    assert renamed[1]["name"] == "Berlin_Karte.pdf"
    assert renamed[1]["type"] == "pdf"
    assert (renamed[1]["x"], renamed[1]["y"]) == orig_pos
    # Original list is not mutated
    assert icons[1]["name"] != "Berlin_Karte.pdf"

    # Explicit icon_type overrides extension inference
    renamed2 = rename_virtual_icon(renamed, 1, "Ankara_Cephesi", icon_type="folder")
    assert renamed2[1]["name"] == "Ankara_Cephesi"
    assert renamed2[1]["type"] == "folder"

    # Out-of-bounds or blank name leaves icon safely unchanged
    assert rename_virtual_icon(renamed2, 99, "X")[1]["name"] == "Ankara_Cephesi"
    assert rename_virtual_icon(renamed2, 1, "   ")[1]["name"] == "Ankara_Cephesi"

    # Long names are trimmed to 48 chars
    long_name = "A" * 80
    assert len(rename_virtual_icon(renamed2, 0, long_name)[0]["name"]) == 48

    assert infer_icon_type("Budget.xlsx") == "xlsx"
    assert infer_icon_type("Screen.png") == "png"
    assert infer_icon_type("Archive.zip") == "zip"
    assert infer_icon_type("Notes.md") == "txt"
    assert infer_icon_type("FolderOnly", fallback="folder") == "folder"
