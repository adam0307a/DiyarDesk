import Cocoa
import Darwin
import OpenGL.GL3
import DiyarDeskCore

/// Dynamic loader for `libdiyardesk_gl.dylib` (embedded OpenGL 3.3 Core C API).
final class NativeGlLibrary {
    typealias CreateFn = @convention(c) (Int32, Int32, Float) -> UnsafeMutableRawPointer?
    typealias DestroyFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    typealias ResizeFn = @convention(c) (UnsafeMutableRawPointer?, Int32, Int32, Float) -> Void
    typealias SetPausedFn = @convention(c) (UnsafeMutableRawPointer?, Int32) -> Void
    typealias SetLowModeFn = @convention(c) (UnsafeMutableRawPointer?, Int32) -> Void
    typealias PostJsonFn = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void
    typealias RenderFn = @convention(c) (UnsafeMutableRawPointer?, Float, UInt32) -> Int32
    typealias PollEventFn = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutablePointer<CChar>?, Int32) -> Int32

    static let shared: NativeGlLibrary? = NativeGlLibrary()
    static var isAvailable: Bool { return shared != nil }

    private let handle: UnsafeMutableRawPointer
    let create: CreateFn
    let destroy: DestroyFn
    let resize: ResizeFn
    let setPaused: SetPausedFn
    let setLowMode: SetLowModeFn
    let postJson: PostJsonFn
    let render: RenderFn
    let pollEvent: PollEventFn

    private init?() {
        let candidates = NativeGlLibrary.candidateDylibPaths()
        var opened: UnsafeMutableRawPointer? = nil
        for path in candidates {
            if let h = dlopen(path, RTLD_LAZY | RTLD_LOCAL) {
                opened = h
                break
            }
        }
        guard let lib = opened else { return nil }

        func sym<T>(_ name: String, as type: T.Type) -> T? {
            guard let ptr = dlsym(lib, name) else { return nil }
            return unsafeBitCast(ptr, to: T.self)
        }

        guard let fCreate = sym("ddgl_embedded_create", as: CreateFn.self),
              let fDestroy = sym("ddgl_embedded_destroy", as: DestroyFn.self),
              let fResize = sym("ddgl_embedded_resize", as: ResizeFn.self),
              let fSetPaused = sym("ddgl_embedded_set_paused", as: SetPausedFn.self),
              let fSetLowMode = sym("ddgl_embedded_set_low_mode", as: SetLowModeFn.self),
              let fPostJson = sym("ddgl_embedded_post_json", as: PostJsonFn.self),
              let fRender = sym("ddgl_embedded_render", as: RenderFn.self),
              let fPollEvent = sym("ddgl_embedded_poll_event", as: PollEventFn.self) else {
            dlclose(lib)
            return nil
        }

        self.handle = lib
        self.create = fCreate
        self.destroy = fDestroy
        self.resize = fResize
        self.setPaused = fSetPaused
        self.setLowMode = fSetLowMode
        self.postJson = fPostJson
        self.render = fRender
        self.pollEvent = fPollEvent
    }

    private static func candidateDylibPaths() -> [String] {
        var paths: [String] = []
        if let fw = Bundle.main.privateFrameworksURL?.appendingPathComponent("libdiyardesk_gl.dylib").path {
            paths.append(fw)
        }
        if let exeDir = Bundle.main.executableURL?.deletingLastPathComponent() {
            paths.append(exeDir.appendingPathComponent("libdiyardesk_gl.dylib").path)
            paths.append(exeDir.deletingLastPathComponent().appendingPathComponent("Frameworks/libdiyardesk_gl.dylib").path)
        }
        if let res = Bundle.main.resourceURL?.appendingPathComponent("libdiyardesk_gl.dylib").path {
            paths.append(res)
        }
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        for base in [cwd, cwd.deletingLastPathComponent()] {
            paths.append(base.appendingPathComponent("native/diyardesk-gl/target/release/libdiyardesk_gl.dylib").path)
            paths.append(base.appendingPathComponent("native/diyardesk-gl/target/debug/libdiyardesk_gl.dylib").path)
        }
        paths.append("libdiyardesk_gl.dylib")
        return paths
    }
}

/// Hardware OpenGL 3.2/3.3 Core wallpaper view backed by Rust `libdiyardesk_gl.dylib`.
final class NativeGlView: NSOpenGLView {
    private let glLib: NativeGlLibrary
    private var engine: UnsafeMutableRawPointer?
    private var pendingMessages: [String] = []
    private var renderTimer: Timer?
    private var lastTick = CFAbsoluteTimeGetCurrent()
    private var paused = false
    private var lowMode = false
    private var dpiScale: Float = 1.0
    private var tornDown = false

    var onMessage: ((String) -> Void)?

    convenience init?(frame: NSRect, lib: NativeGlLibrary) {
        let attrs: [NSOpenGLPixelFormatAttribute] = [
            UInt32(NSOpenGLPFAOpenGLProfile), UInt32(NSOpenGLProfileVersion3_2Core),
            UInt32(NSOpenGLPFADoubleBuffer),
            UInt32(NSOpenGLPFAColorSize), 24,
            UInt32(NSOpenGLPFAAlphaSize), 8,
            UInt32(NSOpenGLPFADepthSize), 0,
            UInt32(NSOpenGLPFAAccelerated),
            0
        ]
        guard let pf = NSOpenGLPixelFormat(attributes: attrs) else { return nil }
        self.init(frame: frame, pixelFormat: pf, lib: lib)
    }

    init?(frame: NSRect, pixelFormat: NSOpenGLPixelFormat, lib: NativeGlLibrary) {
        self.glLib = lib
        super.init(frame: frame, pixelFormat: pixelFormat)
        self.wantsBestResolutionOpenGLSurface = true
        self.wantsLayer = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func prepareOpenGL() {
        super.prepareOpenGL()
        guard !tornDown, engine == nil, let ctx = openGLContext, let cgl = ctx.cglContextObj else { return }
        CGLLockContext(cgl)
        defer { CGLUnlockContext(cgl) }
        ctx.makeCurrentContext()

        var swapInterval: GLint = 1
        ctx.setValues(&swapInterval, for: .swapInterval)

        let backing = convertToBacking(bounds)
        let scale = bounds.width > 0 ? Float(backing.width / bounds.width) : 1.0
        dpiScale = max(1.0, scale)
        let w = max(64, Int32(backing.width.rounded()))
        let h = max(64, Int32(backing.height.rounded()))

        engine = glLib.create(w, h, dpiScale)
        if let eng = engine {
            glLib.setLowMode(eng, lowMode ? 1 : 0)
            glLib.setPaused(eng, paused ? 1 : 0)
            for msg in pendingMessages {
                msg.withCString { glLib.postJson(eng, $0) }
            }
            pendingMessages.removeAll()
        }
        rescheduleTimer()
    }

    override func reshape() {
        super.reshape()
        guard let eng = engine, let ctx = openGLContext, let cgl = ctx.cglContextObj else { return }
        CGLLockContext(cgl)
        defer { CGLUnlockContext(cgl) }
        ctx.makeCurrentContext()

        let backing = convertToBacking(bounds)
        let scale = bounds.width > 0 ? Float(backing.width / bounds.width) : 1.0
        dpiScale = max(1.0, scale)
        let w = max(64, Int32(backing.width.rounded()))
        let h = max(64, Int32(backing.height.rounded()))
        glLib.resize(eng, w, h, dpiScale)
    }

    override func draw(_ dirtyRect: NSRect) {
        renderFrame(dt: 0.0)
    }

    private func renderFrame(dt: Float) {
        guard !tornDown, let eng = engine, let ctx = openGLContext, let cgl = ctx.cglContextObj else { return }
        CGLLockContext(cgl)
        defer { CGLUnlockContext(cgl) }
        ctx.makeCurrentContext()

        _ = glLib.render(eng, dt, UInt32.max)
        ctx.flushBuffer()
        drainEvents(eng)
    }

    private func drainEvents(_ eng: UnsafeMutableRawPointer) {
        var buf = [CChar](repeating: 0, count: 16384)
        while true {
            let n = buf.withUnsafeMutableBufferPointer { ptr -> Int32 in
                guard let base = ptr.baseAddress else { return 0 }
                return glLib.pollEvent(eng, base, Int32(ptr.count))
            }
            if n <= 0 { break }
            let str = String(cString: buf)
            onMessage?(str)
        }
    }

    private func rescheduleTimer() {
        renderTimer?.invalidate()
        renderTimer = nil
        guard !tornDown, !paused, engine != nil else { return }
        lastTick = CFAbsoluteTimeGetCurrent()
        let interval = lowMode ? (1.0 / 15.0) : (1.0 / 30.0)
        renderTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { [weak self] _ in
            guard let self = self, !self.tornDown, !self.paused else { return }
            let now = CFAbsoluteTimeGetCurrent()
            let dt = Float(min(0.1, max(0.0, now - self.lastTick)))
            self.lastTick = now
            self.renderFrame(dt: dt)
        }
    }

    func postJSON(_ obj: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: obj),
              let str = String(data: data, encoding: .utf8) else { return }
        postRaw(str)
    }

    func postRaw(_ jsonStr: String) {
        guard !tornDown else { return }
        if let eng = engine {
            jsonStr.withCString { glLib.postJson(eng, $0) }
        } else {
            if pendingMessages.count > 64 { pendingMessages.removeFirst() }
            pendingMessages.append(jsonStr)
        }
    }

    func setPaused(_ pause: Bool) {
        guard paused != pause else { return }
        paused = pause
        if let eng = engine {
            glLib.setPaused(eng, pause ? 1 : 0)
        }
        rescheduleTimer()
    }

    func setLowMode(_ low: Bool) {
        guard lowMode != low else { return }
        lowMode = low
        if let eng = engine {
            glLib.setLowMode(eng, low ? 1 : 0)
        }
        rescheduleTimer()
    }

    func teardown() {
        guard !tornDown else { return }
        tornDown = true
        renderTimer?.invalidate()
        renderTimer = nil
        pendingMessages.removeAll()
        if let eng = engine {
            if let ctx = openGLContext, let cgl = ctx.cglContextObj {
                CGLLockContext(cgl)
                ctx.makeCurrentContext()
                glLib.destroy(eng)
                CGLUnlockContext(cgl)
            } else {
                glLib.destroy(eng)
            }
            engine = nil
        }
        clearGLContext()
        removeFromSuperview()
    }

    deinit {
        renderTimer?.invalidate()
    }
}
