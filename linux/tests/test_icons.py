from pathlib import Path

from deskland_linux.icons import (
    GioMetadataAdapter,
    PlasmaAdapter,
    XfceAdapter,
    grid_rect,
    parse_position,
    xfce_cell_rect,
)


def test_parse_position_formats():
    assert parse_position("120,340") == (120.0, 340.0)
    assert parse_position("(12; 34)") == (12.0, 34.0)
    assert parse_position("7, 9") == (7.0, 9.0)
    assert parse_position("nope") is None


def test_grid_rect_scales_and_wraps():
    rect = grid_rect(0, 2, 1920, 1080)
    assert (rect.x, rect.y, rect.w, rect.h) == (8, 12, 152, 200)
    wrapped = grid_rect(20, 1, 800, 300)
    assert wrapped.x >= 0 and wrapped.y >= 0


def test_gio_output_parser():
    output = """uri: file:///home/u/Desktop/a.txt
attributes:
  metadata::nautilus-icon-position: 100,250
  standard::name: a.txt
uri: file:///home/u/Desktop/folder
attributes:
  metadata::ding-icon-position: 32, 64
"""
    parsed = GioMetadataAdapter._parse_output(output)
    assert parsed["/home/u/Desktop/a.txt"] == (100.0, 250.0)
    assert parsed["/home/u/Desktop/folder"] == (32.0, 64.0)


def test_xfce_parser_row_col():
    text = """
[row0]
col2=file:///home/u/Desktop/a.txt
[row1]
col0=file:///home/u/Desktop/b.txt
"""
    parsed = XfceAdapter._parse(text)
    assert parsed["/home/u/Desktop/a.txt"] == (2.0, 0.0)
    assert parsed["/home/u/Desktop/b.txt"] == (0.0, 1.0)


def test_plasma_parser_json_positions():
    text = 'positions={"file:///home/u/Desktop/a.txt": [10, 20], "file:///home/u/Desktop/b": {"x": 3, "y": 4}}\n'
    parsed = PlasmaAdapter._parse(text)
    assert parsed["/home/u/Desktop/a.txt"] == (10.0, 20.0)
    assert parsed["/home/u/Desktop/b"] == (3.0, 4.0)


def test_plasma_parser_kde6_flat_triples():
    text = (
        '[Containments][1][General]\n'
        'positions={"1536x864":["4","14","desktop:/a.txt","1","6","desktop:/b","0","13","desktop:/c%20d.txt","2","12"]}\n'
        'changedPositions={"desktop:/a.txt":["1536x864","5","2"]}\n'
        'sortMode=-1\n'
    )
    parsed = PlasmaAdapter._parse(text)
    # changedPositions overrides the flat entry (last writer wins)
    assert parsed["desktop:/a.txt"] == (5.0, 2.0)
    assert parsed["desktop:/b"] == (1.0, 6.0)
    assert parsed["desktop:/c%20d.txt"] == (0.0, 13.0)
    # trailing incomplete pair ("2","12") is ignored
    assert len(parsed) == 3


def test_plasma_changed_positions_tail_prefers_xy():
    text = 'changedPositions={"desktop:/a.txt":["1536x864","5","2"],"desktop:/b.txt":[3,4]}\n'
    parsed = PlasmaAdapter._parse(text)
    assert parsed["desktop:/a.txt"] == (5.0, 2.0)
    assert parsed["desktop:/b.txt"] == (3.0, 4.0)


def test_rect_array_matches_protocol():
    rect = grid_rect(1, 1, 1920, 1080)
    assert rect.as_array() == [rect.x, rect.y, rect.x + rect.w, rect.y + rect.h]


def test_xfce_cell_rect_matches_xfdesktop_grid():
    r0 = xfce_cell_rect(0, 0, 1, 1920, 1080)
    assert (r0.x, r0.y, r0.w, r0.h) == (14, 10, 103, 103)
    r_right = xfce_cell_rect(14, 7, 1, 1920, 1080)
    assert (r_right.x, r_right.y, r_right.w, r_right.h) == (1486, 753, 103, 103)
    r_hidpi = xfce_cell_rect(14, 7, 2, 3840, 2160)
    assert (r_hidpi.x, r_hidpi.y, r_hidpi.w, r_hidpi.h) == (1486 * 2, 753 * 2, 103 * 2, 103 * 2)


def test_xfce_adapter_reads_yaml_and_custom_icon_size(tmp_path, monkeypatch):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "cfg"))
    desktop = tmp_path / "Desktop"
    desktop.mkdir()
    (desktop / "far.jpg").write_text("", encoding="utf-8")

    xfce_dir = tmp_path / "cfg" / "xfce4" / "desktop"
    xfce_dir.mkdir(parents=True)
    (xfce_dir / "icons.screen0.yaml").write_text(
        f'icons:\n  "{desktop / "far.jpg"}":\n    row: 7\n    col: 14\n',
        encoding="utf-8",
    )

    adapter = XfceAdapter(desktop=desktop, scale=1, screen_w=1920, screen_h=1080)
    ok, rects = adapter.read()
    assert ok and len(rects) == 1
    assert (rects[0].x, rects[0].y, rects[0].w, rects[0].h) == (1486, 753, 103, 103)

    xfconf_dir = tmp_path / "cfg" / "xfce4" / "xfconf" / "xfce-perchannel-xml"
    xfconf_dir.mkdir(parents=True)
    (xfconf_dir / "xfce4-desktop.xml").write_text(
        '<channel name="xfce4-desktop"><property name="desktop-icons" type="empty">'
        '<property name="icon-size" type="uint" value="64"/></property></channel>',
        encoding="utf-8",
    )
    ok, rects_big = adapter.read()
    assert ok and len(rects_big) == 1
    expected = xfce_cell_rect(14, 7, 1, 1920, 1080, icon_size=64)
    assert (rects_big[0].x, rects_big[0].y) == (expected.x, expected.y)
    assert rects_big[0].w == expected.w > 103
