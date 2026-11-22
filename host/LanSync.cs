using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Net;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Windows.Forms;

// Cloud-free local Wi-Fi (LAN) pairing & custom theme transfer server for Android companions.
// Disabled by default; starts only on explicit user action with a short-lived 4-digit PIN.
sealed class LanSyncServer
{
    const int PairTtlSeconds = 600;
    const int MaxFailedAttempts = 5;

    readonly object sync = new object();
    TcpListener listener;
    Thread acceptThread;
    volatile bool running;
    string pin = "";
    string hostIp = "127.0.0.1";
    int port;
    DateTime expiresUtc = DateTime.MinValue;
    int failedAttempts;

    public static string DetectLanIp()
    {
        try
        {
            using (var sock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp))
            {
                sock.Connect("192.0.2.1", 80);
                var ep = sock.LocalEndPoint as IPEndPoint;
                if (ep != null)
                {
                    string ip = ep.Address.ToString();
                    if (!string.IsNullOrEmpty(ip) && !ip.StartsWith("127.", StringComparison.Ordinal))
                        return ip;
                }
            }
        }
        catch { }
        return "127.0.0.1";
    }

    static string GeneratePin()
    {
        byte[] buf = new byte[4];
        using (var rng = new RNGCryptoServiceProvider()) rng.GetBytes(buf);
        uint val = BitConverter.ToUInt32(buf, 0) % 9000u + 1000u;
        return val.ToString("D4", CultureInfo.InvariantCulture);
    }

    public Dictionary<string, object> Start()
    {
        Stop();
        var tcp = new TcpListener(IPAddress.Any, 0);
        tcp.Start();
        int boundPort = ((IPEndPoint)tcp.LocalEndpoint).Port;
        string generatedPin = GeneratePin();
        string ip = DetectLanIp();

        lock (sync)
        {
            listener = tcp;
            port = boundPort;
            pin = generatedPin;
            hostIp = ip;
            expiresUtc = DateTime.UtcNow.AddSeconds(PairTtlSeconds);
            failedAttempts = 0;
            running = true;
        }

        acceptThread = new Thread(AcceptLoop);
        acceptThread.IsBackground = true;
        acceptThread.Name = "DiyarDesk-LanPair";
        acceptThread.Start();
        return Info();
    }

    public Dictionary<string, object> Info()
    {
        lock (sync)
        {
            int rem = running ? Math.Max(0, (int)(expiresUtc - DateTime.UtcNow).TotalSeconds) : 0;
            bool active = running && rem > 0 && failedAttempts < MaxFailedAttempts;
            return Json.Obj(
                "active", active,
                "host", hostIp,
                "port", port,
                "pin", active ? pin : "",
                "uri", active ? ("diyardesk://pair?host=" + hostIp + "&port=" + port + "&pin=" + pin) : "",
                "expiresIn", rem
            );
        }
    }

    public void Stop()
    {
        TcpListener old;
        lock (sync)
        {
            running = false;
            old = listener;
            listener = null;
            pin = "";
            port = 0;
            expiresUtc = DateTime.MinValue;
            failedAttempts = 0;
        }
        if (old != null)
        {
            try { old.Stop(); } catch { }
        }
    }

    void AcceptLoop()
    {
        while (running)
        {
            TcpClient client = null;
            try
            {
                TcpListener l;
                lock (sync) { l = listener; }
                if (l == null) break;
                client = l.AcceptTcpClient();
            }
            catch
            {
                if (!running) break;
                continue;
            }
            ThreadPool.QueueUserWorkItem(delegate { HandleClient(client); });
        }
    }

    void HandleClient(TcpClient client)
    {
        using (client)
        {
            try
            {
                client.ReceiveTimeout = 5000;
                client.SendTimeout = 15000;
                using (NetworkStream ns = client.GetStream())
                using (var reader = new StreamReader(ns, Encoding.ASCII, false, 4096, true))
                {
                    string reqLine = reader.ReadLine();
                    if (string.IsNullOrEmpty(reqLine)) return;
                    string[] parts = reqLine.Split(' ');
                    if (parts.Length < 2 || !string.Equals(parts[0], "GET", StringComparison.OrdinalIgnoreCase))
                    {
                        WriteResponse(ns, 405, "Method Not Allowed", "text/plain", Encoding.UTF8.GetBytes("Method Not Allowed"), null);
                        return;
                    }
                    string headerPin = null;
                    string line;
                    int headerCount = 0;
                    while (!string.IsNullOrEmpty(line = reader.ReadLine()) && headerCount < 64)
                    {
                        headerCount++;
                        int idx = line.IndexOf(':');
                        if (idx > 0 && string.Equals(line.Substring(0, idx).Trim(), "X-Diyar-Pin", StringComparison.OrdinalIgnoreCase))
                            headerPin = line.Substring(idx + 1).Trim();
                    }

                    string rawTarget = parts[1];
                    string path = rawTarget;
                    string query = "";
                    int qIdx = rawTarget.IndexOf('?');
                    if (qIdx >= 0)
                    {
                        path = rawTarget.Substring(0, qIdx);
                        query = rawTarget.Substring(qIdx + 1);
                    }
                    string suppliedPin = !string.IsNullOrEmpty(headerPin) ? headerPin : ExtractQueryParam(query, "pin");

                    int authCode = ValidatePin(suppliedPin);
                    if (authCode == 410)
                    {
                        WriteResponse(ns, 410, "Gone", "text/plain", Encoding.UTF8.GetBytes("Pairing session expired"), null);
                        return;
                    }
                    if (authCode == 403)
                    {
                        WriteResponse(ns, 403, "Forbidden", "text/plain", Encoding.UTF8.GetBytes("Invalid pairing PIN"), null);
                        return;
                    }

                    if (path == "/pair/info")
                    {
                        var customs = CustomThemes.Load();
                        var customList = new List<Dictionary<string, object>>();
                        for (int i = 0; i < customs.Count; i++)
                        {
                            object idObj, nameObj, descObj;
                            if (!customs[i].TryGetValue("id", out idObj) || !(idObj is string)) continue;
                            customs[i].TryGetValue("name", out nameObj);
                            customs[i].TryGetValue("desc", out descObj);
                            customList.Add(Json.Obj(
                                "id", (string)idObj,
                                "name", nameObj as string ?? (string)idObj,
                                "desc", descObj as string ?? ""
                            ));
                        }
                        var payload = Json.Obj(
                            "app", "DiyarDesk",
                            "version", Application.ProductVersion,
                            "activeTheme", Settings.Theme,
                            "params", Settings.For(Settings.Theme),
                            "customThemes", customList
                        );
                        byte[] body = Encoding.UTF8.GetBytes(Json.Stringify(payload));
                        WriteResponse(ns, 200, "OK", "application/json; charset=utf-8", body, null);
                        return;
                    }

                    if (path.StartsWith("/pair/theme/", StringComparison.Ordinal) && path.EndsWith(".deskland", StringComparison.Ordinal))
                    {
                        string themeId = Uri.UnescapeDataString(path.Substring("/pair/theme/".Length, path.Length - "/pair/theme/".Length - ".deskland".Length));
                        byte[] archive = PackCustomTheme(themeId);
                        if (archive == null)
                        {
                            WriteResponse(ns, 404, "Not Found", "text/plain", Encoding.UTF8.GetBytes("Theme not found"), null);
                            return;
                        }
                        WriteResponse(ns, 200, "OK", "application/zip", archive, "Content-Disposition: attachment; filename=\"" + Path.GetFileName(themeId) + ".deskland\"\r\n");
                        return;
                    }

                    WriteResponse(ns, 404, "Not Found", "text/plain", Encoding.UTF8.GetBytes("Not Found"), null);
                }
            }
            catch { }
        }
    }

    int ValidatePin(string supplied)
    {
        lock (sync)
        {
            if (!running || DateTime.UtcNow >= expiresUtc || failedAttempts >= MaxFailedAttempts)
                return 410;
            if (string.IsNullOrEmpty(supplied) || supplied.Length != pin.Length)
            {
                failedAttempts++;
                return 403;
            }
            int diff = 0;
            for (int i = 0; i < pin.Length; i++) diff |= pin[i] ^ supplied[i];
            if (diff != 0)
            {
                failedAttempts++;
                return 403;
            }
            return 200;
        }
    }

    static string ExtractQueryParam(string query, string key)
    {
        if (string.IsNullOrEmpty(query)) return "";
        string[] pairs = query.Split('&');
        for (int i = 0; i < pairs.Length; i++)
        {
            int eq = pairs[i].IndexOf('=');
            if (eq <= 0) continue;
            if (string.Equals(pairs[i].Substring(0, eq), key, StringComparison.OrdinalIgnoreCase))
                return Uri.UnescapeDataString(pairs[i].Substring(eq + 1)).Trim();
        }
        return "";
    }

    static byte[] PackCustomTheme(string themeId)
    {
        if (string.IsNullOrEmpty(themeId) || !themeId.StartsWith("custom-", StringComparison.Ordinal)) return null;
        string safe = Path.GetFileName(themeId);
        if (safe != themeId) return null;
        string root = Path.GetFullPath(CustomThemes.Dir);
        string dir = Path.GetFullPath(Path.Combine(root, safe));
        if (!dir.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase) || !Directory.Exists(dir))
            return null;

        using (var ms = new MemoryStream())
        {
            long total = 0;
            using (var zf = new ZipArchive(ms, ZipArchiveMode.Create, true))
            {
                string[] files = Directory.GetFiles(dir, "*", SearchOption.AllDirectories);
                Array.Sort(files, StringComparer.OrdinalIgnoreCase);
                string prefix = dir.TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
                for (int i = 0; i < files.Length; i++)
                {
                    var fi = new FileInfo(files[i]);
                    if ((fi.Attributes & FileAttributes.ReparsePoint) != 0) continue;
                    total += fi.Length;
                    if (total > CustomThemes.MaxArchiveBytes) return null;
                    string rel = fi.FullName.Substring(prefix.Length).Replace('\\', '/');
                    var entry = zf.CreateEntry(rel, CompressionLevel.Fastest);
                    using (Stream dst = entry.Open())
                    using (FileStream src = File.OpenRead(fi.FullName))
                        src.CopyTo(dst);
                }
            }
            return ms.ToArray();
        }
    }

    static void WriteResponse(NetworkStream ns, int code, string status, string contentType, byte[] body, string extraHeaders)
    {
        string hdr = "HTTP/1.1 " + code + " " + status + "\r\n"
            + "Content-Type: " + contentType + "\r\n"
            + "Content-Length: " + body.Length + "\r\n"
            + "Cache-Control: no-store\r\n"
            + "Connection: close\r\n"
            + (extraHeaders ?? "")
            + "\r\n";
        byte[] hdrBytes = Encoding.ASCII.GetBytes(hdr);
        ns.Write(hdrBytes, 0, hdrBytes.Length);
        if (body.Length > 0) ns.Write(body, 0, body.Length);
        ns.Flush();
    }
}
