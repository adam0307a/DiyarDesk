/*
 * HTML/CSS/JS WebEngine wallpaper layer for KDE Plasma 6.
 * Loaded on demand when rustOpenGl is disabled, when a custom HTML/CSS/JS/image/video
 * theme is selected, or when the native Rust OpenGL plugin is unavailable.
 */
import QtQuick
import QtWebEngine

Item {
    id: webLayer
    anchors.fill: parent

    property string prototypeRoot: "file:///usr/lib/deskland/prototype/"
    property string homeDir: ""
    property string themeId: "meadow"
    property bool effectivePaused: false
    property bool lowMode: false
    property var iconMover: null

    property bool pageReady: false
    property bool initialLoaded: false
    property bool iconMoveActive: false
    property double lastIconMoveAt: 0

    signal lowModeObserved(bool v)
    signal disableAutoPauseObserved(bool v)
    signal rustOpenGlObserved(bool v, string themeId)

    function dispatchPointer(kind, x, y) {
        if (effectivePaused || !pageReady) return;
        view.runJavaScript(
            "if (window.__desklandDispatchPointer) window.__desklandDispatchPointer("
            + JSON.stringify(kind) + "," + Number(x) + "," + Number(y) + ");"
            + "else if (window.roachHostPointer) window.roachHostPointer("
            + JSON.stringify(kind) + "," + Number(x) + "," + Number(y) + ");"
        );
    }

    function applyEffectivePause() {
        if (!pageReady) return;
        if (effectivePaused) {
            view.runJavaScript("window.__desklandSetHostPaused&&window.__desklandSetHostPaused(true);");
            freezeTimer.restart();
        } else {
            freezeTimer.stop();
            try { view.lifecycleState = WebEngineView.LifecycleState.Active; } catch (e) {}
            view.runJavaScript("window.__desklandSetHostPaused&&window.__desklandSetHostPaused(false);");
        }
    }

    function ensureInitialLoad() {
        if (initialLoaded || !homeDir || homeDir.length === 0) return;
        initialLoaded = true;
        // Start on meadow.html (guaranteed present in all prototype installs);
        // inject.js immediately navigates to the active built-in or custom theme.
        var pageName = "meadow.html";
        view.userScripts.collection = [
            { name: "deskland-bridge",
              sourceUrl: Qt.resolvedUrl("inject.js"),
              injectionPoint: WebEngineScript.DocumentCreation,
              worldId: WebEngineScript.MainWorld },
            { name: "deskland-low-mode",
              sourceUrl: webLayer.prototypeRoot + "low-mode.js",
              injectionPoint: WebEngineScript.DocumentCreation,
              worldId: WebEngineScript.MainWorld }
        ];
        view.url = webLayer.prototypeRoot + pageName
            + "?wallpaper=1&vicons=0"
            + "&v=" + Date.now()
            + "&theme=meadow"
            + "&home=" + encodeURIComponent(webLayer.homeDir);
    }

    onHomeDirChanged: ensureInitialLoad()
    onEffectivePausedChanged: applyEffectivePause()

    Timer {
        id: freezeTimer
        interval: 180
        repeat: false
        onTriggered: {
            if (webLayer.effectivePaused) {
                try { view.lifecycleState = WebEngineView.LifecycleState.Frozen; } catch (e) {}
            }
        }
    }

    Timer {
        id: readyGraceTimer
        interval: 800
        repeat: false
        onTriggered: {
            webLayer.pageReady = true;
            webLayer.applyEffectivePause();
        }
    }

    Timer {
        id: settingsSyncTimer
        interval: 1000
        repeat: true
        running: webLayer.pageReady && !webLayer.effectivePaused
        onTriggered: {
            view.runJavaScript(
                "[window.__desklandLowModeState ? '1' : '0', window.__desklandDisableAutoPause ? '1' : '0', window.__desklandRustOpenGl ? '1' : '0', String(window.__desklandThemeId || '')]",
                function (res) {
                    if (!res || res.length < 2) return;
                    webLayer.lowModeObserved(res[0] === '1');
                    webLayer.disableAutoPauseObserved(res[1] === '1');
                    if (res.length >= 4 && res[3]) {
                        webLayer.rustOpenGlObserved(res[2] === '1', res[3]);
                    }
                }
            );
        }
    }

    Timer {
        id: iconMoveTimer
        interval: webLayer.lowMode ? (webLayer.iconMoveActive ? 150 : 1200) : (webLayer.iconMoveActive ? 40 : 480)
        repeat: true
        running: webLayer.pageReady && !webLayer.effectivePaused && webLayer.iconMover !== null
        onTriggered: {
            view.runJavaScript(
                "[window.__desklandLowModeState ? '1' : '0', window.__desklandDisableAutoPause ? '1' : '0', window.__desklandTakeMoveIcons ? window.__desklandTakeMoveIcons() : '']",
                function (res) {
                    if (!res || res.length < 3) return;
                    webLayer.lowModeObserved(res[0] === '1');
                    webLayer.disableAutoPauseObserved(res[1] === '1');
                    var raw = res[2];
                    var mover = webLayer.iconMover;
                    if (!mover || typeof mover.applyMoveIcons !== "function") return;
                    if (raw && raw.length > 0) {
                        webLayer.iconMoveActive = true;
                        webLayer.lastIconMoveAt = Date.now();
                        mover.applyMoveIcons(raw);
                    } else if (webLayer.iconMoveActive && (Date.now() - webLayer.lastIconMoveAt) > 1500) {
                        webLayer.iconMoveActive = false;
                    }
                }
            );
        }
    }

    WebEngineView {
        id: view
        anchors.fill: parent
        url: ""
        backgroundColor: "#000000"
        settings.localContentCanAccessFileUrls: true
        settings.localStorageEnabled: true
        settings.playbackRequiresUserGesture: true
        settings.errorPageEnabled: false

        Component.onCompleted: webLayer.ensureInitialLoad()

        onLoadingChanged: function (loadingInfo) {
            if (loadingInfo.status === WebEngineView.LoadStartedStatus) {
                webLayer.pageReady = false;
                freezeTimer.stop();
                readyGraceTimer.stop();
                try { view.lifecycleState = WebEngineView.LifecycleState.Active; } catch (e) {}
            } else if (loadingInfo.status === WebEngineView.LoadSucceededStatus) {
                readyGraceTimer.restart();
            }
        }
    }
}
