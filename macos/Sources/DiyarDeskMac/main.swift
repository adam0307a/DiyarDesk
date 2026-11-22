import Cocoa
import DiyarDeskCore

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var store: ThemeStore!
    private var schemeHandler: LocalSchemeHandler!
    private var wallpapers: [WallpaperWindow] = []
    private var settingsController: SettingsWindowController?
    private var statusItem: NSStatusItem!
    private let iconsReader = DesktopIconsReader()

    private var pollTimer: Timer?
    private var manualPause = false
    private var screenLocked = false
    private var lastEnvSent = Date.distantPast
    private var lastPlaylistSwitch = Date()
    private var lastShareURL: URL?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
            .appendingPathComponent("DiyarDesk", isDirectory: true)
        let protoDir = resolvePrototypeDir()
        self.store = ThemeStore(dataDir: appSupport, prototypeDir: protoDir)
        self.schemeHandler = LocalSchemeHandler(store: store)

        setupMenuBar()
        spawnWallpapers()
        observeSystemNotifications()

        pollTimer = Timer.scheduledTimer(withTimeInterval: 0.7, repeats: true) { [weak self] _ in
            self?.tick()
        }

        if !CommandLine.arguments.contains("--autostart") {
            openSettings()
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        pollTimer?.invalidate()
        teardownWallpapers()
        store.save()
    }

    private func resolvePrototypeDir() -> URL {
        if let res = Bundle.main.resourceURL?.appendingPathComponent("prototype", isDirectory: true),
           FileManager.default.fileExists(atPath: res.appendingPathComponent("themes.json").path) {
            return res
        }
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        for cand in [
            cwd.appendingPathComponent("prototype", isDirectory: true),
            cwd.deletingLastPathComponent().appendingPathComponent("prototype", isDirectory: true)
        ] {
            if FileManager.default.fileExists(atPath: cand.appendingPathComponent("themes.json").path) {
                return cand
            }
        }
        return cwd.appendingPathComponent("prototype", isDirectory: true)
    }

    private func setupMenuBar() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let btn = statusItem.button {
            btn.image = NSImage(systemSymbolName: "sparkles", accessibilityDescription: "DiyarDesk")
            if btn.image == nil { btn.title = "DD" }
        }
        rebuildMenu()
    }

    private func rebuildMenu() {
        let menu = NSMenu()
        let settingsItem = NSMenuItem(title: "Settings…", action: #selector(openSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)

        let pauseItem = NSMenuItem(title: manualPause ? "Resume Animation" : "Pause Animation", action: #selector(togglePause), keyEquivalent: "p")
        pauseItem.target = self
        menu.addItem(pauseItem)

        let lowItem = NSMenuItem(title: "Low Mode (Eco)", action: #selector(toggleLowMode), keyEquivalent: "")
        lowItem.state = store.settings.lowMode ? .on : .off
        lowItem.target = self
        menu.addItem(lowItem)

        let reloadItem = NSMenuItem(title: "Reload", action: #selector(reloadWallpapers), keyEquivalent: "r")
        reloadItem.target = self
        menu.addItem(reloadItem)

        menu.addItem(NSMenuItem.separator())
        let quitItem = NSMenuItem(title: "Quit DiyarDesk", action: #selector(quitApp), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)
        statusItem.menu = menu
    }

    @objc func openSettings() {
        if settingsController == nil {
            settingsController = SettingsWindowController(store: store, schemeHandler: schemeHandler, coordinator: self)
        }
        settingsController?.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func onSettingsWindowClosed() {
        settingsController = nil
    }

    @objc private func togglePause() {
        manualPause.toggle()
        rebuildMenu()
        tick()
    }

    @objc private func toggleLowMode() {
        store.settings.lowMode.toggle()
        store.save()
        rebuildMenu()
        for wp in wallpapers { wp.postJSON(["t": "lowMode", "v": store.settings.lowMode]) }
        settingsController?.postJSON(["t": "lowMode", "v": store.settings.lowMode])
    }

    @objc private func reloadWallpapers() {
        for wp in wallpapers { wp.reloadPage() }
    }

    @objc private func quitApp() {
        NSApp.terminate(nil)
    }

    private func teardownWallpapers() {
        for wp in wallpapers { wp.teardown() }
        wallpapers.removeAll()
    }

    private func spawnWallpapers() {
        teardownWallpapers()
        let screens: [NSScreen]
        if store.settings.displayMode == "primary", let main = NSScreen.main ?? NSScreen.screens.first {
            screens = [main]
        } else {
            screens = NSScreen.screens
        }
        let page = store.pageFor(themeId: store.settings.theme)
        for scr in screens {
            let wp = WallpaperWindow(screen: scr, store: store, schemeHandler: schemeHandler)
            wp.onReady = { [weak self, weak wp] in
                guard let self = self, let wp = wp else { return }
                self.pushInitialWallpaperState(wp)
            }
            wp.orderFrontRegardless()
            wp.loadPage(page)
            wallpapers.append(wp)
        }
    }

    private func isSystemDarkMode() -> Bool {
        let match = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua])
        return match == .darkAqua
    }

    private func pushInitialWallpaperState(_ wp: WallpaperWindow) {
        wp.postJSON(["t": "lowMode", "v": store.settings.lowMode])
        let reduced = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        let lowPower = ProcessInfo.processInfo.isLowPowerModeEnabled
        wp.postJSON(["t": "perf", "adaptiveFps": true, "onBattery": lowPower, "reducedMotion": reduced])
        wp.postJSON(SolarHelper.computeEnvironment(darkMode: isSystemDarkMode(), solarSync: store.settings.solarSync))
        wp.postJSON(["t": "settings", "v": store.paramsFor(store.settings.theme)])
        tick()
    }

    private func observeSystemNotifications() {
        let nc = NotificationCenter.default
        nc.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            self?.spawnWallpapers()
        }
        let wnc = NSWorkspace.shared.notificationCenter
        wnc.addObserver(forName: NSWorkspace.screensDidSleepNotification, object: nil, queue: .main) { [weak self] _ in
            self?.screenLocked = true
            self?.tick()
        }
        wnc.addObserver(forName: NSWorkspace.screensDidWakeNotification, object: nil, queue: .main) { [weak self] _ in
            self?.screenLocked = false
            self?.tick()
        }
        let dnc = DistributedNotificationCenter.default()
        dnc.addObserver(forName: NSNotification.Name("com.apple.screenIsLocked"), object: nil, queue: .main) { [weak self] _ in
            self?.screenLocked = true
            self?.tick()
        }
        dnc.addObserver(forName: NSNotification.Name("com.apple.screenIsUnlocked"), object: nil, queue: .main) { [weak self] _ in
            self?.screenLocked = false
            self?.tick()
        }
    }

    private func tick() {
        let lowPower = ProcessInfo.processInfo.isLowPowerModeEnabled
        let reduced = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        let pushEnv = Date().timeIntervalSince(lastEnvSent) >= 60.0
        if pushEnv { lastEnvSent = Date() }
        let envMsg = SolarHelper.computeEnvironment(darkMode: isSystemDarkMode(), solarSync: store.settings.solarSync)

        for (idx, wp) in wallpapers.enumerated() {
            let obs = iconsReader.observe(screen: wp.targetScreen, windowAware: store.settings.windowAware)
            let paused = manualPause || screenLocked || (!store.settings.disableAutoPause && (obs.occluded || lowPower))
            wp.postJSON(["t": "pause", "v": paused])
            wp.postJSON(["t": "perf", "adaptiveFps": true, "onBattery": lowPower, "reducedMotion": reduced])
            if pushEnv { wp.postJSON(envMsg) }
            let iconMsg: [String: Any] = ["t": "icons", "vis": !obs.iconRects.isEmpty, "r": obs.iconRects]
            wp.postJSON(iconMsg)
            if idx == 0 {
                settingsController?.postJSON(iconMsg)
            }
            if store.settings.windowAware {
                wp.postJSON(["t": "windows", "r": obs.windowRects])
            }
        }

        if store.settings.playlistEnabled && !manualPause && !screenLocked {
            let elapsedMin = Date().timeIntervalSince(lastPlaylistSwitch) / 60.0
            if elapsedMin >= Double(store.settings.playlistIntervalMin) {
                let ids = store.readyThemeIds()
                if ids.count > 1 {
                    let curIdx = ids.firstIndex(of: store.settings.theme) ?? 0
                    let nextId = ids[(curIdx + 1) % ids.count]
                    switchTheme(id: nextId)
                }
                lastPlaylistSwitch = Date()
            }
        }
    }

    private func switchTheme(id: String) {
        store.settings.theme = id
        store.save()
        lastPlaylistSwitch = Date()
        let page = store.pageFor(themeId: id)
        for wp in wallpapers { wp.loadPage(page) }
        settingsController?.postJSON(["t": "themeChanged", "id": id])
    }

    func handleSettingsMessage(kind: String, msg: [String: Any], controller: SettingsWindowController) {
        switch kind {
        case "hello":
            let scr = NSScreen.main ?? NSScreen.screens.first
            let scale = Double(scr?.backingScaleFactor ?? 2.0)
            let w = Int(round(Double(scr?.frame.width ?? 1440) * scale))
            let h = Int(round(Double(scr?.frame.height ?? 900) * scale))
            let sysLang = Locale.preferredLanguages.first ?? "en"
            let effLang: String
            if store.settings.lang != "auto" { effLang = store.settings.lang }
            else if sysLang.hasPrefix("tr") { effLang = "tr" }
            else if sysLang.hasPrefix("zh-Hant") || sysLang.hasPrefix("zh-TW") { effLang = "zh-TW" }
            else if sysLang.hasPrefix("zh") { effLang = "zh-CN" }
            else if sysLang.hasPrefix("de") { effLang = "de" }
            else if sysLang.hasPrefix("fr") { effLang = "fr" }
            else if sysLang.hasPrefix("ja") { effLang = "ja" }
            else if sysLang.hasPrefix("ko") { effLang = "ko" }
            else { effLang = "en" }

            controller.postJSON([
                "t": "init",
                "theme": store.settings.theme,
                "params": store.params,
                "screen": ["w": w, "h": h],
                "autostart": false,
                "autostartWarn": false,
                "autostartSw": "",
                "lang": store.settings.lang,
                "langEff": effLang,
                "langSys": effLang,
                "customBase": "\(DiyarSecurity.scheme)://\(DiyarSecurity.customHost)/",
                "virtualBarMode": store.settings.virtualBarMode,
                "lowMode": store.settings.lowMode,
                "disableAutoPause": store.settings.disableAutoPause,
                "displayMode": store.settings.displayMode,
                "solarSync": store.settings.solarSync,
                "windowAware": store.settings.windowAware,
                "playlistEnabled": store.settings.playlistEnabled,
                "playlistIntervalMin": store.settings.playlistIntervalMin,
                "canSaveSnapshot": true
            ])
        case "set":
            if let th = msg["theme"] as? String, let key = msg["key"] as? String, let val = msg["value"] {
                store.setParam(themeId: th, key: key, value: val)
                if th == store.settings.theme {
                    for wp in wallpapers { wp.postJSON(["t": "settings", "v": [key: val]]) }
                }
            }
        case "reset":
            if let th = msg["theme"] as? String {
                store.resetParams(themeId: th)
                if th == store.settings.theme {
                    for wp in wallpapers { wp.postJSON(["t": "reset"]) }
                }
            }
        case "theme":
            if let id = msg["id"] as? String { switchTheme(id: id) }
        case "lang":
            if let v = msg["v"] as? String {
                store.settings.lang = v
                store.save()
                let eff = v == "auto" ? "en" : v
                controller.postJSON(["t": "lang", "v": v, "sys": eff, "eff": eff])
            }
        case "lowMode":
            store.settings.lowMode = (msg["v"] as? Bool) ?? false
            store.save()
            rebuildMenu()
            for wp in wallpapers { wp.postJSON(["t": "lowMode", "v": store.settings.lowMode]) }
        case "disableAutoPause":
            store.settings.disableAutoPause = (msg["v"] as? Bool) ?? false
            store.save()
            tick()
        case "displayMode":
            if let dm = msg["v"] as? String {
                store.settings.displayMode = dm
                store.save()
                spawnWallpapers()
            }
        case "solarSync":
            store.settings.solarSync = (msg["v"] as? Bool) ?? false
            store.save()
            lastEnvSent = Date.distantPast
            tick()
        case "windowAware":
            store.settings.windowAware = (msg["v"] as? Bool) ?? false
            store.save()
            tick()
        case "playlist":
            if let en = msg["enabled"] as? Bool { store.settings.playlistEnabled = en }
            if let iv = msg["intervalMin"] as? Int { store.settings.playlistIntervalMin = max(1, min(1440, iv)) }
            lastPlaylistSwitch = Date()
            store.save()
        case "importTheme":
            controller.openImportDialog { [weak self] chosen in
                guard let self = self, let fileURL = chosen else { return }
                do {
                    let entry = try self.store.importFile(at: fileURL)
                    if let newId = entry["id"] as? String {
                        self.switchTheme(id: newId)
                        controller.postJSON(["t": "themeImported", "theme": entry, "select": true])
                    }
                } catch {
                    controller.postJSON(["t": "importError", "msg": error.localizedDescription])
                }
            }
        case "removeTheme":
            if let id = msg["id"] as? String, store.removeCustomTheme(id: id) {
                let page = store.pageFor(themeId: store.settings.theme)
                for wp in wallpapers { wp.loadPage(page) }
                controller.postJSON(["t": "themeRemoved", "id": id, "active": store.settings.theme])
            }
        case "share":
            guard let wp = wallpapers.first else {
                controller.postJSON(["t": "shareResult", "ok": false, "msg": "Wallpaper not ready"])
                return
            }
            wp.requestSnapshot { [weak self] thumb, png, err in
                guard let self = self else { return }
                if let err = err {
                    controller.postJSON(["t": "shareResult", "ok": false, "msg": err])
                    return
                }
                guard let png = png, let comma = png.firstIndex(of: ","),
                      let bytes = Data(base64Encoded: String(png[png.index(after: comma)...])) else {
                    controller.postJSON(["t": "shareResult", "ok": false, "msg": "Invalid image data"])
                    return
                }
                let pics = FileManager.default.urls(for: .picturesDirectory, in: .userDomainMask).first!
                    .appendingPathComponent("DiyarDesk", isDirectory: true)
                try? FileManager.default.createDirectory(at: pics, withIntermediateDirectories: true)
                let outURL = pics.appendingPathComponent("DiyarDesk-\(Int(Date().timeIntervalSince1970)).png")
                do {
                    try bytes.write(to: outURL)
                    self.lastShareURL = outURL
                    if let img = NSImage(data: bytes) {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.writeObjects([img])
                    }
                    controller.postJSON(["t": "shareResult", "ok": true, "kind": "image", "thumb": thumb ?? png, "path": outURL.path])
                } catch {
                    controller.postJSON(["t": "shareResult", "ok": false, "msg": error.localizedDescription])
                }
            }
        case "openShare":
            if let u = lastShareURL { NSWorkspace.shared.open(u) }
        case "revealShare":
            if let u = lastShareURL { NSWorkspace.shared.activateFileViewerSelecting([u]) }
        case "exportDiagnostics":
            let diag: [String: Any] = [
                "app": "DiyarDesk",
                "platform": "macOS",
                "osVersion": ProcessInfo.processInfo.operatingSystemVersionString,
                "theme": store.settings.theme,
                "displayMode": store.settings.displayMode,
                "lowMode": store.settings.lowMode,
                "solarSync": store.settings.solarSync,
                "windowAware": store.settings.windowAware
            ]
            let outURL = store.dataDir.appendingPathComponent("diyardesk-diagnostics.json")
            if let data = try? JSONSerialization.data(withJSONObject: diag, options: [.prettyPrinted]) {
                try? data.write(to: outURL)
                controller.postJSON(["t": "diagExported", "ok": true, "path": outURL.path])
            }
        default:
            break
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
