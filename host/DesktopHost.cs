// Deskland · 桌面宿主（原型）
//
// 做四件事：
//   1. 把一个无边框窗口挂到桌面图标下面（壁纸层），里面用 WebView2 显示当前主题的页面（如 prototype/flat.html?wallpaper=1）
//   0. 设置窗口（prototype/settings.html）：选主题、调参数，实时同步到壁纸并保存到 settings.json；一键分享（壁纸页面画一张打好马赛克的桌面图）
//   2. 每 400ms 读取桌面图标的真实位置（桌面 ListView 的跨进程消息），有变化才发给页面
//   3. 前台窗口最大化 / 全屏时暂停动画；托盘图标可以暂停、重新载入、退出（退出后恢复原壁纸）
//   4. 资源管理器重建壁纸层时（例如按 Win+D），我们的窗口会被连带销毁 → 自动重新挂载
//
// 用 Windows 自带的 .NET Framework 编译器编译（C# 5 语法），见 build.ps1。

using System;
using System.Diagnostics;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;
using System.Windows.Forms;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;
using Microsoft.Win32;

static class Program
{
    public static readonly string ExitEventName = "DesklandHost_Exit" + InstallProfile.Suffix;
    public static readonly string MenuEventName = "DesklandHost_Menu" + InstallProfile.Suffix;
    public static readonly string SettingsEventName = "DesklandHost_Settings" + InstallProfile.Suffix;

    // 通知正在运行的实例（退出 / 弹出菜单）
    static void Signal(string name)
    {
        Native.AllowSetForegroundWindow(-1);            // 让运行中的实例能把菜单弹到最前面
        EventWaitHandle ev;
        if (EventWaitHandle.TryOpenExisting(name, out ev)) ev.Set();
    }

    [STAThread]
    static void Main(string[] args)
    {
        // Deskland.exe --exit ：通知正在运行的实例正常退出（恢复原壁纸）
        // Deskland.exe --menu ：让正在运行的实例在鼠标位置弹出菜单
        // Deskland.exe --settings ：让正在运行的实例打开设置窗口
        if (args.Length > 0 && args[0] == "--exit") { Signal(ExitEventName); return; }
        if (args.Length > 0 && args[0] == "--menu") { Signal(MenuEventName); return; }
        if (args.Length > 0 && args[0] == "--settings") { Signal(SettingsEventName); return; }
        if (args.Length > 0 && args[0] == "--exit-installed") { Environment.ExitCode = InstallProfile.StopInstalled(); return; }
        // Preview never migrates or shares the development edition's settings.                                 // 老名字（Habitat Wallpaper）下的数据目录、开机自启项搬过来
        bool fromAutostart = Array.IndexOf(args, "--autostart") >= 0;
        if (fromAutostart) AutoStartCheck.MarkLaunched();  // 开机自启跑起来了（不管后面是不是已经在运行）
        bool created;
        var mutex = new Mutex(true, "DesklandHost_Singleton" + InstallProfile.Suffix, out created);
        if (!created)
        {
            if (!fromAutostart) Signal(SettingsEventName); // 已经在运行：再次双击程序 = 打开设置（开机自启那次就悄悄退出）
            return;
        }
        // 按物理像素工作：桌面图标坐标、屏幕尺寸都是物理像素
        if (!Native.SetProcessDpiAwarenessContext(new IntPtr(-4))) Native.SetProcessDPIAware();
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        // 任何意外都写进日志，不弹框也不静默退出
        Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
        Application.ThreadException += delegate(object s, ThreadExceptionEventArgs e) { Log.Write("未处理的异常：" + e.Exception); };
        AppDomain.CurrentDomain.UnhandledException += delegate(object s, UnhandledExceptionEventArgs e) { Log.Write("致命异常：" + e.ExceptionObject); };
        string exeDir = AppDomain.CurrentDomain.BaseDirectory;
        string pageDir = Path.GetFullPath(Path.Combine(exeDir, @"..\..\prototype"));
        // 24H2+ 的挂法：默认做 Progman 的分层子窗口（换系统壁纸时会闪一下）；
        //   --defview：挂进图标层里、图标列表之下（实验：实测画不出来）；--legacy：挂进 WorkerW（换壁纸时会被连带销毁）
        string attach = "progman";
        bool autostart = false;                           // --autostart：开机自动启动，不弹设置窗口
        foreach (string a in args)
        {
            if (a == "--legacy") attach = "legacy";
            else if (a == "--defview") attach = "defview";
            else if (a == "--autostart") autostart = true;
            else if (!a.StartsWith("--")) pageDir = a;
        }
        WallpaperForm.AttachMode = attach;
        Application.Run(new HostContext(pageDir, !autostart));
        GC.KeepAlive(mutex);
    }
}

static class Log
{
    public static readonly string DataDir = InstallProfile.DataDir;
    public static readonly string FilePath = Path.Combine(DataDir, "host.log");

    public static void Write(string msg)
    {
        try
        {
            Directory.CreateDirectory(DataDir);
            File.AppendAllText(FilePath, DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss ") + msg + "\r\n");
        }
        catch { }
    }
}

// 2026-10-10 改名（Habitat Wallpaper → Deskland）：老用户的东西照旧能用
//   - 数据目录 %LOCALAPPDATA%\HabitatWallpaper（设置、开机自启的记录、WebView2 缓存）整个搬到 %LOCALAPPDATA%\Deskland；
//     旧程序还开着、WebView2 的文件被占着搬不动时，只把设置这些小文件拷过去
//   - 用户开着开机自启：注册表 Run 项里的 HabitatWallpaper 换成指向 Deskland.exe 的 Deskland（用户原来的选择照旧，不会替没开的人打开）
static class Rename
{
    const string OldName = "HabitatWallpaper";
    public static void Migrate()
    {
        string oldDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), OldName);
        try
        {
            if (Directory.Exists(oldDir) && !Directory.Exists(Log.DataDir))
            {
                try { Directory.Move(oldDir, Log.DataDir); }
                catch
                {
                    Directory.CreateDirectory(Log.DataDir);
                    foreach (string src in Directory.GetFiles(oldDir))
                        if (!src.EndsWith(".log", StringComparison.OrdinalIgnoreCase)) File.Copy(src, Path.Combine(Log.DataDir, Path.GetFileName(src)), false);
                }
                Log.Write("改名：数据目录从 " + oldDir + " 搬到 " + Log.DataDir);
            }
        }
        catch (Exception ex) { Log.Write("改名：搬数据目录失败：" + ex.Message); }
        try
        {
            using (RegistryKey k = Registry.CurrentUser.OpenSubKey(HostContext.RunKey, true))
            {
                if (k != null && k.GetValue(OldName) != null)
                {
                    k.DeleteValue(OldName, false);
                    k.SetValue(HostContext.RunName, "\"" + Application.ExecutablePath + "\" --autostart");
                    Log.Write("改名：开机自启的启动项 " + OldName + " 换成 " + HostContext.RunName);
                }
            }
        }
        catch (Exception ex) { Log.Write("改名：换开机自启的启动项失败：" + ex.Message); }
    }
}

// 开机自启有没有被拦（比如被火绒、360 的"启动项管理"禁止了：注册表里的启动项还在，但登录时不让运行）：
// 每次带 --autostart 启动时记下时间；用户手动打开程序时，如果开着开机自启、本次登录已经过了 3 分钟，
// 却没有本次登录之后的自启记录 → 登录时那次没跑起来
static class AutoStartCheck
{
    static string LastPath { get { return Path.Combine(Log.DataDir, "autostart.last"); } }
    static string EnabledPath { get { return Path.Combine(Log.DataDir, "autostart.enabled"); } }

    static void Stamp(string path)
    {
        try { Directory.CreateDirectory(Log.DataDir); File.WriteAllText(path, DateTime.UtcNow.Ticks.ToString()); } catch { }
    }
    static DateTime ReadStamp(string path)
    {
        try { return new DateTime(long.Parse(File.ReadAllText(path).Trim()), DateTimeKind.Utc); } catch { return DateTime.MinValue; }
    }
    public static void MarkLaunched() { Stamp(LastPath); }
    public static void MarkEnabled() { Stamp(EnabledPath); }

    // 本次登录的时间（UTC）：WTSQuerySessionInformation(WTSSessionInfo) 里的 LogonTime
    public static DateTime LogonTimeUtc()
    {
        IntPtr buf; int bytes;
        if (!Native.WTSQuerySessionInformation(IntPtr.Zero, -1, 24, out buf, out bytes)) return DateTime.MinValue;
        try { return bytes >= 216 ? DateTime.FromFileTimeUtc(Marshal.ReadInt64(buf, 200)) : DateTime.MinValue; }
        catch { return DateTime.MinValue; }
        finally { Native.WTSFreeMemory(buf); }
    }

    // 正在运行的安全软件（只用来在提示里点名，方便用户去对的地方改）
    public static string SecuritySoftware()
    {
        string[,] known = { { "HipsTray", "火绒" }, { "HipsDaemon", "火绒" }, { "360Tray", "360 安全卫士" }, { "360sd", "360 杀毒" },
                            { "QQPCTray", "腾讯电脑管家" }, { "kxetray", "金山毒霸" }, { "2345SafeTray", "2345 安全卫士" } };
        for (int i = 0; i < known.GetLength(0); i++)
            if (Process.GetProcessesByName(known[i, 0]).Length > 0) return known[i, 1];
        return "";
    }

    // 没问题返回 null；被拦了返回正在运行的安全软件名（没认出来是 ""），提示文字由设置页按界面语言拼
    public static string Detect(bool autoStartOn)
    {
        if (!autoStartOn) return null;
        DateTime logon = LogonTimeUtc();
        if (logon == DateTime.MinValue || (DateTime.UtcNow - logon).TotalMinutes < 3) return null;   // 刚登录：开机启动项可能还没轮到
        if (ReadStamp(EnabledPath) > logon) return null;                                           // 这次登录之后才打开的开机自启
        if (ReadStamp(LastPath) > logon) return null;                                              // 这次登录时自启过了
        string sw = SecuritySoftware();
        Log.Write("这次登录时开机自启没有运行（登录于 " + logon.ToLocalTime().ToString("yyyy-MM-dd HH:mm") + "）" + (sw != "" ? "，正在运行的安全软件：" + sw : ""));
        return sw;
    }
}

static class Json
{
    static readonly JavaScriptSerializer S = new JavaScriptSerializer();
    public static object Parse(string s) { return S.DeserializeObject(s); }
    public static string Stringify(object o) { return S.Serialize(o); }
    public static Dictionary<string, object> Obj(params object[] kv)
    {
        var d = new Dictionary<string, object>();
        for (int i = 0; i + 1 < kv.Length; i += 2) d[(string)kv[i]] = kv[i + 1];
        return d;
    }
}

// 用户设置：当前主题 + 每个主题的参数（只存用户改过的，没改过的用主题默认值）
// 保存在 %LOCALAPPDATA%\Deskland\settings.json
static class Settings
{
    static readonly string FilePath = Path.Combine(Log.DataDir, "settings.json");
    public static string Theme = "papercut";
    public static string Lang = "auto";                // 界面语言："auto"（跟随系统）或 I18n.Supported 里的一种
    public static Dictionary<string, Dictionary<string, object>> Params = new Dictionary<string, Dictionary<string, object>>();
    public static List<Dictionary<string, object>> VirtualIcons = new List<Dictionary<string, object>>();
    public static object VirtualBarCollapsed = null;
    public static string VirtualBarMode = "auto";
    public static bool LowMode = false;
    public static bool DisableAutoPause = false;

    public static List<Dictionary<string, object>> SanitizeVirtualIcons(object raw)
    {
        var result = new List<Dictionary<string, object>>();
        var arr = raw as object[];
        if (arr == null)
        {
            var lst = raw as System.Collections.ArrayList;
            if (lst != null) arr = lst.ToArray();
        }
        if (arr == null) return result;
        int limit = Math.Min(arr.Length, 64);
        for (int i = 0; i < limit; i++)
        {
            var item = arr[i] as Dictionary<string, object>;
            if (item == null) continue;
            try
            {
                int x = (int)Math.Round(Convert.ToDouble(item["x"]));
                int y = (int)Math.Round(Convert.ToDouble(item["y"]));
                int w = item.ContainsKey("w") ? Math.Max(24, Math.Min(256, (int)Math.Round(Convert.ToDouble(item["w"])))) : 76;
                int h = item.ContainsKey("h") ? Math.Max(24, Math.Min(256, (int)Math.Round(Convert.ToDouble(item["h"])))) : 86;
                string type = item.ContainsKey("type") ? item["type"] as string : "folder";
                if (type != "folder" && type != "txt" && type != "pdf" && type != "xlsx" && type != "png" && type != "zip") type = "folder";
                string name = item.ContainsKey("name") ? item["name"] as string : null;
                if (string.IsNullOrWhiteSpace(name)) name = "Yeni Klasör";
                name = name.Trim();
                if (name.Length > 48) name = name.Substring(0, 48);
                result.Add(Json.Obj("name", name, "type", type, "x", x, "y", y, "w", w, "h", h));
            }
            catch { }
        }
        return result;
    }

    public static void Load()
    {
        try
        {
            if (!File.Exists(FilePath)) return;
            var root = Json.Parse(File.ReadAllText(FilePath, Encoding.UTF8)) as Dictionary<string, object>;
            if (root == null) return;
            object v;
            if (root.TryGetValue("theme", out v) && v is string) Theme = (string)v;
            if (root.TryGetValue("lang", out v) && v is string) Lang = (string)v;
            if (root.TryGetValue("params", out v) && v is Dictionary<string, object>)
                foreach (var kv in (Dictionary<string, object>)v)
                {
                    var d = kv.Value as Dictionary<string, object>;
                    if (d != null) Params[kv.Key] = d;
                }
            if (root.TryGetValue("virtualIcons", out v)) VirtualIcons = SanitizeVirtualIcons(v);
            if (root.TryGetValue("virtualBarCollapsed", out v) && (v is bool || v == null)) VirtualBarCollapsed = v;
            if (root.TryGetValue("virtualBarMode", out v) && v is string)
            {
                string mode = (string)v;
                if (mode == "auto" || mode == "always" || mode == "off") VirtualBarMode = mode;
            }
            if (root.TryGetValue("lowMode", out v) && v is bool) LowMode = (bool)v;
            if (root.TryGetValue("disableAutoPause", out v) && v is bool) DisableAutoPause = (bool)v;
            Log.Write("已读取设置：主题 " + Theme);
        }
        catch (Exception ex) { Log.Write("读取设置失败（将使用默认值）：" + ex.Message); }
    }

    public static void Save()
    {
        try
        {
            Directory.CreateDirectory(Log.DataDir);
            File.WriteAllText(FilePath, Json.Stringify(Json.Obj(
                "version", 1,
                "theme", Theme,
                "lang", Lang,
                "params", Params,
                "virtualIcons", VirtualIcons,
                "virtualBarCollapsed", VirtualBarCollapsed,
                "virtualBarMode", VirtualBarMode,
                "lowMode", LowMode,
                "disableAutoPause", DisableAutoPause)), new UTF8Encoding(false));
        }
        catch (Exception ex) { Log.Write("保存设置失败：" + ex.Message); }
    }

    public static Dictionary<string, object> For(string theme)
    {
        Dictionary<string, object> d;
        if (!Params.TryGetValue(theme, out d)) Params[theme] = d = new Dictionary<string, object>();
        return d;
    }
}

// 界面语言（2026-10-14）：文字表在 prototype/i18n/<语言>.json——ui 给设置页、host 给托盘菜单和提示框、themes 给主题名和参数（简体中文的主题文字就是 themes.json 本身）。
// 设置里的"语言"默认"跟随系统"：用 Windows 的界面语言；不支持的语言用英文
static class I18n
{
    // 增加语言时同步设置页 LANGS 与体验版导出的语言列表。
    public static readonly string[] Supported = { "zh-CN", "en", "tr" };
    public static string Code = "zh-CN", Sys = "en";     // 现在用的语言；"跟随系统"时会用的语言
    static Dictionary<string, object> cur, en, zh;

    public static string FromCulture(System.Globalization.CultureInfo c)
    {
        string n = c.Name.ToLowerInvariant(), two = c.TwoLetterISOLanguageName;
        bool zhHant = n.StartsWith("zh") && (n.Contains("hant") || n.EndsWith("-tw") || n.EndsWith("-hk") || n.EndsWith("-mo"));
        string want = n.StartsWith("zh") ? (zhHant ? "zh-TW" : "zh-CN") : two == "pt" ? "pt-BR" : two;
        if (Array.IndexOf(Supported, want) >= 0) return want;
        return n.StartsWith("zh") ? "zh-CN" : "en";       // 繁体中文还没做：先用简体
    }

    public static void Load(string pageDir, string setting)
    {
        Sys = FromCulture(System.Globalization.CultureInfo.CurrentUICulture);
        Code = Array.IndexOf(Supported, setting) >= 0 ? setting : Sys;
        zh = Read(pageDir, "zh-CN");
        en = Read(pageDir, "en");
        cur = Code == "zh-CN" ? zh : Code == "en" ? en : Read(pageDir, Code);
        Log.Write("界面语言：" + Code + "（设置：" + setting + "，系统：" + Sys + "）");
    }

    static Dictionary<string, object> Read(string pageDir, string code)
    {
        try
        {
            var root = Json.Parse(File.ReadAllText(Path.Combine(Path.Combine(pageDir, "i18n"), code + ".json"), Encoding.UTF8)) as Dictionary<string, object>;
            object h;
            return root != null && root.TryGetValue("host", out h) ? h as Dictionary<string, object> : null;
        }
        catch (Exception ex) { Log.Write("读取语言文件失败（" + code + "）：" + ex.Message); return null; }
    }

    // 取一句话：当前语言 → 英文 → 简体中文；{名字} 换成后面成对给的值
    public static string T(string key, params string[] kv)
    {
        object v = null;
        bool found = (cur != null && cur.TryGetValue(key, out v)) || (en != null && en.TryGetValue(key, out v)) || (zh != null && zh.TryGetValue(key, out v));
        string str = found ? (v as string ?? key) : key;
        for (int i = 0; i + 1 < kv.Length; i += 2) str = str.Replace("{" + kv[i] + "}", kv[i + 1]);
        return str;
    }
}

// 整个程序的生命周期：托盘图标常驻；壁纸窗口没了就重新建一个
// 打开菜单的方式：托盘图标左键 / 右键、快捷键 Ctrl+Alt+H（或 --menu）
// 打开设置的方式：双击程序（没运行就先启动再打开）、双击托盘图标、菜单 → 设置…（或 --settings）
class HostContext : ApplicationContext
{
    public const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run", RunName = "DesklandPreview";
    static readonly MethodInfo ShowTrayMenu =
        typeof(NotifyIcon).GetMethod("ShowContextMenu", BindingFlags.Instance | BindingFlags.NonPublic);

    readonly string pageDir;
    readonly NotifyIcon tray;
    readonly MenuItem pauseItem, lowModeItem, autoStartItem, settingsItem, reloadItem, exitItem;
    readonly System.Windows.Forms.Timer watch;
    readonly EventWaitHandle exitEvent, menuEvent, settingsEvent;
    readonly HotkeyWindow hotkey = new HotkeyWindow();
    readonly Queue<DateTime> restarts = new Queue<DateTime>();
    WallpaperForm form, dead;
    SettingsForm settingsWin;
    DateTime nextStart = DateTime.MaxValue, saveDue = DateTime.MaxValue;
    bool manualPause, wallpaperPaused, exiting;
    string autoStartSw;                                 // 开机自启没跑起来（只在手动打开时检查）：正在运行的安全软件名（没认出来是 ""）；没问题是 null
    string updateVersion, updateUrl, updateStatus = "", updateError = "";
    double updateProgress = -1;                         // -1 = 当前阶段没有可显示的百分比

    public HostContext(string pageDir, bool showSettings)
    {
        this.pageDir = pageDir;
        Log.Write("启动，页面目录：" + pageDir + (showSettings ? "" : "（开机自动启动）"));
        if (showSettings) autoStartSw = AutoStartCheck.Detect(IsAutoStart());
        Settings.Load();
        I18n.Load(pageDir, Settings.Lang);
        exitEvent = new EventWaitHandle(false, EventResetMode.AutoReset, Program.ExitEventName);
        menuEvent = new EventWaitHandle(false, EventResetMode.AutoReset, Program.MenuEventName);
        settingsEvent = new EventWaitHandle(false, EventResetMode.AutoReset, Program.SettingsEventName);

        // 快捷键 Ctrl+Alt+H：在鼠标位置弹出菜单
        bool hotkeyOk = hotkey.Register(Native.MOD_CONTROL | Native.MOD_ALT | Native.MOD_NOREPEAT, Keys.H);
        hotkey.Pressed += delegate { ShowMenu(); };
        Log.Write(hotkeyOk ? "快捷键 Ctrl+Alt+H 已注册" : "快捷键 Ctrl+Alt+H 注册失败（可能被其他程序占用）");

        pauseItem = new MenuItem(I18n.T("trayPause"), delegate
        {
            manualPause = !manualPause;
            pauseItem.Checked = manualPause;
            if (form != null) form.SetManualPause(manualPause);
        });
        lowModeItem = new MenuItem(I18n.T("trayLowMode"), delegate { SetLowMode(!Settings.LowMode); });
        lowModeItem.Checked = Settings.LowMode;
        autoStartItem = new MenuItem(I18n.T("trayAutostart"), delegate { SetAutoStart(!autoStartItem.Checked); });
        autoStartItem.Checked = IsAutoStart();
        if (autoStartItem.Checked && AutoStartValue() != "")
        {
            try
            {
                if (CreateAutoStartShortcut())
                {
                    DeleteRunAutoStart();
                    Log.Write("开机自动启动已迁移到当前用户启动文件夹");
                }
                else if (AutoStartValue().IndexOf("--autostart") < 0)
                    using (RegistryKey k = Registry.CurrentUser.CreateSubKey(RunKey))   // 回退项缺参数时补上
                        k.SetValue(RunName, "\"" + Application.ExecutablePath + "\" --autostart");
            }
            catch (Exception ex) { Log.Write("迁移开机自动启动失败，保留原启动项：" + ex.Message); }
        }
        settingsItem = new MenuItem(I18n.T("traySettings"), delegate { OpenSettings(); });
        settingsItem.DefaultItem = true;                // 粗体：主入口

        tray = new NotifyIcon();
        tray.Icon = AppIcon.Create();
        tray.Text = "DiyarDesk";
        tray.ContextMenu = new ContextMenu(new[] {
            settingsItem,
            pauseItem,
            lowModeItem,
            reloadItem = new MenuItem(I18n.T("trayReload"), delegate { if (form != null) form.Reload(); }),
            autoStartItem,
            new MenuItem("-"),
            exitItem = new MenuItem(I18n.T("trayExit"), delegate { Exit(); }),
        });
        tray.MouseUp += delegate(object s, MouseEventArgs e) { if (e.Button == MouseButtons.Left) ShowMenu(); };
        tray.DoubleClick += delegate { OpenSettings(); };   // 双击托盘图标：直接打开设置
        tray.Visible = true;
        ShowWelcomeOnce(hotkeyOk);

        watch = new System.Windows.Forms.Timer();
        watch.Interval = 500;
        watch.Tick += delegate { Watch(); };
        watch.Start();
        StartForm();
        if (showSettings) OpenSettings();                // 用户双击程序启动：直接打开设置
    }

    // 换了界面语言：托盘菜单的文字跟着换
    void ApplyTrayTexts()
    {
        settingsItem.Text = I18n.T("traySettings");
        pauseItem.Text = I18n.T("trayPause");
        lowModeItem.Text = I18n.T("trayLowMode");
        reloadItem.Text = I18n.T("trayReload");
        autoStartItem.Text = I18n.T("trayAutostart");
        exitItem.Text = I18n.T("trayExit");
    }

    static void ApplyProcessPriority(bool low)
    {
        try { Process.GetCurrentProcess().PriorityClass = low ? ProcessPriorityClass.BelowNormal : ProcessPriorityClass.Normal; }
        catch { }
    }

    void SetLowMode(bool enabled)
    {
        Settings.LowMode = enabled;
        lowModeItem.Checked = enabled;
        ApplyProcessPriority(enabled && wallpaperPaused);
        SaveSoon();
        if (form != null) form.SetLowMode(enabled);
        if (settingsWin != null) settingsWin.Post(Json.Stringify(Json.Obj("t", "lowMode", "v", enabled)));
        Log.Write(enabled ? "已开启低功耗模式（Low Mode）" : "已关闭低功耗模式");
    }

    void SetDisableAutoPause(bool enabled)
    {
        Settings.DisableAutoPause = enabled;
        SaveSoon();
        if (form != null) form.SetDisableAutoPause(enabled);
        if (settingsWin != null) settingsWin.Post(Json.Stringify(Json.Obj("t", "disableAutoPause", "v", enabled)));
        Log.Write(enabled ? "已关闭自动暂停（最大化/全屏保持运行）" : "已开启自动暂停");
    }

    // ---------------------------------------------------------------- 设置窗口
    public void OpenSettings()
    {
        if (exiting) return;
        if (settingsWin == null || settingsWin.IsDisposed)
        {
            settingsWin = new SettingsForm(pageDir, this);
            settingsWin.FormClosed += delegate { settingsWin = null; };
            Log.Write("打开设置窗口");
        }
        if (settingsWin.WindowState == FormWindowState.Minimized) settingsWin.WindowState = FormWindowState.Normal;
        settingsWin.Show();
        // 外部以 SW_HIDE 启动时，WinForms 的 Visible 状态可能已是 true，
        // 但原生窗口仍被隐藏；Activate / 再次 Show 不会修复，必须显式还原。
        // SW_RESTORE 同时处理隐藏和最小化，SW_SHOW 不会展开已最小化的窗口。
        Native.ShowWindow(settingsWin.Handle, Native.SW_RESTORE);
        settingsWin.Activate();
    }

    string SettingsMessage()
    {
        return Json.Stringify(Json.Obj("t", "settings", "v", Settings.For(Settings.Theme)));
    }

    string VirtualStateMessage()
    {
        return Json.Stringify(Json.Obj(
            "t", "virtualState",
            "icons", Settings.VirtualIcons,
            "collapsed", Settings.VirtualBarCollapsed,
            "mode", Settings.VirtualBarMode));
    }

    string UpdateStateMessage()
    {
        return Json.Stringify(Json.Obj("t", "updateState", "version", updateVersion,
            "status", updateStatus, "progress", updateProgress, "error", updateError));
    }

    void PostUpdateState()
    {
        if (settingsWin != null) settingsWin.Post(UpdateStateMessage());
    }

    // 仅用于开发联调或非商店更新源；商店版始终使用 StoreContext，不接受网页传入地址。
    public void OfferUpdate(string version, string url)
    {
        Uri uri;
        if (string.IsNullOrWhiteSpace(version) || !Uri.TryCreate(url, UriKind.Absolute, out uri) || uri.Scheme != Uri.UriSchemeHttps)
            throw new ArgumentException("更新信息需要版本号和有效的 HTTPS 地址");
        updateVersion = version.Trim();
        updateUrl = uri.AbsoluteUri;
        updateStatus = "available";
        updateProgress = -1;
        updateError = "";
        PostUpdateState();
    }

    void SaveSoon() { saveDue = DateTime.Now.AddMilliseconds(600); }

    bool ThemeAllowed(string id)
    {
        return Themes.Allowed(pageDir, id, delegate(string product) { return false; });
    }

    // 设置页发来的消息：hello / set / reset / theme / autostart / lang / updateProgram
    public void OnSettingsMessage(string s)
    {
        var m = Json.Parse(s) as Dictionary<string, object>;
        if (m == null || settingsWin == null) return;
        object t;
        m.TryGetValue("t", out t);
        switch (t as string)
        {
            case "hello":
                Rectangle scr = Screen.PrimaryScreen.Bounds;
                settingsWin.Post(Json.Stringify(Json.Obj("t", "init", "theme", Settings.Theme, "params", Settings.Params,
                    "screen", Json.Obj("w", scr.Width, "h", scr.Height), "autostart", IsAutoStart(),
                    "autostartWarn", autoStartSw != null, "autostartSw", autoStartSw ?? "",
                    "lang", Settings.Lang, "langEff", I18n.Code, "langSys", I18n.Sys,
                    "customBase", WebCustomHelper.CustomBaseUrl, "customThemes", CustomThemes.Load(),
                    "virtualBarMode", Settings.VirtualBarMode, "lowMode", Settings.LowMode, "disableAutoPause", Settings.DisableAutoPause)));
                if (updateVersion != null) PostUpdateState();
                if (form != null && form.LastIcons != "") settingsWin.Post(form.LastIcons);
                break;
            case "set":
            {
                string theme = m["theme"] as string, key = m["key"] as string;
                if (theme == null || key == null) return;
                var p = Settings.For(theme);
                if (key == "audioFile" && string.IsNullOrEmpty(m["value"] as string))
                {
                    object oldVal;
                    if (p.TryGetValue("audioFile", out oldVal)) CleanupAudioFile(oldVal as string);
                }
                p[key] = m["value"];
                if (theme == Settings.Theme && form != null)
                    form.Post(Json.Stringify(Json.Obj("t", "settings", "v", Json.Obj(key, m["value"]))));
                SaveSoon();
                break;
            }
            case "pickAudio":
                PickAudio(m["theme"] as string);
                break;
            case "importTheme":
                ImportTheme();
                break;
            case "removeTheme":
                RemoveTheme(m.ContainsKey("id") ? m["id"] as string : null);
                break;
            case "reset":
            {
                string theme = m["theme"] as string;
                if (theme == null) return;
                Settings.Params.Remove(theme);
                if (theme == Settings.Theme && form != null) form.Post("{\"t\":\"reset\"}");
                SaveSoon();
                Log.Write("恢复默认设置：" + theme);
                break;
            }
            case "theme":
            {
                string id = m["id"] as string;
                if (id == null || !ThemeAllowed(id))
                {
                    settingsWin.Post(Json.Stringify(Json.Obj("t", "themeChanged", "id", Settings.Theme)));
                    Log.Write("拒绝未授权或未知主题切换：" + (id ?? "null"));
                    return;
                }
                string page = Themes.PageFor(pageDir, id);
                if (page == null || !Themes.PageExists(pageDir, page))
                {
                    settingsWin.Post(Json.Stringify(Json.Obj("t", "themeChanged", "id", Settings.Theme)));
                    Log.Write("拒绝缺失页面的主题切换：" + id);
                    return;
                }
                if (id == Settings.Theme) return;
                Settings.Theme = id;
                SaveSoon();
                if (form != null) form.LoadPage(page);
                Log.Write("切换主题：" + id);
                break;
            }
            case "autostartWarnSeen":
                autoStartSw = null;
                break;
            case "lang":                                    // 换界面语言：存下来，托盘菜单跟着换，告诉设置页实际用哪种
            {
                object v;
                if (!m.TryGetValue("v", out v) || !(v is string)) return;
                Settings.Lang = (string)v;
                SaveSoon();
                I18n.Load(pageDir, Settings.Lang);
                ApplyTrayTexts();
                settingsWin.Post(Json.Stringify(Json.Obj("t", "lang", "v", Settings.Lang, "eff", I18n.Code, "sys", I18n.Sys)));
                break;
            }
            case "virtualBarMode":
            {
                object v;
                if (!m.TryGetValue("v", out v) || !(v is string)) return;
                string mode = (string)v;
                if (mode != "auto" && mode != "always" && mode != "off") return;
                Settings.VirtualBarMode = mode;
                SaveSoon();
                if (form != null) form.Post(VirtualStateMessage());
                break;
            }
            case "share":
                if (form == null) { PostShareResult(false, null, I18n.T("errNotReady")); break; }
                form.RequestSnapshot(OnShareImage);
                break;
            case "shareVideo":
                if (form == null) { PostShareResult(false, null, I18n.T("errNotReady")); break; }
                form.RequestRecording(OnShareVideo);
                break;
            case "openShare":
                if (lastShare != null && File.Exists(lastShare)) Process.Start(lastShare);
                break;
            case "openWebsite":
                Process.Start("https://deskland.app/");
                break;
            case "revealShare":
                if (lastShare != null && File.Exists(lastShare)) Process.Start("explorer.exe", "/select,\"" + lastShare + "\"");
                break;
            case "updateProgram":
                if (updateUrl != null) Process.Start(updateUrl);
                break;
            case "autostart":
                SetAutoStart(m["v"] is bool && (bool)m["v"]);
                settingsWin.Post(Json.Stringify(Json.Obj("t", "autostart", "v", IsAutoStart())));
                break;
            case "lowMode":
                SetLowMode(m.ContainsKey("v") && m["v"] is bool && (bool)m["v"]);
                break;
            case "disableAutoPause":
                SetDisableAutoPause(m.ContainsKey("v") && m["v"] is bool && (bool)m["v"]);
                break;
        }
    }

    static string AudioDir()
    {
        return Path.Combine(InstallProfile.DataDir, "audio");
    }

    static void CleanupAudioFile(string oldName)
    {
        if (string.IsNullOrEmpty(oldName)) return;
        try
        {
            string dir = Path.GetFullPath(AudioDir());
            string target = Path.GetFullPath(Path.Combine(dir, Path.GetFileName(oldName)));
            if (target.StartsWith(dir + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase) && File.Exists(target))
                File.Delete(target);
        }
        catch { }
    }

    void PickAudio(string theme)
    {
        if (string.IsNullOrEmpty(theme) || !ThemeAllowed(theme)) return;
        string chosen = null;
        using (var dlg = new OpenFileDialog())
        {
            dlg.Filter = "Audio (*.mp3;*.wav;*.ogg;*.opus;*.flac;*.m4a;*.aac;*.webm)|*.mp3;*.wav;*.ogg;*.opus;*.flac;*.m4a;*.aac;*.webm|All Files (*.*)|*.*";
            dlg.RestoreDirectory = true;
            if (dlg.ShowDialog(settingsWin) != DialogResult.OK) return;
            chosen = dlg.FileName;
        }
        if (string.IsNullOrEmpty(chosen) || !File.Exists(chosen)) return;
        string dir = AudioDir();
        string ext = (Path.GetExtension(chosen) ?? "").ToLowerInvariant();
        if (string.IsNullOrEmpty(ext)) ext = ".audio";
        string safeName = theme + "_" + DateTime.UtcNow.Ticks.ToString() + ext;
        string origName = Path.GetFileName(chosen);
        try
        {
            Directory.CreateDirectory(dir);
            File.Copy(chosen, Path.Combine(dir, safeName), true);
        }
        catch (Exception ex)
        {
            Log.Write("复制音频失败：" + ex.Message);
            return;
        }
        var p = Settings.For(theme);
        object prev;
        if (p.TryGetValue("audioFile", out prev)) CleanupAudioFile(prev as string);
        p["audioFile"] = safeName;
        p["audioName"] = origName;
        p["audioPlay"] = 1;
        SaveSoon();
        settingsWin.Post(Json.Stringify(Json.Obj("t", "audioPicked", "theme", theme, "file", safeName, "name", origName, "play", 1)));
        if (theme == Settings.Theme && form != null)
            form.Post(Json.Stringify(Json.Obj("t", "settings", "v", Json.Obj("audioFile", safeName, "audioName", origName, "audioPlay", 1))));
    }

    void ImportTheme()
    {
        if (settingsWin == null) return;
        string chosen = null;
        using (var dlg = new OpenFileDialog())
        {
            string title = I18n.T("importTheme");
            dlg.Title = string.IsNullOrEmpty(title) || title == "importTheme" ? "Import theme" : title;
            dlg.Filter = "Theme packages, HTML, images, videos (*.zip;*.deskland;*.html;*.htm;*.png;*.jpg;*.jpeg;*.webp;*.gif;*.svg;*.avif;*.bmp;*.mp4;*.webm;*.mkv;*.mov)|*.zip;*.deskland;*.html;*.htm;*.png;*.jpg;*.jpeg;*.webp;*.gif;*.svg;*.avif;*.bmp;*.mp4;*.webm;*.mkv;*.mov|All Files (*.*)|*.*";
            dlg.RestoreDirectory = true;
            if (dlg.ShowDialog(settingsWin) != DialogResult.OK) return;
            chosen = dlg.FileName;
        }
        if (string.IsNullOrEmpty(chosen)) return;
        try
        {
            var themeObj = CustomThemes.ImportFile(chosen, pageDir);
            string newId = themeObj["id"] as string;
            string newPage = themeObj["page"] as string;
            Settings.Theme = newId;
            SaveSoon();
            if (form != null && !string.IsNullOrEmpty(newPage)) form.LoadPage(newPage);
            settingsWin.Post(Json.Stringify(Json.Obj("t", "themeImported", "theme", themeObj, "select", true)));
            Log.Write("已导入自定义主题：" + newId);
        }
        catch (Exception ex)
        {
            Log.Write("导入主题失败：" + ex);
            settingsWin.Post(Json.Stringify(Json.Obj("t", "importError", "msg", ex.Message)));
        }
    }

    void RemoveTheme(string themeId)
    {
        if (string.IsNullOrEmpty(themeId)) return;
        if (!CustomThemes.Remove(themeId)) return;
        Settings.Params.Remove(themeId);
        if (Settings.Theme == themeId)
        {
            string fallback = Themes.FirstReadyBuiltIn(pageDir);
            Settings.Theme = fallback;
            string page = Themes.PageFor(pageDir, fallback);
            if (form != null && !string.IsNullOrEmpty(page)) form.LoadPage(page);
        }
        SaveSoon();
        if (settingsWin != null)
            settingsWin.Post(Json.Stringify(Json.Obj("t", "themeRemoved", "id", themeId, "active", Settings.Theme)));
        Log.Write("已移除自定义主题：" + themeId);
    }

    // ---- 一键分享：壁纸页面画好的图 → 存到"图片\Deskland"，复制到剪贴板，缩略图发回设置页 ----
    string lastShare;                                   // 最近一张分享图的路径（设置页只能让我们打开这一张）
    void OnShareImage(string thumb, string png, string err)
    {
        if (err != null || png == null) { PostShareResult(false, null, err ?? I18n.T("errNoImage")); Log.Write("生成分享图失败：" + err); return; }
        try
        {
            int comma = png.IndexOf(',');
            byte[] bytes = Convert.FromBase64String(comma >= 0 ? png.Substring(comma + 1) : png);
            string dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyPictures), "DiyarDesk");
            Directory.CreateDirectory(dir);
            string path = Path.Combine(dir, "DiyarDesk " + DateTime.Now.ToString("yyyy-MM-dd HHmmss") + ".png");
            File.WriteAllBytes(path, bytes);
            lastShare = path;
            try
            {
                using (var ms = new MemoryStream(bytes))
                using (var img = Image.FromStream(ms))
                {
                    var data = new DataObject();
                    data.SetImage(new Bitmap(img));                       // 一般的程序（聊天软件等）认这个
                    data.SetData("PNG", false, new MemoryStream(bytes));  // 认 PNG 的程序（浏览器、Office）用这个，更清楚
                    Clipboard.SetDataObject(data, true);
                }
            }
            catch (Exception ex) { Log.Write("复制分享图到剪贴板失败：" + ex.Message); }
            Log.Write("已生成分享图：" + path);
            if (settingsWin != null)
                settingsWin.Post(Json.Stringify(Json.Obj("t", "shareResult", "ok", true, "thumb", thumb, "path", path)));
        }
        catch (Exception ex)
        {
            Log.Write("保存分享图失败：" + ex);
            PostShareResult(false, null, ex.Message);
        }
    }
    void PostShareResult(bool ok, string path, string msg)
    {
        if (settingsWin != null) settingsWin.Post(Json.Stringify(Json.Obj("t", "shareResult", "ok", ok, "path", path, "msg", msg)));
    }
    // 分享动画：存成 MP4（不支持时是 WebM），把文件放到剪贴板（在聊天窗口里直接粘贴就能发出去），
    // 设置页用 https://share.local/文件名 直接播放
    void OnShareVideo(string mime, string poster, string video, string err)
    {
        if (err != null || video == null) { PostShareResult(false, null, err ?? I18n.T("errNoVideo")); Log.Write("录分享动画失败：" + err); return; }
        try
        {
            int comma = video.IndexOf(',');
            byte[] bytes = Convert.FromBase64String(comma >= 0 ? video.Substring(comma + 1) : video);
            string dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyPictures), "DiyarDesk");
            Directory.CreateDirectory(dir);
            string name = "DiyarDesk " + DateTime.Now.ToString("yyyy-MM-dd HHmmss") + (mime == "video/webm" ? ".webm" : ".mp4");
            string path = Path.Combine(dir, name);
            File.WriteAllBytes(path, bytes);
            lastShare = path;
            try
            {
                var files = new System.Collections.Specialized.StringCollection();
                files.Add(path);
                Clipboard.SetFileDropList(files);
            }
            catch (Exception ex) { Log.Write("复制分享动画到剪贴板失败：" + ex.Message); }
            Log.Write("已录好分享动画：" + path + "（" + (bytes.Length / 1024) + " KB）");
            if (settingsWin != null)
            {
                settingsWin.MapShareFolder(dir);
                settingsWin.Post(Json.Stringify(Json.Obj("t", "shareResult", "ok", true, "kind", "video", "thumb", poster, "path", path,
                    "url", "https://share.local/" + Uri.EscapeDataString(name))));
            }
        }
        catch (Exception ex)
        {
            Log.Write("保存分享动画失败：" + ex);
            PostShareResult(false, null, ex.Message);
        }
    }

    // 在鼠标位置弹出托盘菜单（NotifyIcon 自带的弹出逻辑会处理前台窗口，菜单点别处能正常收起）
    void ShowMenu()
    {
        if (exiting) return;
        autoStartItem.Checked = IsAutoStart();
        if (ShowTrayMenu != null) ShowTrayMenu.Invoke(tray, null);
    }

    // 第一次运行时弹一条系统通知：告诉用户菜单在哪
    void ShowWelcomeOnce(bool hotkeyOk)
    {
        try
        {
            string flag = Path.Combine(Log.DataDir, "welcome.shown");
            if (File.Exists(flag)) return;
            Directory.CreateDirectory(Log.DataDir);
            tray.ShowBalloonTip(10000, I18n.T("welcomeTitle"),
                I18n.T("welcomeBody", "hotkey", hotkeyOk ? I18n.T("welcomeHotkey") : ""), ToolTipIcon.Info);
            File.WriteAllText(flag, DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"));
        }
        catch (Exception ex) { Log.Write("首次提示失败：" + ex.Message); }
    }

    // 开机自动启动：优先用当前用户“启动”文件夹里的快捷方式；旧 Run 项启动时迁移。
    // 不使用需要额外权限的任务计划；快捷方式创建失败时仍保留 Run 项作为回退。
    static string AutoStartShortcutPath
    {
        get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Startup), "Deskland.lnk"); }
    }

    static object OpenAutoStartShortcut()
    {
        object shell = Activator.CreateInstance(Type.GetTypeFromProgID("WScript.Shell"));
        try
        {
            return shell.GetType().InvokeMember("CreateShortcut", BindingFlags.InvokeMethod,
                null, shell, new object[] { AutoStartShortcutPath });
        }
        finally { Marshal.FinalReleaseComObject(shell); }
    }

    static bool AutoStartShortcutEnabled()
    {
        if (!File.Exists(AutoStartShortcutPath)) return false;
        object link = null;
        try
        {
            link = OpenAutoStartShortcut();
            string target = link.GetType().InvokeMember("TargetPath", BindingFlags.GetProperty,
                null, link, null) as string;
            string arguments = link.GetType().InvokeMember("Arguments", BindingFlags.GetProperty,
                null, link, null) as string;
            return string.Equals(target, Application.ExecutablePath, StringComparison.OrdinalIgnoreCase)
                && arguments != null && arguments.IndexOf("--autostart", StringComparison.OrdinalIgnoreCase) >= 0;
        }
        catch { return false; }
        finally { if (link != null) Marshal.FinalReleaseComObject(link); }
    }

    static bool CreateAutoStartShortcut()
    {
        object link = null;
        try
        {
            link = OpenAutoStartShortcut();
            link.GetType().InvokeMember("TargetPath", BindingFlags.SetProperty,
                null, link, new object[] { Application.ExecutablePath });
            link.GetType().InvokeMember("Arguments", BindingFlags.SetProperty,
                null, link, new object[] { "--autostart" });
            link.GetType().InvokeMember("WorkingDirectory", BindingFlags.SetProperty,
                null, link, new object[] { Path.GetDirectoryName(Application.ExecutablePath) });
            link.GetType().InvokeMember("IconLocation", BindingFlags.SetProperty,
                null, link, new object[] { Application.ExecutablePath + ",0" });
            link.GetType().InvokeMember("Description", BindingFlags.SetProperty,
                null, link, new object[] { "Deskland" });
            link.GetType().InvokeMember("Save", BindingFlags.InvokeMethod, null, link, null);
            return File.Exists(AutoStartShortcutPath);
        }
        catch (Exception ex) { Log.Write("创建启动文件夹快捷方式失败：" + ex.Message); return false; }
        finally { if (link != null) Marshal.FinalReleaseComObject(link); }
    }

    static string AutoStartValue()
    {
        if (InstallProfile.Isolated) return "";
        using (RegistryKey k = Registry.CurrentUser.OpenSubKey(RunKey))
        {
            string v = k == null ? null : k.GetValue(RunName) as string;
            return v ?? "";
        }
    }

    static void DeleteRunAutoStart()
    {
        using (RegistryKey k = Registry.CurrentUser.OpenSubKey(RunKey, true))
            if (k != null) k.DeleteValue(RunName, false);
    }

    static bool IsAutoStart()
    {
        return AutoStartShortcutEnabled()
            || AutoStartValue().IndexOf(Application.ExecutablePath, StringComparison.OrdinalIgnoreCase) >= 0;
    }

    void SetAutoStart(bool on)
    {
        if (InstallProfile.Isolated) return;
        try
        {
            if (on)
            {
                bool shortcut = CreateAutoStartShortcut();
                if (shortcut) DeleteRunAutoStart();
                else
                    using (RegistryKey k = Registry.CurrentUser.CreateSubKey(RunKey))
                        k.SetValue(RunName, "\"" + Application.ExecutablePath + "\" --autostart");
                AutoStartCheck.MarkEnabled();
                Log.Write(shortcut ? "已开启开机自动启动（当前用户启动文件夹）" : "已开启开机自动启动（传统启动项回退）");
            }
            else
            {
                DeleteRunAutoStart();
                if (File.Exists(AutoStartShortcutPath)) File.Delete(AutoStartShortcutPath);
                Log.Write(IsAutoStart() ? "关闭开机自动启动失败，启动项仍存在" : "已关闭开机自动启动");
            }
        }
        catch (Exception ex) { Log.Write("设置开机自动启动失败：" + ex.Message); }
        autoStartItem.Checked = IsAutoStart();
    }

    void StartForm()
    {
        nextStart = DateTime.MaxValue;
        if (!ThemeAllowed(Settings.Theme)) Settings.Theme = ThemeAllowed("papercut") ? "papercut" : (ThemeAllowed("software-world") ? "software-world" : (ThemeAllowed("beach") ? "beach" : "meadow"));
        string page = ThemeAllowed(Settings.Theme) ? Themes.PageFor(pageDir, Settings.Theme) : null;
        if (string.IsNullOrEmpty(page) || !Themes.PageExists(pageDir, page))
        {
            string fallback = Themes.FirstReadyBuiltIn(pageDir);
            string fallbackPage = Themes.PageFor(pageDir, fallback);
            if (!string.IsNullOrEmpty(fallbackPage) && Themes.PageExists(pageDir, fallbackPage))
            {
                Log.Write("主题页面不存在（" + (Settings.Theme ?? "") + "），已回退到内置主题：" + fallback);
                Settings.Theme = fallback;
                page = fallbackPage;
                SaveSoon();
            }
        }
        if (page == null)
        {
            Log.Write("没有可加载的已授权主题，5 秒后重试");
            nextStart = DateTime.Now.AddSeconds(5);
            return;
        }
        form = new WallpaperForm(pageDir, page, manualPause, Settings.LowMode, Settings.DisableAutoPause);
        form.SettingsJson = SettingsMessage;
        form.VirtualStateJson = VirtualStateMessage;
        form.AudioPlayChanged += delegate(bool play)
        {
            string theme = Settings.Theme;
            int val = play ? 1 : 0;
            Settings.For(theme)["audioPlay"] = val;
            SaveSoon();
            if (settingsWin != null)
                settingsWin.Post(Json.Stringify(Json.Obj("t", "audioPlay", "theme", theme, "play", val)));
        };
        form.VirtualIconsSaved += delegate(object raw)
        {
            Settings.VirtualIcons = Settings.SanitizeVirtualIcons(raw);
            SaveSoon();
        };
        form.VirtualBarCollapsedSaved += delegate(object val)
        {
            if (val is bool || val == null)
            {
                Settings.VirtualBarCollapsed = val;
                SaveSoon();
            }
        };
        form.IconsChanged += delegate(string json) { if (settingsWin != null) settingsWin.Post(json); };
        form.PauseChanged += delegate(bool paused)
        {
            wallpaperPaused = paused;
            ApplyProcessPriority(Settings.LowMode && paused);
        };
        form.RecordProgress += delegate(string p) { if (settingsWin != null) settingsWin.Post("{\"t\":\"recProgress\",\"p\":" + p + "}"); };
        form.HandleDestroyed += OnFormGone;
        form.Show();
    }

    void OnFormGone(object sender, EventArgs e)
    {
        var f = (WallpaperForm)sender;
        if (f != form) return;
        form = null;
        wallpaperPaused = false;
        ApplyProcessPriority(false);
        dead = f;                                        // 稍后在计时器里释放，避免在销毁回调里重入
        if (exiting) return;
        if (f.Fatal) { Exit(); return; }

        // 1 分钟内反复被销毁：不再重试，避免和资源管理器打架
        restarts.Enqueue(DateTime.Now);
        while (restarts.Count > 0 && (DateTime.Now - restarts.Peek()).TotalSeconds > 60) restarts.Dequeue();
        if (restarts.Count > 5)
        {
            Log.Write("壁纸窗口 1 分钟内被销毁超过 5 次，停止重试");
            MessageBox.Show(I18n.T("errRebuilt") + "\n\n" + I18n.T("errSeeLog", "path", Log.FilePath), "DiyarDesk");
            Exit();
            return;
        }
        Log.Write("壁纸窗口被销毁（资源管理器重建了壁纸层），1 秒后重新挂载");
        nextStart = DateTime.Now.AddSeconds(1);
    }

    void Watch()
    {
        if (dead != null) { try { dead.Dispose(); } catch { } dead = null; }
        if (exitEvent.WaitOne(0)) { Exit(); return; }
        if (menuEvent.WaitOne(0)) ShowMenu();
        if (settingsEvent.WaitOne(0)) OpenSettings();
        if (DateTime.Now >= saveDue) { saveDue = DateTime.MaxValue; Settings.Save(); }
        if (form == null && !exiting && DateTime.Now >= nextStart) StartForm();
    }

    void Exit()
    {
        if (exiting) return;
        exiting = true;
        watch.Stop();
        if (saveDue != DateTime.MaxValue) Settings.Save();
        if (settingsWin != null) { try { settingsWin.Close(); } catch { } }
        hotkey.Dispose();
        tray.Visible = false;
        if (form != null) form.Shutdown();
        Log.Write("退出");
        ExitThread();
    }
}

// 接收全局快捷键的隐藏窗口（只收消息，不显示）
class HotkeyWindow : NativeWindow, IDisposable
{
    public event EventHandler Pressed;
    bool registered;

    public HotkeyWindow()
    {
        var cp = new CreateParams();
        cp.Parent = new IntPtr(-3);                     // HWND_MESSAGE：纯消息窗口
        CreateHandle(cp);
    }

    public bool Register(uint modifiers, Keys key)
    {
        registered = Native.RegisterHotKey(Handle, 1, modifiers, (uint)key);
        return registered;
    }

    protected override void WndProc(ref Message m)
    {
        if (m.Msg == 0x0312 /* WM_HOTKEY */ && Pressed != null) Pressed(this, EventArgs.Empty);
        base.WndProc(ref m);
    }

    public void Dispose()
    {
        if (registered) Native.UnregisterHotKey(Handle, 1);
        registered = false;
        if (Handle != IntPtr.Zero) DestroyHandle();
    }
}

static class WebCustomHelper
{
    public const string CustomHost = "custom.local";
    public const string CustomBaseUrl = "https://" + CustomHost + "/";

    public static void Attach(CoreWebView2 core, string pageDir)
    {
        // WebView2's SetVirtualHostNameToFolderMapping bypasses WebResourceRequested for mapped hosts (app.local),
        // so custom themes are mapped to their own virtual host (custom.local -> %LOCALAPPDATA%\DesklandPreview\themes).
        Directory.CreateDirectory(CustomThemes.Dir);
        core.SetVirtualHostNameToFolderMapping(CustomHost, CustomThemes.Dir, CoreWebView2HostResourceAccessKind.Allow);
    }

    public static string EncodePathSegments(string relPath)
    {
        if (string.IsNullOrEmpty(relPath)) return "";
        string[] segs = relPath.Replace('\\', '/').Split('/');
        var encoded = new List<string>(segs.Length);
        for (int i = 0; i < segs.Length; i++)
        {
            if (string.IsNullOrEmpty(segs[i])) continue;
            string s = segs[i];
            try { s = Uri.UnescapeDataString(s); } catch { }
            encoded.Add(Uri.EscapeDataString(s));
        }
        return string.Join("/", encoded.ToArray());
    }

    public static string PageUrl(string page)
    {
        if (!string.IsNullOrEmpty(page) && page.StartsWith("__custom/", StringComparison.Ordinal))
            return CustomBaseUrl + EncodePathSegments(page.Substring("__custom/".Length));
        return "https://app.local/" + EncodePathSegments(page);
    }
}

// 托盘和设置窗口共用 Deskland 图标；构建脚本会把图标复制到程序目录。
static class AppIcon
{
    public static Icon Create()
    {
        return new Icon(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "DesklandPreview.ico"));
    }

    static GraphicsPath RoundRect(Rectangle r, int radius)
    {
        int d = radius * 2;
        var p = new GraphicsPath();
        p.AddArc(r.X, r.Y, d, d, 180, 90);
        p.AddArc(r.Right - d, r.Y, d, d, 270, 90);
        p.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
        p.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
        p.CloseFigure();
        return p;
    }
}

// 设置窗口：普通的桌面窗口，里面用 WebView2 显示 prototype/settings.html
static class WebViewConfig
{
    public const string BrowserArguments =
        "--autoplay-policy=no-user-gesture-required"
        + " --disable-background-timer-throttling"
        + " --disable-renderer-backgrounding"
        + " --disable-backgrounding-occluded-windows";

    public static CoreWebView2CreationProperties CreateProperties()
    {
        var props = new CoreWebView2CreationProperties();
        props.UserDataFolder = Path.Combine(Log.DataDir, "WebView2");
        props.AdditionalBrowserArguments = BrowserArguments;
        return props;
    }
}

class SettingsForm : Form
{
    readonly WebView2 web;
    readonly HostContext host;
    readonly string pageDir;

    public SettingsForm(string pageDir, HostContext host)
    {
        this.pageDir = pageDir;
        this.host = host;
        Text = "DiyarDesk";
        AccessibleName = "DiyarDesk";
        ShowIcon = true;
        Icon = AppIcon.Create();
        StartPosition = FormStartPosition.CenterScreen;
        float s = Native.GetDpiForSystem() / 96f;       // 进程按物理像素工作，窗口尺寸要自己按缩放比例算
        ClientSize = new Size((int)(1120 * s), (int)(800 * s));     // 左边主题排三列，右边上面预览、下面参数
        MinimumSize = new Size((int)(940 * s), (int)(680 * s));
        BackColor = Color.White;

        web = new WebView2();
        web.Dock = DockStyle.Fill;
        web.DefaultBackgroundColor = BackColor;
        web.CreationProperties = WebViewConfig.CreateProperties();
        Controls.Add(web);
    }

    // 标题栏（系统原生那条）刷成和页面底色一样：settings.html 跟随系统深浅色（浅 #ffffff / 深 #171717），
    // 标题栏也按系统深浅色取同一个颜色。DWMWA_CAPTION_COLOR / 深色模式要 Win11，旧系统调用失败就保持系统原样
    void ThemeTitleBar()
    {
        bool light = true;
        try
        {
            object v = Registry.GetValue(@"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "AppsUseLightTheme", 1);
            light = v == null || Convert.ToInt32(v) != 0;
        }
        catch { }
        int dark = light ? 0 : 1;
        int color = light ? 0x00FFFFFF : 0x00171717;      // COLORREF 是 0x00BBGGRR
        Native.DwmSetWindowAttribute(Handle, 20, ref dark, 4);    // DWMWA_USE_IMMERSIVE_DARK_MODE：标题文字、按钮跟着深浅色
        Native.DwmSetWindowAttribute(Handle, 35, ref color, 4);   // DWMWA_CAPTION_COLOR
    }
    protected override void OnHandleCreated(EventArgs e) { base.OnHandleCreated(e); ThemeTitleBar(); }
    protected override void WndProc(ref Message m)
    {
        if (m.Msg == 0x001A /* WM_SETTINGCHANGE */) ThemeTitleBar();   // 系统切深浅色时标题栏跟着换
        base.WndProc(ref m);
    }

    protected override async void OnShown(EventArgs e)
    {
        base.OnShown(e);
        try
        {
            await web.EnsureCoreWebView2Async(null);
            if (IsDisposed) return;
            var core = web.CoreWebView2;
            core.SetVirtualHostNameToFolderMapping("app.local", pageDir, CoreWebView2HostResourceAccessKind.Allow);
            string audioDir = Path.Combine(InstallProfile.DataDir, "audio");
            Directory.CreateDirectory(audioDir);
            core.SetVirtualHostNameToFolderMapping("audio.local", audioDir, CoreWebView2HostResourceAccessKind.Allow);
            WebCustomHelper.Attach(core, pageDir);
            core.Settings.AreDefaultContextMenusEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
            core.Settings.IsZoomControlEnabled = false;
            core.WebMessageReceived += delegate(object s, CoreWebView2WebMessageReceivedEventArgs a)
            {
                try { host.OnSettingsMessage(a.TryGetWebMessageAsString()); }
                catch (Exception ex) { Log.Write("处理设置消息失败：" + ex.Message); }
            };
            string settingsUrl = "https://app.local/settings.html?v=" + Uri.EscapeDataString(Application.ProductVersion);
            Log.Write("设置页面：" + settingsUrl);
            core.Navigate(settingsUrl);
        }
        catch (Exception ex)
        {
            Log.Write("设置窗口启动失败：" + ex);
            MessageBox.Show(I18n.T("errSettings", "msg", ex.Message), "DiyarDesk");
            Close();
        }
    }

    public void Post(string json)
    {
        // 设置窗口是临时窗口：它的 WebView2 失效就先不发（关掉重开即恢复），别把异常抛回壁纸的轮询
        try { if (!IsDisposed && web.CoreWebView2 != null) web.CoreWebView2.PostWebMessageAsString(json); }
        catch { }
    }

    // 分享的图片 / 动画所在的文件夹：映射成 https://share.local/，设置页可以直接显示、播放
    bool shareMapped;
    public void MapShareFolder(string dir)
    {
        if (shareMapped || IsDisposed || web.CoreWebView2 == null) return;
        web.CoreWebView2.SetVirtualHostNameToFolderMapping("share.local", dir, CoreWebView2HostResourceAccessKind.Allow);
        shareMapped = true;
    }
}

class WallpaperForm : Form
{
    readonly WebView2 web;
    readonly System.Windows.Forms.Timer iconTimer, fgTimer, winTimer;
    readonly string pageDir;
    readonly IconReader reader = new IconReader();
    Rectangle screen;
    IntPtr parentHwnd, insertAfter;
    string lastIcons = "";
    string lastWindows = "";
    int lastPause = -1;
    bool manualPause, lowMode, disableAutoPause, shuttingDown, allowMoveIcons;
    long lastMoveIconsMs;

    public bool Fatal { get; private set; }

    string page;                                        // 当前主题的页面（themes.json 里的 page）
    public Func<string> SettingsJson;                   // 页面就绪时先发给它的设置消息
    public Func<string> VirtualStateJson;
    public event Action<bool> AudioPlayChanged;
    public event Action<object> VirtualIconsSaved;
    public event Action<object> VirtualBarCollapsedSaved;
    public event Action<string> IconsChanged;           // 图标变化时转给设置窗口的预览
    public event Action<bool> PauseChanged;
    public string LastIcons { get { return lastIcons; } }
    protected override bool ShowWithoutActivation { get { return true; } }

    public WallpaperForm(string pageDir, string page, bool manualPause, bool lowMode, bool disableAutoPause = false)
    {
        this.pageDir = pageDir;
        this.page = page;
        this.manualPause = manualPause;
        this.lowMode = lowMode;
        this.disableAutoPause = disableAutoPause;
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        screen = Screen.PrimaryScreen.Bounds;
        Bounds = screen;
        ClientSize = screen.Size;
        BackColor = Color.FromArgb(26, 163, 196);
        Text = "DesklandWindow";

        web = new WebView2();
        web.Bounds = new Rectangle(0, 0, screen.Width, screen.Height);
        web.Dock = DockStyle.Fill;
        web.DefaultBackgroundColor = BackColor;
        web.CreationProperties = WebViewConfig.CreateProperties();
        Controls.Add(web);

        iconTimer = new System.Windows.Forms.Timer();
        iconTimer.Interval = lowMode ? 1500 : 400;
        iconTimer.Tick += delegate { PollIcons(); };
        fgTimer = new System.Windows.Forms.Timer();
        fgTimer.Interval = lowMode ? 1000 : 500;
        fgTimer.Tick += delegate { PollForeground(); };
        winTimer = new System.Windows.Forms.Timer();
        winTimer.Interval = lowMode ? 320 : 120;
        winTimer.Tick += delegate { PollWindows(); };
    }

    public void SetManualPause(bool v) { manualPause = v; PollForeground(); }
    public void SetDisableAutoPause(bool v) { disableAutoPause = v; lastPause = -1; PollForeground(); }

    void ApplyWebViewLowMode(bool enabled)
    {
        if (web == null || web.CoreWebView2 == null) return;
        try
        {
            web.CoreWebView2.MemoryUsageTargetLevel = (enabled && lastPause == 1)
                ? CoreWebView2MemoryUsageTargetLevel.Low
                : CoreWebView2MemoryUsageTargetLevel.Normal;
        }
        catch { }
    }

    public void SetLowMode(bool v)
    {
        lowMode = v;
        iconTimer.Interval = v ? 1500 : 400;
        fgTimer.Interval = v ? 1000 : 500;
        winTimer.Interval = v ? 320 : 120;
        ApplyWebViewLowMode(v);
        Post(Json.Stringify(Json.Obj("t", "lowMode", "v", v)));
        lastPause = -1;
        PollForeground();
    }

    // ---- 一键分享：请页面画一张打好马赛克的桌面图（页面不截屏，只用图标的位置） ----
    Action<string, string, string> snapshotDone;           // (缩略图, 整张 PNG, 出错信息)
    System.Windows.Forms.Timer snapshotTimeout;
    public void RequestSnapshot(Action<string, string, string> done)
    {
        if (web.CoreWebView2 == null) { done(null, null, I18n.T("errNotReady")); return; }
        if (snapshotDone != null) { done(null, null, I18n.T("errBusyImage")); return; }
        snapshotDone = done;
        snapshotTimeout = new System.Windows.Forms.Timer();
        snapshotTimeout.Interval = 20000;
        snapshotTimeout.Tick += delegate { FinishSnapshot(null, null, I18n.T("errImageTimeout")); };
        snapshotTimeout.Start();
        try { web.CoreWebView2.PostWebMessageAsString("{\"t\":\"snapshot\"}"); }
        catch (InvalidOperationException ex) { FinishSnapshot(null, null, I18n.T("errNotReady")); WebDead(ex); }
        catch (COMException ex) { FinishSnapshot(null, null, I18n.T("errNotReady")); WebDead(ex); }
    }
    void OnSnapshot(string msg)
    {
        string[] p = msg.Split(new[] { '|' }, 3);
        if (p.Length < 3) FinishSnapshot(null, null, I18n.T("errBadData"));
        else if (p[1] == "error") FinishSnapshot(null, null, p[2]);
        else FinishSnapshot(p[1], p[2], null);
    }
    void FinishSnapshot(string thumb, string png, string err)
    {
        if (snapshotTimeout != null) { snapshotTimeout.Stop(); snapshotTimeout.Dispose(); snapshotTimeout = null; }
        var done = snapshotDone;
        snapshotDone = null;
        if (done != null) done(thumb, png, err);
    }

    // ---- 分享动画：按当前图标布局录制主题动画，途中发进度，录完发 "record|格式|封面|整段视频" ----
    Action<string, string, string, string> recordDone;    // (格式, 封面, 整段视频, 出错信息)
    System.Windows.Forms.Timer recordTimeout;
    public event Action<string> RecordProgress;            // 0..1（字符串）
    public void RequestRecording(Action<string, string, string, string> done)
    {
        if (web.CoreWebView2 == null) { done(null, null, null, I18n.T("errNotReady")); return; }
        if (recordDone != null) { done(null, null, null, I18n.T("errBusyVideo")); return; }
        recordDone = done;
        recordTimeout = new System.Windows.Forms.Timer();
        recordTimeout.Interval = 45000;
        recordTimeout.Tick += delegate { FinishRecord(null, null, null, I18n.T("errVideoTimeout")); };
        recordTimeout.Start();
        try { web.CoreWebView2.PostWebMessageAsString("{\"t\":\"record\"}"); }
        catch (InvalidOperationException ex) { FinishRecord(null, null, null, I18n.T("errNotReady")); WebDead(ex); }
        catch (COMException ex) { FinishRecord(null, null, null, I18n.T("errNotReady")); WebDead(ex); }
    }
    void OnRecord(string msg)
    {
        string[] p = msg.Split(new[] { '|' }, 4);
        if (p.Length >= 3 && p[1] == "error") FinishRecord(null, null, null, msg.Substring(13));
        else if (p.Length < 4) FinishRecord(null, null, null, I18n.T("errBadData"));
        else FinishRecord(p[1], p[2], p[3], null);
    }
    void FinishRecord(string mime, string poster, string video, string err)
    {
        if (recordTimeout != null) { recordTimeout.Stop(); recordTimeout.Dispose(); recordTimeout = null; }
        var done = recordDone;
        recordDone = null;
        if (done != null) done(mime, poster, video, err);
    }
    public void Reload()
    {
        reader.ResetOrbitalPositions();
        if (web.CoreWebView2 == null) return;
        try { web.CoreWebView2.Reload(); }
        catch (InvalidOperationException ex) { WebDead(ex); }
        catch (COMException ex) { WebDead(ex); }
    }
    public void Post(string json)
    {
        if (web.CoreWebView2 == null) return;
        try { web.CoreWebView2.PostWebMessageAsString(json); }
        catch (InvalidOperationException ex) { WebDead(ex); }
        catch (COMException ex) { WebDead(ex); }
    }

    // WebView2 运行时后台更新 / 浏览器进程被杀后，CoreWebView2 不为 null 但已 disposed，任何调用都抛异常
    // （前面的 null 检查挡不住这种状态）。按"壁纸窗口被系统销毁"的既有路径处理：拆掉本窗口，HostContext 1 秒后自动重建
    void WebDead(Exception ex)
    {
        if (shuttingDown) return;
        Log.Write("WebView2 失效（运行时更新或进程退出），重新挂载壁纸窗口：" + ex.Message);
        Shutdown();
    }
    string PageUrl { get { return WebCustomHelper.PageUrl(page) + "?wallpaper=1" + (lowMode ? "&low=1" : ""); } }

    // 切换主题：换页面；新页面就绪后会重新收到设置和图标
    public void LoadPage(string newPage)
    {
        allowMoveIcons = false;
        reader.ResetOrbitalPositions();
        uiRectGroups.Clear();
        uiMouseCaptured = false;
        page = newPage;
        if (web.CoreWebView2 == null) return;
        try { web.CoreWebView2.Navigate(PageUrl); }
        catch (InvalidOperationException ex) { WebDead(ex); }
        catch (COMException ex) { WebDead(ex); }
    }

    static bool TryParseRectItem(object raw, out Rectangle rect)
    {
        rect = Rectangle.Empty;
        var arr = raw as object[];
        if (arr == null)
        {
            var lst = raw as System.Collections.ArrayList;
            if (lst != null) arr = lst.ToArray();
        }
        if (arr == null || arr.Length < 4) return false;
        try
        {
            int x = (int)Math.Floor(Convert.ToDouble(arr[0]));
            int y = (int)Math.Floor(Convert.ToDouble(arr[1]));
            int w = (int)Math.Ceiling(Convert.ToDouble(arr[2]));
            int h = (int)Math.Ceiling(Convert.ToDouble(arr[3]));
            if (w <= 0 || h <= 0) return false;
            rect = new Rectangle(x, y, w, h);
            return true;
        }
        catch { return false; }
    }

    void UpdateUiRectGroup(string kind, Dictionary<string, object> data)
    {
        object srcObj;
        string src = data.TryGetValue("src", out srcObj) && srcObj is string ? (string)srcObj : "default";
        object raw;
        if (!data.TryGetValue("r", out raw) || raw == null) { uiRectGroups.Remove(src); return; }
        var rects = new List<Rectangle>();
        if (kind == "uiRects")
        {
            var arr = raw as object[];
            if (arr == null)
            {
                var lst = raw as System.Collections.ArrayList;
                if (lst != null) arr = lst.ToArray();
            }
            if (arr != null)
            {
                for (int i = 0; i < arr.Length; i++)
                {
                    Rectangle r;
                    if (TryParseRectItem(arr[i], out r)) rects.Add(r);
                }
            }
        }
        else
        {
            Rectangle r;
            if (TryParseRectItem(raw, out r)) rects.Add(r);
        }
        if (rects.Count > 0) uiRectGroups[src] = rects;
        else uiRectGroups.Remove(src);
    }

    void OnWallJsonMessage(string msg)
    {
        var m = Json.Parse(msg) as Dictionary<string, object>;
        if (m == null) return;
        object tObj;
        if (!m.TryGetValue("t", out tObj) || !(tObj is string)) return;
        string kind = (string)tObj;
        if (kind == "uiRect" || kind == "uiRects")
        {
            UpdateUiRectGroup(kind, m);
        }
        else if (kind == "setAudioPlay")
        {
            object vObj;
            bool play = false;
            if (m.TryGetValue("v", out vObj) && vObj != null)
            {
                if (vObj is bool) play = (bool)vObj;
                else try { play = Convert.ToInt32(vObj) != 0; } catch { }
            }
            if (AudioPlayChanged != null) AudioPlayChanged(play);
        }
        else if (kind == "saveVirtualIcons")
        {
            object vObj;
            if (m.TryGetValue("v", out vObj) && VirtualIconsSaved != null) VirtualIconsSaved(vObj);
        }
        else if (kind == "saveVirtualBarCollapsed")
        {
            object vObj;
            if (m.TryGetValue("v", out vObj) && VirtualBarCollapsedSaved != null) VirtualBarCollapsedSaved(vObj);
        }
        else if (kind == "moveIcons")
        {
            HandleMoveIconsMessage(m);
        }
    }

    void HandleMoveIconsMessage(Dictionary<string, object> m)
    {
        if (!allowMoveIcons) return;
        object resetObj;
        bool reset = m.TryGetValue("reset", out resetObj) && resetObj is bool && (bool)resetObj;
        if (reset)
        {
            reader.ResetOrbitalPositions();
            return;
        }
        object rawIcons;
        if (!m.TryGetValue("icons", out rawIcons) || rawIcons == null)
        {
            reader.ResetOrbitalPositions();
            return;
        }
        var arr = rawIcons as object[];
        if (arr == null)
        {
            var lst = rawIcons as System.Collections.ArrayList;
            if (lst != null) arr = lst.ToArray();
        }
        if (arr == null || arr.Length == 0)
        {
            reader.ResetOrbitalPositions();
            return;
        }
        var targets = new List<MoveIconTarget>(arr.Length);
        for (int i = 0; i < arr.Length; i++)
        {
            var spec = arr[i] as Dictionary<string, object>;
            if (spec == null) continue;
            object idxObj, xObj, yObj;
            if (!spec.TryGetValue("index", out idxObj) || !spec.TryGetValue("x", out xObj) || !spec.TryGetValue("y", out yObj)) continue;
            try
            {
                targets.Add(new MoveIconTarget
                {
                    Index = Convert.ToInt32(idxObj),
                    X = Convert.ToInt32(xObj),
                    Y = Convert.ToInt32(yObj)
                });
            }
            catch { }
        }
        if (targets.Count == 0)
        {
            reader.ResetOrbitalPositions();
            return;
        }
        long nowMs = DateTime.UtcNow.Ticks / 10000L;
        int minIntervalMs = lowMode ? 60 : 30;
        if (nowMs - lastMoveIconsMs < minIntervalMs) return;
        lastMoveIconsMs = nowMs;
        reader.QueueMoveIcons(targets, screen, Screen.PrimaryScreen.WorkingArea, reader.CurrentMoveEpoch);
    }

    protected override async void OnShown(EventArgs e)
    {
        base.OnShown(e);
        try
        {
            if (!Themes.PageExists(pageDir, page))
            {
                Fail(I18n.T("errNoPage", "path", Themes.PageDisplayPath(pageDir, page)));
                return;
            }
            if (!AttachToDesktop())
            {
                Fail(I18n.T("errNoDesktop"));
                return;
            }
            await web.EnsureCoreWebView2Async(null);
            if (shuttingDown || IsDisposed) return;
            var core = web.CoreWebView2;
            core.SetVirtualHostNameToFolderMapping("app.local", pageDir, CoreWebView2HostResourceAccessKind.Allow);
            string audioDir = Path.Combine(InstallProfile.DataDir, "audio");
            Directory.CreateDirectory(audioDir);
            core.SetVirtualHostNameToFolderMapping("audio.local", audioDir, CoreWebView2HostResourceAccessKind.Allow);
            WebCustomHelper.Attach(core, pageDir);
            core.Settings.AreDefaultContextMenusEnabled = false;
            core.Settings.AreDevToolsEnabled = true;
            core.Settings.IsStatusBarEnabled = false;
            ApplyWebViewLowMode(lowMode);
            string lowJsPath = Path.Combine(pageDir, "low-mode.js");
            if (File.Exists(lowJsPath))
            {
                try { await core.AddScriptToExecuteOnDocumentCreatedAsync(File.ReadAllText(lowJsPath, Encoding.UTF8)); }
                catch { }
            }
            if (shuttingDown || IsDisposed) return;
            // 页面就绪（发来 "ready"）：先发设置，再发图标和暂停状态（岸线要按设置里的离岸距离来算）
            // 一键分享：页面画好分享图后发来 "snapshot|缩略图|整张 PNG"（都是 data URL；整张图很大，不走 JSON）
            core.WebMessageReceived += delegate(object s, CoreWebView2WebMessageReceivedEventArgs a)
            {
                string msg = null;
                try { msg = a.TryGetWebMessageAsString(); } catch { }
                if (msg == null) return;
                if (msg.StartsWith("snapshot|")) { OnSnapshot(msg); return; }
                if (msg.StartsWith("recprogress|")) { if (RecordProgress != null) RecordProgress(msg.Substring(12)); return; }
                if (msg.StartsWith("record|")) { OnRecord(msg); return; }
                if (msg == "ready")
                {
                    allowMoveIcons = string.Equals(page, "solar-system.html", StringComparison.OrdinalIgnoreCase);
                    if (!allowMoveIcons) reader.ResetOrbitalPositions();
                    Rectangle wa = Screen.PrimaryScreen.WorkingArea;
                    int wLeft = Math.Max(0, wa.Left - screen.Left);
                    int wTop = Math.Max(0, wa.Top - screen.Top);
                    int wRight = Math.Max(0, screen.Right - wa.Right);
                    int wBottom = Math.Max(0, screen.Bottom - wa.Bottom);
                    core.PostWebMessageAsString(Json.Stringify(Json.Obj(
                        "t", "hostCaps",
                        "moveIcons", true,
                        "orbitUi", true,
                        "lang", I18n.Code,
                        "workInsets", new object[] { wLeft, wTop, wRight, wBottom }
                    )));
                    if (VirtualStateJson != null) core.PostWebMessageAsString(VirtualStateJson());
                    core.PostWebMessageAsString(Json.Stringify(Json.Obj("t", "lowMode", "v", lowMode)));
                    if (SettingsJson != null) core.PostWebMessageAsString(SettingsJson());
                    lastIcons = ""; lastWindows = ""; lastPause = -1; PollIcons(); PollWindows(); PollForeground();
                    return;
                }
                if (msg.StartsWith("{"))
                {
                    try { OnWallJsonMessage(msg); } catch { }
                }
            };
            core.NavigationStarting += delegate
            {
                allowMoveIcons = false;
                reader.ResetOrbitalPositions();
            };
            core.Navigate(PageUrl);
            EnsureWallpaperBounds();
            StartWindowTrackHooks();
            iconTimer.Start();
            fgTimer.Start();
            winTimer.Start();
            Log.Write("WebView2 就绪：" + core.Environment.BrowserVersionString);
        }
        catch (Exception ex)
        {
            if (shuttingDown || IsDisposed) return;       // 启动途中窗口被系统销毁：交给 HostContext 重试
            Log.Write("启动失败：" + ex);
            Fail(I18n.T("errStart", "msg", ex.Message));
        }
    }

    void Fail(string msg)
    {
        Log.Write(msg);
        MessageBox.Show(msg + "\n\n" + I18n.T("errSeeLog", "path", Log.FilePath), "DiyarDesk");
        Fatal = true;
        Shutdown();
    }

    // ---------------------------------------------------------------- 挂到壁纸层
    // 两种桌面结构：
    //   · Windows 11 24H2 起（"抬高的桌面"）：Progman 下依次是 图标层（SHELLDLL_DefView）→ 系统壁纸层（WorkerW）。
    //     系统壁纸层会在换壁纸（幻灯片 / 聚焦）时被销毁重建，所以我们不挂进它里面，而是做 Progman 的
    //     分层子窗口（WS_EX_LAYERED，alpha 255），排在图标层正下方、系统壁纸层之上，并持续守住这个顺序。
    //     （Progman 没有可供普通子窗口绘制的表面，所以必须是分层窗口；分层子窗口需要 app.manifest 声明支持 Win8+）
    //   · 旧版：图标层在某个顶层 WorkerW 里，它后面紧跟着负责画壁纸的顶层 WorkerW → 挂进这个 WorkerW
    public static string AttachMode = "progman";
    bool raised;
    IntPtr zAnchor;                                     // 我们要紧跟在它下面的窗口（图标列表 / 图标层）
    IntPtr winEventHook;
    Native.WinEventDelegate winEventProc;               // 保持引用，防止被 GC
    IntPtr winLocHook, winStateHook;
    Native.WinEventDelegate winTrackProc;               // 实时跟踪顶层窗口移动/大小/开关
    int lastWinPollTick;
    readonly Dictionary<string, List<Rectangle>> uiRectGroups = new Dictionary<string, List<Rectangle>>();
    IntPtr mouseHook;
    Native.LowLevelMouseProc mouseHookProc;
    double uiScale = 1.0;
    bool uiMouseCaptured, desktopMouseDown;
    int lastDownTick = -1, lastClickCount = 1, lastHoverTick;
    Native.POINT lastDownPt;

    bool HasAnyUiRect()
    {
        foreach (var kv in uiRectGroups)
            if (kv.Value.Count > 0) return true;
        return false;
    }

    bool IsPointInUi(int cssX, int cssY)
    {
        foreach (var kv in uiRectGroups)
        {
            var list = kv.Value;
            for (int i = 0; i < list.Count; i++)
            {
                Rectangle r = list[i];
                if (cssX >= r.Left && cssX <= r.Right && cssY >= r.Top && cssY <= r.Bottom) return true;
            }
        }
        return false;
    }

    bool IsDesktopHit(Native.POINT pt)
    {
        IntPtr h = Native.WindowFromPoint(pt);
        for (int depth = 0; depth < 5 && h != IntPtr.Zero; depth++)
        {
            if (h == Handle || Native.IsChild(Handle, h)) return true;
            var sb = new StringBuilder(64);
            Native.GetClassName(h, sb, sb.Capacity);
            string cls = sb.ToString();
            if (cls == "SysListView32" || cls == "SHELLDLL_DefView" || cls == "Progman" || cls == "WorkerW") return true;
            h = Native.GetParent(h);
        }
        return false;
    }

    void DispatchWebMouse(string type, int x, int y, string button, int clickCount, int buttons)
    {
        if (shuttingDown || IsDisposed || web.CoreWebView2 == null) return;
        try
        {
            string payload = Json.Stringify(Json.Obj(
                "type", type,
                "x", x,
                "y", y,
                "button", button,
                "clickCount", clickCount,
                "buttons", buttons));
            web.CoreWebView2.CallDevToolsProtocolMethodAsync("Input.dispatchMouseEvent", payload);
        }
        catch { }
    }

    IntPtr OnMouseHook(int nCode, IntPtr wParam, IntPtr lParam)
    {
        if (nCode < 0 || shuttingDown || IsDisposed || web.CoreWebView2 == null)
            return Native.CallNextHookEx(mouseHook, nCode, wParam, lParam);

        int msg = wParam.ToInt32();
        if (msg != Native.WM_LBUTTONDOWN && msg != Native.WM_LBUTTONUP && msg != Native.WM_MOUSEMOVE)
            return Native.CallNextHookEx(mouseHook, nCode, wParam, lParam);

        var hs = (Native.MSLLHOOKSTRUCT)Marshal.PtrToStructure(lParam, typeof(Native.MSLLHOOKSTRUCT));
        double scale = uiScale > 0 ? uiScale : 1.0;
        int cssX = (int)Math.Round((hs.pt.X - screen.X) / scale);
        int cssY = (int)Math.Round((hs.pt.Y - screen.Y) / scale);

        if (msg == Native.WM_LBUTTONDOWN)
        {
            if (HasAnyUiRect() && IsPointInUi(cssX, cssY) && IsDesktopHit(hs.pt))
            {
                int now = Environment.TickCount;
                int dblMs = SystemInformation.DoubleClickTime;
                bool isDbl = lastDownTick >= 0 && unchecked(now - lastDownTick) <= dblMs
                    && Math.Abs(hs.pt.X - lastDownPt.X) <= 6 && Math.Abs(hs.pt.Y - lastDownPt.Y) <= 6;
                lastClickCount = isDbl ? 2 : 1;
                lastDownTick = now;
                lastDownPt = hs.pt;
                uiMouseCaptured = true;
                int cc = lastClickCount;
                BeginInvoke((Action)delegate { DispatchWebMouse("mousePressed", cssX, cssY, "left", cc, 1); });
                return (IntPtr)1;
            }
            if (lastPause != 1 && screen.Contains(hs.pt.X, hs.pt.Y) && IsDesktopHit(hs.pt))
            {
                desktopMouseDown = true;
                BeginInvoke((Action)delegate { DispatchWebMouse("mousePressed", cssX, cssY, "left", 1, 1); });
            }
        }
        else if (msg == Native.WM_MOUSEMOVE)
        {
            if (uiMouseCaptured)
            {
                BeginInvoke((Action)delegate { DispatchWebMouse("mouseMoved", cssX, cssY, "left", 0, 1); });
            }
            else if (lastPause != 1 && screen.Contains(hs.pt.X, hs.pt.Y))
            {
                int now = Environment.TickCount;
                int minMoveMs = lowMode ? 45 : 20;
                if (unchecked(now - lastHoverTick) >= minMoveMs && IsDesktopHit(hs.pt))
                {
                    lastHoverTick = now;
                    bool down = desktopMouseDown;
                    BeginInvoke((Action)delegate { DispatchWebMouse("mouseMoved", cssX, cssY, down ? "left" : "none", 0, down ? 1 : 0); });
                }
            }
        }
        else if (msg == Native.WM_LBUTTONUP)
        {
            if (uiMouseCaptured)
            {
                uiMouseCaptured = false;
                int cc = lastClickCount;
                BeginInvoke((Action)delegate { DispatchWebMouse("mouseReleased", cssX, cssY, "left", cc, 0); });
                return (IntPtr)1;
            }
            if (desktopMouseDown)
            {
                desktopMouseDown = false;
                BeginInvoke((Action)delegate { DispatchWebMouse("mouseReleased", cssX, cssY, "left", 1, 0); });
            }
        }
        return Native.CallNextHookEx(mouseHook, nCode, wParam, lParam);
    }

    void StartMouseHook()
    {
        if (mouseHook != IntPtr.Zero) return;
        try
        {
            using (Graphics g = Graphics.FromHwnd(Handle)) { uiScale = g.DpiX / 96.0; }
        }
        catch { uiScale = Native.GetDpiForSystem() / 96.0; }
        mouseHookProc = OnMouseHook;
        mouseHook = Native.SetWindowsHookEx(Native.WH_MOUSE_LL, mouseHookProc, Native.GetModuleHandle(null), 0);
    }

    void StopMouseHook()
    {
        uiMouseCaptured = false;
        desktopMouseDown = false;
        if (mouseHook != IntPtr.Zero)
        {
            Native.UnhookWindowsHookEx(mouseHook);
            mouseHook = IntPtr.Zero;
        }
    }

    static IntPtr FindClassicWorker()
    {
        IntPtr host = IntPtr.Zero;
        Native.EnumWindows(delegate(IntPtr h, IntPtr l)
        {
            if (Native.FindWindowEx(h, IntPtr.Zero, "SHELLDLL_DefView", null) != IntPtr.Zero) { host = h; return false; }
            return true;
        }, IntPtr.Zero);
        return host != IntPtr.Zero ? Native.FindWindowEx(IntPtr.Zero, host, "WorkerW", null) : IntPtr.Zero;
    }

    // 请资源管理器生成壁纸层（只发一次 0xD,1；再发 0xD,0 会把刚生成的壁纸层删掉），最多等 2 秒
    static void RequestWorkerW(IntPtr progman, Func<bool> ready)
    {
        IntPtr res;
        Native.SendMessageTimeout(progman, 0x052C, new IntPtr(0xD), new IntPtr(1), 0, 1000, out res);
        for (int i = 0; i < 20 && !ready(); i++) Thread.Sleep(100);
    }

    bool AttachToDesktop()
    {
        screen = Screen.PrimaryScreen.Bounds;          // 进程是 DPI 感知的 → 物理像素
        IntPtr progman = Native.FindWindow("Progman", null);
        if (progman == IntPtr.Zero) { Log.Write("找不到 Progman"); return false; }
        IntPtr defView = Native.FindWindowEx(progman, IntPtr.Zero, "SHELLDLL_DefView", null);
        string mode;
        raised = defView != IntPtr.Zero;

        if (raised)
        {
            if (Native.FindWindowEx(progman, IntPtr.Zero, "WorkerW", null) == IntPtr.Zero)
                RequestWorkerW(progman, delegate { return Native.FindWindowEx(progman, IntPtr.Zero, "WorkerW", null) != IntPtr.Zero; });
            IntPtr listView = Native.FindWindowEx(defView, IntPtr.Zero, "SysListView32", null);
            if (AttachMode == "legacy")
            {
                parentHwnd = Native.FindWindowEx(progman, IntPtr.Zero, "WorkerW", null);
                if (parentHwnd == IntPtr.Zero) { Log.Write("找不到 WorkerW"); return false; }
                insertAfter = IntPtr.Zero; raised = false; mode = "24H2+ 旧做法：WorkerW 子窗口";
            }
            else if (AttachMode == "defview" && listView != IntPtr.Zero)
            {
                // 实验：挂进图标层里、图标列表之下（换壁纸时不会被重排，但实测在 24H2 上画不出来）
                parentHwnd = defView; insertAfter = listView; zAnchor = listView;
                mode = "24H2+：图标层（DefView）的分层子窗口，位于图标列表之下";
            }
            else
            {
                parentHwnd = progman; insertAfter = defView; zAnchor = defView;
                mode = "24H2+：Progman 的分层子窗口，位于图标层之下";
            }
            if (raised)
            {
                int ex = Native.GetWindowLong(Handle, Native.GWL_EXSTYLE);
                Native.SetWindowLong(Handle, Native.GWL_EXSTYLE, ex | Native.WS_EX_LAYERED);
                Native.SetLayeredWindowAttributes(Handle, 0, 255, Native.LWA_ALPHA);
            }
        }
        else
        {
            IntPtr worker = FindClassicWorker();
            if (worker == IntPtr.Zero)
            {
                RequestWorkerW(progman, delegate { return FindClassicWorker() != IntPtr.Zero; });
                worker = FindClassicWorker();
            }
            if (worker == IntPtr.Zero) { Log.Write("找不到 WorkerW / SHELLDLL_DefView"); return false; }
            parentHwnd = worker; insertAfter = IntPtr.Zero; mode = "经典：顶层 WorkerW 子窗口";
        }

        int style = Native.GetWindowLong(Handle, Native.GWL_STYLE);
        style = (style & ~(Native.WS_POPUP | Native.WS_CAPTION | Native.WS_THICKFRAME)) | Native.WS_CHILD | Native.WS_VISIBLE;
        Native.SetWindowLong(Handle, Native.GWL_STYLE, style);
        Native.SetParent(Handle, parentHwnd);

        var pt = new Native.POINT { X = screen.X, Y = screen.Y };
        Native.ScreenToClient(parentHwnd, ref pt);
        Native.SetWindowPos(Handle, insertAfter, pt.X, pt.Y, screen.Width, screen.Height,
            Native.SWP_NOACTIVATE | Native.SWP_SHOWWINDOW | Native.SWP_FRAMECHANGED);
        EnsureWallpaperBounds();
        EnsureZOrder();
        StartMouseHook();

        if (raised)
        {
            // 资源管理器新建 / 重排壁纸层时立刻修正顺序（换壁纸时它会把新的 WorkerW 插到我们上面）
            uint pid;
            Native.GetWindowThreadProcessId(progman, out pid);
            winEventProc = delegate(IntPtr hook, uint ev, IntPtr hwnd, int idObject, int idChild, uint thread, uint time)
            {
                if (idObject == 0 && !shuttingDown && IsHandleCreated) EnsureZOrder();
            };
            winEventHook = Native.SetWinEventHook(0x8000 /* OBJECT_CREATE */, 0x8004 /* OBJECT_REORDER */, IntPtr.Zero,
                winEventProc, pid, 0, 0 /* WINEVENT_OUTOFCONTEXT */);
        }
        Log.Write(string.Format("已挂载（{0}），屏幕 {1}", mode, screen));
        return true;
    }

    void EnsureWallpaperBounds()
    {
        if (shuttingDown || IsDisposed || !IsHandleCreated || parentHwnd == IntPtr.Zero) return;
        Rectangle cur = Screen.PrimaryScreen.Bounds;
        bool screenChanged = cur != screen;
        if (screenChanged) screen = cur;

        Native.RECT r;
        bool sizeBad = !Native.GetWindowRect(Handle, out r)
            || (r.Right - r.Left) != screen.Width
            || (r.Bottom - r.Top) != screen.Height;
        if (screenChanged || sizeBad)
        {
            var pt = new Native.POINT { X = screen.X, Y = screen.Y };
            Native.ScreenToClient(parentHwnd, ref pt);
            Native.SetWindowPos(Handle, insertAfter, pt.X, pt.Y, screen.Width, screen.Height,
                Native.SWP_NOACTIVATE | Native.SWP_SHOWWINDOW | Native.SWP_FRAMECHANGED);
        }
        if (web != null && (web.Width != screen.Width || web.Height != screen.Height || web.Left != 0 || web.Top != 0))
        {
            web.Dock = DockStyle.None;
            web.Bounds = new Rectangle(0, 0, screen.Width, screen.Height);
            web.Dock = DockStyle.Fill;
        }
    }

    void StartWindowTrackHooks()
    {
        if (winLocHook != IntPtr.Zero || winStateHook != IntPtr.Zero) return;
        winTrackProc = delegate(IntPtr hook, uint ev, IntPtr hwnd, int idObject, int idChild, uint thread, uint time)
        {
            if (shuttingDown || IsDisposed || lastPause == 1) return;
            if (idObject != 0 || idChild != 0 || hwnd == IntPtr.Zero || hwnd == Handle || hwnd == parentHwnd || hwnd == zAnchor) return;
            int now = Environment.TickCount;
            int minGap = lowMode ? 48 : 12; // ~80 Hz while dragging for near-zero snow lag
            if (ev == 0x800B /* EVENT_OBJECT_LOCATIONCHANGE */ && unchecked(now - lastWinPollTick) < minGap)
                return;
            lastWinPollTick = now;
            PollWindows();
        };
        winLocHook = Native.SetWinEventHook(0x800B /* EVENT_OBJECT_LOCATIONCHANGE */, 0x800B, IntPtr.Zero,
            winTrackProc, 0, 0, 0 /* WINEVENT_OUTOFCONTEXT */);
        winStateHook = Native.SetWinEventHook(0x0003 /* EVENT_SYSTEM_FOREGROUND */, 0x0017 /* EVENT_SYSTEM_MINIMIZEEND */, IntPtr.Zero,
            winTrackProc, 0, 0, 0 /* WINEVENT_OUTOFCONTEXT */);
    }

    void StopWindowTrackHooks()
    {
        if (winLocHook != IntPtr.Zero) { Native.UnhookWinEvent(winLocHook); winLocHook = IntPtr.Zero; }
        if (winStateHook != IntPtr.Zero) { Native.UnhookWinEvent(winStateHook); winStateHook = IntPtr.Zero; }
    }

    // 守住顺序：紧跟在 zAnchor（图标列表 / 图标层）下面；Progman 挂法还要把系统壁纸层推到最底（被打乱才修）
    int zFixes;
    void EnsureZOrder()
    {
        if (!raised || zAnchor == IntPtr.Zero || !Native.IsWindow(zAnchor)) return;
        bool fixedAny = false;
        if (Native.GetWindow(zAnchor, 2 /* GW_HWNDNEXT */) != Handle)
        {
            Native.SetWindowPos(Handle, zAnchor, 0, 0, 0, 0, Native.SWP_NOMOVE | Native.SWP_NOSIZE | Native.SWP_NOACTIVATE);
            fixedAny = true;
        }
        if (AttachMode == "progman")
        {
            for (IntPtr w = Native.FindWindowEx(parentHwnd, IntPtr.Zero, "WorkerW", null); w != IntPtr.Zero;
                 w = Native.FindWindowEx(parentHwnd, w, "WorkerW", null))
            {
                if (Native.GetWindow(w, 2) != IntPtr.Zero)   // 不在最底下 → 推到最底下
                {
                    Native.SetWindowPos(w, new IntPtr(1) /* HWND_BOTTOM */, 0, 0, 0, 0, Native.SWP_NOMOVE | Native.SWP_NOSIZE | Native.SWP_NOACTIVATE);
                    fixedAny = true;
                }
            }
        }
        if (fixedAny && ++zFixes <= 20) Log.Write("修正了桌面层级（资源管理器重排了窗口）");
    }

    // ---------------------------------------------------------------- 读取桌面图标
    void PollIcons()
    {
        EnsureWallpaperBounds();
        EnsureZOrder();
        if (web.CoreWebView2 == null) return;
        bool visible;
        List<DesktopIcon> items = reader.Read(out visible);
        if (items == null) return;
        var sb = new StringBuilder();
        sb.Append("{\"t\":\"icons\",\"vis\":").Append(visible ? "true" : "false").Append(",\"r\":[");
        for (int i = 0; i < items.Count; i++)
        {
            Native.RECT r = items[i].Rect;
            if (i > 0) sb.Append(',');
            sb.Append('[').Append(r.Left - screen.X).Append(',').Append(r.Top - screen.Y).Append(',')
              .Append(r.Right - screen.X).Append(',').Append(r.Bottom - screen.Y).Append(']');
        }
        sb.Append("],\"n\":[");
        for (int i = 0; i < items.Count; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append(Json.Stringify(items[i].Name ?? ""));
        }
        sb.Append("]}");
        string json = sb.ToString();
        if (json == lastIcons) return;
        lastIcons = json;
        if (IconsChanged != null) IconsChanged(json);
        Post(json);
        Log.Write(string.Format("图标变化：{0} 个，可见={1}", items.Count, visible));
    }

    // ---------------------------------------------------------------- 可见顶层窗口矩形（供雪花/雨滴等主题在窗口边缘堆积与滑落）
    static bool IsIgnoredWindowClass(string cls)
    {
        return cls == "Progman" || cls == "WorkerW"
            || cls == "Shell_TrayWnd" || cls == "Shell_SecondaryTrayWnd"
            || cls == "Windows.UI.Core.CoreWindow"
            || cls == "ApplicationManager_DesktopShellWindow"
            || cls == "ForegroundStaging" || cls == "MultitaskingViewFrame"
            || cls == "EdgeUiInputTopWndClass" || cls == "DummyDwmListenerWindow"
            || cls == "TaskListThumbnailWnd" || cls == "NotifyIconOverflowWindow"
            || cls == "IME" || cls == "MSCTFIME UI";
    }

    void PollWindows()
    {
        if (web.CoreWebView2 == null || lastPause == 1) return;
        var sb = new StringBuilder(512);
        sb.Append("{\"t\":\"windows\",\"r\":[");
        int count = 0;
        var clsBuf = new StringBuilder(64);
        int rectSize = Marshal.SizeOf(typeof(Native.RECT));
        Native.EnumWindows(delegate(IntPtr h, IntPtr l)
        {
            if (count >= 24) return false;
            if (h == Handle || !Native.IsWindowVisible(h) || Native.IsIconic(h) || Native.IsZoomed(h))
                return true;
            int ex = Native.GetWindowLong(h, Native.GWL_EXSTYLE);
            if ((ex & 0x00000080 /* WS_EX_TOOLWINDOW */) != 0) return true;
            int st = Native.GetWindowLong(h, Native.GWL_STYLE);
            if ((st & Native.WS_CHILD) != 0) return true;
            int cloaked = 0;
            if (Native.DwmGetWindowAttribute(h, 14 /* DWMWA_CLOAKED */, out cloaked, 4) == 0 && cloaked != 0)
                return true;
            clsBuf.Length = 0;
            Native.GetClassName(h, clsBuf, clsBuf.Capacity);
            string cls = clsBuf.ToString();
            if (string.IsNullOrEmpty(cls) || IsIgnoredWindowClass(cls)) return true;

            Native.RECT wr;
            if (Native.DwmGetWindowAttribute(h, 9 /* DWMWA_EXTENDED_FRAME_BOUNDS */, out wr, rectSize) != 0)
            {
                if (!Native.GetWindowRect(h, out wr)) return true;
            }
            int ww = wr.Right - wr.Left;
            int wh = wr.Bottom - wr.Top;
            if (ww < 120 || wh < 80) return true;
            if (wr.Right <= screen.Left || wr.Left >= screen.Right || wr.Bottom <= screen.Top || wr.Top >= screen.Bottom)
                return true;
            // Skip full-screen / monitor-covering windows
            if (wr.Left <= screen.Left && wr.Top <= screen.Top && wr.Right >= screen.Right && wr.Bottom >= screen.Bottom)
                return true;

            if (count > 0) sb.Append(',');
            sb.Append('[').Append(h.ToInt64()).Append(',')
              .Append(wr.Left - screen.Left).Append(',')
              .Append(wr.Top - screen.Top).Append(',')
              .Append(wr.Right - screen.Left).Append(',')
              .Append(wr.Bottom - screen.Top).Append(']');
            count++;
            return true;
        }, IntPtr.Zero);
        sb.Append("]}");
        string json = sb.ToString();
        if (json == lastWindows) return;
        lastWindows = json;
        Post(json);
    }

    // ---------------------------------------------------------------- 最大化 / 全屏时暂停
    void PollForeground()
    {
        if (web.CoreWebView2 == null) return;
        bool pause = manualPause;
        string pauseBy = "";
        IntPtr fg = Native.GetForegroundWindow();
        int cloaked = 0;
        if (!pause && !disableAutoPause && fg != IntPtr.Zero && fg != Handle && Native.IsWindowVisible(fg)
            && (Native.DwmGetWindowAttribute(fg, 14 /* DWMWA_CLOAKED */, out cloaked, 4) != 0 || cloaked == 0))
        {
            var cls = new StringBuilder(64);
            Native.GetClassName(fg, cls, 64);
            string c = cls.ToString();
            if (c != "Progman" && c != "WorkerW" && c != "Shell_TrayWnd" && c != "Shell_SecondaryTrayWnd")
            {
                Native.RECT wr;
                Native.GetWindowRect(fg, out wr);
                bool covers = wr.Left <= screen.Left && wr.Top <= screen.Top && wr.Right >= screen.Right && wr.Bottom >= screen.Bottom;
                int cx = (wr.Left + wr.Right) / 2, cy = (wr.Top + wr.Bottom) / 2;
                bool lowCovers = false;
                if (lowMode)
                {
                    int iw = Math.Max(0, Math.Min(screen.Right, wr.Right) - Math.Max(screen.Left, wr.Left));
                    int ih = Math.Max(0, Math.Min(screen.Bottom, wr.Bottom) - Math.Max(screen.Top, wr.Top));
                    long scrArea = Math.Max(1L, (long)screen.Width * screen.Height);
                    if ((long)iw * ih * 100L >= scrArea * 65L) lowCovers = true;
                }
                if (screen.Contains(cx, cy) && (covers || Native.IsZoomed(fg) || lowCovers)) { pause = true; pauseBy = c; }
            }
        }
        int p = pause ? 1 : 0;
        if (p == lastPause) return;
        lastPause = p;
        ApplyWebViewLowMode(lowMode);
        if (PauseChanged != null) PauseChanged(pause);
        Post("{\"t\":\"pause\",\"v\":" + (pause ? "true" : "false") + "}");
        Log.Write(pause ? "暂停（有窗口最大化 / 全屏，或手动暂停）" + (pauseBy == "" ? "" : "，前台窗口类 " + pauseBy) : "继续播放");
    }

    // ---------------------------------------------------------------- 退出：拆掉窗口，让资源管理器重画原壁纸
    public void Shutdown()
    {
        if (shuttingDown) return;
        shuttingDown = true;
        StopMouseHook();
        StopWindowTrackHooks();
        iconTimer.Stop(); fgTimer.Stop(); winTimer.Stop();
        reader.ResetOrbitalPositions();
        reader.Dispose();
        IntPtr parent = parentHwnd;
        try { web.Dispose(); } catch { }
        if (IsHandleCreated) { Native.SetParent(Handle, IntPtr.Zero); Hide(); }
        // 不改动壁纸设置（幻灯片 / 聚焦壁纸也不受影响），只让资源管理器重画
        if (parent != IntPtr.Zero && Native.IsWindow(parent))
            Native.RedrawWindow(parent, IntPtr.Zero, IntPtr.Zero, 0x1 | 0x4 | 0x80 | 0x100);
        Close();
    }

    protected override void OnHandleDestroyed(EventArgs e)
    {
        StopMouseHook();
        StopWindowTrackHooks();
        if (winEventHook != IntPtr.Zero) { Native.UnhookWinEvent(winEventHook); winEventHook = IntPtr.Zero; }
        iconTimer.Stop(); fgTimer.Stop(); winTimer.Stop();
        reader.ResetOrbitalPositions();
        reader.Dispose();
        base.OnHandleDestroyed(e);
    }
}

struct DesktopIcon
{
    public Native.RECT Rect;
    public string Name;
}

struct MoveIconTarget
{
    public int Index, X, Y;
}

struct HomeIconInfo
{
    public Native.RECT Rect;
    public Native.POINT Pos;
    public string Name;
}

// 通过桌面 ListView 的跨进程消息读取图标矩形与名称（图标 + 文字，物理像素，屏幕坐标），并支持在轨道主题中临时移动图标位置
class IconReader : IDisposable
{
    const int AllocBytes = 1024;
    const int TextOffset = 128;
    const int MaxTextChars = 260;
    const int LvmGetItemPosition = 0x1010;
    const int LvmSetItemPosition32 = 0x1031;
    const int LvmSetExtendedListViewStyle = 0x1036;
    const int LvmGetExtendedListViewStyle = 0x1037;
    const uint LvsExSnapToGrid = 0x00080000;
    const int LvsAutoArrange = 0x0100;

    readonly object sync = new object();
    readonly object moveLock = new object();
    readonly AutoResetEvent moveSignal = new AutoResetEvent(false);
    readonly Dictionary<int, Native.POINT> lastMovedPts = new Dictionary<int, Native.POINT>();
    IntPtr listView, proc, mem;
    bool orbitalActive, snapGridWasOn, snapGridDisabled, autoArrangeWasOn, autoArrangeDisabled;
    List<HomeIconInfo> homeIcons;
    Thread moveWorker;
    List<MoveIconTarget> pendingTargets;
    Rectangle pendingScreen, pendingWorkArea;
    bool hasPendingMove, workerStopping;
    int moveEpoch, pendingEpoch;

    public int CurrentMoveEpoch
    {
        get { return Thread.VolatileRead(ref moveEpoch); }
    }

    static IntPtr FindListView()
    {
        IntPtr defView = IntPtr.Zero;
        IntPtr progman = Native.FindWindow("Progman", null);
        if (progman != IntPtr.Zero) defView = Native.FindWindowEx(progman, IntPtr.Zero, "SHELLDLL_DefView", null);
        if (defView == IntPtr.Zero)
        {
            Native.EnumWindows(delegate(IntPtr h, IntPtr l)
            {
                IntPtr d = Native.FindWindowEx(h, IntPtr.Zero, "SHELLDLL_DefView", null);
                if (d != IntPtr.Zero) { defView = d; return false; }
                return true;
            }, IntPtr.Zero);
        }
        return defView == IntPtr.Zero ? IntPtr.Zero : Native.FindWindowEx(defView, IntPtr.Zero, "SysListView32", null);
    }

    bool Open()
    {
        Close();
        listView = FindListView();
        if (listView == IntPtr.Zero) return false;
        uint pid;
        Native.GetWindowThreadProcessId(listView, out pid);
        proc = Native.OpenProcess(0x0008 | 0x0010 | 0x0020, false, pid);   // VM_OPERATION | VM_READ | VM_WRITE
        if (proc == IntPtr.Zero) return false;
        mem = Native.VirtualAllocEx(proc, IntPtr.Zero, new IntPtr(AllocBytes), 0x1000 | 0x2000, 0x04);
        return mem != IntPtr.Zero;
    }

    string ReadItemText(int index, IntPtr textPtr, byte[] buf)
    {
        var lv = new Native.LVITEMW();
        lv.mask = 0x0001;                                // LVIF_TEXT
        lv.iItem = index;
        lv.iSubItem = 0;
        lv.pszText = textPtr;
        lv.cchTextMax = MaxTextChars;
        IntPtr n;
        if (!Native.WriteProcessMemory(proc, mem, ref lv, Marshal.SizeOf(typeof(Native.LVITEMW)), out n)) return "";
        int len = (int)Native.SendMessage(listView, 0x1073 /* LVM_GETITEMTEXTW */, new IntPtr(index), mem);
        if (len <= 0) return "";
        if (len >= MaxTextChars) len = MaxTextChars - 1;
        int bytes = len * 2;
        if (!Native.ReadProcessMemory(proc, textPtr, buf, bytes, out n)) return "";
        int got = Math.Min(bytes, (int)n);
        int end = got - (got % 2);
        for (int j = 0; j + 1 < end; j += 2)
        {
            if (buf[j] == 0 && buf[j + 1] == 0) { end = j; break; }
        }
        return end <= 0 ? "" : Encoding.Unicode.GetString(buf, 0, end);
    }

    bool ReadLiveItemRectAndName(int index, Native.POINT origin, IntPtr textPtr, byte[] textBuf, out Native.RECT rect, out string name)
    {
        rect = new Native.RECT();
        name = "";
        IntPtr n;
        if (!Native.WriteProcessMemory(proc, mem, ref rect, 16, out n)) return false;
        Native.SendMessage(listView, 0x100E /* LVM_GETITEMRECT */, new IntPtr(index), mem);
        if (!Native.ReadProcessMemory(proc, mem, out rect, 16, out n)) return false;
        rect.Left += origin.X; rect.Right += origin.X; rect.Top += origin.Y; rect.Bottom += origin.Y;
        name = ReadItemText(index, textPtr, textBuf);
        return true;
    }

    bool CaptureHomeIcons()
    {
        if (listView == IntPtr.Zero || mem == IntPtr.Zero || !Native.IsWindow(listView)) { if (!Open()) return false; }
        if (!Native.IsWindowVisible(listView)) return false;
        int count = (int)Native.SendMessage(listView, 0x1004 /* LVM_GETITEMCOUNT */, IntPtr.Zero, IntPtr.Zero);
        if (count <= 0) return false;
        var origin = new Native.POINT();
        Native.ClientToScreen(listView, ref origin);
        IntPtr textPtr = new IntPtr(mem.ToInt64() + TextOffset);
        var textBuf = new byte[MaxTextChars * 2];
        var captured = new List<HomeIconInfo>(count);
        for (int i = 0; i < count; i++)
        {
            Native.RECT r;
            string name;
            if (!ReadLiveItemRectAndName(i, origin, textPtr, textBuf, out r, out name)) { Close(); return false; }
            var pos = new Native.POINT();
            IntPtr n;
            Native.SendMessage(listView, LvmGetItemPosition, new IntPtr(i), mem);
            if (!Native.ReadProcessMemory(proc, mem, out pos, 8, out n)) { Close(); return false; }
            captured.Add(new HomeIconInfo { Rect = r, Pos = pos, Name = name });
        }
        homeIcons = captured;
        uint exStyle = (uint)Native.SendMessage(listView, LvmGetExtendedListViewStyle, IntPtr.Zero, IntPtr.Zero).ToInt64();
        snapGridWasOn = (exStyle & LvsExSnapToGrid) != 0;
        if (snapGridWasOn)
        {
            IntPtr res;
            Native.SendMessageTimeout(listView, LvmSetExtendedListViewStyle, new IntPtr(unchecked((int)LvsExSnapToGrid)), IntPtr.Zero, 0x0002, 40, out res);
            snapGridDisabled = true;
        }
        int style = Native.GetWindowLong(listView, Native.GWL_STYLE);
        autoArrangeWasOn = (style & LvsAutoArrange) != 0;
        if (autoArrangeWasOn)
        {
            Native.SetWindowLong(listView, Native.GWL_STYLE, style & ~LvsAutoArrange);
            autoArrangeDisabled = true;
        }
        orbitalActive = true;
        lastMovedPts.Clear();
        return true;
    }

    public void QueueMoveIcons(List<MoveIconTarget> targets, Rectangle screen, Rectangle workArea, int expectedEpoch)
    {
        if (targets == null || targets.Count == 0) return;
        lock (moveLock)
        {
            if (workerStopping || expectedEpoch != moveEpoch) return;
            if (moveWorker == null)
            {
                moveWorker = new Thread(MoveWorkerLoop);
                moveWorker.IsBackground = true;
                moveWorker.Name = "DesklandIconMover";
                moveWorker.Start();
            }
            pendingTargets = targets;
            pendingScreen = screen;
            pendingWorkArea = workArea;
            pendingEpoch = expectedEpoch;
            hasPendingMove = true;
        }
        try { moveSignal.Set(); } catch { }
    }

    void MoveWorkerLoop()
    {
        while (true)
        {
            try { moveSignal.WaitOne(); }
            catch { break; }
            List<MoveIconTarget> targets;
            Rectangle scr, wa;
            int epoch;
            lock (moveLock)
            {
                if (workerStopping) break;
                if (!hasPendingMove) continue;
                targets = pendingTargets;
                scr = pendingScreen;
                wa = pendingWorkArea;
                epoch = pendingEpoch;
                hasPendingMove = false;
                pendingTargets = null;
            }
            if (targets != null && targets.Count > 0 && epoch == Thread.VolatileRead(ref moveEpoch))
            {
                try { ApplyMoveIconsSync(targets, scr, wa, epoch); }
                catch { }
            }
        }
    }

    void ApplyMoveIconsSync(List<MoveIconTarget> targets, Rectangle screen, Rectangle workingArea, int expectedEpoch)
    {
        if (targets == null || targets.Count == 0) return;
        if (expectedEpoch != Thread.VolatileRead(ref moveEpoch)) return;
        // 用户正在拖拽 / 按住鼠标或右键菜单打开时让出控制权，避免打断原生桌面操作
        if ((Native.GetAsyncKeyState(0x01) & 0x8000) != 0 || (Native.GetAsyncKeyState(0x02) & 0x8000) != 0) return;
        if (Native.FindWindow("#32768", null) != IntPtr.Zero) return;

        lock (sync)
        {
            if (expectedEpoch != Thread.VolatileRead(ref moveEpoch)) return;
            if (listView == IntPtr.Zero || mem == IntPtr.Zero || !Native.IsWindow(listView)) { if (!Open()) return; }
            if (!Native.IsWindowVisible(listView)) return;
            if (!orbitalActive || homeIcons == null)
            {
                if (!CaptureHomeIcons()) return;
            }
            int liveCount = (int)Native.SendMessage(listView, 0x1004 /* LVM_GETITEMCOUNT */, IntPtr.Zero, IntPtr.Zero);
            if (liveCount != homeIcons.Count)
            {
                ResetOrbitalPositionsLocked();
                return;
            }

            var origin = new Native.POINT();
            Native.ClientToScreen(listView, ref origin);
            Rectangle clampArea = workingArea;
            if (!clampArea.IntersectsWith(screen)) clampArea = screen;

            for (int i = 0; i < targets.Count; i++)
            {
                MoveIconTarget t = targets[i];
                if (t.Index < 0 || t.Index >= homeIcons.Count) continue;
                HomeIconInfo home = homeIcons[t.Index];
                int w = Math.Max(48, home.Rect.Right - home.Rect.Left);
                int h = Math.Max(48, home.Rect.Bottom - home.Rect.Top);
                int halfW = w / 2, halfH = h / 2;
                int minX = clampArea.Left + halfW;
                int maxX = Math.Max(minX, clampArea.Right - halfW);
                int minY = clampArea.Top + halfH;
                int maxY = Math.Max(minY, clampArea.Bottom - halfH);

                int targetScreenX = Math.Max(minX, Math.Min(maxX, screen.X + t.X));
                int targetScreenY = Math.Max(minY, Math.Min(maxY, screen.Y + t.Y));

                int homeCxScreen = (home.Rect.Left + home.Rect.Right) / 2;
                int homeCyScreen = (home.Rect.Top + home.Rect.Bottom) / 2;
                int homePosScreenX = home.Pos.X + origin.X;
                int homePosScreenY = home.Pos.Y + origin.Y;
                int centerOffsetX = homeCxScreen - homePosScreenX;
                int centerOffsetY = homeCyScreen - homePosScreenY;

                var pt = new Native.POINT();
                pt.X = (targetScreenX - origin.X) - centerOffsetX;
                pt.Y = (targetScreenY - origin.Y) - centerOffsetY;

                Native.POINT prev;
                if (lastMovedPts.TryGetValue(t.Index, out prev) && prev.X == pt.X && prev.Y == pt.Y) continue;

                IntPtr n, res;
                if (!Native.WriteProcessMemory(proc, mem, ref pt, 8, out n))
                {
                    orbitalActive = false; homeIcons = null; snapGridDisabled = false; autoArrangeDisabled = false;
                    lastMovedPts.Clear();
                    Close();
                    return;
                }
                Native.SendMessageTimeout(listView, LvmSetItemPosition32, new IntPtr(t.Index), mem, 0x0002, 40, out res);
                lastMovedPts[t.Index] = pt;
            }
        }
    }

    public void ResetOrbitalPositions()
    {
        lock (moveLock)
        {
            moveEpoch++;
            hasPendingMove = false;
            pendingTargets = null;
        }
        lock (sync)
        {
            ResetOrbitalPositionsLocked();
        }
    }

    void ResetOrbitalPositionsLocked()
    {
        if (!orbitalActive && !snapGridDisabled && !autoArrangeDisabled && homeIcons == null) return;
        if (listView != IntPtr.Zero && mem != IntPtr.Zero && Native.IsWindow(listView))
        {
            IntPtr res;
            if (homeIcons != null)
            {
                int liveCount = (int)Native.SendMessage(listView, 0x1004 /* LVM_GETITEMCOUNT */, IntPtr.Zero, IntPtr.Zero);
                int nItems = Math.Min(liveCount, homeIcons.Count);
                for (int i = 0; i < nItems; i++)
                {
                    Native.POINT pt = homeIcons[i].Pos;
                    IntPtr n;
                    if (!Native.WriteProcessMemory(proc, mem, ref pt, 8, out n)) break;
                    Native.SendMessageTimeout(listView, LvmSetItemPosition32, new IntPtr(i), mem, 0x0002, 40, out res);
                }
            }
            if (snapGridDisabled)
            {
                Native.SendMessageTimeout(listView, LvmSetExtendedListViewStyle,
                    new IntPtr(unchecked((int)LvsExSnapToGrid)),
                    snapGridWasOn ? new IntPtr(unchecked((int)LvsExSnapToGrid)) : IntPtr.Zero,
                    0x0002, 40, out res);
            }
            if (autoArrangeDisabled)
            {
                int style = Native.GetWindowLong(listView, Native.GWL_STYLE);
                Native.SetWindowLong(listView, Native.GWL_STYLE, autoArrangeWasOn ? (style | LvsAutoArrange) : (style & ~LvsAutoArrange));
            }
        }
        orbitalActive = false;
        snapGridDisabled = false;
        snapGridWasOn = false;
        autoArrangeDisabled = false;
        autoArrangeWasOn = false;
        homeIcons = null;
        lastMovedPts.Clear();
    }

    public List<DesktopIcon> Read(out bool visible)
    {
        lock (sync)
        {
            visible = false;
            if (listView == IntPtr.Zero || mem == IntPtr.Zero || !Native.IsWindow(listView)) { if (!Open()) return null; }
            visible = Native.IsWindowVisible(listView);
            var result = new List<DesktopIcon>();
            if (!visible)
            {
                if (orbitalActive) ResetOrbitalPositionsLocked();
                return result;                   // 用户隐藏了桌面图标 → 全是海
            }
            int count = (int)Native.SendMessage(listView, 0x1004 /* LVM_GETITEMCOUNT */, IntPtr.Zero, IntPtr.Zero);
            if (orbitalActive && homeIcons != null)
            {
                if (count == homeIcons.Count)
                {
                    IntPtr tp = new IntPtr(mem.ToInt64() + TextOffset);
                    var tb = new byte[MaxTextChars * 2];
                    bool namesMatch = true;
                    for (int i = 0; i < count; i++)
                    {
                        if (!string.Equals(ReadItemText(i, tp, tb), homeIcons[i].Name, StringComparison.Ordinal))
                        {
                            namesMatch = false;
                            break;
                        }
                    }
                    if (namesMatch)
                    {
                        for (int i = 0; i < homeIcons.Count; i++)
                            result.Add(new DesktopIcon { Rect = homeIcons[i].Rect, Name = homeIcons[i].Name });
                        return result;
                    }
                }
                ResetOrbitalPositionsLocked();
            }
            var origin = new Native.POINT();
            Native.ClientToScreen(listView, ref origin);
            IntPtr textPtr = new IntPtr(mem.ToInt64() + TextOffset);
            var textBuf = new byte[MaxTextChars * 2];
            for (int i = 0; i < count; i++)
            {
                Native.RECT r;                 // Left = LVIR_BOUNDS（0）：图标 + 文字的整体范围
                string name;
                if (!ReadLiveItemRectAndName(i, origin, textPtr, textBuf, out r, out name)) { Close(); return null; }
                var item = new DesktopIcon();
                item.Rect = r;
                item.Name = name;
                result.Add(item);
            }
            return result;
        }
    }

    void Close()
    {
        if (mem != IntPtr.Zero && proc != IntPtr.Zero) Native.VirtualFreeEx(proc, mem, IntPtr.Zero, 0x8000);
        if (proc != IntPtr.Zero) Native.CloseHandle(proc);
        mem = proc = IntPtr.Zero;
    }

    public void Dispose()
    {
        Thread worker;
        lock (moveLock)
        {
            workerStopping = true;
            hasPendingMove = false;
            pendingTargets = null;
            worker = moveWorker;
            moveWorker = null;
        }
        try { moveSignal.Set(); } catch { }
        if (worker != null && worker.IsAlive)
        {
            try { worker.Join(300); } catch { }
        }
        ResetOrbitalPositions();
        lock (sync) { Close(); }
        try { moveSignal.Close(); } catch { }
    }
}

static class Native
{
    public const int GWL_STYLE = -16, GWL_EXSTYLE = -20, WS_EX_LAYERED = 0x00080000;
    public const uint LWA_ALPHA = 0x2, SWP_NOSIZE = 0x0001, SWP_NOMOVE = 0x0002;
    public const uint MOD_ALT = 0x1, MOD_CONTROL = 0x2, MOD_NOREPEAT = 0x4000;
    public const int SW_SHOW = 5, SW_RESTORE = 9;
    [DllImport("user32.dll")] public static extern bool RegisterHotKey(IntPtr h, int id, uint modifiers, uint vk);
    [DllImport("dwmapi.dll")] public static extern int DwmSetWindowAttribute(IntPtr h, int attr, ref int val, int size);
    [DllImport("user32.dll")] public static extern bool UnregisterHotKey(IntPtr h, int id);
    [DllImport("user32.dll")] public static extern bool AllowSetForegroundWindow(int pid);
    [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
    [DllImport("wtsapi32.dll", CharSet = CharSet.Unicode)] public static extern bool WTSQuerySessionInformation(IntPtr server, int session, int infoClass, out IntPtr buffer, out int bytes);
    [DllImport("wtsapi32.dll")] public static extern void WTSFreeMemory(IntPtr p);
    public delegate void WinEventDelegate(IntPtr hook, uint ev, IntPtr hwnd, int idObject, int idChild, uint thread, uint time);
    [DllImport("user32.dll")] public static extern bool SetLayeredWindowAttributes(IntPtr h, uint key, byte alpha, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
    [DllImport("user32.dll")] public static extern IntPtr SetWinEventHook(uint min, uint max, IntPtr mod, WinEventDelegate proc, uint pid, uint tid, uint flags);
    [DllImport("user32.dll")] public static extern bool UnhookWinEvent(IntPtr hook);
    public const int WS_CHILD = 0x40000000, WS_VISIBLE = 0x10000000, WS_POPUP = unchecked((int)0x80000000);
    public const int WS_CAPTION = 0x00C00000, WS_THICKFRAME = 0x00040000;
    public const uint SWP_NOACTIVATE = 0x0010, SWP_SHOWWINDOW = 0x0040, SWP_FRAMECHANGED = 0x0020;
    public const int WH_MOUSE_LL = 14, WM_MOUSEMOVE = 0x0200, WM_LBUTTONDOWN = 0x0201, WM_LBUTTONUP = 0x0202;
    public delegate IntPtr LowLevelMouseProc(int nCode, IntPtr wParam, IntPtr lParam);
    [StructLayout(LayoutKind.Sequential)] public struct MSLLHOOKSTRUCT { public POINT pt; public uint mouseData, flags, time; public IntPtr dwExtraInfo; }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr GetModuleHandle(string lpModuleName);
    [DllImport("user32.dll", SetLastError = true)] public static extern IntPtr SetWindowsHookEx(int idHook, LowLevelMouseProc lpfn, IntPtr hMod, uint dwThreadId);
    [DllImport("user32.dll")] public static extern bool UnhookWindowsHookEx(IntPtr hhk);
    [DllImport("user32.dll")] public static extern IntPtr CallNextHookEx(IntPtr hhk, int nCode, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT pt);
    [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsChild(IntPtr parent, IntPtr child);

    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct LVITEMW
    {
        public uint mask;
        public int iItem, iSubItem;
        public uint state, stateMask;
        public IntPtr pszText;
        public int cchTextMax, iImage;
        public IntPtr lParam;
        public int iGroupId;
        public uint cColumns;
        public IntPtr puColumns;
        public IntPtr piColFmt;
        public int iGroup;
    }
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr after, string cls, string title);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr h, uint msg, IntPtr w, IntPtr l, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SetParent(IntPtr child, IntPtr parent);
    [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int index);
    [DllImport("user32.dll")] public static extern int SetWindowLong(IntPtr h, int index, int value);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder sb, int max);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool RedrawWindow(IntPtr h, IntPtr rect, IntPtr rgn, uint flags);
    [DllImport("user32.dll")] public static extern short GetAsyncKeyState(int vKey);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attribute, out int value, int size);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attribute, out RECT value, int size);
    [DllImport("kernel32.dll")] public static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
    [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
    [DllImport("kernel32.dll")] public static extern IntPtr VirtualAllocEx(IntPtr proc, IntPtr addr, IntPtr size, uint type, uint protect);
    [DllImport("kernel32.dll")] public static extern bool VirtualFreeEx(IntPtr proc, IntPtr addr, IntPtr size, uint type);
    [DllImport("kernel32.dll")] public static extern bool WriteProcessMemory(IntPtr proc, IntPtr addr, ref RECT buf, int size, out IntPtr written);
    [DllImport("kernel32.dll")] public static extern bool WriteProcessMemory(IntPtr proc, IntPtr addr, ref POINT buf, int size, out IntPtr written);
    [DllImport("kernel32.dll")] public static extern bool WriteProcessMemory(IntPtr proc, IntPtr addr, ref LVITEMW buf, int size, out IntPtr written);
    [DllImport("kernel32.dll")] public static extern bool ReadProcessMemory(IntPtr proc, IntPtr addr, out RECT buf, int size, out IntPtr read);
    [DllImport("kernel32.dll")] public static extern bool ReadProcessMemory(IntPtr proc, IntPtr addr, out POINT buf, int size, out IntPtr read);
    [DllImport("kernel32.dll")] public static extern bool ReadProcessMemory(IntPtr proc, IntPtr addr, [Out] byte[] buf, int size, out IntPtr read);
}

// Deskland Preview has its own data and instance identity; isolated profiles support installation QA.
static class InstallProfile
{
    public static readonly string Override = Environment.GetEnvironmentVariable("DESKLAND_PROFILE_DIR");
    public static readonly bool Isolated = !string.IsNullOrEmpty(Override);
    public static readonly string DataDir = Isolated ? Path.GetFullPath(Override) : Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "DesklandPreview");
    public static readonly string Suffix = MakeSuffix();
    static string MakeSuffix()
    {
        if (!Isolated) return "_Preview";
        using (var hash = System.Security.Cryptography.SHA256.Create())
            return "_Preview_" + BitConverter.ToString(hash.ComputeHash(Encoding.UTF8.GetBytes(DataDir.ToUpperInvariant()))).Replace("-", "").Substring(0, 24);
    }

    // An installer may only close the instance using this exact executable path.
    public static int StopInstalled()
    {
        foreach (Process process in Process.GetProcessesByName("Deskland"))
        {
            using (process)
            {
                if (process.Id == Process.GetCurrentProcess().Id) continue;
                try
                {
                    if (!string.Equals(process.MainModule.FileName, Application.ExecutablePath, StringComparison.OrdinalIgnoreCase)) continue;
                    EventWaitHandle ev;
                    if (!EventWaitHandle.TryOpenExisting(Program.ExitEventName, out ev)) return 1;
                    using (ev) ev.Set();
                    if (!process.WaitForExit(10000)) return 1;
                }
                catch (InvalidOperationException) { }
                catch { return 1; }
            }
        }
        return 0;
    }
}
