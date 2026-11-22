// Local Named Pipe CLI IPC server/client (`Deskland.exe --ctl ...`) + local Solar/Dark-Mode helper.
// 100% local-only, C# 5 compatible (.NET Framework 4.x csc.exe).

using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using Microsoft.Win32;

static class EnvironmentHelper
{
    public static bool IsSystemDarkMode()
    {
        try
        {
            using (RegistryKey k = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", false))
            {
                if (k != null)
                {
                    object v = k.GetValue("AppsUseLightTheme");
                    if (v is int) return ((int)v) == 0;
                }
            }
        }
        catch { }
        return false;
    }

    public static string BuildSolarMessage(bool solarSync)
    {
        DateTime now = DateTime.Now;
        double h = now.Hour + (now.Minute / 60.0);
        string phase = "night";
        if (h >= 5.0 && h < 8.0) phase = "dawn";
        else if (h >= 8.0 && h < 17.0) phase = "day";
        else if (h >= 17.0 && h < 20.0) phase = "dusk";
        double daylight = Math.Sin(((h - 6.0) / 12.0) * Math.PI);
        if (daylight < 0.0) daylight = 0.0;
        if (daylight > 1.0) daylight = 1.0;
        return Json.Stringify(Json.Obj(
            "t", "environment",
            "hour", Math.Round(h, 2),
            "phase", phase,
            "daylight", Math.Round(daylight, 3),
            "darkMode", IsSystemDarkMode(),
            "solarSync", solarSync));
    }
}

sealed class IpcControlServer
{
    public static string PipeName
    {
        get { return "DiyarDesk_IPC" + InstallProfile.Suffix; }
    }

    readonly Func<Dictionary<string, object>, Dictionary<string, object>> handler;
    volatile bool running;
    Thread worker;

    public IpcControlServer(Func<Dictionary<string, object>, Dictionary<string, object>> handler)
    {
        this.handler = handler;
    }

    public void Start()
    {
        if (running) return;
        running = true;
        worker = new Thread(AcceptLoop);
        worker.IsBackground = true;
        worker.Name = "DiyarDesk-IPC";
        worker.Start();
    }

    public void Stop()
    {
        if (!running) return;
        running = false;
        try
        {
            using (var wake = new NamedPipeClientStream(".", PipeName, PipeDirection.InOut))
            {
                wake.Connect(150);
            }
        }
        catch { }
    }

    void AcceptLoop()
    {
        while (running)
        {
            NamedPipeServerStream server = null;
            try
            {
                server = new NamedPipeServerStream(PipeName, PipeDirection.InOut, 4, PipeTransmissionMode.Byte, PipeOptions.None);
                server.WaitForConnection();
                if (!running) break;
                ServeClient(server);
            }
            catch
            {
                if (!running) break;
                Thread.Sleep(150);
            }
            finally
            {
                if (server != null)
                {
                    try { server.Dispose(); } catch { }
                }
            }
        }
    }

    void ServeClient(NamedPipeServerStream pipe)
    {
        try
        {
            var reader = new StreamReader(pipe, new UTF8Encoding(false), false, 4096, true);
            string line = reader.ReadLine();
            Dictionary<string, object> resp;
            if (string.IsNullOrWhiteSpace(line))
            {
                resp = Json.Obj("ok", false, "error", "empty request");
            }
            else
            {
                var req = Json.Parse(line) as Dictionary<string, object>;
                if (req == null)
                    resp = Json.Obj("ok", false, "error", "invalid json object");
                else
                    resp = handler(req);
            }
            byte[] outBytes = new UTF8Encoding(false).GetBytes(Json.Stringify(resp) + "\n");
            pipe.Write(outBytes, 0, outBytes.Length);
            pipe.Flush();
        }
        catch { }
    }
}

static class IpcControlClient
{
    [DllImport("kernel32.dll")]
    static extern bool AttachConsole(int dwProcessId);

    static object ParseValue(string raw)
    {
        if (raw == null) return "";
        string low = raw.Trim().ToLowerInvariant();
        if (low == "true") return true;
        if (low == "false") return false;
        int iv;
        if (int.TryParse(raw, NumberStyles.Integer, CultureInfo.InvariantCulture, out iv)) return iv;
        double dv;
        if (double.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out dv)) return dv;
        return raw;
    }

    public static int Run(string[] args)
    {
        try { AttachConsole(-1); } catch { }
        // args starts at "--ctl"
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: Deskland.exe --ctl <status|setwp|setprop|pause|play|toggle|random|lowmode|solar|window-aware|playlist|reload|exit> [args...]");
            return 2;
        }
        bool asJson = false;
        var positional = new List<string>();
        for (int i = 1; i < args.Length; i++)
        {
            if (args[i] == "--json") asJson = true;
            else positional.Add(args[i]);
        }
        if (positional.Count == 0) return 2;

        string cmd = positional[0].ToLowerInvariant();
        var req = Json.Obj("cmd", cmd);
        if (cmd == "setwp" && positional.Count >= 2)
        {
            req["theme"] = positional[1];
        }
        else if (cmd == "setprop" && positional.Count >= 3)
        {
            req["key"] = positional[1];
            req["value"] = ParseValue(positional[2]);
            if (positional.Count >= 4) req["theme"] = positional[3];
        }
        else if ((cmd == "lowmode" || cmd == "solar" || cmd == "window-aware" || cmd == "system-bridge" || cmd == "mpv"))
        {
            req["state"] = positional.Count >= 2 ? positional[1].ToLowerInvariant() : "toggle";
        }
        else if (cmd == "playlist")
        {
            req["action"] = positional.Count >= 2 ? positional[1].ToLowerInvariant() : "status";
            int mins;
            if (positional.Count >= 3 && int.TryParse(positional[2], out mins) && mins > 0)
                req["interval"] = mins;
        }

        try
        {
            using (var client = new NamedPipeClientStream(".", IpcControlServer.PipeName, PipeDirection.InOut))
            {
                client.Connect(2500);
                byte[] bytes = new UTF8Encoding(false).GetBytes(Json.Stringify(req) + "\n");
                client.Write(bytes, 0, bytes.Length);
                client.Flush();
                var reader = new StreamReader(client, new UTF8Encoding(false), false, 4096, true);
                string line = reader.ReadLine();
                if (string.IsNullOrEmpty(line))
                {
                    Console.Error.WriteLine("error: empty response");
                    return 1;
                }
                var resp = Json.Parse(line) as Dictionary<string, object>;
                bool ok = resp != null && resp.ContainsKey("ok") && resp["ok"] is bool && (bool)resp["ok"];
                if (asJson)
                {
                    Console.WriteLine(line);
                }
                else if (ok)
                {
                    object msgObj;
                    if (resp.TryGetValue("message", out msgObj) && msgObj != null)
                        Console.WriteLine(Convert.ToString(msgObj, CultureInfo.InvariantCulture));
                    else
                        Console.WriteLine(line);
                }
                else
                {
                    object errObj = null;
                    if (resp != null) resp.TryGetValue("error", out errObj);
                    Console.Error.WriteLine("error: " + (errObj != null ? Convert.ToString(errObj, CultureInfo.InvariantCulture) : "command failed"));
                }
                return ok ? 0 : 1;
            }
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine("error: DiyarDesk is not running (" + ex.Message + ")");
            return 1;
        }
    }
}
