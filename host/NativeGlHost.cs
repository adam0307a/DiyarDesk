using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

// WinForms child control with CS_OWNDC dedicated device context for OpenGL 3.3 WGL rendering.
sealed class GlSurfaceControl : Control
{
    const int CS_OWNDC = 0x0020;
    const int WS_CLIPSIBLINGS = 0x04000000;
    const int WS_CLIPCHILDREN = 0x02000000;

    public GlSurfaceControl()
    {
        SetStyle(ControlStyles.Opaque | ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint, true);
        DoubleBuffered = false;
        TabStop = false;
    }

    protected override CreateParams CreateParams
    {
        get
        {
            CreateParams cp = base.CreateParams;
            cp.ClassStyle |= CS_OWNDC;
            cp.Style |= WS_CLIPSIBLINGS | WS_CLIPCHILDREN;
            return cp;
        }
    }

    protected override void OnPaintBackground(PaintEventArgs pevent) { }
    protected override void OnPaint(PaintEventArgs e) { }
}

// P/Invoke bridge to native Rust OpenGL 3.3 Core renderer (diyardesk_gl.dll).
sealed class NativeGlHost : IDisposable
{
    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern IntPtr ddgl_create(IntPtr hwnd, int width, int height, float dpiScale);

    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern void ddgl_destroy(IntPtr engine);

    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern void ddgl_resize(IntPtr engine, int width, int height, float dpiScale);

    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern void ddgl_set_paused(IntPtr engine, int paused);

    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern void ddgl_set_low_mode(IntPtr engine, int low);

    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern void ddgl_post_json(IntPtr engine, byte[] utf8NullTerminated);

    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern int ddgl_mouse(IntPtr engine, int kind, float cssX, float cssY);

    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern void ddgl_key(IntPtr engine, int kind, int vk, byte[] utf8NullTerminated);

    [DllImport("diyardesk_gl.dll", CallingConvention = CallingConvention.Cdecl)]
    static extern int ddgl_poll_event(IntPtr engine, byte[] buf, int bufLen);

    public static bool IsAvailable()
    {
        try
        {
            string dll = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "diyardesk_gl.dll");
            return File.Exists(dll);
        }
        catch { return false; }
    }

    public GlSurfaceControl Surface { get; private set; }
    public event Action<string> EventReceived;

    IntPtr engine;
    Timer eventTimer;
    readonly byte[] pollBuf = new byte[16384];
    float lastDpi = 1.0f;
    bool disposed;

    public bool Start(Control parent, float dpiScale)
    {
        if (disposed || engine != IntPtr.Zero) return engine != IntPtr.Zero;
        if (!IsAvailable()) return false;
        lastDpi = dpiScale > 0.1f ? dpiScale : 1.0f;

        Surface = new GlSurfaceControl();
        Surface.Dock = DockStyle.Fill;
        parent.Controls.Add(Surface);
        Surface.BringToFront();
        Surface.CreateControl();
        Surface.Resize += delegate { Resize(lastDpi); };

        int w = Math.Max(64, Surface.Width);
        int h = Math.Max(64, Surface.Height);
        try
        {
            engine = ddgl_create(Surface.Handle, w, h, lastDpi);
        }
        catch (Exception ex)
        {
            Log.Write("Native OpenGL başlatma hatası: " + ex.Message);
            engine = IntPtr.Zero;
        }

        if (engine == IntPtr.Zero)
        {
            Dispose();
            return false;
        }

        eventTimer = new Timer();
        eventTimer.Interval = 40;
        eventTimer.Tick += delegate { DrainEvents(); };
        eventTimer.Start();
        Log.Write("Rust OpenGL 3.3 Core motoru başlatıldı (" + w + "x" + h + ", dpi=" + lastDpi.ToString("0.00") + ")");
        return true;
    }

    public bool IsRunning { get { return !disposed && engine != IntPtr.Zero; } }

    public void Resize(float dpiScale)
    {
        if (!IsRunning || Surface == null || Surface.IsDisposed) return;
        lastDpi = dpiScale > 0.1f ? dpiScale : 1.0f;
        try { ddgl_resize(engine, Math.Max(64, Surface.Width), Math.Max(64, Surface.Height), lastDpi); }
        catch { }
    }

    public void SetPaused(bool paused)
    {
        if (!IsRunning) return;
        try { ddgl_set_paused(engine, paused ? 1 : 0); } catch { }
    }

    public void SetLowMode(bool low)
    {
        if (!IsRunning) return;
        try { ddgl_set_low_mode(engine, low ? 1 : 0); } catch { }
    }

    public void PostJson(string json)
    {
        if (!IsRunning || string.IsNullOrEmpty(json)) return;
        try
        {
            byte[] raw = Encoding.UTF8.GetBytes(json);
            byte[] nt = new byte[raw.Length + 1];
            Buffer.BlockCopy(raw, 0, nt, 0, raw.Length);
            ddgl_post_json(engine, nt);
        }
        catch { }
    }

    // kind: 0=move, 1=down, 2=up, 3=leave
    public bool DispatchMouse(int kind, float cssX, float cssY)
    {
        if (!IsRunning) return false;
        try
        {
            bool handled = ddgl_mouse(engine, kind, cssX, cssY) != 0;
            if (kind == 1 || kind == 2) DrainEvents();
            return handled;
        }
        catch { return false; }
    }

    // kind: 0=text insert, 1=special key (vk: 0x08 Backspace, 0x0D Enter, 0x1B Escape)
    public void DispatchKey(int kind, int vk, string text)
    {
        if (!IsRunning) return;
        try
        {
            byte[] nt = null;
            if (!string.IsNullOrEmpty(text))
            {
                byte[] raw = Encoding.UTF8.GetBytes(text);
                nt = new byte[raw.Length + 1];
                Buffer.BlockCopy(raw, 0, nt, 0, raw.Length);
            }
            ddgl_key(engine, kind, vk, nt);
        }
        catch { }
    }

    void DrainEvents()
    {
        if (!IsRunning) return;
        for (int i = 0; i < 16; i++)
        {
            int n = 0;
            try { n = ddgl_poll_event(engine, pollBuf, pollBuf.Length); }
            catch { break; }
            if (n <= 0) break;
            string msg = Encoding.UTF8.GetString(pollBuf, 0, n);
            var cb = EventReceived;
            if (cb != null)
            {
                try { cb(msg); } catch { }
            }
        }
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        if (eventTimer != null)
        {
            try { eventTimer.Stop(); eventTimer.Dispose(); } catch { }
            eventTimer = null;
        }
        if (engine != IntPtr.Zero)
        {
            try { ddgl_destroy(engine); } catch { }
            engine = IntPtr.Zero;
        }
        if (Surface != null)
        {
            try
            {
                if (Surface.Parent != null) Surface.Parent.Controls.Remove(Surface);
                Surface.Dispose();
            }
            catch { }
            Surface = null;
        }
    }
}
