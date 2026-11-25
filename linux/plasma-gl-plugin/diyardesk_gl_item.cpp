#include "diyardesk_gl_item.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMetaObject>
#include <QOpenGLFramebufferObject>
#include <QOpenGLFramebufferObjectFormat>
#include <QQuickOpenGLUtils>
#include <QQuickWindow>
#include <QSGRendererInterface>
#include <QSGSimpleTextureNode>
#include <QStandardPaths>
#include <QtQml/qqml.h>

#include <algorithm>
#include <dlfcn.h>

namespace {

QStringList candidateLibraryPaths() {
    QStringList out;
    const QByteArray env = qgetenv("DESKLAND_GL_LIB");
    if (!env.isEmpty()) {
        out << QString::fromLocal8Bit(env);
    }

    // Sibling of this plugin (.so)
    Dl_info info{};
    if (dladdr(reinterpret_cast<const void *>(&candidateLibraryPaths), &info) != 0 && info.dli_fname) {
        const QFileInfo fi(QString::fromLocal8Bit(info.dli_fname));
        const QDir dir = fi.absoluteDir();
        out << dir.filePath(QStringLiteral("libdiyardesk_gl.so"));
        out << dir.filePath(QStringLiteral("../libdiyardesk_gl.so"));
    }

    const QString home = QDir::homePath();
    out << (home + QStringLiteral("/.local/lib/libdiyardesk_gl.so"));
    out << (home + QStringLiteral("/.local/share/deskland/libdiyardesk_gl.so"));
    out << QStringLiteral("/usr/lib/deskland/libdiyardesk_gl.so");
    out << QStringLiteral("/usr/lib64/deskland/libdiyardesk_gl.so");
    out << QStringLiteral("/usr/local/lib/libdiyardesk_gl.so");
    out << QStringLiteral("/usr/lib/libdiyardesk_gl.so");
    out << QStringLiteral("/usr/lib64/libdiyardesk_gl.so");
    return out;
}

template <typename T>
bool loadSym(void *handle, const char *name, T &out) {
    void *p = dlsym(handle, name);
    if (!p) {
        return false;
    }
    out = reinterpret_cast<T>(p);
    return true;
}

QString readTextFile(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) {
        return QString();
    }
    return QString::fromUtf8(f.readAll());
}

} // namespace

// ---------------------------------------------------------------------------
// DdglLib
// ---------------------------------------------------------------------------

DdglLib &DdglLib::instance() {
    static DdglLib inst;
    return inst;
}

DdglLib::DdglLib() {
    const QStringList candidates = candidateLibraryPaths();
    for (const QString &cand : candidates) {
        if (!QFileInfo::exists(cand)) {
            continue;
        }
        void *h = dlopen(cand.toLocal8Bit().constData(), RTLD_NOW | RTLD_LOCAL);
        if (!h) {
            continue;
        }
        bool ok = true;
        ok &= loadSym(h, "ddgl_is_native_theme", is_native_theme);
        ok &= loadSym(h, "ddgl_embedded_create", emb_create);
        ok &= loadSym(h, "ddgl_embedded_destroy", emb_destroy);
        ok &= loadSym(h, "ddgl_embedded_resize", emb_resize);
        ok &= loadSym(h, "ddgl_embedded_set_paused", emb_set_paused);
        ok &= loadSym(h, "ddgl_embedded_set_low_mode", emb_set_low_mode);
        ok &= loadSym(h, "ddgl_embedded_post_json", emb_post_json);
        ok &= loadSym(h, "ddgl_embedded_mouse", emb_mouse);
        ok &= loadSym(h, "ddgl_embedded_poll_event", emb_poll_event);
        ok &= loadSym(h, "ddgl_embedded_render", emb_render);
        ok &= loadSym(h, "ddgl_headless_create", hl_create);
        ok &= loadSym(h, "ddgl_headless_destroy", hl_destroy);
        ok &= loadSym(h, "ddgl_headless_resize", hl_resize);
        ok &= loadSym(h, "ddgl_headless_set_paused", hl_set_paused);
        ok &= loadSym(h, "ddgl_headless_set_low_mode", hl_set_low_mode);
        ok &= loadSym(h, "ddgl_headless_post_json", hl_post_json);
        ok &= loadSym(h, "ddgl_headless_mouse", hl_mouse);
        ok &= loadSym(h, "ddgl_headless_poll_event", hl_poll_event);
        ok &= loadSym(h, "ddgl_headless_render_rgba", hl_render_rgba);

        if (ok) {
            handle_ = h;
            loaded_ = true;
            path_ = cand;
            return;
        }
        dlclose(h);
    }
}

bool DdglLib::isNativeTheme(const QString &themeId) const {
    if (!loaded_ || !is_native_theme || themeId.isEmpty()) {
        return false;
    }
    const QByteArray utf8 = themeId.toUtf8();
    return is_native_theme(utf8.constData()) != 0;
}

// ---------------------------------------------------------------------------
// DiyarDeskGlRenderer (Qt Quick OpenGL render thread)
// ---------------------------------------------------------------------------

class DiyarDeskGlRenderer : public QQuickFramebufferObject::Renderer {
public:
    explicit DiyarDeskGlRenderer(DiyarDeskGlItem *item) : item_(item) {
        frameTimer_.start();
    }

    ~DiyarDeskGlRenderer() override {
        if (engine_ && DdglLib::instance().isLoaded()) {
            DdglLib::instance().emb_destroy(engine_);
            engine_ = nullptr;
        }
    }

    QOpenGLFramebufferObject *createFramebufferObject(const QSize &size) override {
        QOpenGLFramebufferObjectFormat fmt;
        fmt.setAttachment(QOpenGLFramebufferObject::Depth);
        fmt.setInternalTextureFormat(GL_RGBA8);
        fboSize_ = size;
        if (engine_ && DdglLib::instance().isLoaded()) {
            DdglLib::instance().emb_resize(
                engine_,
                std::max(64, size.width()),
                std::max(64, size.height()),
                static_cast<float>(std::max<qreal>(0.5, dpr_)));
        }
        return new QOpenGLFramebufferObject(size, fmt);
    }

    void synchronize(QQuickFramebufferObject *obj) override {
        auto *glItem = static_cast<DiyarDeskGlItem *>(obj);
        item_ = glItem;
        QString newTheme;
        bool themeDirty = false;
        QStringList msgs;
        QVector<DdglMouseEvent> mice;
        glItem->takePending(newTheme, themeDirty, paused_, lowMode_, dpr_, msgs, mice);
        if (themeDirty) {
            themeId_ = newTheme;
            pendingTheme_ = true;
        }
        pendingJson_.append(msgs);
        pendingMouse_.append(mice);
    }

    void render() override {
        DdglLib &lib = DdglLib::instance();
        if (!lib.isLoaded()) {
            return;
        }
        QOpenGLFramebufferObject *fbo = framebufferObject();
        if (!fbo) {
            return;
        }

        const int w = std::max(64, fbo->width());
        const int h = std::max(64, fbo->height());
        const float dpr = static_cast<float>(std::max<qreal>(0.5, dpr_));

        if (!engine_) {
            engine_ = lib.emb_create(w, h, dpr);
            if (!engine_) {
                qWarning("DiyarDeskGlRenderer: ddgl_embedded_create failed (%dx%d dpr=%.2f)", w, h, dpr);
                return;
            }
            qInfo("DiyarDeskGlRenderer: zero-copy OpenGL 3.3 Core engine started (%dx%d dpr=%.2f theme=%s fbo=%u)",
                  w, h, dpr, qPrintable(themeId_), static_cast<unsigned int>(fbo->handle()));
            pendingTheme_ = true;
            lastAppliedLow_ = !lowMode_;
            lastAppliedPaused_ = !paused_;
            frameTimer_.restart();
        }

        if (lastAppliedLow_ != lowMode_) {
            lastAppliedLow_ = lowMode_;
            lib.emb_set_low_mode(engine_, lowMode_ ? 1 : 0);
            const QByteArray lowMsg = lowMode_
                ? QByteArrayLiteral("{\"t\":\"lowMode\",\"v\":true}")
                : QByteArrayLiteral("{\"t\":\"lowMode\",\"v\":false}");
            lib.emb_post_json(engine_, lowMsg.constData());
        }

        if (pendingTheme_) {
            pendingTheme_ = false;
            qInfo("DiyarDeskGlRenderer: theme -> %s", qPrintable(themeId_));
            QJsonObject obj;
            obj.insert(QStringLiteral("t"), QStringLiteral("theme"));
            obj.insert(QStringLiteral("id"), themeId_);
            const QByteArray raw = QJsonDocument(obj).toJson(QJsonDocument::Compact);
            lib.emb_post_json(engine_, raw.constData());
        }

        for (const QString &msg : pendingJson_) {
            const QByteArray utf8 = msg.toUtf8();
            lib.emb_post_json(engine_, utf8.constData());
        }
        pendingJson_.clear();

        for (const DdglMouseEvent &ev : pendingMouse_) {
            lib.emb_mouse(engine_, ev.kind, ev.x, ev.y);
        }
        pendingMouse_.clear();

        if (lastAppliedPaused_ != paused_) {
            lastAppliedPaused_ = paused_;
            lib.emb_set_paused(engine_, paused_ ? 1 : 0);
        }

        const qint64 elapsedMs = frameTimer_.restart();
        const float dt = paused_ ? 0.0f : std::clamp(static_cast<float>(elapsedMs) / 1000.0f, 0.0f, 0.08f);
        lib.emb_render(engine_, dt, static_cast<unsigned int>(fbo->handle()));

        char buf[16384];
        for (int i = 0; i < 16; ++i) {
            const int n = lib.emb_poll_event(engine_, buf, static_cast<int>(sizeof(buf)));
            if (n <= 0) {
                break;
            }
            if (item_) {
                item_->emitEngineEventQueued(QString::fromUtf8(buf, n));
            }
        }

        QQuickOpenGLUtils::resetOpenGLState();
    }

private:
    DiyarDeskGlItem *item_ = nullptr;
    void *engine_ = nullptr;
    QSize fboSize_;
    QString themeId_ = QStringLiteral("papercut");
    bool pendingTheme_ = true;
    bool paused_ = false;
    bool lowMode_ = false;
    bool lastAppliedPaused_ = false;
    bool lastAppliedLow_ = false;
    qreal dpr_ = 1.0;
    QStringList pendingJson_;
    QVector<DdglMouseEvent> pendingMouse_;
    QElapsedTimer frameTimer_;
};

// ---------------------------------------------------------------------------
// DiyarDeskGlItem
// ---------------------------------------------------------------------------

DiyarDeskGlItem::DiyarDeskGlItem(QQuickItem *parent) : QQuickFramebufferObject(parent) {
    setMirrorVertically(true);
    setTextureFollowsItemSize(true);
    connect(this, &QQuickItem::windowChanged, this, &DiyarDeskGlItem::onWindowChanged);
    connect(&tickTimer_, &QTimer::timeout, this, &DiyarDeskGlItem::onTick);
    updateTimer();
}

DiyarDeskGlItem::~DiyarDeskGlItem() {
    tickTimer_.stop();
}

QQuickFramebufferObject::Renderer *DiyarDeskGlItem::createRenderer() const {
    return new DiyarDeskGlRenderer(const_cast<DiyarDeskGlItem *>(this));
}

void DiyarDeskGlItem::setThemeId(const QString &id) {
    {
        QMutexLocker lock(&mutex_);
        if (themeId_ == id) {
            return;
        }
        themeId_ = id;
        themeDirty_ = true;
    }
    emit themeIdChanged();
    update();
}

void DiyarDeskGlItem::setPaused(bool p) {
    {
        QMutexLocker lock(&mutex_);
        if (paused_ == p) {
            return;
        }
        paused_ = p;
    }
    emit pausedChanged();
    updateTimer();
    update();
}

void DiyarDeskGlItem::setLowMode(bool low) {
    {
        QMutexLocker lock(&mutex_);
        if (lowMode_ == low) {
            return;
        }
        lowMode_ = low;
    }
    emit lowModeChanged();
    updateTimer();
    update();
}

bool DiyarDeskGlItem::available() const {
    return DdglLib::instance().isLoaded();
}

QString DiyarDeskGlItem::libraryPath() const {
    return DdglLib::instance().libraryPath();
}

void DiyarDeskGlItem::postJson(const QString &json) {
    if (json.isEmpty()) {
        return;
    }
    {
        QMutexLocker lock(&mutex_);
        if (pendingJson_.size() < 256) {
            pendingJson_.append(json);
        }
    }
    update();
}

void DiyarDeskGlItem::dispatchMouse(int kind, qreal cssX, qreal cssY) {
    {
        QMutexLocker lock(&mutex_);
        if (pendingMouse_.size() < 64) {
            pendingMouse_.append(DdglMouseEvent{kind, static_cast<float>(cssX), static_cast<float>(cssY)});
        }
    }
    if (!paused_) {
        update();
    }
}

bool DiyarDeskGlItem::isNativeTheme(const QString &themeId) const {
    return DdglLib::instance().isNativeTheme(themeId);
}

QString DiyarDeskGlItem::readFileText(const QString &path) const {
    return readTextFile(path);
}

void DiyarDeskGlItem::takePending(QString &themeOut,
                                  bool &themeDirtyOut,
                                  bool &pausedOut,
                                  bool &lowModeOut,
                                  qreal &dprOut,
                                  QStringList &jsonOut,
                                  QVector<DdglMouseEvent> &mouseOut) {
    QMutexLocker lock(&mutex_);
    themeOut = themeId_;
    themeDirtyOut = themeDirty_;
    themeDirty_ = false;
    pausedOut = paused_;
    lowModeOut = lowMode_;
    dprOut = window() ? window()->effectiveDevicePixelRatio() : 1.0;
    jsonOut.swap(pendingJson_);
    mouseOut.swap(pendingMouse_);
}

void DiyarDeskGlItem::emitEngineEventQueued(const QString &json) {
    QMetaObject::invokeMethod(
        this,
        [this, json]() { emit engineEvent(json); },
        Qt::QueuedConnection);
}

void DiyarDeskGlItem::onTick() {
    if (!paused_ && openGlRhi_ && isVisible()) {
        update();
    }
}

void DiyarDeskGlItem::onSceneGraphInitialized() {
    QQuickWindow *win = window();
    bool isGl = true;
    QSGRendererInterface::GraphicsApi api = QQuickWindow::graphicsApi();
    if (win && win->rendererInterface()) {
        const auto winApi = win->rendererInterface()->graphicsApi();
        if (winApi != QSGRendererInterface::Unknown) {
            api = winApi;
        }
    }
    if (api != QSGRendererInterface::Unknown) {
        isGl = (api == QSGRendererInterface::OpenGL);
    }
    if (openGlRhi_ != isGl) {
        openGlRhi_ = isGl;
        emit openGlRhiChanged();
    }
    updateTimer();
}

void DiyarDeskGlItem::onWindowChanged(QQuickWindow *win) {
    onSceneGraphInitialized();
    if (win) {
        connect(win,
                &QQuickWindow::sceneGraphInitialized,
                this,
                &DiyarDeskGlItem::onSceneGraphInitialized,
                Qt::UniqueConnection);
    }
}

void DiyarDeskGlItem::updateTimer() {
    if (paused_ || !openGlRhi_ || !available()) {
        tickTimer_.stop();
        return;
    }
    tickTimer_.setInterval(lowMode_ ? 66 : 33);
    if (!tickTimer_.isActive()) {
        tickTimer_.start();
    }
}

// ---------------------------------------------------------------------------
// DiyarDeskGlHeadlessItem (Vulkan / Software Qt6 RHI fallback via EGL)
// ---------------------------------------------------------------------------

DiyarDeskGlHeadlessItem::DiyarDeskGlHeadlessItem(QQuickItem *parent) : QQuickItem(parent) {
    setFlag(ItemHasContents, true);
    frameTimer_.start();
    connect(&tickTimer_, &QTimer::timeout, this, &DiyarDeskGlHeadlessItem::onTick);
    updateTimer();
}

DiyarDeskGlHeadlessItem::~DiyarDeskGlHeadlessItem() {
    tickTimer_.stop();
    if (engine_ && DdglLib::instance().isLoaded()) {
        DdglLib::instance().hl_destroy(engine_);
        engine_ = nullptr;
    }
}

void DiyarDeskGlHeadlessItem::setThemeId(const QString &id) {
    if (themeId_ == id) {
        return;
    }
    themeId_ = id;
    QJsonObject obj;
    obj.insert(QStringLiteral("t"), QStringLiteral("theme"));
    obj.insert(QStringLiteral("id"), themeId_);
    postJson(QString::fromUtf8(QJsonDocument(obj).toJson(QJsonDocument::Compact)));
    emit themeIdChanged();
}

void DiyarDeskGlHeadlessItem::setPaused(bool p) {
    if (paused_ == p) {
        return;
    }
    paused_ = p;
    if (engine_ && DdglLib::instance().isLoaded()) {
        DdglLib::instance().hl_set_paused(engine_, paused_ ? 1 : 0);
    }
    emit pausedChanged();
    updateTimer();
    update();
}

void DiyarDeskGlHeadlessItem::setLowMode(bool low) {
    if (lowMode_ == low) {
        return;
    }
    lowMode_ = low;
    if (engine_ && DdglLib::instance().isLoaded()) {
        DdglLib::instance().hl_set_low_mode(engine_, lowMode_ ? 1 : 0);
    }
    postJson(lowMode_ ? QStringLiteral("{\"t\":\"lowMode\",\"v\":true}")
                      : QStringLiteral("{\"t\":\"lowMode\",\"v\":false}"));
    emit lowModeChanged();
    updateTimer();
    update();
}

bool DiyarDeskGlHeadlessItem::available() const {
    return DdglLib::instance().isLoaded();
}

void DiyarDeskGlHeadlessItem::postJson(const QString &json) {
    if (json.isEmpty()) {
        return;
    }
    if (engine_ && DdglLib::instance().isLoaded()) {
        const QByteArray utf8 = json.toUtf8();
        DdglLib::instance().hl_post_json(engine_, utf8.constData());
    } else if (pendingJson_.size() < 256) {
        pendingJson_.append(json);
    }
    update();
}

void DiyarDeskGlHeadlessItem::dispatchMouse(int kind, qreal cssX, qreal cssY) {
    if (engine_ && DdglLib::instance().isLoaded()) {
        DdglLib::instance().hl_mouse(engine_, kind, static_cast<float>(cssX), static_cast<float>(cssY));
    }
}

bool DiyarDeskGlHeadlessItem::isNativeTheme(const QString &themeId) const {
    return DdglLib::instance().isNativeTheme(themeId);
}

QString DiyarDeskGlHeadlessItem::readFileText(const QString &path) const {
    return readTextFile(path);
}

void DiyarDeskGlHeadlessItem::ensureEngine(int w, int h, float dpr) {
    DdglLib &lib = DdglLib::instance();
    if (!lib.isLoaded()) {
        return;
    }
    w = std::max(64, w);
    h = std::max(64, h);
    dpr = std::max(0.5f, dpr);
    if (!engine_) {
        engine_ = lib.hl_create(w, h, dpr);
        if (!engine_) {
            return;
        }
        engineW_ = w;
        engineH_ = h;
        engineDpr_ = dpr;
        lib.hl_set_low_mode(engine_, lowMode_ ? 1 : 0);
        lib.hl_set_paused(engine_, paused_ ? 1 : 0);
        QJsonObject obj;
        obj.insert(QStringLiteral("t"), QStringLiteral("theme"));
        obj.insert(QStringLiteral("id"), themeId_);
        const QByteArray raw = QJsonDocument(obj).toJson(QJsonDocument::Compact);
        lib.hl_post_json(engine_, raw.constData());
        for (const QString &msg : pendingJson_) {
            const QByteArray utf8 = msg.toUtf8();
            lib.hl_post_json(engine_, utf8.constData());
        }
        pendingJson_.clear();
        frameTimer_.restart();
    } else if (engineW_ != w || engineH_ != h || std::abs(engineDpr_ - dpr) > 0.01f) {
        engineW_ = w;
        engineH_ = h;
        engineDpr_ = dpr;
        lib.hl_resize(engine_, w, h, dpr);
    }
}

void DiyarDeskGlHeadlessItem::drainEvents() {
    if (!engine_ || !DdglLib::instance().isLoaded()) {
        return;
    }
    char buf[16384];
    for (int i = 0; i < 16; ++i) {
        const int n = DdglLib::instance().hl_poll_event(engine_, buf, static_cast<int>(sizeof(buf)));
        if (n <= 0) {
            break;
        }
        const QString msg = QString::fromUtf8(buf, n);
        QMetaObject::invokeMethod(
            this,
            [this, msg]() { emit engineEvent(msg); },
            Qt::QueuedConnection);
    }
}

QSGNode *DiyarDeskGlHeadlessItem::updatePaintNode(QSGNode *oldNode, UpdatePaintNodeData *) {
    DdglLib &lib = DdglLib::instance();
    if (!lib.isLoaded() || !window()) {
        return oldNode;
    }
    const qreal dpr = std::max<qreal>(0.5, window()->effectiveDevicePixelRatio());
    const int w = std::max(64, static_cast<int>(std::round(width() * dpr)));
    const int h = std::max(64, static_cast<int>(std::round(height() * dpr)));
    ensureEngine(w, h, static_cast<float>(dpr));
    if (!engine_) {
        return oldNode;
    }

    const int needed = w * h * 4;
    if (static_cast<int>(rgbaBuf_.size()) != needed) {
        rgbaBuf_.resize(needed);
    }
    const qint64 elapsedMs = frameTimer_.restart();
    const float dt = paused_ ? 0.0f : std::clamp(static_cast<float>(elapsedMs) / 1000.0f, 0.0f, 0.08f);
    const int written = lib.hl_render_rgba(engine_, dt, rgbaBuf_.data(), needed);
    drainEvents();
    if (written != needed) {
        return oldNode;
    }

    QImage img(rgbaBuf_.data(), w, h, w * 4, QImage::Format_RGBA8888);
    QSGTexture *tex = window()->createTextureFromImage(img);
    if (!tex) {
        return oldNode;
    }
    auto *node = static_cast<QSGSimpleTextureNode *>(oldNode);
    if (!node) {
        node = new QSGSimpleTextureNode();
        node->setOwnsTexture(true);
    }
    node->setTexture(tex);
    node->setRect(boundingRect());
    return node;
}

void DiyarDeskGlHeadlessItem::onTick() {
    if (!paused_ && isVisible()) {
        update();
    }
}

void DiyarDeskGlHeadlessItem::updateTimer() {
    if (paused_ || !available()) {
        tickTimer_.stop();
        return;
    }
    tickTimer_.setInterval(lowMode_ ? 66 : 33);
    if (!tickTimer_.isActive()) {
        tickTimer_.start();
    }
}

// ---------------------------------------------------------------------------
// DiyarDeskBridge (lightweight QFile / theme query helper for QML)
// ---------------------------------------------------------------------------

bool DiyarDeskBridge::available() const {
    return DdglLib::instance().isLoaded();
}

QString DiyarDeskBridge::libraryPath() const {
    return DdglLib::instance().libraryPath();
}

QString DiyarDeskBridge::readFileText(const QString &path) const {
    return readTextFile(path);
}

bool DiyarDeskBridge::fileExists(const QString &path) const {
    return QFileInfo::exists(path);
}

bool DiyarDeskBridge::isNativeTheme(const QString &themeId) const {
    return DdglLib::instance().isNativeTheme(themeId);
}

// ---------------------------------------------------------------------------
// Plugin registration
// ---------------------------------------------------------------------------

void DiyarDeskGlPlugin::registerTypes(const char *uri) {
    qmlRegisterType<DiyarDeskBridge>(uri, 1, 0, "DiyarDeskBridge");
    qmlRegisterType<DiyarDeskGlItem>(uri, 1, 0, "DiyarDeskGlItem");
    qmlRegisterType<DiyarDeskGlHeadlessItem>(uri, 1, 0, "DiyarDeskGlHeadlessItem");
}
