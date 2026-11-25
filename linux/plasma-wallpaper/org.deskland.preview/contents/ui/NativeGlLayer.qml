/*
 * Native Rust + OpenGL 3.3 Core wallpaper layer for KDE Plasma 6.
 * Loads libdiyardesk_plasma_gl.so from contents/ui/gl/ (zero QtWebEngineProcess).
 * Uses zero-copy QQuickFramebufferObject on OpenGL RHI and automatically falls
 * back to headless EGL QQuickItem on Vulkan/Software RHI.
 */
import QtQuick
import "gl" 1.0

Item {
    id: layer
    anchors.fill: parent

    property string homeDir: ""
    property string themeId: "papercut"
    property bool hostPaused: false
    property bool lowMode: false
    property var themeParams: ({})

    readonly property bool useHeadless: !glFbo.openGlRhi
    readonly property var activeItem: useHeadless ? glHeadless : glFbo
    readonly property bool available: glFbo.available

    property bool filePaused: false
    property string lastIconsMsg: ""
    property string lastSettingsMsg: ""

    signal filePausedChangedExternal(bool paused)
    signal moveIconsRequested(string rawJson)

    function dispatchPointer(kind, x, y) {
        if (!activeItem || !available) return;
        var k = 0;
        if (kind === "down") k = 1;
        else if (kind === "up" || kind === "cancel") k = 2;
        else if (kind === "leave") k = 3;
        else k = 0; // move / hover
        activeItem.dispatchMouse(k, Number(x) || 0, Number(y) || 0);
    }

    function pushSettings() {
        if (!activeItem || !available) return;
        var msg = JSON.stringify({ t: "settings", v: layer.themeParams || {} });
        if (msg !== lastSettingsMsg) {
            lastSettingsMsg = msg;
            activeItem.postJson(msg);
        }
    }

    function pollIcons() {
        if (!activeItem || !available || !homeDir) return;
        var raw = activeItem.readFileText(homeDir + "/.cache/deskland/icons.json");
        var msg = "";
        if (raw && raw.length > 0) {
            try {
                var parsed = JSON.parse(raw);
                var fresh = parsed && (!parsed.ts || (Date.now() - Number(parsed.ts)) < 90000);
                if (fresh && Array.isArray(parsed.r)) {
                    var payload = { t: "icons", vis: parsed.vis !== false, r: parsed.r };
                    if (Array.isArray(parsed.n) && parsed.n.length > 0) payload.n = parsed.n;
                    msg = JSON.stringify(payload);
                }
            } catch (e) {}
        }
        if (!msg) {
            msg = JSON.stringify({ t: "icons", vis: false, r: [] });
        }
        if (msg !== lastIconsMsg) {
            lastIconsMsg = msg;
            activeItem.postJson(msg);
        }
    }

    function pollPause() {
        if (!activeItem || !available || !homeDir) return;
        var raw = activeItem.readFileText(homeDir + "/.cache/deskland/pause.json");
        var next = false;
        if (raw && raw.length > 0) {
            try {
                var parsed = JSON.parse(raw);
                next = !!(parsed && parsed.paused);
            } catch (e) {
                next = false;
            }
        }
        if (next !== filePaused) {
            filePaused = next;
            filePausedChangedExternal(next);
        }
    }

    function handleEngineEvent(raw) {
        if (!raw || raw.length === 0) return;
        try {
            var obj = JSON.parse(raw);
            if (obj && (obj.t === "moveIcons" || obj.type === "moveIcons")) {
                moveIconsRequested(raw);
            }
        } catch (e) {}
    }

    onThemeIdChanged: {
        lastSettingsMsg = "";
        lastIconsMsg = "";
        pushSettings();
        pollIcons();
    }

    onThemeParamsChanged: pushSettings()

    DiyarDeskGlItem {
        id: glFbo
        anchors.fill: parent
        visible: !layer.useHeadless
        themeId: layer.themeId
        paused: layer.useHeadless || layer.hostPaused || layer.filePaused
        lowMode: layer.lowMode
        onEngineEvent: function (raw) { layer.handleEngineEvent(raw); }
    }

    DiyarDeskGlHeadlessItem {
        id: glHeadless
        anchors.fill: parent
        visible: layer.useHeadless
        themeId: layer.themeId
        paused: !layer.useHeadless || layer.hostPaused || layer.filePaused
        lowMode: layer.lowMode
        onEngineEvent: function (raw) { layer.handleEngineEvent(raw); }
    }

    Timer {
        id: iconsTimer
        interval: layer.lowMode ? 1500 : 600
        repeat: true
        running: layer.available && !layer.hostPaused && !layer.filePaused
        onTriggered: layer.pollIcons()
    }

    Timer {
        id: pauseTimer
        interval: 1200
        repeat: true
        running: layer.available
        onTriggered: layer.pollPause()
    }

    Component.onCompleted: {
        console.info("DiyarDesk Plasma: Native Rust OpenGL 3.3 aktif (tema=" + layer.themeId + ", openGlRhi=" + glFbo.openGlRhi + ")");
        pushSettings();
        pollIcons();
        pollPause();
    }
}
