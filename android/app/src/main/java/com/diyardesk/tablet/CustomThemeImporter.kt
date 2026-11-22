package com.diyardesk.tablet

import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedInputStream
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.Locale
import java.util.UUID
import java.util.zip.ZipInputStream

/**
 * Custom theme validator, package importer and merged themes.json builder for Android.
 * Mirrors desktop `linux/deskland_linux/importer.py` & Windows `WebCustomHelper`.
 */
class CustomThemeImporter(private val customThemesRoot: File) {

    private val registryFile = File(customThemesRoot, "registry.json")

    init {
        customThemesRoot.mkdirs()
    }

    fun loadCustomThemes(): List<JSONObject> {
        if (!registryFile.exists()) return emptyList()
        return try {
            val arr = JSONArray(registryFile.readText(Charsets.UTF_8))
            buildList {
                for (i in 0 until arr.length()) {
                    arr.optJSONObject(i)?.let { add(it) }
                }
            }
        } catch (_: Exception) {
            emptyList()
        }
    }

    private fun saveCustomThemes(entries: List<JSONObject>) {
        customThemesRoot.mkdirs()
        val arr = JSONArray()
        for (e in entries) arr.put(e)
        registryFile.writeText(arr.toString(2), Charsets.UTF_8)
    }

    fun mergedThemesJson(builtInThemesJson: String): String {
        val root = JSONObject(builtInThemesJson)
        val themes = root.optJSONArray("themes") ?: JSONArray()
        for (custom in loadCustomThemes()) {
            themes.put(custom)
        }
        root.put("themes", themes)
        return root.toString()
    }

    fun importStream(input: InputStream, originalFileName: String): JSONObject {
        val cleanName = originalFileName.substringAfterLast('/').substringAfterLast('\\').ifBlank { "theme.zip" }
        var ext = cleanName.substringAfterLast('.', "").lowercase(Locale.US)
        val buffered = BufferedInputStream(input, 16384)
        if (ext !in SUPPORTED_EXTS) {
            buffered.mark(64)
            val head = ByteArray(64)
            val n = buffered.read(head)
            buffered.reset()
            if (n >= 4 && head[0] == 0x50.toByte() && head[1] == 0x4B.toByte() &&
                (head[2] == 0x03.toByte() || head[2] == 0x05.toByte() || head[2] == 0x07.toByte())
            ) {
                ext = "zip"
            } else if (n > 0) {
                val snippet = String(head, 0, n, Charsets.UTF_8).trimStart().lowercase(Locale.US)
                if (snippet.startsWith("<!doctype html") || snippet.startsWith("<html")) {
                    ext = "html"
                }
            }
        }
        val baseTitle = cleanName.substringBeforeLast('.').ifBlank { "Custom Theme" }
        val id = "custom-${slugify(baseTitle).take(24)}-${UUID.randomUUID().toString().take(8)}"
        val destDir = File(customThemesRoot, id)
        if (!destDir.mkdirs() && !destDir.isDirectory) {
            throw IllegalStateException("Cannot create theme directory")
        }

        try {
            val entry = when (ext) {
                "zip", "deskland" -> importZipPackage(buffered, destDir, id, baseTitle)
                "html", "htm" -> importSingleHtml(buffered, destDir, id, baseTitle)
                "glsl", "frag" -> importSingleShader(buffered, destDir, id, baseTitle)
                in IMAGE_EXTS -> importImageWallpaper(buffered, cleanName, destDir, id, baseTitle)
                in VIDEO_EXTS -> importVideoWallpaper(buffered, cleanName, destDir, id, baseTitle)
                else -> throw IllegalArgumentException("Unsupported format: .$ext (supported: .zip, .deskland, .html, .glsl, .frag, images, videos)")
            }
            val current = loadCustomThemes().toMutableList()
            current.add(entry)
            saveCustomThemes(current)
            return entry
        } catch (t: Throwable) {
            destDir.deleteRecursively()
            throw t
        }
    }

    fun removeTheme(themeId: String): Boolean {
        if (!themeId.startsWith("custom-")) return false
        val current = loadCustomThemes()
        val filtered = current.filterNot { it.optString("id") == themeId }
        if (filtered.size == current.size) return false
        saveCustomThemes(filtered)
        val targetDir = resolveCustomAsset(themeId)
        if (targetDir != null && targetDir.exists()) {
            targetDir.deleteRecursively()
        }
        return true
    }

    fun resolveCustomAsset(subPath: String): File? {
        val trimmed = subPath.trimStart('/')
        if (trimmed.isEmpty()) return null
        val rootCanonical = customThemesRoot.canonicalFile
        val target = File(rootCanonical, trimmed).canonicalFile
        val rootPrefix = rootCanonical.path + File.separator
        return if (target == rootCanonical || target.path.startsWith(rootPrefix)) target else null
    }

    private fun importSingleShader(input: InputStream, destDir: File, id: String, fallbackName: String): JSONObject {
        val shaderFile = File(destDir, "shader.glsl")
        copyLimited(input, shaderFile, 2L * 1024L * 1024L)
        val bytes = shaderFile.readBytes()
        if (bytes.isEmpty() || bytes.any { it == 0.toByte() }) {
            throw IllegalArgumentException("Shader file is empty or corrupted")
        }
        val glsl = String(bytes, Charsets.UTF_8)
        val title = fallbackName.take(60)
        val encodedGlsl = JSONObject.quote(glsl)
        val safeTitle = title.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")
        val wrapper = """<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>$safeTitle</title>
<style>html,body,canvas{margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#090c14;display:block;}</style>
</head><body><canvas id="gl"></canvas><script>
(() => {
  const canvas = document.getElementById('gl');
  const gl = canvas.getContext('webgl', { preserveDrawingBuffer: true, antialias: false });
  if (!gl) return;
  const rawFrag = $encodedGlsl;
  const hasMainImage = /void\s+mainImage\s*\(/.test(rawFrag);
  const header = 'precision highp float;\nuniform float u_time;\nuniform vec2 u_resolution;\nuniform vec2 u_mouse;\nuniform float u_speed;\nuniform float u_brightness;\nuniform float u_audio;\nuniform int u_iconCount;\nuniform vec4 u_icons[32];\nuniform vec3 iResolution;\nuniform float iTime;\nuniform vec4 iMouse;\n';
  const stripped = rawFrag.replace(/^\s*precision\s+\w+\s+float\s*;/gm, '');
  const fragSrc = header + stripped + (hasMainImage ? '\nvoid main(){ vec4 c=vec4(0.0); mainImage(c, gl_FragCoord.xy); gl_FragColor=vec4(c.rgb*u_brightness, 1.0); }\n' : '');
  const vertSrc = 'attribute vec2 a_pos; void main(){ gl_Position = vec4(a_pos, 0.0, 1.0); }';
  function compile(type, src){ const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) || 'shader error'); return s; }
  const prog = gl.createProgram();
  try { gl.attachShader(prog, compile(gl.VERTEX_SHADER, vertSrc)); gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, fragSrc)); gl.linkProgram(prog); }
  catch (err) { document.body.innerHTML = '<pre style="color:#f78;padding:20px;font:13px monospace">' + String(err) + '</pre>'; return; }
  gl.useProgram(prog);
  const buf = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);
  const locPos = gl.getAttribLocation(prog, 'a_pos'); gl.enableVertexAttribArray(locPos); gl.vertexAttribPointer(locPos, 2, gl.FLOAT, false, 0, 0);
  const U = n => gl.getUniformLocation(prog, n);
  const uTime=U('u_time'), uRes=U('u_resolution'), uMouse=U('u_mouse'), uSpeed=U('u_speed'), uBright=U('u_brightness'), uAudio=U('u_audio'), uIconCount=U('u_iconCount'), uIcons=U('u_icons'), iRes=U('iResolution'), iTime=U('iTime'), iMouse=U('iMouse');
  const cfg = { speed: 1.0, brightness: 1.0 };
  let paused = false, mouseX = 0, mouseY = 0, audioRms = 0, icons = new Float32Array(128), iconCount = 0, simTime = 0, lastTs = performance.now();
  function resize(){ const dpr = Math.min(window.devicePixelRatio || 1, 1.5); canvas.width = Math.max(1, Math.floor(innerWidth * dpr)); canvas.height = Math.max(1, Math.floor(innerHeight * dpr)); gl.viewport(0, 0, canvas.width, canvas.height); }
  addEventListener('resize', resize); resize();
  function applyMsg(raw){ let m = raw; if (typeof m === 'string') { try { m = JSON.parse(m); } catch (_) { return; } } if (!m || typeof m !== 'object') return;
    if (m.t === 'settings' && m.v) { if (typeof m.v.speed === 'number') cfg.speed = m.v.speed; if (typeof m.v.brightness === 'number') cfg.brightness = m.v.brightness; }
    else if (m.t === 'reset') { cfg.speed = 1.0; cfg.brightness = 1.0; }
    else if (m.t === 'pause') { paused = !!m.v; }
    else if (m.t === 'icons' && Array.isArray(m.r)) { iconCount = Math.min(32, m.r.length); for (let i = 0; i < iconCount; i++) { const r = m.r[i] || [0,0,0,0]; icons[i*4]=r[0]||0; icons[i*4+1]=r[1]||0; icons[i*4+2]=r[2]||0; icons[i*4+3]=r[3]||0; } }
  }
  addEventListener('message', e => applyMsg(e.data));
  if (window.chrome && window.chrome.webview) window.chrome.webview.addEventListener('message', e => applyMsg(e.data));
  function render(ts){ requestAnimationFrame(render); const dt = Math.min(0.1, Math.max(0, (ts - lastTs) / 1000)); lastTs = ts; if (paused) return; simTime += dt * cfg.speed;
    if (uTime) gl.uniform1f(uTime, simTime); if (iTime) gl.uniform1f(iTime, simTime); if (uRes) gl.uniform2f(uRes, canvas.width, canvas.height); if (iRes) gl.uniform3f(iRes, canvas.width, canvas.height, 1.0);
    if (uMouse) gl.uniform2f(uMouse, mouseX, mouseY); if (iMouse) gl.uniform4f(iMouse, mouseX, mouseY, 0, 0); if (uSpeed) gl.uniform1f(uSpeed, cfg.speed); if (uBright) gl.uniform1f(uBright, cfg.brightness); if (uAudio) gl.uniform1f(uAudio, audioRms);
    if (uIconCount) gl.uniform1i(uIconCount, iconCount); if (uIcons) gl.uniform4fv(uIcons, icons); gl.drawArrays(gl.TRIANGLES, 0, 6); }
  window.desklandSnapshot = () => canvas.toDataURL('image/png');
  requestAnimationFrame(render);
  if (new URLSearchParams(location.search).get('preview') === '1') parent.postMessage(JSON.stringify({ t: 'ready' }), '*');
  else if (window.chrome && window.chrome.webview) window.chrome.webview.postMessage('ready');
})();
</script></body></html>"""
        File(destDir, "index.html").writeText(wrapper, Charsets.UTF_8)
        val groups = JSONArray().put(
            JSONObject().apply {
                put("name", "Shader")
                put(
                    "params",
                    JSONArray()
                        .put(JSONObject().put("key", "speed").put("label", "Speed").put("min", 0.1).put("max", 3.0).put("step", 0.05).put("default", 1.0).put("format", "times"))
                        .put(JSONObject().put("key", "brightness").put("label", "Brightness").put("min", 0.4).put("max", 1.4).put("step", 0.01).put("default", 1.0).put("format", "percent"))
                )
            }
        )
        return buildEntry(
            id = id,
            name = title,
            desc = "Imported GLSL WebGL fragment shader",
            page = "__custom/$id/index.html",
            adaptive = true,
            groups = groups,
            thumb = "radial-gradient(circle at 28% 35%, rgba(168,95,255,0.52) 0%, transparent 50%), radial-gradient(circle at 75% 68%, rgba(72,210,255,0.42) 0%, transparent 48%), linear-gradient(150deg, #131126 0%, #1d2642 55%, #111829 100%)"
        )
    }

    private fun importZipPackage(input: InputStream, destDir: File, id: String, fallbackName: String): JSONObject {
        var totalBytes = 0L
        var fileCount = 0
        val buffer = ByteArray(16384)
        val destCanonical = destDir.canonicalFile
        val destPrefix = destCanonical.path + File.separator

        val tmpZip = File.createTempFile("diyardesk-import-", ".zip", customThemesRoot)
        try {
            FileOutputStream(tmpZip).use { fos ->
                var compressedBytes = 0L
                while (true) {
                    val n = input.read(buffer)
                    if (n <= 0) break
                    compressedBytes += n
                    if (compressedBytes > MAX_ARCHIVE_BYTES) {
                        throw IllegalArgumentException("Archive exceeds 150 MB limit")
                    }
                    fos.write(buffer, 0, n)
                }
            }
            if (containsZipSymlink(tmpZip)) {
                throw IllegalArgumentException("Archive contains unsupported symbolic link")
            }

            ZipInputStream(BufferedInputStream(tmpZip.inputStream())).use { zis ->
                while (true) {
                    val ze = zis.nextEntry ?: break
                    val rawName = ze.name.replace('\\', '/')
                    if (rawName.startsWith("/") || rawName.contains("..") || rawName.startsWith("./..") || rawName.contains(':')) {
                        throw IllegalArgumentException("Unsafe archive path: ${ze.name}")
                    }
                    val cleanRel = rawName.trimStart('/')
                    if (cleanRel.isEmpty()) {
                        zis.closeEntry()
                        continue
                    }
                    val firstSeg = cleanRel.substringBefore('/')
                    if (firstSeg.equals("__MACOSX", ignoreCase = true)) {
                        zis.closeEntry()
                        continue
                    }

                    val outFile = File(destCanonical, cleanRel).canonicalFile
                    if (!outFile.path.startsWith(destPrefix) && outFile != destCanonical) {
                        throw IllegalArgumentException("Zip-slip traversal detected: ${ze.name}")
                    }

                    if (ze.isDirectory) {
                        outFile.mkdirs()
                    } else {
                        if (!isAllowedArchiveMember(outFile.name)) {
                            throw IllegalArgumentException("Unsupported file type in theme archive: ${ze.name}")
                        }
                        fileCount++
                        if (fileCount > MAX_ARCHIVE_FILES) {
                            throw IllegalArgumentException("Archive exceeds maximum $MAX_ARCHIVE_FILES files")
                        }
                        outFile.parentFile?.mkdirs()
                        FileOutputStream(outFile).use { fos ->
                            while (true) {
                                val n = zis.read(buffer)
                                if (n <= 0) break
                                totalBytes += n
                                if (totalBytes > MAX_ARCHIVE_BYTES) {
                                    throw IllegalArgumentException("Archive exceeds 150 MB limit")
                                }
                                fos.write(buffer, 0, n)
                            }
                        }
                    }
                    zis.closeEntry()
                }
            }
        } finally {
            tmpZip.delete()
        }

        // Flatten single wrapping directory if present
        val children = destDir.listFiles()?.filter { it.name != "__MACOSX" } ?: emptyList()
        if (children.size == 1 && children[0].isDirectory) {
            val wrapper = children[0]
            wrapper.listFiles()?.forEach { inner ->
                inner.renameTo(File(destDir, inner.name))
            }
            wrapper.deleteRecursively()
        }

        val manifestFile = listOf("manifest.json", "theme.json").map { File(destDir, it) }.firstOrNull { it.isFile }
        val manifest = manifestFile?.let {
            try { JSONObject(it.readText(Charsets.UTF_8)) } catch (_: Exception) { null }
        }
        val livelyInfo = File(destDir, "LivelyInfo.json").takeIf { it.isFile }?.let {
            try { JSONObject(it.readText(Charsets.UTF_8)) } catch (_: Exception) { null }
        }
        val livelyProps = File(destDir, "LivelyProperties.json").takeIf { it.isFile }?.let {
            try { JSONObject(it.readText(Charsets.UTF_8)) } catch (_: Exception) { null }
        }

        val requestedEntry = listOf("entry", "page", "main", "file")
            .firstNotNullOfOrNull { k -> manifest?.optString(k)?.trim()?.ifBlank { null } }
            ?: livelyInfo?.optString("FileName")?.trim()?.ifBlank { null }
        val entryRel = if (requestedEntry != null) {
            val cleanEntry = requestedEntry.trimStart('/')
            val entryFile = File(destDir, cleanEntry).canonicalFile
            if (!entryFile.path.startsWith(destPrefix) || !entryFile.isFile ||
                entryFile.extension.lowercase(Locale.US) !in setOf("html", "htm")
            ) {
                throw IllegalArgumentException("Entry HTML file not found in archive: $cleanEntry")
            }
            cleanEntry
        } else {
            val idx = File(destDir, "index.html")
            if (idx.isFile) {
                "index.html"
            } else {
                val firstHtml = destDir.walkTopDown()
                    .filter { it.isFile && (it.extension.equals("html", true) || it.extension.equals("htm", true)) }
                    .firstOrNull()
                    ?: throw IllegalArgumentException("Archive contains no HTML entry page")
                firstHtml.relativeTo(destDir).path.replace(File.separatorChar, '/')
            }
        }

        val name = (manifest?.optString("name")?.ifBlank { null }
            ?: manifest?.optString("title")?.ifBlank { null }
            ?: livelyInfo?.optString("Title")?.ifBlank { null }
            ?: extractHtmlTitle(File(destDir, entryRel))
            ?: fallbackName).take(60)
        val desc = manifest?.optString("desc")?.ifBlank { null }
            ?: manifest?.optString("description")?.ifBlank { null }
            ?: livelyInfo?.optString("Desc")?.ifBlank { null }
            ?: "Imported HTML/Canvas theme"
        val adaptive = manifest?.optBoolean("adaptive", true) ?: true
        val thumbSpec = listOf("thumb", "thumbnail", "preview")
            .firstNotNullOfOrNull { k -> manifest?.optString(k)?.trim()?.ifBlank { null } }
            ?: livelyInfo?.optString("Thumbnail")?.trim()?.ifBlank { null }
        val thumbCss = findArchiveThumb(destDir, id, thumbSpec)
        val sanitizedGroups = sanitizeGroups(manifest?.optJSONArray("groups"))
        val groups = when {
            sanitizedGroups.length() > 0 -> sanitizedGroups
            manifest?.opt("settings") != null -> groupsFromSettings(manifest.opt("settings"))
            livelyProps != null -> groupsFromSettings(livelyProps)
            else -> JSONArray()
        }

        return buildEntry(
            id = id,
            name = name,
            desc = desc,
            page = "__custom/$id/$entryRel",
            adaptive = adaptive,
            groups = groups,
            thumb = thumbCss
        )
    }

    private fun findArchiveThumb(destDir: File, id: String, manifestThumb: String?): String {
        if (!manifestThumb.isNullOrBlank()) {
            val trimmed = manifestThumb.trim()
            if (trimmed.startsWith("radial-gradient", true) ||
                trimmed.startsWith("linear-gradient", true) ||
                trimmed.startsWith("#")
            ) {
                return trimmed
            }
            val rel = trimmed.trimStart('/')
            val destCanonical = destDir.canonicalFile
            val cand = File(destCanonical, rel).canonicalFile
            if (cand.path.startsWith(destCanonical.path + File.separator) && cand.isFile) {
                return "url(__custom/$id/$rel) center / cover no-repeat"
            }
        }
        val candidates = listOf(
            "thumb.webp", "thumb.png", "thumb.jpg", "thumb.svg",
            "preview.webp", "preview.png", "preview.jpg", "preview.svg"
        )
        for (name in candidates) {
            if (File(destDir, name).isFile) {
                return "url(__custom/$id/$name) center / cover no-repeat"
            }
        }
        return DEFAULT_ARCHIVE_THUMB
    }

    private fun sanitizeColor(raw: Any?): String {
        val s = raw?.toString()?.trim() ?: ""
        if (Regex("^#[0-9a-fA-F]{6}$").matches(s)) return s.lowercase(Locale.US)
        if (Regex("^#[0-9a-fA-F]{3}$").matches(s)) {
            return "#${s[1]}${s[1]}${s[2]}${s[2]}${s[3]}${s[3]}".lowercase(Locale.US)
        }
        return "#4a90e2"
    }

    private fun sanitizeGroups(rawGroups: JSONArray?): JSONArray {
        val out = JSONArray()
        if (rawGroups == null) return out
        for (gi in 0 until rawGroups.length()) {
            val g = rawGroups.optJSONObject(gi) ?: continue
            val rawParams = g.optJSONArray("params") ?: continue
            val cleanParams = JSONArray()
            for (pi in 0 until rawParams.length()) {
                val p = rawParams.optJSONObject(pi) ?: continue
                val key = p.optString("key").trim()
                if (key.isEmpty() || !KEY_REGEX.matches(key)) continue
                val label = p.optString("label").trim().ifBlank { key }.take(48)
                val item = JSONObject().apply {
                    put("key", key)
                    put("label", label)
                }
                val hint = p.optString("hint").trim()
                if (hint.isNotEmpty()) item.put("hint", hint.take(180))

                if (p.optString("type").trim().lowercase(Locale.US) == "color") {
                    item.put("type", "color")
                    val cDef = if (p.has("default")) p.opt("default") else p.opt("value")
                    item.put("default", sanitizeColor(cDef))
                    cleanParams.put(item)
                    continue
                }

                val rawOpts = p.optJSONArray("options")
                if (rawOpts != null && rawOpts.length() > 0) {
                    val opts = JSONArray()
                    val vals = JSONArray()
                    var hasObjOpts = false
                    val limit = minOf(8, rawOpts.length())
                    for (oi in 0 until limit) {
                        val o = rawOpts.opt(oi)
                        if (o is JSONObject) {
                            hasObjOpts = true
                            val vv = if (o.has("value")) o.opt("value") else o.opt("label")
                            val lbl = o.optString("label").ifBlank { vv?.toString() ?: "" }.take(48)
                            opts.put(lbl)
                            vals.put(vv)
                        } else if (o != null) {
                            opts.put(o.toString().take(48))
                        }
                    }
                    val rawVals = p.optJSONArray("values")
                    if (!hasObjOpts && rawVals != null && rawVals.length() >= opts.length()) {
                        for (vi in 0 until opts.length()) vals.put(rawVals.opt(vi))
                    }
                    if (opts.length() == 0) continue
                    item.put("options", opts)
                    val rawDef = if (p.has("default")) p.opt("default") else null
                    if (vals.length() > 0) {
                        item.put("values", vals)
                        var matched: Any? = vals.opt(0)
                        var found = false
                        if (rawDef != null && rawDef !== JSONObject.NULL) {
                            for (vi in 0 until vals.length()) {
                                val v = vals.opt(vi)
                                if (v == rawDef || v?.toString() == rawDef.toString()) {
                                    matched = v
                                    found = true
                                    break
                                }
                            }
                            if (!found && rawDef is String) {
                                for (oi in 0 until opts.length()) {
                                    if (opts.optString(oi) == rawDef) {
                                        matched = vals.opt(oi)
                                        found = true
                                        break
                                    }
                                }
                            }
                            if (!found && rawDef is Number) {
                                val idx = rawDef.toInt().coerceIn(0, vals.length() - 1)
                                matched = vals.opt(idx)
                            }
                        }
                        item.put("default", matched)
                    } else {
                        val defIdx = (rawDef as? Number)?.toInt()?.coerceIn(0, opts.length() - 1) ?: 0
                        item.put("default", defIdx)
                    }
                    cleanParams.put(item)
                } else {
                    val pMin = p.optDouble("min", 0.0)
                    val pMax = p.optDouble("max", 1.0)
                    val pStep = p.optDouble("step", 0.01)
                    val pDef = p.optDouble("default", pMin)
                    if (pMin.isNaN() || pMax.isNaN() || pStep.isNaN() || pMax <= pMin || pStep <= 0.0) continue
                    item.put("min", pMin)
                    item.put("max", pMax)
                    item.put("step", pStep)
                    item.put("default", pDef.coerceIn(pMin, pMax))
                    val fmt = p.optString("format")
                    if (fmt == "times" || fmt == "percent") {
                        item.put("format", fmt)
                    } else {
                        val unit = p.optString("unit").trim()
                        if (unit.isNotEmpty()) item.put("unit", unit.take(12))
                    }
                    cleanParams.put(item)
                }
            }
            if (cleanParams.length() > 0) {
                val gName = g.optString("name").trim().ifBlank { "Settings" }.take(48)
                out.put(JSONObject().apply {
                    put("name", gName)
                    put("params", cleanParams)
                })
            }
        }
        return out
    }

    private fun groupsFromSettings(rawSettings: Any?): JSONArray {
        val items = mutableListOf<Pair<String, JSONObject>>()
        when (rawSettings) {
            is JSONObject -> {
                val keys = rawSettings.keys()
                while (keys.hasNext()) {
                    val k = keys.next()
                    val spec = rawSettings.optJSONObject(k) ?: continue
                    val key = spec.optString("key").ifBlank { k }
                    items.add(key to spec)
                }
            }
            is JSONArray -> {
                for (i in 0 until rawSettings.length()) {
                    val spec = rawSettings.optJSONObject(i) ?: continue
                    val key = spec.optString("key").trim()
                    if (key.isEmpty()) continue
                    items.add(key to spec)
                }
            }
        }
        if (items.isEmpty()) return JSONArray()

        val grouped = linkedMapOf<String, JSONArray>()
        for ((key, spec) in items) {
            val groupName = spec.optString("group").trim()
                .ifBlank { spec.optString("category").trim() }
                .ifBlank { "Settings" }
            val ptype = spec.optString("type").trim().lowercase(Locale.US)
            if (ptype in setOf("button", "label", "folderdropdown", "file")) continue
            val param = JSONObject().apply {
                put("key", key)
                put("label", spec.optString("label").ifBlank { spec.optString("title").ifBlank { spec.optString("text").ifBlank { key } } })
            }
            val hint = spec.optString("hint").ifBlank { spec.optString("description") }.trim()
            if (hint.isNotEmpty()) param.put("hint", hint)

            val hasDef = spec.has("default") || spec.has("value")
            val rawDef = if (spec.has("default")) spec.opt("default") else spec.opt("value")

            when {
                ptype == "color" -> {
                    param.put("type", "color")
                    param.put("default", sanitizeColor(rawDef))
                }
                ptype in setOf("checkbox", "boolean", "bool", "toggle") -> {
                    param.put("options", JSONArray(listOf("Off", "On")))
                    param.put("values", JSONArray(listOf(false, true)))
                    val b = when (rawDef) {
                        is Boolean -> rawDef
                        is Number -> rawDef.toDouble() != 0.0
                        is String -> rawDef.trim().lowercase(Locale.US) in setOf("1", "true", "yes", "on")
                        else -> false
                    }
                    param.put("default", b)
                }
                (spec.optJSONArray("options")?.takeIf { it.length() > 0 } != null) ||
                    (ptype in setOf("dropdown", "select") && spec.optJSONArray("items")?.takeIf { it.length() > 0 } != null) -> {
                    param.put("options", spec.optJSONArray("options")?.takeIf { it.length() > 0 } ?: spec.optJSONArray("items"))
                    if (hasDef) param.put("default", rawDef)
                }
                else -> {
                    val pMin = spec.optDouble("min", 0.0)
                    param.put("min", pMin)
                    param.put("max", spec.optDouble("max", 1.0))
                    param.put("step", spec.optDouble("step", 0.01))
                    param.put("default", if (rawDef is Number) rawDef.toDouble() else pMin)
                    val fmt = spec.optString("format")
                    if (fmt == "times" || fmt == "percent") {
                        param.put("format", fmt)
                    } else {
                        val unit = spec.optString("unit").trim()
                        if (unit.isNotEmpty()) param.put("unit", unit)
                    }
                }
            }
            grouped.getOrPut(groupName) { JSONArray() }.put(param)
        }

        val rawGroups = JSONArray()
        for ((gName, params) in grouped) {
            rawGroups.put(JSONObject().apply {
                put("name", gName)
                put("params", params)
            })
        }
        return sanitizeGroups(rawGroups)
    }

    private fun importSingleHtml(input: InputStream, destDir: File, id: String, fallbackName: String): JSONObject {
        val outFile = File(destDir, "index.html")
        copyLimited(input, outFile, MAX_ARCHIVE_BYTES)
        val title = extractHtmlTitle(outFile) ?: fallbackName
        return buildEntry(
            id = id,
            name = title,
            desc = "Imported HTML wallpaper",
            page = "__custom/$id/index.html",
            adaptive = true,
            groups = JSONArray()
        )
    }

    private fun importImageWallpaper(
        input: InputStream,
        cleanName: String,
        destDir: File,
        id: String,
        fallbackName: String
    ): JSONObject {
        val ext = cleanName.substringAfterLast('.', "png").lowercase(Locale.US)
        val mediaFile = File(destDir, "media.$ext")
        copyLimited(input, mediaFile, MAX_ARCHIVE_BYTES)
        File(destDir, "index.html").writeText(buildImageWrapperHtml("media.$ext", fallbackName), Charsets.UTF_8)
        val groups = JSONArray().apply {
            put(JSONObject().apply {
                put("name", "Image")
                put("params", JSONArray().apply {
                    put(slider("brightness", "Brightness", 0.25, 1.35, 0.05, 1.0))
                    put(slider("motion", "Ambient Drift", 0.0, 1.0, 0.05, 0.35))
                    put(slider("dimIcons", "Dim Under Icons", 0.0, 0.6, 0.05, 0.15))
                })
            })
        }
        return buildEntry(
            id = id,
            name = fallbackName,
            desc = "Imported image wallpaper",
            page = "__custom/$id/index.html",
            adaptive = true,
            groups = groups,
            thumb = "url(__custom/$id/media.$ext) center / cover no-repeat"
        )
    }

    private fun importVideoWallpaper(
        input: InputStream,
        cleanName: String,
        destDir: File,
        id: String,
        fallbackName: String
    ): JSONObject {
        val ext = cleanName.substringAfterLast('.', "mp4").lowercase(Locale.US)
        val mediaFile = File(destDir, "media.$ext")
        copyLimited(input, mediaFile, MAX_ARCHIVE_BYTES)
        File(destDir, "index.html").writeText(buildVideoWrapperHtml("media.$ext", fallbackName), Charsets.UTF_8)
        val groups = JSONArray().apply {
            put(JSONObject().apply {
                put("name", "Video")
                put("params", JSONArray().apply {
                    put(slider("brightness", "Brightness", 0.25, 1.35, 0.05, 1.0))
                    put(slider("speed", "Playback Speed", 0.25, 2.0, 0.05, 1.0))
                    put(slider("dimIcons", "Dim Under Icons", 0.0, 0.6, 0.05, 0.15))
                })
            })
        }
        return buildEntry(
            id = id,
            name = fallbackName,
            desc = "Imported video wallpaper",
            page = "__custom/$id/index.html",
            adaptive = true,
            groups = groups,
            thumb = DEFAULT_VIDEO_THUMB
        )
    }

    private fun buildEntry(
        id: String,
        name: String,
        desc: String,
        page: String,
        adaptive: Boolean,
        groups: JSONArray,
        thumb: String = DEFAULT_ARCHIVE_THUMB
    ): JSONObject = JSONObject().apply {
        put("id", id)
        put("name", name)
        put("desc", desc)
        put("page", page)
        put("status", "ready")
        put("product", "free")
        put("adaptive", adaptive)
        put("imported", true)
        put("thumb", thumb)
        if (groups.length() > 0) put("groups", groups)
    }

    private fun slider(key: String, label: String, min: Double, max: Double, step: Double, def: Double): JSONObject =
        JSONObject().apply {
            put("key", key)
            put("label", label)
            put("min", min)
            put("max", max)
            put("step", step)
            put("default", def)
        }

    private fun copyLimited(input: InputStream, dest: File, maxBytes: Long) {
        val buffer = ByteArray(16384)
        var total = 0L
        FileOutputStream(dest).use { fos ->
            while (true) {
                val n = input.read(buffer)
                if (n <= 0) break
                total += n
                if (total > maxBytes) {
                    throw IllegalArgumentException("File exceeds 150 MB limit")
                }
                fos.write(buffer, 0, n)
            }
        }
    }

    private fun extractHtmlTitle(htmlFile: File): String? {
        if (!htmlFile.isFile) return null
        val head = htmlFile.bufferedReader(Charsets.UTF_8).use { r ->
            val buf = CharArray(4096)
            val n = r.read(buf)
            if (n > 0) String(buf, 0, n) else ""
        }
        val m = Regex("<title[^>]*>(.*?)</title>", setOf(RegexOption.IGNORE_CASE, RegexOption.DOT_MATCHES_ALL)).find(head)
        return m?.groupValues?.get(1)?.replace(Regex("\\s+"), " ")?.trim()?.ifBlank { null }
    }

    private fun slugify(s: String): String =
        s.lowercase(Locale.US).replace(Regex("[^a-z0-9]+"), "-").trim('-').ifBlank { "theme" }

    private fun escHtml(s: String): String = s
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")

    private fun buildImageWrapperHtml(src: String, title: String): String = """
        <!doctype html><html><head><meta charset="utf-8"><title>${escHtml(title)}</title>
        <style>html,body,canvas{margin:0;width:100%;height:100%;overflow:hidden;background:#0e1714;display:block}</style>
        </head><body><canvas id="c"></canvas><script>
        const c=document.getElementById('c'),x=c.getContext('2d'),img=new Image();img.src=${JSONObject.quote(src)};
        let P={brightness:1,motion:0.35,dimIcons:0.15},icons=[],paused=false,t0=performance.now();
        function fit(){const d=devicePixelRatio||1;c.width=innerWidth*d;c.height=innerHeight*d;}
        addEventListener('resize',fit);fit();
        function draw(now){
          if(!paused&&img.complete&&img.naturalWidth){
            const W=c.width,H=c.height,t=(now-t0)*0.0004*P.motion;
            const sc=Math.max(W/img.naturalWidth,H/img.naturalHeight)*(1+0.03*P.motion);
            const dw=img.naturalWidth*sc,dh=img.naturalHeight*sc;
            const ox=(W-dw)/2+Math.sin(t)*W*0.012*P.motion,oy=(H-dh)/2+Math.cos(t*0.8)*H*0.012*P.motion;
            x.save();x.filter='brightness('+P.brightness+')';x.drawImage(img,ox,oy,dw,dh);x.restore();
            if(P.dimIcons>0&&icons.length){
              x.fillStyle='rgba(0,0,0,'+P.dimIcons+')';
              for(const r of icons)x.fillRect(r[0],r[1],r[2]-r[0],r[3]-r[1]);
            }
          }
          requestAnimationFrame(draw);
        }
        requestAnimationFrame(draw);
        const h=window.chrome&&window.chrome.webview;
        if(h){h.addEventListener('message',e=>{const m=JSON.parse(e.data);
          if(m.t==='settings'&&m.params)Object.assign(P,m.params);
          else if(m.t==='icons')icons=m.vis&&m.r?m.r:[];
          else if(m.t==='pause')paused=!!m.v;
        });h.postMessage('ready');}
        </script></body></html>
    """.trimIndent()

    private fun buildVideoWrapperHtml(src: String, title: String): String = """
        <!doctype html><html><head><meta charset="utf-8"><title>${escHtml(title)}</title>
        <style>html,body,video{margin:0;width:100%;height:100%;object-fit:cover;overflow:hidden;background:#0e1714;display:block}
        canvas{position:fixed;inset:0;width:100%;height:100%;pointer-events:none}</style></head>
        <body><video id="v" src=${JSONObject.quote(src)} autoplay loop muted playsinline></video><canvas id="ov"></canvas><script>
        const v=document.getElementById('v'),ov=document.getElementById('ov'),ox=ov.getContext('2d');
        let P={brightness:1,speed:1,dimIcons:0.15},icons=[];
        function fit(){const d=devicePixelRatio||1;ov.width=innerWidth*d;ov.height=innerHeight*d;drawIcons();}
        function drawIcons(){ox.clearRect(0,0,ov.width,ov.height);if(P.dimIcons>0&&icons.length){ox.fillStyle='rgba(0,0,0,'+P.dimIcons+')';for(const r of icons)ox.fillRect(r[0],r[1],r[2]-r[0],r[3]-r[1]);}}
        addEventListener('resize',fit);fit();
        function apply(){v.style.filter='brightness('+P.brightness+')';v.playbackRate=P.speed||1;drawIcons();}
        const h=window.chrome&&window.chrome.webview;
        if(h){h.addEventListener('message',e=>{const m=JSON.parse(e.data);
          if(m.t==='settings'&&m.params){Object.assign(P,m.params);apply();}
          else if(m.t==='icons'){icons=m.vis&&m.r?m.r:[];drawIcons();}
          else if(m.t==='pause'){if(m.v)v.pause();else v.play().catch(()=>{});}
        });h.postMessage('ready');}
        </script></body></html>
    """.trimIndent()

    companion object {
        const val MAX_ARCHIVE_BYTES = 150L * 1024L * 1024L
        const val MAX_ARCHIVE_FILES = 2000
        private val KEY_REGEX = Regex("^[a-zA-Z0-9_-]+$")
        private const val DEFAULT_ARCHIVE_THUMB =
            "radial-gradient(circle at 28% 35%, rgba(120,210,255,0.45) 0%, transparent 45%), " +
                "radial-gradient(circle at 75% 70%, rgba(180,120,255,0.38) 0%, transparent 48%), " +
                "linear-gradient(145deg, #1b2738 0%, #243b55 55%, #16222f 100%)"
        private const val DEFAULT_VIDEO_THUMB =
            "radial-gradient(circle at 32% 38%, rgba(255,170,95,0.45) 0%, transparent 48%), " +
                "linear-gradient(150deg, #1e2130 0%, #342f48 55%, #1b1d29 100%)"
        private val IMAGE_EXTS = setOf("png", "jpg", "jpeg", "webp", "gif", "svg", "avif", "bmp")
        private val VIDEO_EXTS = setOf("mp4", "webm", "mkv", "mov")
        private val SUPPORTED_EXTS = setOf("zip", "deskland", "html", "htm", "glsl", "frag") + IMAGE_EXTS + VIDEO_EXTS
        private val ARCHIVE_ALLOWED_EXTS = setOf(
            "html", "htm", "css", "js", "mjs", "json",
            "glsl", "vert", "frag", "wgsl", "wasm",
            "txt", "md", "xml", "csv", "map",
            "png", "jpg", "jpeg", "webp", "gif", "svg", "avif", "bmp", "ico",
            "mp4", "webm", "mkv", "mov", "ogv",
            "mp3", "wav", "ogg", "opus", "m4a", "aac", "flac",
            "woff", "woff2", "ttf", "otf", "eot",
            "obj", "mtl", "gltf", "glb", "bin", "hdr", "exr", "ktx", "ktx2", "dds", "ply", "stl", "fbx"
        )
        private val ARCHIVE_ALLOWED_BASENAMES = setOf("license", "notice", "readme", "copying", "authors", "changelog")

        fun isAllowedArchiveMember(fileName: String): Boolean {
            if (fileName.isBlank()) return false
            val dot = fileName.lastIndexOf('.')
            if (dot > 0 && dot < fileName.length - 1) {
                val ext = fileName.substring(dot + 1).lowercase(Locale.US)
                return ext in ARCHIVE_ALLOWED_EXTS
            }
            return fileName.lowercase(Locale.US) in ARCHIVE_ALLOWED_BASENAMES
        }

        /** Inspects ZIP Central Directory Unix external attributes for symbolic links (S_IFLNK = 0o120000). */
        fun containsZipSymlink(zipFile: File): Boolean {
            val length = zipFile.length()
            if (length < 22L) return false
            RandomAccessFile(zipFile, "r").use { raf ->
                val tailLen = minOf(length, 65557L).toInt()
                val tail = ByteArray(tailLen)
                raf.seek(length - tailLen)
                raf.readFully(tail)
                var eocdOffset = -1
                for (i in tailLen - 22 downTo 0) {
                    if (tail[i] == 0x50.toByte() && tail[i + 1] == 0x4B.toByte() &&
                        tail[i + 2] == 0x05.toByte() && tail[i + 3] == 0x06.toByte()
                    ) {
                        eocdOffset = i
                        break
                    }
                }
                if (eocdOffset < 0) return false
                val eocd = ByteBuffer.wrap(tail, eocdOffset, 22).slice().order(ByteOrder.LITTLE_ENDIAN)
                val totalEntries = eocd.getShort(10).toInt() and 0xFFFF
                val cdOffset = eocd.getInt(16).toLong() and 0xFFFFFFFFL
                if (cdOffset < 0 || cdOffset >= length) return false
                raf.seek(cdOffset)
                val hdr = ByteArray(46)
                for (i in 0 until totalEntries) {
                    if (raf.filePointer + 46 > length) break
                    raf.readFully(hdr)
                    val bb = ByteBuffer.wrap(hdr).order(ByteOrder.LITTLE_ENDIAN)
                    if (bb.getInt(0) != 0x02014B50) break
                    val nameLen = bb.getShort(28).toInt() and 0xFFFF
                    val extraLen = bb.getShort(30).toInt() and 0xFFFF
                    val commentLen = bb.getShort(32).toInt() and 0xFFFF
                    val extAttrs = bb.getInt(38)
                    val unixType = (extAttrs ushr 16) and 0xF000
                    if (unixType == 0xA000) return true
                    val skip = (nameLen + extraLen + commentLen).toLong()
                    if (raf.filePointer + skip > length) break
                    raf.seek(raf.filePointer + skip)
                }
            }
            return false
        }

        fun guessMimeType(name: String): String {
            return when (name.substringAfterLast('.', "").lowercase(Locale.US)) {
                "html", "htm" -> "text/html; charset=utf-8"
                "js", "mjs" -> "application/javascript; charset=utf-8"
                "css" -> "text/css; charset=utf-8"
                "json" -> "application/json; charset=utf-8"
                "svg" -> "image/svg+xml"
                "png" -> "image/png"
                "jpg", "jpeg" -> "image/jpeg"
                "webp" -> "image/webp"
                "gif" -> "image/gif"
                "avif" -> "image/avif"
                "bmp" -> "image/bmp"
                "mp3" -> "audio/mpeg"
                "ogg" -> "audio/ogg"
                "wav" -> "audio/wav"
                "mp4" -> "video/mp4"
                "webm" -> "video/webm"
                "mov" -> "video/quicktime"
                "mkv" -> "video/x-matroska"
                "woff2" -> "font/woff2"
                "woff" -> "font/woff"
                "ttf" -> "font/ttf"
                else -> "application/octet-stream"
            }
        }
    }
}
