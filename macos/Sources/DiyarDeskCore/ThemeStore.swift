import Foundation

public struct MacSettings: Codable {
    public var theme: String = "papercut"
    public var lang: String = "auto"
    public var lowMode: Bool = false
    public var disableAutoPause: Bool = false
    public var displayMode: String = "primary"
    public var virtualBarMode: String = "auto"
    public var solarSync: Bool = false
    public var windowAware: Bool = false
    public var playlistEnabled: Bool = false
    public var playlistIntervalMin: Int = 30

    public init() {}
}

public final class ThemeStore {
    public let dataDir: URL
    public let prototypeDir: URL
    public let customThemesDir: URL
    public let settingsURL: URL
    public let customRegistryURL: URL

    public var settings: MacSettings = MacSettings()
    public var params: [String: [String: Any]] = [:]

    public init(dataDir: URL, prototypeDir: URL) {
        self.dataDir = dataDir
        self.prototypeDir = prototypeDir
        self.customThemesDir = dataDir.appendingPathComponent("themes", isDirectory: true)
        self.settingsURL = dataDir.appendingPathComponent("settings.json")
        self.customRegistryURL = customThemesDir.appendingPathComponent("registry.json")
        try? FileManager.default.createDirectory(at: dataDir, withIntermediateDirectories: true)
        try? FileManager.default.createDirectory(at: customThemesDir, withIntermediateDirectories: true)
        load()
    }

    public func load() {
        guard let data = try? Data(contentsOf: settingsURL),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return
        }
        if let v = root["theme"] as? String, !v.isEmpty { settings.theme = v }
        if let v = root["lang"] as? String, !v.isEmpty { settings.lang = v }
        if let v = root["lowMode"] as? Bool { settings.lowMode = v }
        if let v = root["disableAutoPause"] as? Bool { settings.disableAutoPause = v }
        if let v = root["displayMode"] as? String, ["primary", "independent", "span", "duplicate"].contains(v) {
            settings.displayMode = v
        }
        if let v = root["virtualBarMode"] as? String, ["auto", "always", "off"].contains(v) {
            settings.virtualBarMode = v
        }
        if let v = root["solarSync"] as? Bool { settings.solarSync = v }
        if let v = root["windowAware"] as? Bool { settings.windowAware = v }
        if let v = root["playlistEnabled"] as? Bool { settings.playlistEnabled = v }
        if let v = root["playlistIntervalMin"] as? Int { settings.playlistIntervalMin = max(1, min(1440, v)) }
        if let p = root["params"] as? [String: [String: Any]] {
            params = p
        }
    }

    public func save() {
        let obj: [String: Any] = [
            "version": 1,
            "theme": settings.theme,
            "lang": settings.lang,
            "lowMode": settings.lowMode,
            "disableAutoPause": settings.disableAutoPause,
            "displayMode": settings.displayMode,
            "virtualBarMode": settings.virtualBarMode,
            "solarSync": settings.solarSync,
            "windowAware": settings.windowAware,
            "playlistEnabled": settings.playlistEnabled,
            "playlistIntervalMin": settings.playlistIntervalMin,
            "params": params
        ]
        if let data = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted]) {
            try? FileManager.default.createDirectory(at: dataDir, withIntermediateDirectories: true)
            let tmp = settingsURL.appendingPathExtension("tmp")
            try? data.write(to: tmp, options: .atomic)
            _ = try? FileManager.default.replaceItemAt(settingsURL, withItemAt: tmp)
        }
    }

    public func paramsFor(_ themeId: String) -> [String: Any] {
        return params[themeId] ?? [:]
    }

    public func setParam(themeId: String, key: String, value: Any) {
        var dict = params[themeId] ?? [:]
        dict[key] = value
        params[themeId] = dict
        save()
    }

    public func resetParams(themeId: String) {
        params.removeValue(forKey: themeId)
        save()
    }

    public func loadBuiltInThemes() -> [[String: Any]] {
        let url = prototypeDir.appendingPathComponent("themes.json")
        guard let data = try? Data(contentsOf: url),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let list = root["themes"] as? [[String: Any]] else {
            return []
        }
        return list
    }

    public func loadCustomThemes() -> [[String: Any]] {
        guard let data = try? Data(contentsOf: customRegistryURL),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let list = root["themes"] as? [[String: Any]] else {
            return []
        }
        return list
    }

    public func saveCustomThemes(_ list: [[String: Any]]) {
        let obj: [String: Any] = ["themes": list]
        if let data = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted]) {
            try? FileManager.default.createDirectory(at: customThemesDir, withIntermediateDirectories: true)
            try? data.write(to: customRegistryURL, options: .atomic)
        }
    }

    public func mergedThemesJSON() -> Data {
        var seen = Set<String>()
        var merged: [[String: Any]] = []
        for item in loadBuiltInThemes() + loadCustomThemes() {
            guard let id = item["id"] as? String, !seen.contains(id) else { continue }
            seen.insert(id)
            merged.append(item)
        }
        let payload: [String: Any] = ["version": 1, "themes": merged]
        return (try? JSONSerialization.data(withJSONObject: payload)) ?? Data("{\"themes\":[]}".utf8)
    }

    public func readyThemeIds() -> [String] {
        var out: [String] = []
        for item in loadBuiltInThemes() + loadCustomThemes() {
            if let id = item["id"] as? String, (item["status"] as? String) == "ready" {
                out.append(id)
            }
        }
        return out
    }

    public func pageFor(themeId: String) -> String {
        for item in loadBuiltInThemes() + loadCustomThemes() {
            if (item["id"] as? String) == themeId, let page = item["page"] as? String, !page.isEmpty {
                return page
            }
        }
        return "papercut.html"
    }

    public func removeCustomTheme(id: String) -> Bool {
        guard id.hasPrefix("custom-") else { return false }
        let current = loadCustomThemes()
        let filtered = current.filter { ($0["id"] as? String) != id }
        if filtered.count == current.count { return false }
        saveCustomThemes(filtered)
        let dir = customThemesDir.appendingPathComponent(id, isDirectory: true)
        try? FileManager.default.removeItem(at: dir)
        params.removeValue(forKey: id)
        if settings.theme == id {
            settings.theme = readyThemeIds().first ?? "papercut"
        }
        save()
        return true
    }

    public func buildShaderWrapperHTML(title: String, glslSource: String) -> String {
        let safeTitle = title
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
        let encodedData = (try? JSONSerialization.data(withJSONObject: [glslSource])) ?? Data("[\"\"]".utf8)
        let encodedArr = String(data: encodedData, encoding: .utf8) ?? "[\"\"]"
        return """
        <!doctype html>
        <html lang="en"><head><meta charset="utf-8"><title>\(safeTitle)</title>
        <style>html,body,canvas{margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#090c14;display:block;}</style>
        </head><body><canvas id="gl"></canvas><script>
        (() => {
          const canvas = document.getElementById('gl');
          const gl = canvas.getContext('webgl', { preserveDrawingBuffer: true, antialias: false });
          if (!gl) return;
          const rawFrag = (\(encodedArr))[0];
          const hasMainImage = /void\\s+mainImage\\s*\\(/.test(rawFrag);
          const header = 'precision highp float;\\nuniform float u_time;\\nuniform vec2 u_resolution;\\nuniform vec2 u_mouse;\\nuniform float u_speed;\\nuniform float u_brightness;\\nuniform float u_audio;\\nuniform int u_iconCount;\\nuniform vec4 u_icons[32];\\nuniform vec3 iResolution;\\nuniform float iTime;\\nuniform vec4 iMouse;\\n';
          const stripped = rawFrag.replace(/^\\s*precision\\s+\\w+\\s+float\\s*;/gm, '');
          const fragSrc = header + stripped + (hasMainImage ? '\\nvoid main(){ vec4 c=vec4(0.0); mainImage(c, gl_FragCoord.xy); gl_FragColor=vec4(c.rgb*u_brightness, 1.0); }\\n' : '');
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
        </script></body></html>
        """
    }

    public func importFile(at sourceURL: URL) throws -> [String: Any] {
        let ext = sourceURL.pathExtension.lowercased()
        let attrs = try FileManager.default.attributesOfItem(atPath: sourceURL.path)
        let fileSize = (attrs[.size] as? NSNumber)?.int64Value ?? 0
        if fileSize <= 0 || fileSize > DiyarSecurity.maxArchiveBytes {
            throw NSError(domain: "DiyarDesk", code: 1, userInfo: [NSLocalizedDescriptionKey: "File is empty or exceeds 150 MB limit."])
        }

        let stem = sourceURL.deletingPathExtension().lastPathComponent
        let slug = stem.lowercased().replacingOccurrences(of: "[^a-z0-9]+", with: "-", options: .regularExpression).trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        let id = "custom-\(slug.isEmpty ? "theme" : String(slug.prefix(24)))-\(String(UUID().uuidString.prefix(8)).lowercased())"
        let destDir = customThemesDir.appendingPathComponent(id, isDirectory: true)
        try FileManager.default.createDirectory(at: destDir, withIntermediateDirectories: true)

        do {
            let entry: [String: Any]
            if DiyarSecurity.htmlExts.contains(ext) {
                let raw = try Data(contentsOf: sourceURL)
                if raw.contains(0) { throw NSError(domain: "DiyarDesk", code: 2, userInfo: [NSLocalizedDescriptionKey: "Invalid HTML file."]) }
                try raw.write(to: destDir.appendingPathComponent("index.html"))
                entry = makeEntry(id: id, name: stem, desc: "Imported HTML wallpaper.", page: "__custom/\(id)/index.html", adaptive: true, groups: [])
            } else if DiyarSecurity.shaderExts.contains(ext) {
                let glsl = try String(contentsOf: sourceURL, encoding: .utf8)
                try glsl.write(to: destDir.appendingPathComponent("shader.glsl"), atomically: true, encoding: .utf8)
                let html = buildShaderWrapperHTML(title: stem, glslSource: glsl)
                try html.write(to: destDir.appendingPathComponent("index.html"), atomically: true, encoding: .utf8)
                let groups: [[String: Any]] = [[
                    "name": "Shader",
                    "params": [
                        ["key": "speed", "label": "Speed", "min": 0.1, "max": 3.0, "step": 0.05, "default": 1.0, "format": "times"],
                        ["key": "brightness", "label": "Brightness", "min": 0.4, "max": 1.4, "step": 0.01, "default": 1.0, "format": "percent"]
                    ]
                ]]
                entry = makeEntry(id: id, name: stem, desc: "Imported GLSL WebGL fragment shader.", page: "__custom/\(id)/index.html", adaptive: true, groups: groups)
            } else if DiyarSecurity.imageExts.contains(ext) || DiyarSecurity.videoExts.contains(ext) {
                let isVideo = DiyarSecurity.videoExts.contains(ext)
                let mediaName = "media.\(ext)"
                try FileManager.default.copyItem(at: sourceURL, to: destDir.appendingPathComponent(mediaName))
                let mediaTag = isVideo
                    ? "<video id=\"m\" src=\"\(mediaName)\" autoplay loop muted playsinline style=\"width:100%;height:100%;object-fit:cover\"></video>"
                    : "<img id=\"m\" src=\"\(mediaName)\" style=\"width:100%;height:100%;object-fit:cover\">"
                let html = "<!doctype html><html><head><meta charset=\"utf-8\"><style>html,body{margin:0;width:100%;height:100%;overflow:hidden;background:#000}</style></head><body>\(mediaTag)<script>if(window.chrome&&window.chrome.webview)window.chrome.webview.postMessage('ready');else if(parent!==window)parent.postMessage(JSON.stringify({t:'ready'}),'*');</script></body></html>"
                try html.write(to: destDir.appendingPathComponent("index.html"), atomically: true, encoding: .utf8)
                entry = makeEntry(id: id, name: stem, desc: isVideo ? "Imported video wallpaper." : "Imported image wallpaper.", page: "__custom/\(id)/index.html", adaptive: false, groups: [])
            } else if DiyarSecurity.archiveExts.contains(ext) {
                entry = try importArchive(sourceURL: sourceURL, destDir: destDir, id: id, fallbackName: stem)
            } else {
                throw NSError(domain: "DiyarDesk", code: 3, userInfo: [NSLocalizedDescriptionKey: "Unsupported theme format: .\(ext)"])
            }
            var list = loadCustomThemes()
            list.append(entry)
            saveCustomThemes(list)
            return entry
        } catch {
            try? FileManager.default.removeItem(at: destDir)
            throw error
        }
    }

    private func importArchive(sourceURL: URL, destDir: URL, id: String, fallbackName: String) throws -> [String: Any] {
        // Inspect archive entries with `/usr/bin/unzip -Z1` before extracting to enforce security policy
        let listProc = Process()
        listProc.executableURL = URL(fileURLWithPath: "/usr/bin/unzip")
        listProc.arguments = ["-Z1", sourceURL.path]
        let listPipe = Pipe()
        listProc.standardOutput = listPipe
        listProc.standardError = Pipe()
        try listProc.run()
        listProc.waitUntilExit()
        if listProc.terminationStatus != 0 {
            throw NSError(domain: "DiyarDesk", code: 4, userInfo: [NSLocalizedDescriptionKey: "Invalid or corrupted ZIP archive."])
        }
        let output = String(data: listPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        let lines = output.split(separator: "\n").map(String.init)
        var fileCount = 0
        for rawLine in lines {
            if rawLine.hasSuffix("/") { continue }
            if !DiyarSecurity.isSafeRelativePath(rawLine) {
                throw NSError(domain: "DiyarDesk", code: 5, userInfo: [NSLocalizedDescriptionKey: "Unsafe archive entry path: \(rawLine)"])
            }
            if rawLine.hasPrefix("__MACOSX/") { continue }
            if !DiyarSecurity.isAllowedArchiveMember(rawLine) {
                throw NSError(domain: "DiyarDesk", code: 6, userInfo: [NSLocalizedDescriptionKey: "Unsupported file type in archive: \(rawLine)"])
            }
            fileCount += 1
            if fileCount > DiyarSecurity.maxArchiveFiles {
                throw NSError(domain: "DiyarDesk", code: 7, userInfo: [NSLocalizedDescriptionKey: "Archive exceeds 2,000 file limit."])
            }
        }

        let unzipProc = Process()
        unzipProc.executableURL = URL(fileURLWithPath: "/usr/bin/unzip")
        unzipProc.arguments = ["-qq", "-o", sourceURL.path, "-d", destDir.path]
        try unzipProc.run()
        unzipProc.waitUntilExit()

        // Reject any extracted symlinks
        let fm = FileManager.default
        if let enumerator = fm.enumerator(at: destDir, includingPropertiesForKeys: [.isSymbolicLinkKey]) {
            for case let fileURL as URL in enumerator {
                if let vals = try? fileURL.resourceValues(forKeys: [.isSymbolicLinkKey]), vals.isSymbolicLink == true {
                    throw NSError(domain: "DiyarDesk", code: 8, userInfo: [NSLocalizedDescriptionKey: "Archive contains disallowed symbolic link."])
                }
            }
        }

        // Flatten single wrapping folder if present
        let topItems = (try? fm.contentsOfDirectory(at: destDir, includingPropertiesForKeys: [.isDirectoryKey]))?.filter { $0.lastPathComponent != "__MACOSX" } ?? []
        if topItems.count == 1, let only = topItems.first, (try? only.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true {
            let children = (try? fm.contentsOfDirectory(at: only, includingPropertiesForKeys: nil)) ?? []
            for ch in children {
                try? fm.moveItem(at: ch, to: destDir.appendingPathComponent(ch.lastPathComponent))
            }
            try? fm.removeItem(at: only)
        }

        var manifest: [String: Any] = [:]
        for mf in ["manifest.json", "theme.json", "LivelyInfo.json"] {
            let url = destDir.appendingPathComponent(mf)
            if let d = try? Data(contentsOf: url), let obj = try? JSONSerialization.jsonObject(with: d) as? [String: Any] {
                manifest = obj
                break
            }
        }

        let entryRel = (manifest["entry"] as? String)
            ?? (manifest["page"] as? String)
            ?? (manifest["FileName"] as? String)
            ?? (fm.fileExists(atPath: destDir.appendingPathComponent("index.html").path) ? "index.html" : findFirstHTML(in: destDir))
        guard let finalEntry = entryRel, DiyarSecurity.isSafeRelativePath(finalEntry) else {
            throw NSError(domain: "DiyarDesk", code: 9, userInfo: [NSLocalizedDescriptionKey: "No HTML entry page found in archive."])
        }
        let name = (manifest["name"] as? String) ?? (manifest["title"] as? String) ?? (manifest["Title"] as? String) ?? fallbackName
        let desc = (manifest["desc"] as? String) ?? (manifest["description"] as? String) ?? (manifest["Desc"] as? String) ?? "Imported custom theme package."
        let adaptive = (manifest["adaptive"] as? Bool) ?? true
        return makeEntry(id: id, name: String(name.prefix(60)), desc: desc, page: "__custom/\(id)/\(finalEntry)", adaptive: adaptive, groups: [])
    }

    private func findFirstHTML(in root: URL) -> String? {
        let fm = FileManager.default
        guard let en = fm.enumerator(at: root, includingPropertiesForKeys: nil) else { return nil }
        for case let u as URL in en {
            let ext = u.pathExtension.lowercased()
            if ext == "html" || ext == "htm" {
                let rel = u.path.replacingOccurrences(of: root.path + "/", with: "")
                return rel
            }
        }
        return nil
    }

    private func makeEntry(id: String, name: String, desc: String, page: String, adaptive: Bool, groups: [[String: Any]]) -> [String: Any] {
        var obj: [String: Any] = [
            "id": id,
            "name": name,
            "desc": desc,
            "page": page,
            "status": "ready",
            "product": "free",
            "imported": true,
            "adaptive": adaptive,
            "thumb": "radial-gradient(circle at 30% 35%, rgba(98,195,255,0.45) 0%, transparent 48%), linear-gradient(150deg, #162231 0%, #243e54 55%, #172738 100%)"
        ]
        if !groups.isEmpty { obj["groups"] = groups }
        return obj
    }
}
