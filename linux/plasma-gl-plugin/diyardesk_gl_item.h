#pragma once

#include <QElapsedTimer>
#include <QImage>
#include <QMutex>
#include <QQmlExtensionPlugin>
#include <QQuickFramebufferObject>
#include <QQuickItem>
#include <QString>
#include <QStringList>
#include <QTimer>
#include <QVector>

struct DdglMouseEvent {
    int kind = 0;
    float x = 0.0f;
    float y = 0.0f;
};

class DdglLib {
public:
    using FnIsNativeTheme = int (*)(const char *);

    using FnEmbCreate = void *(*)(int, int, float);
    using FnEmbDestroy = void (*)(void *);
    using FnEmbResize = void (*)(void *, int, int, float);
    using FnEmbSetPaused = void (*)(void *, int);
    using FnEmbSetLowMode = void (*)(void *, int);
    using FnEmbPostJson = void (*)(void *, const char *);
    using FnEmbMouse = int (*)(void *, int, float, float);
    using FnEmbPollEvent = int (*)(void *, char *, int);
    using FnEmbRender = int (*)(void *, float, unsigned int);

    using FnHlCreate = void *(*)(int, int, float);
    using FnHlDestroy = void (*)(void *);
    using FnHlResize = void (*)(void *, int, int, float);
    using FnHlSetPaused = void (*)(void *, int);
    using FnHlSetLowMode = void (*)(void *, int);
    using FnHlPostJson = void (*)(void *, const char *);
    using FnHlMouse = int (*)(void *, int, float, float);
    using FnHlPollEvent = int (*)(void *, char *, int);
    using FnHlRenderRgba = int (*)(void *, float, unsigned char *, int);

    static DdglLib &instance();

    bool isLoaded() const { return loaded_; }
    QString libraryPath() const { return path_; }
    bool isNativeTheme(const QString &themeId) const;

    FnIsNativeTheme is_native_theme = nullptr;

    FnEmbCreate emb_create = nullptr;
    FnEmbDestroy emb_destroy = nullptr;
    FnEmbResize emb_resize = nullptr;
    FnEmbSetPaused emb_set_paused = nullptr;
    FnEmbSetLowMode emb_set_low_mode = nullptr;
    FnEmbPostJson emb_post_json = nullptr;
    FnEmbMouse emb_mouse = nullptr;
    FnEmbPollEvent emb_poll_event = nullptr;
    FnEmbRender emb_render = nullptr;

    FnHlCreate hl_create = nullptr;
    FnHlDestroy hl_destroy = nullptr;
    FnHlResize hl_resize = nullptr;
    FnHlSetPaused hl_set_paused = nullptr;
    FnHlSetLowMode hl_set_low_mode = nullptr;
    FnHlPostJson hl_post_json = nullptr;
    FnHlMouse hl_mouse = nullptr;
    FnHlPollEvent hl_poll_event = nullptr;
    FnHlRenderRgba hl_render_rgba = nullptr;

private:
    DdglLib();
    void *handle_ = nullptr;
    bool loaded_ = false;
    QString path_;
};

// Zero-copy GPU renderer for Qt Quick OpenGL RHI (default on KDE Plasma 6).
class DiyarDeskGlItem : public QQuickFramebufferObject {
    Q_OBJECT
    Q_PROPERTY(QString themeId READ themeId WRITE setThemeId NOTIFY themeIdChanged)
    Q_PROPERTY(bool paused READ paused WRITE setPaused NOTIFY pausedChanged)
    Q_PROPERTY(bool lowMode READ lowMode WRITE setLowMode NOTIFY lowModeChanged)
    Q_PROPERTY(bool available READ available CONSTANT)
    Q_PROPERTY(bool openGlRhi READ openGlRhi NOTIFY openGlRhiChanged)
    Q_PROPERTY(QString libraryPath READ libraryPath CONSTANT)

public:
    explicit DiyarDeskGlItem(QQuickItem *parent = nullptr);
    ~DiyarDeskGlItem() override;

    Renderer *createRenderer() const override;

    QString themeId() const { return themeId_; }
    void setThemeId(const QString &id);

    bool paused() const { return paused_; }
    void setPaused(bool p);

    bool lowMode() const { return lowMode_; }
    void setLowMode(bool low);

    bool available() const;
    bool openGlRhi() const { return openGlRhi_; }
    QString libraryPath() const;

    Q_INVOKABLE void postJson(const QString &json);
    Q_INVOKABLE void dispatchMouse(int kind, qreal cssX, qreal cssY);
    Q_INVOKABLE bool isNativeTheme(const QString &themeId) const;
    Q_INVOKABLE QString readFileText(const QString &path) const;

    // Called from render thread inside synchronize()
    void takePending(QString &themeOut,
                     bool &themeDirtyOut,
                     bool &pausedOut,
                     bool &lowModeOut,
                     qreal &dprOut,
                     QStringList &jsonOut,
                     QVector<DdglMouseEvent> &mouseOut);

    void emitEngineEventQueued(const QString &json);

signals:
    void themeIdChanged();
    void pausedChanged();
    void lowModeChanged();
    void openGlRhiChanged();
    void engineEvent(const QString &json);

private slots:
    void onTick();
    void onWindowChanged(QQuickWindow *win);
    void onSceneGraphInitialized();

private:
    void updateTimer();

    mutable QMutex mutex_;
    QString themeId_ = QStringLiteral("papercut");
    bool themeDirty_ = true;
    bool paused_ = false;
    bool lowMode_ = false;
    bool openGlRhi_ = true;
    QStringList pendingJson_;
    QVector<DdglMouseEvent> pendingMouse_;
    QTimer tickTimer_;
};

// Universal RHI fallback (Vulkan/Software Qt Quick) backed by headless EGL OpenGL 3.3.
class DiyarDeskGlHeadlessItem : public QQuickItem {
    Q_OBJECT
    Q_PROPERTY(QString themeId READ themeId WRITE setThemeId NOTIFY themeIdChanged)
    Q_PROPERTY(bool paused READ paused WRITE setPaused NOTIFY pausedChanged)
    Q_PROPERTY(bool lowMode READ lowMode WRITE setLowMode NOTIFY lowModeChanged)
    Q_PROPERTY(bool available READ available CONSTANT)

public:
    explicit DiyarDeskGlHeadlessItem(QQuickItem *parent = nullptr);
    ~DiyarDeskGlHeadlessItem() override;

    QString themeId() const { return themeId_; }
    void setThemeId(const QString &id);

    bool paused() const { return paused_; }
    void setPaused(bool p);

    bool lowMode() const { return lowMode_; }
    void setLowMode(bool low);

    bool available() const;

    Q_INVOKABLE void postJson(const QString &json);
    Q_INVOKABLE void dispatchMouse(int kind, qreal cssX, qreal cssY);
    Q_INVOKABLE bool isNativeTheme(const QString &themeId) const;
    Q_INVOKABLE QString readFileText(const QString &path) const;

signals:
    void themeIdChanged();
    void pausedChanged();
    void lowModeChanged();
    void engineEvent(const QString &json);

protected:
    QSGNode *updatePaintNode(QSGNode *oldNode, UpdatePaintNodeData *data) override;

private slots:
    void onTick();

private:
    void updateTimer();
    void ensureEngine(int w, int h, float dpr);
    void drainEvents();

    void *engine_ = nullptr;
    int engineW_ = 0;
    int engineH_ = 0;
    float engineDpr_ = 1.0f;
    QString themeId_ = QStringLiteral("papercut");
    bool paused_ = false;
    bool lowMode_ = false;
    QStringList pendingJson_;
    QVector<unsigned char> rgbaBuf_;
    QElapsedTimer frameTimer_;
    QTimer tickTimer_;
};

class DiyarDeskBridge : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool available READ available CONSTANT)
    Q_PROPERTY(QString libraryPath READ libraryPath CONSTANT)

public:
    explicit DiyarDeskBridge(QObject *parent = nullptr) : QObject(parent) {}

    bool available() const;
    QString libraryPath() const;

    Q_INVOKABLE QString readFileText(const QString &path) const;
    Q_INVOKABLE bool fileExists(const QString &path) const;
    Q_INVOKABLE bool isNativeTheme(const QString &themeId) const;
};

class DiyarDeskGlPlugin : public QQmlExtensionPlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID QQmlExtensionInterface_iid)

public:
    void registerTypes(const char *uri) override;
};
