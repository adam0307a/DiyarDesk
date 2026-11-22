package com.diyardesk.tablet

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

class CustomThemeImporterTest {

    @get:Rule
    val tmp = TemporaryFolder()

    @Test
    fun importsZipPackageWithManifestAndMergesRegistry() {
        val root = tmp.newFolder("custom")
        val importer = CustomThemeImporter(root)

        val zipBytes = ByteArrayOutputStream().use { baos ->
            ZipOutputStream(baos).use { zos ->
                zos.putNextEntry(ZipEntry("mytheme/manifest.json"))
                zos.write("""{"name":"Bosphorus Breeze","desc":"Test theme","entry":"main.html","adaptive":true}""".toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry("mytheme/main.html"))
                zos.write("<!doctype html><html><title>Ignored</title><body>hi</body></html>".toByteArray())
                zos.closeEntry()
            }
            baos.toByteArray()
        }

        val entry = importer.importStream(ByteArrayInputStream(zipBytes), "breeze.deskland")
        val id = entry.getString("id")
        assertEquals("Bosphorus Breeze", entry.getString("name"))
        assertEquals("__custom/$id/main.html", entry.getString("page"))
        assertTrue(File(root, "$id/main.html").isFile)

        val merged = JSONObject(importer.mergedThemesJson("""{"themes":[{"id":"beach","status":"ready"}]}"""))
        assertEquals(2, merged.getJSONArray("themes").length())

        assertTrue(importer.removeTheme(id))
        assertFalse(File(root, id).exists())
    }

    @Test
    fun rejectsZipSlipTraversal() {
        val root = tmp.newFolder("custom_sec")
        val importer = CustomThemeImporter(root)

        val malicious = ByteArrayOutputStream().use { baos ->
            ZipOutputStream(baos).use { zos ->
                zos.putNextEntry(ZipEntry("../../evil.txt"))
                zos.write("pwn".toByteArray())
                zos.closeEntry()
            }
            baos.toByteArray()
        }

        assertThrows(IllegalArgumentException::class.java) {
            importer.importStream(ByteArrayInputStream(malicious), "bad.zip")
        }
        assertNull(importer.resolveCustomAsset("../outside.txt"))
        assertNotNull(importer.resolveCustomAsset("safe-theme/index.html"))
    }

    @Test
    fun importsSingleHtmlAndImageWallpaper() {
        val root = tmp.newFolder("custom_single")
        val importer = CustomThemeImporter(root)

        val htmlEntry = importer.importStream(
            ByteArrayInputStream("<html><head><title>Galaxy Shader</title></head><body></body></html>".toByteArray()),
            "shader.html"
        )
        assertEquals("Galaxy Shader", htmlEntry.getString("name"))

        val pngHeader = byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A)
        val imgEntry = importer.importStream(ByteArrayInputStream(pngHeader), "Nebula View.png")
        val id = imgEntry.getString("id")
        assertEquals("Nebula View", imgEntry.getString("name"))
        assertTrue(imgEntry.getBoolean("imported"))
        assertTrue(File(root, "$id/media.png").isFile)
        assertTrue(File(root, "$id/index.html").readText().contains("desklandSnapshot") || File(root, "$id/index.html").readText().contains("media.png"))
    }

    @Test
    fun importsZipWithSettingsObjectSvgPreviewAndExtensionlessZipSniffing() {
        val root = tmp.newFolder("custom_settings")
        val importer = CustomThemeImporter(root)

        val manifest = """
            {
              "name": "Interactive Solar System & Black Hole",
              "description": "Adaptive space theme",
              "entry": "index.html",
              "preview": "preview.svg",
              "adaptive": true,
              "settings": {
                "centerBodyType": {
                  "key": "centerBodyType",
                  "group": "Black Hole & Core",
                  "label": "Central body type",
                  "type": "select",
                  "options": [
                    {"label": "Black Hole", "value": "BlackHole"},
                    {"label": "Sun", "value": "Sun"}
                  ],
                  "default": "BlackHole"
                },
                "shootingStars": {
                  "key": "shootingStars",
                  "group": "Space",
                  "label": "Shooting stars",
                  "type": "checkbox",
                  "default": true
                }
              }
            }
        """.trimIndent()

        val zipBytes = ByteArrayOutputStream().use { baos ->
            ZipOutputStream(baos).use { zos ->
                zos.putNextEntry(ZipEntry("manifest.json"))
                zos.write(manifest.toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry("index.html"))
                zos.write("<!doctype html><html><body></body></html>".toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry("preview.svg"))
                zos.write("<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>".toByteArray())
                zos.closeEntry()
            }
            baos.toByteArray()
        }

        val entry = importer.importStream(ByteArrayInputStream(zipBytes), "document_without_ext")
        val id = entry.getString("id")
        assertTrue(entry.getBoolean("imported"))
        assertEquals("Adaptive space theme", entry.getString("desc"))
        assertTrue(entry.getString("thumb").contains("url(__custom/$id/preview.svg)"))
        val groups = entry.getJSONArray("groups")
        assertEquals(2, groups.length())
        val byName = (0 until groups.length())
            .map { groups.getJSONObject(it) }
            .associateBy { it.getString("name") }
        val cb = byName.getValue("Black Hole & Core").getJSONArray("params").getJSONObject(0)
        assertEquals("centerBodyType", cb.getString("key"))
        assertEquals("BlackHole", cb.getString("default"))
        val ss = byName.getValue("Space").getJSONArray("params").getJSONObject(0)
        assertEquals("shootingStars", ss.getString("key"))
        assertEquals(true, ss.getBoolean("default"))
    }

    @Test
    fun rejectsDisallowedArchiveExtensionsAndSymlinks() {
        val root = tmp.newFolder("custom_ext_sec")
        val importer = CustomThemeImporter(root)

        val badExtZip = ByteArrayOutputStream().use { baos ->
            ZipOutputStream(baos).use { zos ->
                zos.putNextEntry(ZipEntry("index.html"))
                zos.write("<!doctype html><html><body>ok</body></html>".toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry("dropper.exe"))
                zos.write("MZ".toByteArray())
                zos.closeEntry()
            }
            baos.toByteArray()
        }
        assertThrows(IllegalArgumentException::class.java) {
            importer.importStream(ByteArrayInputStream(badExtZip), "evil.zip")
        }

        // Craft a ZIP containing a Unix symlink entry (S_IFLNK = 0o120000 in upper 16 bits of external attributes)
        val rawZip = ByteArrayOutputStream().use { baos ->
            ZipOutputStream(baos).use { zos ->
                zos.putNextEntry(ZipEntry("index.html"))
                zos.write("<!doctype html><html><body>ok</body></html>".toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry("link.txt"))
                zos.write("/etc/passwd".toByteArray())
                zos.closeEntry()
            }
            baos.toByteArray()
        }
        // Locate second Central Directory header (0x02014b50) and set externalFileAttributes to 0xA1FF0000
        var cenCount = 0
        for (i in 0 until rawZip.size - 46) {
            if (rawZip[i] == 0x50.toByte() && rawZip[i + 1] == 0x4B.toByte() &&
                rawZip[i + 2] == 0x01.toByte() && rawZip[i + 3] == 0x02.toByte()
            ) {
                cenCount++
                if (cenCount == 2) {
                    rawZip[i + 38] = 0x00
                    rawZip[i + 39] = 0x00
                    rawZip[i + 40] = 0xFF.toByte()
                    rawZip[i + 41] = 0xA1.toByte()
                    break
                }
            }
        }
        assertEquals(2, cenCount)
        assertThrows(IllegalArgumentException::class.java) {
            importer.importStream(ByteArrayInputStream(rawZip), "symlink.zip")
        }
    }

    @Test
    fun importsLivelyInfoAndPropertiesWithColorAndDropdown() {
        val root = tmp.newFolder("custom_lively")
        val importer = CustomThemeImporter(root)

        val zipBytes = ByteArrayOutputStream().use { baos ->
            ZipOutputStream(baos).use { zos ->
                zos.putNextEntry(ZipEntry("scene.html"))
                zos.write("<!doctype html><html><body>Lively</body></html>".toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry("LivelyInfo.json"))
                zos.write("""{"Title":"Lively Aurora","Desc":"Shader demo","FileName":"scene.html"}""".toByteArray())
                zos.closeEntry()
                zos.putNextEntry(ZipEntry("LivelyProperties.json"))
                zos.write(
                    """
                    {
                      "tint": {"type":"color","text":"Tint","value":"#f0a"},
                      "preset": {"type":"dropdown","text":"Preset","items":["Soft","Vivid"],"value":1},
                      "btn": {"type":"button","text":"Reset"}
                    }
                    """.trimIndent().toByteArray()
                )
                zos.closeEntry()
            }
            baos.toByteArray()
        }

        val entry = importer.importStream(ByteArrayInputStream(zipBytes), "lively.zip")
        assertEquals("Lively Aurora", entry.getString("name"))
        assertEquals("Shader demo", entry.getString("desc"))
        val params = entry.getJSONArray("groups").getJSONObject(0).getJSONArray("params")
        assertEquals(2, params.length())
        val byKey = (0 until params.length()).map { params.getJSONObject(it) }.associateBy { it.getString("key") }
        assertEquals("color", byKey.getValue("tint").getString("type"))
        assertEquals("#ff00aa", byKey.getValue("tint").getString("default"))
        assertEquals(1, byKey.getValue("preset").getInt("default"))
    }

    @Test
    fun importsSingleGlslShaderFile() {
        val root = tmp.newFolder("custom_glsl")
        val importer = CustomThemeImporter(root)
        val glsl = "void mainImage(out vec4 c, in vec2 p){ c = vec4(p / u_resolution, 0.5, 1.0); }"
        val entry = importer.importStream(ByteArrayInputStream(glsl.toByteArray()), "neon_wave.frag")
        assertTrue(entry.getBoolean("imported"))
        assertTrue(entry.getBoolean("adaptive"))
        val html = File(File(root, entry.getString("id")), "index.html").readText(Charsets.UTF_8)
        assertTrue(html.contains("uniform vec4 u_icons[32];"))
        assertTrue(html.contains("desklandSnapshot"))
    }
}
