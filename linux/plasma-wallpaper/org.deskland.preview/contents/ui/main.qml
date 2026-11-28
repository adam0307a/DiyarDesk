/*
 * Deskland / DiyarDesk hybrid wallpaper for KDE Plasma 6 (Wayland).
 *
 * Dynamically selects between:
 *  1. NativeGlLayer.qml — Native Rust + OpenGL 3.3 Core (zero QtWebEngineProcess)
 *     when `rustOpenGl` is enabled in ~/.config/deskland/settings.json and the
 *     active theme is supported natively (or is a GL-only theme).
 *  2. WebEngineLayer.qml — QtWebEngine + inject.js for HTML/CSS/JS themes,
 *     custom imported packages/videos/images, or fallback if the GL plugin is absent.
 */

import QtQuick
import Qt.labs.platform
import org.kde.kwindowsystem
import org.kde.plasma.plasmoid
import org.kde.taskmanager as TaskManager

WallpaperItem {
    id: root
    objectName: "desklandWallpaper"

    property string homeDir: {
        var base = String(StandardPaths.writableLocation(StandardPaths.GenericConfigLocation));
        base = base.replace(/^file:\/\//, "");
        base = base.replace(/\/\.config\/?$/, "");
        return base.replace(/\/+$/, "");
    }

    property string prototypeRoot: "file:///usr/lib/deskland/prototype/"
    property string theme: "meadow"
    property bool rustOpenGl: false
    property bool nativeGlFailed: false
    property bool settingsLoaded: false
    property var themeParams: ({})

    property bool windowPaused: false
    property bool screenshotPaused: false
    property bool lowMode: false
    property bool disableAutoPause: false
    property var iconMover: null
    readonly property bool showingDesktop: KWindowSystem.showingDesktop
    readonly property bool effectivePaused: !disableAutoPause && (screenshotPaused || (windowPaused && !showingDesktop))

    readonly property var nativeGlThemes: ({
        "material-impulse-gl": true,
        "material-impulse": true,
        "liquidglass": true,
        "thinking-orbs": true,
        "solar-system": true,
        "studio-glass": true,
        "beach": true,
        "meadow": true,
        "desert": true,
        "rosebrook": true,
        "glade": true,
        "zen-archipelago": true,
        "aurora": true,
        "polarnight": true,
        "quietsnow": true,
        "winter": true,
        "neumorph": true,
        "claymorph": true,
        "neobrutalism": true,
        "voxelcraft": true,
        "papercut": true,
        "istanbul": true,
        "software-world": true,
        "rift": true,
        "strike": true,
        "pocket": true,
        "roach": true,
        "ai-mascots": true,
        "cat-shore": true,
        "persephone": true,
        "eye-within-words": true,
        "cosmic-eye": true,
        "ascii-clownfish": true,
        "ascii-jellyfish": true,
        "ascii-shark": true,
        "ascii-lotus": true,
        "ascii-cat-knight": true,
        "turkish-flag": true
    })

    readonly property bool isGlOnlyTheme: theme === "material-impulse-gl" || theme === "ai-mascots" || theme === "cat-shore" || theme === "persephone" || theme === "eye-within-words" || theme === "cosmic-eye" || theme === "ascii-clownfish" || theme === "ascii-jellyfish" || theme === "ascii-shark" || theme === "ascii-lotus" || theme === "ascii-cat-knight" || theme === "turkish-flag"
    readonly property bool wantNativeGl: !nativeGlFailed && (isGlOnlyTheme || (rustOpenGl && !!nativeGlThemes[theme]))

    onShowingDesktopChanged: pauseEvalTimer.restart()

    property string _lastParamsJson: ""

    function resolvePrototypeRoot() {
        var bridge = hostBridgeLoader.item;
        if (!bridge || typeof bridge.fileExists !== "function" || !root.homeDir) return;
        var userProto = root.homeDir + "/.local/share/deskland/prototype";
        if (bridge.fileExists(userProto + "/meadow.html")) {
            root.prototypeRoot = "file://" + userProto + "/";
            return;
        }
        if (bridge.fileExists("/usr/lib/deskland/prototype/meadow.html")) {
            root.prototypeRoot = "file:///usr/lib/deskland/prototype/";
        }
    }

    function applySettingsObject(data) {
        if (!data || typeof data !== "object") return;
        if (typeof data.theme === "string" && data.theme.length > 0 && data.theme !== root.theme) {
            root.theme = data.theme;
        }
        var nextGl = !!data.rustOpenGl;
        if (nextGl !== root.rustOpenGl) {
            root.rustOpenGl = nextGl;
        }
        var nextLow = !!data.lowMode;
        if (nextLow !== root.lowMode) {
            root.lowMode = nextLow;
            root.updateWindowPause();
        }
        var nextDisable = !!data.disableAutoPause;
        if (nextDisable !== root.disableAutoPause) {
            root.disableAutoPause = nextDisable;
            root.updateWindowPause();
        }
        var p = (data.params && data.params[root.theme]) ? data.params[root.theme] : {};
        var serialized = JSON.stringify(p);
        if (serialized !== root._lastParamsJson) {
            root._lastParamsJson = serialized;
            root.themeParams = p;
        }
    }

    function pollHostSettings() {
        if (!root.homeDir) return;
        var bridge = hostBridgeLoader.item;
        if (!bridge || typeof bridge.readFileText !== "function") return;
        var txt = bridge.readFileText(root.homeDir + "/.config/deskland/settings.json");
        if (txt && txt.length > 0) {
            try { applySettingsObject(JSON.parse(txt)); } catch (e) {}
        }
    }

    function dispatchPointer(kind, x, y) {
        if (root.effectivePaused) return;
        if (engineLoader.item && typeof engineLoader.item.dispatchPointer === "function") {
            engineLoader.item.dispatchPointer(kind, x, y);
        }
    }

    function updateWindowPause() {
        var covered = false;
        var hasScreenshot = false;
        var count = tasksModel.count;
        for (var i = 0; i < count; i++) {
            var idx = tasksModel.index(i, 0);
            if (!tasksModel.data(idx, TaskManager.AbstractTasksModel.IsWindow))
                continue;
            if (tasksModel.data(idx, TaskManager.AbstractTasksModel.IsMinimized))
                continue;
            if (tasksModel.data(idx, TaskManager.AbstractTasksModel.IsHidden))
                continue;
            var appId = String(tasksModel.data(idx, TaskManager.AbstractTasksModel.AppId) || "").toLowerCase();
            if (appId.indexOf("deskland") !== -1 || appId.indexOf("diyardesk") !== -1)
                continue;
            if (appId.indexOf("spectacle") !== -1 || appId.indexOf("flameshot") !== -1 || appId.indexOf("ksnip") !== -1) {
                hasScreenshot = true;
                covered = true;
                break;
            }
            if (!tasksModel.data(idx, TaskManager.AbstractTasksModel.IsActive))
                continue;
            if (tasksModel.data(idx, TaskManager.AbstractTasksModel.SkipTaskbar))
                continue;
            if (!KWindowSystem.showingDesktop
                    && (Boolean(tasksModel.data(idx, TaskManager.AbstractTasksModel.IsFullScreen))
                        || (root.lowMode && Boolean(tasksModel.data(idx, TaskManager.AbstractTasksModel.IsMaximized))))) {
                covered = true;
                break;
            }
        }
        root.screenshotPaused = hasScreenshot;
        root.windowPaused = (KWindowSystem.showingDesktop && !hasScreenshot) ? false : covered;
    }

    Loader {
        id: hostBridgeLoader
        asynchronous: false
        source: Qt.resolvedUrl("HostBridge.qml")
        onStatusChanged: {
            if (status === Loader.Error) {
                root.nativeGlFailed = true;
            } else if (status === Loader.Ready) {
                if (item && item.available === false) {
                    root.nativeGlFailed = true;
                }
                root.resolvePrototypeRoot();
                root.pollHostSettings();
            }
        }
    }

    Timer {
        id: pauseEvalTimer
        interval: 80
        repeat: false
        onTriggered: root.updateWindowPause()
    }

    Timer {
        id: configPollTimer
        interval: 1000
        repeat: true
        running: hostBridgeLoader.status === Loader.Ready
        onTriggered: root.pollHostSettings()
    }

    TaskManager.ActivityInfo {
        id: activityInfo
    }

    TaskManager.TasksModel {
        id: tasksModel
        groupMode: TaskManager.TasksModel.GroupDisabled
        filterByCurrentVirtualDesktop: true
        filterByActivity: true
        activity: activityInfo.currentActivity
        filterMinimized: true
        filterHidden: true
        onActiveTaskChanged: pauseEvalTimer.restart()
        onCountChanged: pauseEvalTimer.restart()
        onDataChanged: pauseEvalTimer.restart()
        onRowsInserted: pauseEvalTimer.restart()
        onRowsRemoved: pauseEvalTimer.restart()
        onModelReset: pauseEvalTimer.restart()
    }

    Loader {
        id: engineLoader
        anchors.fill: parent
        asynchronous: false
        source: !root.settingsLoaded
                ? ""
                : (root.wantNativeGl ? Qt.resolvedUrl("NativeGlLayer.qml") : Qt.resolvedUrl("WebEngineLayer.qml"))

        onStatusChanged: {
            if (status === Loader.Error && root.wantNativeGl) {
                console.warn("DiyarDesk: NativeGlLayer yüklenemedi, QtWebEngine katmanına düşülüyor.");
                root.nativeGlFailed = true;
            }
        }

        onLoaded: {
            if (!item) return;
            if (root.wantNativeGl && item.available === false) {
                console.warn("DiyarDesk: libdiyardesk_gl.so bulunamadı, QtWebEngine katmanına düşülüyor.");
                root.nativeGlFailed = true;
                return;
            }
            syncLayerProps();
        }

        function syncLayerProps() {
            if (!item) return;
            item.themeId = Qt.binding(function () { return root.theme; });
            item.lowMode = Qt.binding(function () { return root.lowMode; });
            if (root.wantNativeGl) {
                item.hostPaused = Qt.binding(function () { return root.effectivePaused; });
                item.themeParams = Qt.binding(function () { return root.themeParams; });
                item.homeDir = Qt.binding(function () { return root.homeDir; });
            } else {
                item.prototypeRoot = Qt.binding(function () { return root.prototypeRoot; });
                item.effectivePaused = Qt.binding(function () { return root.effectivePaused; });
                item.iconMover = Qt.binding(function () { return root.iconMover; });
                item.homeDir = Qt.binding(function () { return root.homeDir; });
            }
        }
    }

    Binding {
        target: engineLoader.item
        property: "themeId"
        value: root.theme
        when: engineLoader.item !== null
    }
    Binding {
        target: engineLoader.item
        property: "lowMode"
        value: root.lowMode
        when: engineLoader.item !== null
    }
    Binding {
        target: engineLoader.item
        property: "homeDir"
        value: root.homeDir
        when: engineLoader.item !== null
    }
    Binding {
        target: engineLoader.item
        property: "hostPaused"
        value: root.effectivePaused
        when: engineLoader.item !== null && root.wantNativeGl
    }
    Binding {
        target: engineLoader.item
        property: "themeParams"
        value: root.themeParams
        when: engineLoader.item !== null && root.wantNativeGl
    }
    Binding {
        target: engineLoader.item
        property: "effectivePaused"
        value: root.effectivePaused
        when: engineLoader.item !== null && !root.wantNativeGl
    }

    Connections {
        target: engineLoader.item
        ignoreUnknownSignals: true
        function onMoveIconsRequested(rawJson) {
            var mover = root.iconMover;
            if (mover && typeof mover.applyMoveIcons === "function") {
                mover.applyMoveIcons(rawJson);
            }
        }
        function onLowModeObserved(v) {
            if (v !== root.lowMode) {
                root.lowMode = v;
                root.updateWindowPause();
            }
        }
        function onDisableAutoPauseObserved(v) {
            if (v !== root.disableAutoPause) {
                root.disableAutoPause = v;
                root.updateWindowPause();
            }
        }
        function onRustOpenGlObserved(v, tid) {
            if (tid && tid !== root.theme) {
                root.theme = tid;
            }
            if (v !== root.rustOpenGl) {
                root.rustOpenGl = v;
            }
        }
    }

    Component.onCompleted: {
        resolvePrototypeRoot();
        pollHostSettings();
        root.settingsLoaded = true;
        updateWindowPause();
    }

    Component.onDestruction: {
        if (root.iconMover && typeof root.iconMover.resetMoveIcons === "function") {
            root.iconMover.resetMoveIcons(true);
        }
    }
}
