import io
import json
import urllib.request
import zipfile
from pathlib import Path

from deskland_linux.config import Log, ThemeRegistry
from deskland_linux.importer import import_theme_file, load_custom_themes, remove_custom_theme
from deskland_linux.server import LocalServer, SECURITY_CSP, is_allowed_local_uri


def test_import_html_and_remove(tmp_path: Path) -> None:
    custom_dir = tmp_path / "custom"
    src = tmp_path / "neon-grid.html"
    src.write_text("<!doctype html><html><head><title>Neon Grid</title></head><body>ok</body></html>", encoding="utf-8")

    theme = import_theme_file(src, custom_dir=custom_dir, reserved_ids={"meadow", "beach"})
    assert theme["id"] == "custom-neon-grid"
    assert theme["name"] == "Neon Grid"
    assert theme["imported"] is True
    assert theme["status"] == "ready"
    assert theme["product"] == "free"
    assert theme["page"] == "__custom/custom-neon-grid/index.html"
    assert (custom_dir / "custom-neon-grid" / "index.html").is_file()
    assert len(load_custom_themes(custom_dir)) == 1

    assert remove_custom_theme("custom-neon-grid", custom_dir=custom_dir) is True
    assert load_custom_themes(custom_dir) == []
    assert not (custom_dir / "custom-neon-grid").exists()


def test_import_zip_with_single_root_and_manifest(tmp_path: Path) -> None:
    custom_dir = tmp_path / "custom"
    pkg = tmp_path / "sunset-pack.deskland"
    with zipfile.ZipFile(pkg, "w") as zf:
        zf.writestr("sunset/manifest.json", json.dumps({"name": "Sunset Sky", "entry": "scene.html", "adaptive": True}))
        zf.writestr("sunset/scene.html", "<html><body><h1>Sunset</h1></body></html>")
        zf.writestr("sunset/thumb.png", b"\x89PNG\r\n\x1a\n")

    theme = import_theme_file(pkg, custom_dir=custom_dir, reserved_ids=set())
    assert theme["name"] == "Sunset Sky"
    assert theme["adaptive"] is True
    assert theme["entry"] == "scene.html"
    assert theme["page"] == "__custom/custom-sunset-pack/scene.html"
    assert "url(__custom/custom-sunset-pack/thumb.png)" in theme["thumb"]
    assert (custom_dir / theme["id"] / "scene.html").is_file()


def test_import_zip_rejects_traversal(tmp_path: Path) -> None:
    custom_dir = tmp_path / "custom"
    bad = tmp_path / "evil.zip"
    with zipfile.ZipFile(bad, "w") as zf:
        zf.writestr("../escape.html", "bad")

    try:
        import_theme_file(bad, custom_dir=custom_dir)
    except ValueError as exc:
        assert "Unsafe" in str(exc)
    else:
        raise AssertionError("Expected ValueError for path traversal")


def test_import_image_and_server_merge(tmp_path: Path) -> None:
    proto = tmp_path / "proto"
    proto.mkdir()
    (proto / "themes.json").write_text(
        json.dumps({"themes": [{"id": "beach", "name": "Beach", "status": "ready", "product": "free", "page": "flat.html"}]}),
        encoding="utf-8",
    )
    (proto / "share.js").write_text("// share", encoding="utf-8")

    custom_dir = tmp_path / "custom"
    img = tmp_path / "forest_photo.png"
    img.write_bytes(b"\x89PNG\r\n\x1a\nfake")

    theme = import_theme_file(img, custom_dir=custom_dir, reserved_ids={"beach"})
    assert theme["adaptive"] is True
    assert len(theme["groups"][0]["params"]) == 3
    assert (custom_dir / theme["id"] / "media.png").is_file()
    assert (custom_dir / theme["id"] / "index.html").is_file()

    reg = ThemeRegistry(proto, Log(tmp_path / "test.log"), custom_dir=custom_dir)
    assert reg.allowed(theme["id"]) is True
    assert reg.page_for(theme["id"]) == f"__custom/{theme['id']}/index.html"

    srv = LocalServer(
        proto,
        tmp_path / "share",
        custom_themes_dir=custom_dir,
        themes_provider=reg.merged_json_bytes,
    )
    port = srv.start()
    try:
        merged = json.loads(urllib.request.urlopen(f"http://127.0.0.1:{port}/themes.json").read().decode("utf-8"))
        ids = [t["id"] for t in merged["themes"]]
        assert ids == ["beach", theme["id"]]
        html_body = urllib.request.urlopen(f"http://127.0.0.1:{port}/{theme['page']}").read().decode("utf-8")
        assert "forest photo" in html_body
        assert "desklandSnapshot" in html_body
    finally:
        srv.stop()


def test_import_rejects_corrupt_and_invalid_inputs(tmp_path: Path) -> None:
    custom_dir = tmp_path / "custom"

    # 1. Corrupt ZIP bytes
    corrupt = tmp_path / "broken.zip"
    corrupt.write_bytes(b"not a zip archive")
    try:
        import_theme_file(corrupt, custom_dir=custom_dir)
    except ValueError as exc:
        assert "corrupted" in str(exc).lower() or "zip" in str(exc).lower()
    else:
        raise AssertionError("Expected ValueError for corrupt ZIP")

    # 2. ZIP without any HTML entry page
    no_html = tmp_path / "nohtml.zip"
    with zipfile.ZipFile(no_html, "w") as zf:
        zf.writestr("readme.txt", "hello")
    try:
        import_theme_file(no_html, custom_dir=custom_dir)
    except ValueError as exc:
        assert "No .html" in str(exc)
    else:
        raise AssertionError("Expected ValueError for ZIP missing HTML")

    # 3. Invalid JSON manifest
    bad_manifest = tmp_path / "badmanifest.zip"
    with zipfile.ZipFile(bad_manifest, "w") as zf:
        zf.writestr("manifest.json", "{broken json")
        zf.writestr("index.html", "<html></html>")
    try:
        import_theme_file(bad_manifest, custom_dir=custom_dir)
    except ValueError as exc:
        assert "manifest.json" in str(exc)
    else:
        raise AssertionError("Expected ValueError for invalid manifest.json")

    # 4. Configured entry missing from ZIP
    missing_entry = tmp_path / "missingentry.zip"
    with zipfile.ZipFile(missing_entry, "w") as zf:
        zf.writestr("manifest.json", json.dumps({"entry": "does_not_exist.html"}))
        zf.writestr("index.html", "<html></html>")
    try:
        import_theme_file(missing_entry, custom_dir=custom_dir)
    except ValueError as exc:
        assert "Entry HTML file not found" in str(exc)
    else:
        raise AssertionError("Expected ValueError for missing entry")

    # 5. Empty 0-byte file
    empty = tmp_path / "empty.png"
    empty.write_bytes(b"")
    try:
        import_theme_file(empty, custom_dir=custom_dir)
    except ValueError as exc:
        assert "empty" in str(exc).lower()
    else:
        raise AssertionError("Expected ValueError for empty file")

    # Nothing left behind on disk or registry after failed imports
    assert load_custom_themes(custom_dir) == []
    dirs = [p for p in custom_dir.iterdir() if p.is_dir()] if custom_dir.exists() else []
    assert dirs == []


def test_import_zip_with_settings_object_svg_preview_and_description(tmp_path: Path) -> None:
    proto = tmp_path / "proto"
    proto.mkdir()
    (proto / "themes.json").write_text(json.dumps({"themes": []}), encoding="utf-8")
    custom_dir = tmp_path / "custom"

    pkg = tmp_path / "solar-system.zip"
    manifest = {
        "id": "interactive-solar-system",
        "name": "Interactive Solar System & Black Hole",
        "description": "Adaptive space theme with black hole core.",
        "entry": "index.html",
        "preview": "preview.svg",
        "thumbnail": "preview.svg",
        "adaptive": True,
        "settings": {
            "centerBodyType": {
                "key": "centerBodyType",
                "group": "Black Hole & Core",
                "label": "Central body type",
                "type": "select",
                "options": [
                    {"label": "Supermassive Black Hole", "value": "BlackHole"},
                    {"label": "Central Grand Sun", "value": "Sun"},
                    {"label": "None", "value": "None"},
                ],
                "default": "BlackHole",
            },
            "planetCount": {
                "key": "planetCount",
                "group": "Solar System",
                "label": "Planet count",
                "type": "range",
                "min": 1,
                "max": 6,
                "step": 1,
                "default": 3,
            },
            "shootingStars": {
                "key": "shootingStars",
                "group": "Space",
                "label": "Shooting stars",
                "type": "checkbox",
                "default": True,
            },
        },
    }
    with zipfile.ZipFile(pkg, "w") as zf:
        zf.writestr("manifest.json", json.dumps(manifest))
        zf.writestr("index.html", "<!doctype html><html><body><canvas></canvas></body></html>")
        zf.writestr("preview.svg", '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"></svg>')

    theme = import_theme_file(pkg, custom_dir=custom_dir, reserved_ids=set())
    assert theme["name"] == "Interactive Solar System & Black Hole"
    assert theme["desc"] == "Adaptive space theme with black hole core."
    assert theme["adaptive"] is True
    assert f"url(__custom/{theme['id']}/preview.svg)" in theme["thumb"]
    assert len(theme["groups"]) == 3
    assert theme["groups"][0]["name"] == "Black Hole & Core"
    cb = theme["groups"][0]["params"][0]
    assert cb["options"] == ["Supermassive Black Hole", "Central Grand Sun", "None"]
    assert cb["values"] == ["BlackHole", "Sun", "None"]
    assert cb["default"] == "BlackHole"
    ss = theme["groups"][2]["params"][0]
    assert ss["options"] == ["Off", "On"]
    assert ss["values"] == [False, True]
    assert ss["default"] is True

    reg = ThemeRegistry(proto, Log(tmp_path / "test.log"), custom_dir=custom_dir)
    defs = reg.defaults(theme["id"])
    assert defs["centerBodyType"] == "BlackHole"
    assert defs["planetCount"] == 3.0
    assert defs["shootingStars"] is True


def test_import_zip_rejects_disallowed_extensions_and_allows_safe_assets(tmp_path: Path) -> None:
    custom_dir = tmp_path / "custom"

    for bad_name in ("payload.exe", "run.sh", "install.bat", "evil_binary"):
        pkg = tmp_path / f"bad_{bad_name.replace('.', '_')}.zip"
        with zipfile.ZipFile(pkg, "w") as zf:
            zf.writestr("index.html", "<!doctype html><html><body>ok</body></html>")
            zf.writestr(bad_name, b"MZfake")
        try:
            import_theme_file(pkg, custom_dir=custom_dir)
        except ValueError as exc:
            assert "Unsupported file type in theme archive" in str(exc)
        else:
            raise AssertionError(f"Expected rejection for archive member {bad_name}")

    good = tmp_path / "shader-pack.deskland"
    with zipfile.ZipFile(good, "w") as zf:
        zf.writestr("index.html", "<!doctype html><html><body>shader</body></html>")
        zf.writestr("shaders/wave.glsl", "void main(){}")
        zf.writestr("LICENSE", "GPL-3.0")
    theme = import_theme_file(good, custom_dir=custom_dir)
    assert (custom_dir / theme["id"] / "shaders" / "wave.glsl").is_file()
    assert (custom_dir / theme["id"] / "LICENSE").is_file()


def test_local_server_security_headers_and_no_directory_listing(tmp_path: Path) -> None:
    proto = tmp_path / "proto"
    proto.mkdir()
    (proto / "index.html").write_text("<html><body>hi</body></html>", encoding="utf-8")
    (proto / "sub").mkdir()
    (proto / "sub" / "secret.txt").write_text("x", encoding="utf-8")

    srv = LocalServer(proto, tmp_path / "share")
    port = srv.start()
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/index.html") as resp:
            assert resp.headers.get("Content-Security-Policy") == SECURITY_CSP
            assert resp.headers.get("X-Content-Type-Options") == "nosniff"
            assert resp.headers.get("Referrer-Policy") == "no-referrer"
        try:
            urllib.request.urlopen(f"http://127.0.0.1:{port}/sub/")
        except urllib.error.HTTPError as err:
            assert err.code == 403
        else:
            raise AssertionError("Expected 403 Forbidden for directory listing")
    finally:
        srv.stop()


def test_is_allowed_local_uri() -> None:
    assert is_allowed_local_uri("http://127.0.0.1:5178/flat.html", allowed_port=5178) is True
    assert is_allowed_local_uri("http://localhost:5178/settings.html", allowed_port=5178) is True
    assert is_allowed_local_uri("about:blank", allowed_port=5178) is True
    assert is_allowed_local_uri("data:image/png;base64,AA==", allowed_port=5178) is True
    assert is_allowed_local_uri("blob:http://127.0.0.1:5178/abc", allowed_port=5178) is True
    assert is_allowed_local_uri("http://127.0.0.1:9999/other", allowed_port=5178) is False
    assert is_allowed_local_uri("https://example.com/tracker.js", allowed_port=5178) is False
    assert is_allowed_local_uri("file:///etc/passwd", allowed_port=5178) is False
    assert is_allowed_local_uri("javascript:alert(1)", allowed_port=5178) is False


