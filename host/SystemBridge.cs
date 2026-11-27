// Windows Phase 5 helpers:
//   - WinResourceWatchdog: detects runaway WebView2 renderer RAM/CPU leaks
//   - WinMpvBackend: optional hardware-accelerated mpv (--vo=gpu-next --hwdec=auto-safe) for imported videos
//   - WinSystemBridge: opt-in local CPU / RAM / Battery telemetry bridge (zero network)
//   - ScreenSaverForm: safe local Windows screensaver mode (/s, /c, /p)
// Strictly C# 5 compatible (.NET Framework 4.x csc.exe).

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using Microsoft.Web.WebView2.WinForms;

sealed class WinResourceWatchdog
{
    readonly long maxRssBytes;
    readonly int strikesNeeded;
    int strikes;
    DateTime lastCheck = DateTime.MinValue;

    public WinResourceWatchdog(int maxRssMb, int strikesNeeded)
    {
        this.maxRssBytes = (long)Math.Max(256, maxRssMb) * 1024L * 1024L;
        this.strikesNeeded = Math.Max(1, strikesNeeded);
    }

    public void Reset()
    {
        strikes = 0;
    }

    public string Poll(uint webViewBrowserPid)
    {
        if (webViewBrowserPid == 0) return null;
        if ((DateTime.UtcNow - lastCheck).TotalSeconds < 10.0) return null;
        lastCheck = DateTime.UtcNow;
        try
        {
            long totalWs = 0;
            using (Process browser = Process.GetProcessById((int)webViewBrowserPid))
            {
                totalWs += browser.WorkingSet64;
            }
            // Also sum msedgewebview2 renderer siblings if excessive
            Process[] procs = Process.GetProcessesByName("msedgewebview2");
            long maxSingle = totalWs;
            for (int i = 0; i < procs.Length; i++)
            {
                try
                {
                    long ws = procs[i].WorkingSet64;
                    if (ws > maxSingle) maxSingle = ws;
                }
                catch { }
                finally { try { procs[i].Dispose(); } catch { } }
            }
            if (maxSingle >= maxRssBytes)
            {
                strikes++;
                if (strikes >= strikesNeeded)
                {
                    strikes = 0;
                    return "memory_limit (" + (maxSingle / (1024L * 1024L)) + " MB)";
                }
            }
            else
            {
                strikes = 0;
            }
        }
        catch { }
        return null;
    }
}

sealed class WinMpvBackend
{
    Process proc;

    public static string FindMpvExe()
    {
        string local = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "mpv.exe");
        if (File.Exists(local)) return local;
        string pathEnv = Environment.GetEnvironmentVariable("PATH") ?? "";
        string[] dirs = pathEnv.Split(Path.PathSeparator);
        for (int i = 0; i < dirs.Length; i++)
        {
            try
            {
                string cand = Path.Combine(dirs[i].Trim(), "mpv.exe");
                if (File.Exists(cand)) return cand;
            }
            catch { }
        }
        return null;
    }

    public static string FindThemeVideo(string themeId)
    {
        if (string.IsNullOrEmpty(themeId)) return null;
        string dir = Path.Combine(CustomThemes.Dir, themeId);
        if (!Directory.Exists(dir)) return null;
        string[] files = Directory.GetFiles(dir);
        for (int i = 0; i < files.Length; i++)
        {
            string ext = (Path.GetExtension(files[i]) ?? "").ToLowerInvariant();
            if (CustomThemes.HasExt(ext, CustomThemes.VideoExts)) return files[i];
        }
        return null;
    }

    public bool Start(string videoPath, IntPtr hostHwnd, double speed, double brightness)
    {
        Stop();
        string mpvExe = FindMpvExe();
        if (string.IsNullOrEmpty(mpvExe) || string.IsNullOrEmpty(videoPath) || !File.Exists(videoPath) || hostHwnd == IntPtr.Zero)
            return false;
        double spd = Math.Max(0.25, Math.Min(2.0, speed));
        int mpvBright = (int)Math.Round((Math.Max(0.4, Math.Min(1.4, brightness)) - 1.0) * 60.0);
        string args = "--vo=gpu-next --hwdec=auto-safe --no-audio --loop-file=inf --no-osc --no-osd-bar "
            + "--no-input-default-bindings --no-input-cursor --really-quiet --keep-open=yes "
            + "--speed=" + spd.ToString("0.00", CultureInfo.InvariantCulture)
            + " --brightness=" + mpvBright.ToString(CultureInfo.InvariantCulture)
            + " --wid=" + hostHwnd.ToInt64().ToString(CultureInfo.InvariantCulture)
            + " \"" + videoPath + "\"";
        try
        {
            var psi = new ProcessStartInfo(mpvExe, args);
            psi.UseShellExecute = false;
            psi.CreateNoWindow = true;
            proc = Process.Start(psi);
            Log.Write("mpv 硬件视频后端已启动：" + Path.GetFileName(videoPath));
            return proc != null;
        }
        catch (Exception ex)
        {
            Log.Write("mpv 启动失败（回退到 WebView2）：" + ex.Message);
            proc = null;
            return false;
        }
    }

    public void Stop()
    {
        if (proc == null) return;
        try
        {
            if (!proc.HasExited)
            {
                proc.Kill();
                proc.WaitForExit(1000);
            }
        }
        catch { }
        try { proc.Dispose(); } catch { }
        proc = null;
    }
}

sealed class WinAudioPeakMeter : IDisposable
{
    static readonly Guid CLSID_MMDeviceEnumerator = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
    static readonly Guid IID_IAudioMeterInformation = new Guid("C02216F6-8C67-4B5B-9D00-D008E73E0064");

    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDeviceEnumerator
    {
        [PreserveSig]
        int EnumAudioEndpoints(int dataFlow, uint dwStateMask, [MarshalAs(UnmanagedType.IUnknown)] out object ppDevices);
        [PreserveSig]
        int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice ppEndpoint);
    }

    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDevice
    {
        [PreserveSig]
        int Activate(ref Guid iid, uint dwClsCtx, IntPtr pActivationParams, [MarshalAs(UnmanagedType.IUnknown)] out object ppInterface);
    }

    [ComImport, Guid("C02216F6-8C67-4B5B-9D00-D008E73E0064"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IAudioMeterInformation
    {
        [PreserveSig]
        int GetPeakValue(out float pfPeak);
    }

    IMMDeviceEnumerator enumerator;
    IMMDevice device;
    IAudioMeterInformation meter;
    DateTime nextRetryUtc = DateTime.MinValue;

    bool EnsureMeter()
    {
        if (meter != null) return true;
        DateTime now = DateTime.UtcNow;
        if (now < nextRetryUtc) return false;
        nextRetryUtc = now.AddSeconds(2.0);
        try
        {
            Type t = Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator, false);
            if (t == null) return false;
            enumerator = Activator.CreateInstance(t) as IMMDeviceEnumerator;
            if (enumerator == null) return false;
            IMMDevice dev;
            if (enumerator.GetDefaultAudioEndpoint(0 /* eRender */, 1 /* eMultimedia */, out dev) != 0 || dev == null)
                return false;
            device = dev;
            Guid iid = IID_IAudioMeterInformation;
            object iface;
            if (device.Activate(ref iid, 0x17 /* CLSCTX_ALL */, IntPtr.Zero, out iface) != 0 || iface == null)
                return false;
            meter = iface as IAudioMeterInformation;
            return meter != null;
        }
        catch
        {
            ReleaseCom();
            return false;
        }
    }

    void ReleaseCom()
    {
        if (meter != null) { try { Marshal.ReleaseComObject(meter); } catch { } meter = null; }
        if (device != null) { try { Marshal.ReleaseComObject(device); } catch { } device = null; }
        if (enumerator != null) { try { Marshal.ReleaseComObject(enumerator); } catch { } enumerator = null; }
    }

    public float SamplePeak()
    {
        if (!EnsureMeter()) return 0.0f;
        try
        {
            float peak;
            if (meter.GetPeakValue(out peak) == 0)
            {
                if (float.IsNaN(peak) || float.IsInfinity(peak)) return 0.0f;
                return Math.Max(0.0f, Math.Min(1.0f, peak));
            }
            ReleaseCom();
            nextRetryUtc = DateTime.UtcNow.AddSeconds(2.0);
        }
        catch
        {
            ReleaseCom();
            nextRetryUtc = DateTime.UtcNow.AddSeconds(2.0);
        }
        return 0.0f;
    }

    public void Dispose()
    {
        ReleaseCom();
    }
}

sealed class WinSystemBridge : IDisposable
{
    readonly WinAudioPeakMeter audioMeter = new WinAudioPeakMeter();
    [StructLayout(LayoutKind.Sequential)]
    struct FILETIME { public uint Low; public uint High; }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    class MEMORYSTATUSEX
    {
        public uint dwLength = (uint)Marshal.SizeOf(typeof(MEMORYSTATUSEX));
        public uint dwMemoryLoad;
        public ulong ullTotalPhys;
        public ulong ullAvailPhys;
        public ulong ullTotalPageFile;
        public ulong ullAvailPageFile;
        public ulong ullTotalVirtual;
        public ulong ullAvailVirtual;
        public ulong ullAvailExtendedVirtual;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct SYSTEM_POWER_STATUS
    {
        public byte ACLineStatus;
        public byte BatteryFlag;
        public byte BatteryLifePercent;
        public byte SystemStatusFlag;
        public int BatteryLifeTime;
        public int BatteryFullLifeTime;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetSystemTimes(out FILETIME idleTime, out FILETIME kernelTime, out FILETIME userTime);

    [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    static extern bool GlobalMemoryStatusEx([In, Out] MEMORYSTATUSEX lpBuffer);

    [DllImport("kernel32.dll")]
    static extern bool GetSystemPowerStatus(out SYSTEM_POWER_STATUS lpSystemPowerStatus);

    ulong prevIdle, prevKernel, prevUser;
    bool hasPrev;

    static ulong ToUInt64(FILETIME ft) { return ((ulong)ft.High << 32) | ft.Low; }

    public float SampleAudioPeak()
    {
        return audioMeter.SamplePeak();
    }

    public void Dispose()
    {
        audioMeter.Dispose();
    }

    public string SampleJson()
    {
        double cpuPct = 0.0;
        FILETIME fi, fk, fu;
        if (GetSystemTimes(out fi, out fk, out fu))
        {
            ulong idle = ToUInt64(fi), kernel = ToUInt64(fk), user = ToUInt64(fu);
            if (hasPrev)
            {
                ulong dIdle = idle >= prevIdle ? idle - prevIdle : 0;
                ulong dTotal = (kernel >= prevKernel ? kernel - prevKernel : 0) + (user >= prevUser ? user - prevUser : 0);
                if (dTotal > 0)
                {
                    cpuPct = Math.Max(0.0, Math.Min(100.0, (1.0 - ((double)dIdle / (double)dTotal)) * 100.0));
                }
            }
            prevIdle = idle; prevKernel = kernel; prevUser = user; hasPrev = true;
        }

        double memPct = 0.0;
        try
        {
            var ms = new MEMORYSTATUSEX();
            if (GlobalMemoryStatusEx(ms)) memPct = ms.dwMemoryLoad;
        }
        catch { }

        int battery = 100;
        bool charging = true;
        try
        {
            SYSTEM_POWER_STATUS ps;
            if (GetSystemPowerStatus(out ps))
            {
                if (ps.BatteryLifePercent <= 100) battery = ps.BatteryLifePercent;
                charging = ps.ACLineStatus != 0;
            }
        }
        catch { }

        float audioPeak = audioMeter.SamplePeak();
        return Json.Stringify(Json.Obj(
            "t", "systemStats",
            "cpu", Math.Round(cpuPct, 1),
            "mem", Math.Round(memPct, 1),
            "battery", battery,
            "charging", charging,
            "audioPeak", Math.Round(audioPeak, 3)));
    }
}

sealed class ScreenSaverForm : Form
{
    readonly string pageDir;
    readonly string pageRel;
    readonly WebView2 web = new WebView2();
    Point initialMouse = Point.Empty;
    bool mouseArmed;

    public ScreenSaverForm(string pageDir, string pageRel)
    {
        this.pageDir = pageDir;
        this.pageRel = pageRel;
        FormBorderStyle = FormBorderStyle.None;
        WindowState = FormWindowState.Maximized;
        TopMost = true;
        BackColor = Color.Black;
        ShowInTaskbar = false;
        Bounds = Screen.PrimaryScreen.Bounds;
        web.Dock = DockStyle.Fill;
        Controls.Add(web);
        KeyPreview = true;
        KeyDown += delegate { Close(); };
        MouseDown += delegate { Close(); };
        MouseMove += delegate(object s, MouseEventArgs e)
        {
            if (!mouseArmed) { initialMouse = Cursor.Position; mouseArmed = true; return; }
            Point p = Cursor.Position;
            if (Math.Abs(p.X - initialMouse.X) > 12 || Math.Abs(p.Y - initialMouse.Y) > 12) Close();
        };
        Load += delegate { InitAsync(); };
    }

    async void InitAsync()
    {
        try
        {
            Cursor.Hide();
            var env = await Microsoft.Web.WebView2.Core.CoreWebView2Environment.CreateAsync(null, Path.Combine(Log.DataDir, "WebView2-Scr"));
            await web.EnsureCoreWebView2Async(env);
            var core = web.CoreWebView2;
            core.SetVirtualHostNameToFolderMapping("app.local", pageDir, Microsoft.Web.WebView2.Core.CoreWebView2HostResourceAccessKind.Allow);
            WebCustomHelper.Attach(core, pageDir);
            WebViewSecurity.Harden(core, false);
            try { await WebViewSecurity.InjectCspAsync(core); } catch { }
            string url = WebCustomHelper.PageUrl(pageRel) + "?wallpaper=1";
            core.Navigate(url);
        }
        catch { Close(); }
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        try { Cursor.Show(); } catch { }
        base.OnFormClosed(e);
    }
}
