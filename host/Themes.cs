using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Net;
using System.Reflection;
using System.Text;
using System.Text.RegularExpressions;

// 主题清单（prototype/themes.json + 用户导入主题 registry.json）：
// 页面映射、自定义主题导入与合并读取均在此模块统一处理（C# 5 兼容）。
static class Themes
{
    public static List<Dictionary<string, object>> LoadBuiltIn(string pageDir)
    {
        var list = new List<Dictionary<string, object>>();
        try
        {
            var root = Json.Parse(File.ReadAllText(Path.Combine(pageDir, "themes.json"), Encoding.UTF8)) as Dictionary<string, object>;
            if (root == null || !root.ContainsKey("themes")) return list;
            foreach (object o in (IEnumerable)root["themes"])
            {
                var theme = o as Dictionary<string, object>;
                if (theme != null) list.Add(theme);
            }
        }
        catch (Exception ex) { Log.Write("读取 themes.json 失败：" + ex.Message); }
        return list;
    }

    public static Dictionary<string, object> Find(string pageDir, string id)
    {
        if (string.IsNullOrEmpty(id)) return null;
        var builtIn = LoadBuiltIn(pageDir);
        for (int i = 0; i < builtIn.Count; i++)
        {
            var t = builtIn[i];
            if ((t["id"] as string) == id && t.ContainsKey("status") && (t["status"] as string) == "ready")
                return t;
        }
        var custom = CustomThemes.Load();
        for (int i = 0; i < custom.Count; i++)
        {
            var t = custom[i];
            if ((t["id"] as string) == id && t.ContainsKey("status") && (t["status"] as string) == "ready")
                return t;
        }
        return null;
    }

    public static byte[] MergedThemesJsonBytes(string pageDir)
    {
        try
        {
            var root = Json.Parse(File.ReadAllText(Path.Combine(pageDir, "themes.json"), Encoding.UTF8)) as Dictionary<string, object>;
            if (root == null) root = Json.Obj("version", 1, "themes", new ArrayList());

            var merged = new List<object>();
            var seen = new HashSet<string>(StringComparer.Ordinal);
            object rawThemes;
            if (root.TryGetValue("themes", out rawThemes) && rawThemes is IEnumerable)
            {
                foreach (object o in (IEnumerable)rawThemes)
                {
                    var d = o as Dictionary<string, object>;
                    if (d == null) continue;
                    string tid = d.ContainsKey("id") ? d["id"] as string : null;
                    if (!string.IsNullOrEmpty(tid)) seen.Add(tid);
                    merged.Add(d);
                }
            }
            var custom = CustomThemes.Load();
            for (int i = 0; i < custom.Count; i++)
            {
                var ct = custom[i];
                string tid = ct.ContainsKey("id") ? ct["id"] as string : null;
                if (string.IsNullOrEmpty(tid) || seen.Contains(tid)) continue;
                seen.Add(tid);
                merged.Add(ct);
            }
            root["themes"] = merged;
            return Encoding.UTF8.GetBytes(Json.Stringify(root));
        }
        catch (Exception ex)
        {
            Log.Write("合并 themes.json 失败：" + ex.Message);
            return File.ReadAllBytes(Path.Combine(pageDir, "themes.json"));
        }
    }

    public static string FirstReadyBuiltIn(string pageDir)
    {
        var builtIn = LoadBuiltIn(pageDir);
        for (int i = 0; i < builtIn.Count; i++)
        {
            var t = builtIn[i];
            if ((t.ContainsKey("status") ? t["status"] as string : null) == "ready" &&
                (t.ContainsKey("product") ? t["product"] as string : null) == "free")
            {
                string tid = t["id"] as string;
                if (!string.IsNullOrEmpty(tid)) return tid;
            }
        }
        return "beach";
    }

    public static string PageFor(string pageDir, string id)
    {
        var theme = Find(pageDir, id);
        return theme == null ? null : theme["page"] as string;
    }

    public static string ProductFor(string pageDir, string id)
    {
        var theme = Find(pageDir, id);
        object product;
        return theme != null && theme.TryGetValue("product", out product) ? product as string : null;
    }

    public static bool Allowed(string pageDir, string id, Func<string, bool> owns)
    {
        if (PageFor(pageDir, id) == null) return false;
        string product = ProductFor(pageDir, id);
        if (product == "free") return true;
        return product != null && owns != null && owns(product);
    }

    public static bool PageExists(string pageDir, string page)
    {
        if (string.IsNullOrEmpty(page)) return false;
        if (page.StartsWith("__custom/", StringComparison.Ordinal))
        {
            string fullPath, mime;
            return CustomThemes.ResolveFile(page, out fullPath, out mime);
        }
        return File.Exists(Path.Combine(pageDir, page.Replace('/', Path.DirectorySeparatorChar)));
    }

    public static string PageDisplayPath(string pageDir, string page)
    {
        if (string.IsNullOrEmpty(page)) return pageDir;
        if (page.StartsWith("__custom/", StringComparison.Ordinal))
        {
            string sub = page.Substring("__custom/".Length).Replace('/', Path.DirectorySeparatorChar);
            return Path.Combine(CustomThemes.Dir, sub);
        }
        return Path.Combine(pageDir, page.Replace('/', Path.DirectorySeparatorChar));
    }
}

static class CustomThemes
{
    public static readonly string[] ArchiveExts = new string[] { ".zip", ".deskland" };
    public static readonly string[] HtmlExts = new string[] { ".html", ".htm" };
    public static readonly string[] ImageExts = new string[] { ".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg", ".avif", ".bmp" };
    public static readonly string[] VideoExts = new string[] { ".mp4", ".webm", ".mkv", ".mov" };
    public static readonly string[] ShaderExts = new string[] { ".glsl", ".frag" };
    public static readonly HashSet<string> ArchiveAllowedExts = new HashSet<string>(new string[] {
        ".html", ".htm", ".css", ".js", ".mjs", ".json",
        ".glsl", ".vert", ".frag", ".wgsl", ".wasm",
        ".txt", ".md", ".xml", ".csv", ".map",
        ".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg", ".avif", ".bmp", ".ico",
        ".mp4", ".webm", ".mkv", ".mov", ".ogv",
        ".mp3", ".wav", ".ogg", ".opus", ".m4a", ".aac", ".flac",
        ".woff", ".woff2", ".ttf", ".otf", ".eot",
        ".obj", ".mtl", ".gltf", ".glb", ".bin", ".hdr", ".exr", ".ktx", ".ktx2", ".dds", ".ply", ".stl", ".fbx"
    }, StringComparer.OrdinalIgnoreCase);
    public static readonly HashSet<string> ArchiveAllowedBasenames = new HashSet<string>(new string[] {
        "license", "notice", "readme", "copying", "authors", "changelog"
    }, StringComparer.OrdinalIgnoreCase);
    static readonly PropertyInfo ZipExternalAttrsProp = typeof(ZipArchiveEntry).GetProperty("ExternalAttributes");

    public const long MaxArchiveBytes = 150L * 1024L * 1024L;
    public const int MaxArchiveFiles = 2000;

    public static bool IsAllowedArchiveMember(string fileName)
    {
        if (string.IsNullOrEmpty(fileName)) return false;
        string ext = Path.GetExtension(fileName);
        if (!string.IsNullOrEmpty(ext)) return ArchiveAllowedExts.Contains(ext);
        return ArchiveAllowedBasenames.Contains(fileName);
    }

    static bool IsZipSymlink(ZipArchiveEntry entry)
    {
        if (ZipExternalAttrsProp == null || entry == null) return false;
        try
        {
            object val = ZipExternalAttrsProp.GetValue(entry, null);
            if (val is int)
            {
                int attrs = (int)val;
                return (((attrs >> 16) & 0xF000) == 0xA000);
            }
        }
        catch { }
        return false;
    }

    public static string Dir
    {
        get
        {
            string overrideDir = Environment.GetEnvironmentVariable("DESKLAND_CUSTOM_THEMES_DIR");
            if (!string.IsNullOrEmpty(overrideDir)) return Path.GetFullPath(overrideDir);
            return Path.Combine(Log.DataDir, "themes");
        }
    }

    public static string RegistryPath { get { return Path.Combine(Dir, "registry.json"); } }

    public static bool HasExt(string ext, string[] allowed)
    {
        for (int i = 0; i < allowed.Length; i++)
            if (string.Equals(ext, allowed[i], StringComparison.OrdinalIgnoreCase)) return true;
        return false;
    }

    public static bool IsSupportedExt(string ext)
    {
        return HasExt(ext, ArchiveExts) || HasExt(ext, HtmlExts) || HasExt(ext, ImageExts) || HasExt(ext, VideoExts) || HasExt(ext, ShaderExts);
    }

    public static List<Dictionary<string, object>> Load()
    {
        var list = new List<Dictionary<string, object>>();
        try
        {
            if (!File.Exists(RegistryPath)) return list;
            var root = Json.Parse(File.ReadAllText(RegistryPath, Encoding.UTF8)) as Dictionary<string, object>;
            if (root == null || !root.ContainsKey("themes")) return list;
            var arr = root["themes"] as IEnumerable;
            if (arr == null) return list;
            foreach (object o in arr)
            {
                var d = o as Dictionary<string, object>;
                if (d != null) list.Add(d);
            }
        }
        catch { }
        return list;
    }

    public static void Save(List<Dictionary<string, object>> themes)
    {
        Directory.CreateDirectory(Dir);
        string tmp = RegistryPath + ".tmp";
        File.WriteAllText(tmp, Json.Stringify(Json.Obj("themes", themes)), new UTF8Encoding(false));
        if (File.Exists(RegistryPath)) File.Delete(RegistryPath);
        File.Move(tmp, RegistryPath);
    }

    static string Slugify(string name)
    {
        string s = Regex.Replace((name ?? "").ToLowerInvariant(), "[^a-z0-9]+", "-").Trim('-');
        if (string.IsNullOrEmpty(s)) s = "theme";
        if (s.Length > 36) s = s.Substring(0, 36);
        return "custom-" + s;
    }

    static string UniqueId(string baseId, HashSet<string> taken)
    {
        string cand = baseId;
        int n = 2;
        while (taken.Contains(cand) || Directory.Exists(Path.Combine(Dir, cand)))
        {
            cand = baseId + "-" + n.ToString(CultureInfo.InvariantCulture);
            n++;
        }
        return cand;
    }

    static string PrettyTitle(string stem)
    {
        string cleaned = Regex.Replace(stem ?? "", "[-_]+", " ").Trim();
        return string.IsNullOrEmpty(cleaned) ? "Imported Theme" : cleaned;
    }

    static string ExtractHtmlTitle(string content, string fallback)
    {
        var m = Regex.Match(content ?? "", "<title[^>]*>(.*?)</title>", RegexOptions.IgnoreCase | RegexOptions.Singleline);
        if (m.Success)
        {
            string raw = WebUtility.HtmlDecode(m.Groups[1].Value);
            string title = Regex.Replace(raw, "\\s+", " ").Trim();
            if (!string.IsNullOrEmpty(title)) return title.Length > 60 ? title.Substring(0, 60) : title;
        }
        return fallback;
    }

    static string ValidateHtmlFile(string path)
    {
        byte[] raw = File.ReadAllBytes(path);
        if (raw.Length == 0 || Array.IndexOf(raw, (byte)0) >= 0)
            throw new InvalidOperationException("HTML file is empty or corrupted: " + Path.GetFileName(path));
        return Encoding.UTF8.GetString(raw);
    }

    static bool IsUnderDir(string candidate, string rootDir)
    {
        string fullRoot = Path.GetFullPath(rootDir);
        if (!fullRoot.EndsWith(Path.DirectorySeparatorChar.ToString()))
            fullRoot += Path.DirectorySeparatorChar;
        string fullCand = Path.GetFullPath(candidate);
        return fullCand.StartsWith(fullRoot, StringComparison.OrdinalIgnoreCase);
    }

    static string NormalizeArchiveRel(string rel)
    {
        string s = (rel ?? "").Trim().Replace('\\', '/').TrimStart('/');
        string[] segs = s.Split('/');
        var clean = new List<string>();
        for (int i = 0; i < segs.Length; i++)
        {
            string p = segs[i];
            if (p == "" || p == ".") continue;
            if (p == ".." || p.IndexOf(':') >= 0) return null;
            clean.Add(p);
        }
        return clean.Count == 0 ? null : string.Join("/", clean.ToArray());
    }

    sealed class ZipEntryPlan
    {
        public ZipArchiveEntry Entry;
        public string[] Parts;
    }

    static List<ZipEntryPlan> SafeZipMembers(ZipArchive archive)
    {
        var entries = new List<ZipEntryPlan>();
        long totalSize = 0;
        var topDirs = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        bool hasRootFile = false;

        foreach (ZipArchiveEntry entry in archive.Entries)
        {
            string full = (entry.FullName ?? "").Replace('\\', '/');
            if (full.EndsWith("/") || string.IsNullOrEmpty(entry.Name)) continue;
            if (IsZipSymlink(entry))
                throw new InvalidOperationException("Archive contains unsupported symbolic link.");
            if (full.StartsWith("/") || full.IndexOf(':') >= 0)
                throw new InvalidOperationException("Unsafe archive entry path: " + entry.FullName);

            string[] rawParts = full.Split('/');
            var parts = new List<string>();
            for (int i = 0; i < rawParts.Length; i++)
            {
                string p = rawParts[i];
                if (p == "" || p == ".") continue;
                if (p == "..") throw new InvalidOperationException("Unsafe archive entry path: " + entry.FullName);
                parts.Add(p);
            }
            if (parts.Count == 0) continue;
            if (string.Equals(parts[0], "__MACOSX", StringComparison.OrdinalIgnoreCase)) continue;
            if (!IsAllowedArchiveMember(parts[parts.Count - 1]))
                throw new InvalidOperationException("Unsupported file type in theme archive: " + entry.FullName);

            totalSize += Math.Max(0L, entry.Length);
            if (totalSize > MaxArchiveBytes) throw new InvalidOperationException("Theme archive exceeds 150 MB size limit.");
            if (entries.Count >= MaxArchiveFiles) throw new InvalidOperationException("Theme archive contains too many files.");

            if (parts.Count > 1) topDirs.Add(parts[0]);
            else hasRootFile = true;

            var plan = new ZipEntryPlan();
            plan.Entry = entry;
            plan.Parts = parts.ToArray();
            entries.Add(plan);
        }

        if (entries.Count == 0) throw new InvalidOperationException("Archive contains no files.");

        if (!hasRootFile && topDirs.Count == 1)
        {
            for (int i = 0; i < entries.Count; i++)
            {
                string[] oldParts = entries[i].Parts;
                string[] stripped = new string[oldParts.Length - 1];
                Array.Copy(oldParts, 1, stripped, 0, stripped.Length);
                entries[i].Parts = stripped;
            }
        }
        return entries;
    }

    static long CopyLimited(Stream src, Stream dst, long remaining)
    {
        byte[] buf = new byte[65536];
        long written = 0;
        int n;
        while ((n = src.Read(buf, 0, buf.Length)) > 0)
        {
            written += n;
            if (written > remaining) throw new InvalidOperationException("Archive uncompressed size exceeds 150 MB limit.");
            dst.Write(buf, 0, n);
        }
        return written;
    }

    static string FindEntryHtml(string themeDir, string preferred)
    {
        if (preferred != null)
        {
            string clean = NormalizeArchiveRel(preferred);
            if (string.IsNullOrEmpty(clean)) throw new InvalidOperationException("Unsafe entry path in theme manifest.");
            string cand = Path.Combine(themeDir, clean.Replace('/', Path.DirectorySeparatorChar));
            if (!IsUnderDir(cand, themeDir) || !File.Exists(cand) || !HasExt(Path.GetExtension(cand), HtmlExts))
                throw new InvalidOperationException("Entry HTML file not found in archive: " + clean);
            return clean;
        }
        if (File.Exists(Path.Combine(themeDir, "index.html"))) return "index.html";

        string[] rootFiles = Directory.GetFiles(themeDir);
        Array.Sort(rootFiles, StringComparer.OrdinalIgnoreCase);
        for (int i = 0; i < rootFiles.Length; i++)
            if (HasExt(Path.GetExtension(rootFiles[i]), HtmlExts)) return Path.GetFileName(rootFiles[i]);

        string[] allFiles = Directory.GetFiles(themeDir, "*", SearchOption.AllDirectories);
        Array.Sort(allFiles, StringComparer.OrdinalIgnoreCase);
        string prefix = Path.GetFullPath(themeDir).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
        for (int i = 0; i < allFiles.Length; i++)
        {
            if (HasExt(Path.GetExtension(allFiles[i]), HtmlExts))
            {
                string full = Path.GetFullPath(allFiles[i]);
                if (full.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                    return full.Substring(prefix.Length).Replace('\\', '/');
            }
        }
        throw new InvalidOperationException("No .html entry page found in archive.");
    }

    static string SanitizeColor(object rawVal)
    {
        string s = rawVal != null ? Convert.ToString(rawVal, CultureInfo.InvariantCulture).Trim() : "";
        if (Regex.IsMatch(s, "^#[0-9a-fA-F]{6}$")) return s.ToLowerInvariant();
        if (Regex.IsMatch(s, "^#[0-9a-fA-F]{3}$"))
        {
            return ("#" + s[1] + s[1] + s[2] + s[2] + s[3] + s[3]).ToLowerInvariant();
        }
        return "#4a90e2";
    }

    static List<Dictionary<string, object>> SanitizeGroups(object rawGroups)
    {
        var cleanGroups = new List<Dictionary<string, object>>();
        var groupList = rawGroups as IEnumerable;
        if (groupList == null || rawGroups is string) return cleanGroups;

        foreach (object go in groupList)
        {
            var g = go as Dictionary<string, object>;
            if (g == null || !g.ContainsKey("params")) continue;
            var rawParams = g["params"] as IEnumerable;
            if (rawParams == null || g["params"] is string) continue;

            var cleanParams = new List<Dictionary<string, object>>();
            foreach (object po in rawParams)
            {
                var p = po as Dictionary<string, object>;
                if (p == null) continue;
                string key = (p.ContainsKey("key") ? Convert.ToString(p["key"], CultureInfo.InvariantCulture) : "").Trim();
                if (string.IsNullOrEmpty(key) || !Regex.IsMatch(key, "^[a-zA-Z0-9_-]+$")) continue;
                string label = (p.ContainsKey("label") && p["label"] != null ? Convert.ToString(p["label"], CultureInfo.InvariantCulture) : key).Trim();
                if (label.Length > 48) label = label.Substring(0, 48);

                var item = Json.Obj("key", key, "label", label);
                object hintObj;
                if (p.TryGetValue("hint", out hintObj) && hintObj is string)
                {
                    string h = (string)hintObj;
                    item["hint"] = h.Length > 180 ? h.Substring(0, 180) : h;
                }

                string ptype = p.ContainsKey("type") && p["type"] != null ? Convert.ToString(p["type"], CultureInfo.InvariantCulture).Trim().ToLowerInvariant() : "";
                if (ptype == "color")
                {
                    object cDef = null;
                    if (!p.TryGetValue("default", out cDef)) p.TryGetValue("value", out cDef);
                    item["type"] = "color";
                    item["default"] = SanitizeColor(cDef);
                    cleanParams.Add(item);
                    continue;
                }

                object optsObj;
                if (p.TryGetValue("options", out optsObj) && optsObj is IEnumerable && !(optsObj is string))
                {
                    var opts = new List<string>();
                    var vals = new List<object>();
                    bool hasObjOpts = false;
                    foreach (object oo in (IEnumerable)optsObj)
                    {
                        if (opts.Count >= 8) break;
                        var od = oo as Dictionary<string, object>;
                        if (od != null)
                        {
                            hasObjOpts = true;
                            object lv = od.ContainsKey("label") ? od["label"] : (od.ContainsKey("value") ? od["value"] : "");
                            object vv = od.ContainsKey("value") ? od["value"] : lv;
                            string lbl = Convert.ToString(lv, CultureInfo.InvariantCulture) ?? "";
                            if (lbl.Length > 48) lbl = lbl.Substring(0, 48);
                            if (string.IsNullOrEmpty(lbl)) lbl = Convert.ToString(vv, CultureInfo.InvariantCulture) ?? "";
                            opts.Add(lbl);
                            vals.Add(vv);
                        }
                        else
                        {
                            string s = Convert.ToString(oo, CultureInfo.InvariantCulture) ?? "";
                            if (s.Length > 48) s = s.Substring(0, 48);
                            opts.Add(s);
                        }
                    }
                    object rawValsObj;
                    if (!hasObjOpts && p.TryGetValue("values", out rawValsObj) && rawValsObj is IEnumerable && !(rawValsObj is string))
                    {
                        foreach (object vo in (IEnumerable)rawValsObj)
                        {
                            if (vals.Count >= opts.Count) break;
                            vals.Add(vo);
                        }
                        if (vals.Count < opts.Count) vals.Clear();
                    }
                    if (opts.Count > 0)
                    {
                        item["options"] = opts;
                        object defObj = null;
                        p.TryGetValue("default", out defObj);
                        if (vals.Count > 0)
                        {
                            item["values"] = vals;
                            object matched = vals[0];
                            bool found = false;
                            if (defObj != null)
                            {
                                for (int vi = 0; vi < vals.Count; vi++)
                                {
                                    if (object.Equals(vals[vi], defObj) ||
                                        (vals[vi] is bool && defObj is bool && (bool)vals[vi] == (bool)defObj) ||
                                        string.Equals(Convert.ToString(vals[vi], CultureInfo.InvariantCulture), Convert.ToString(defObj, CultureInfo.InvariantCulture), StringComparison.Ordinal))
                                    {
                                        matched = vals[vi];
                                        found = true;
                                        break;
                                    }
                                }
                                if (!found && defObj is string)
                                {
                                    int oi = opts.IndexOf((string)defObj);
                                    if (oi >= 0 && oi < vals.Count) { matched = vals[oi]; found = true; }
                                }
                                if (!found && !(defObj is bool))
                                {
                                    try
                                    {
                                        int idx = Convert.ToInt32(defObj, CultureInfo.InvariantCulture);
                                        if (idx >= 0 && idx < vals.Count) matched = vals[idx];
                                    }
                                    catch { }
                                }
                            }
                            item["default"] = matched;
                        }
                        else
                        {
                            int defIdx = 0;
                            if (defObj != null && !(defObj is bool))
                            {
                                try { defIdx = Convert.ToInt32(defObj, CultureInfo.InvariantCulture); } catch { defIdx = 0; }
                            }
                            if (defIdx < 0) defIdx = 0;
                            if (defIdx >= opts.Count) defIdx = opts.Count - 1;
                            item["default"] = defIdx;
                        }
                        cleanParams.Add(item);
                        continue;
                    }
                }

                try
                {
                    double pMin = p.ContainsKey("min") ? Convert.ToDouble(p["min"], CultureInfo.InvariantCulture) : 0.0;
                    double pMax = p.ContainsKey("max") ? Convert.ToDouble(p["max"], CultureInfo.InvariantCulture) : 1.0;
                    double pStep = p.ContainsKey("step") ? Convert.ToDouble(p["step"], CultureInfo.InvariantCulture) : 0.01;
                    double pDef = p.ContainsKey("default") ? Convert.ToDouble(p["default"], CultureInfo.InvariantCulture) : pMin;
                    if (pMax <= pMin || pStep <= 0) continue;
                    if (pDef < pMin) pDef = pMin;
                    if (pDef > pMax) pDef = pMax;
                    item["min"] = pMin;
                    item["max"] = pMax;
                    item["step"] = pStep;
                    item["default"] = pDef;
                    object fmtObj, unitObj;
                    if (p.TryGetValue("format", out fmtObj) && fmtObj is string)
                    {
                        string fmt = (string)fmtObj;
                        if (fmt == "times" || fmt == "percent") item["format"] = fmt;
                    }
                    else if (p.TryGetValue("unit", out unitObj) && unitObj is string)
                    {
                        string unit = (string)unitObj;
                        if (!string.IsNullOrEmpty(unit)) item["unit"] = unit.Length > 12 ? unit.Substring(0, 12) : unit;
                    }
                    cleanParams.Add(item);
                }
                catch { }
            }
            if (cleanParams.Count > 0)
            {
                string gName = (g.ContainsKey("name") && g["name"] != null ? Convert.ToString(g["name"], CultureInfo.InvariantCulture) : "Settings");
                if (gName.Length > 48) gName = gName.Substring(0, 48);
                cleanGroups.Add(Json.Obj("name", gName, "params", cleanParams));
            }
        }
        return cleanGroups;
    }

    static List<Dictionary<string, object>> GroupsFromSettings(object rawSettings)
    {
        var items = new List<KeyValuePair<string, Dictionary<string, object>>>();
        var dict = rawSettings as Dictionary<string, object>;
        if (dict != null)
        {
            foreach (KeyValuePair<string, object> kv in dict)
            {
                var spec = kv.Value as Dictionary<string, object>;
                if (spec == null) continue;
                string k = spec.ContainsKey("key") && spec["key"] != null ? Convert.ToString(spec["key"], CultureInfo.InvariantCulture) : kv.Key;
                items.Add(new KeyValuePair<string, Dictionary<string, object>>(k, spec));
            }
        }
        else if (rawSettings is IEnumerable && !(rawSettings is string))
        {
            foreach (object o in (IEnumerable)rawSettings)
            {
                var spec = o as Dictionary<string, object>;
                if (spec != null && spec.ContainsKey("key") && spec["key"] != null)
                    items.Add(new KeyValuePair<string, Dictionary<string, object>>(Convert.ToString(spec["key"], CultureInfo.InvariantCulture), spec));
            }
        }
        if (items.Count == 0) return new List<Dictionary<string, object>>();

        var groupOrder = new List<string>();
        var grouped = new Dictionary<string, List<object>>(StringComparer.Ordinal);
        for (int i = 0; i < items.Count; i++)
        {
            string key = items[i].Key;
            var spec = items[i].Value;
            string groupName = "Settings";
            if (spec.ContainsKey("group") && spec["group"] != null) groupName = Convert.ToString(spec["group"], CultureInfo.InvariantCulture).Trim();
            else if (spec.ContainsKey("category") && spec["category"] != null) groupName = Convert.ToString(spec["category"], CultureInfo.InvariantCulture).Trim();
            if (string.IsNullOrEmpty(groupName)) groupName = "Settings";

            string ptype = spec.ContainsKey("type") && spec["type"] != null ? Convert.ToString(spec["type"], CultureInfo.InvariantCulture).Trim().ToLowerInvariant() : "";
            if (ptype == "button" || ptype == "label" || ptype == "folderdropdown" || ptype == "file")
                continue;
            object labelObj = spec.ContainsKey("label") && spec["label"] != null
                ? spec["label"]
                : (spec.ContainsKey("title") && spec["title"] != null
                    ? spec["title"]
                    : (spec.ContainsKey("text") && spec["text"] != null ? spec["text"] : key));
            var param = Json.Obj("key", key, "label", labelObj);
            if (spec.ContainsKey("hint") && spec["hint"] is string) param["hint"] = spec["hint"];
            else if (spec.ContainsKey("description") && spec["description"] is string) param["hint"] = spec["description"];

            bool hasDef = spec.ContainsKey("default") || spec.ContainsKey("value");
            object rawDef = spec.ContainsKey("default") ? spec["default"] : (spec.ContainsKey("value") ? spec["value"] : null);

            if (ptype == "color")
            {
                param["type"] = "color";
                param["default"] = SanitizeColor(rawDef);
            }
            else if (ptype == "checkbox" || ptype == "boolean" || ptype == "bool" || ptype == "toggle")
            {
                param["options"] = new object[] { "Off", "On" };
                param["values"] = new object[] { false, true };
                bool b = false;
                if (rawDef is bool) b = (bool)rawDef;
                else if (rawDef is int || rawDef is long || rawDef is double) b = Convert.ToDouble(rawDef, CultureInfo.InvariantCulture) != 0;
                else if (rawDef is string)
                {
                    string rs = ((string)rawDef).Trim().ToLowerInvariant();
                    b = rs == "1" || rs == "true" || rs == "yes" || rs == "on";
                }
                param["default"] = b;
            }
            else if ((spec.ContainsKey("options") && spec["options"] is IEnumerable && !(spec["options"] is string)) ||
                     ((ptype == "dropdown" || ptype == "select") && spec.ContainsKey("items") && spec["items"] is IEnumerable && !(spec["items"] is string)))
            {
                param["options"] = spec.ContainsKey("options") && spec["options"] is IEnumerable && !(spec["options"] is string) ? spec["options"] : spec["items"];
                if (hasDef) param["default"] = rawDef;
            }
            else
            {
                param["min"] = spec.ContainsKey("min") ? spec["min"] : 0.0;
                param["max"] = spec.ContainsKey("max") ? spec["max"] : 1.0;
                param["step"] = spec.ContainsKey("step") ? spec["step"] : 0.01;
                param["default"] = rawDef != null ? rawDef : param["min"];
                if (spec.ContainsKey("format") && spec["format"] is string) param["format"] = spec["format"];
                else if (spec.ContainsKey("unit") && spec["unit"] is string) param["unit"] = spec["unit"];
            }

            List<object> list;
            if (!grouped.TryGetValue(groupName, out list))
            {
                list = new List<object>();
                grouped[groupName] = list;
                groupOrder.Add(groupName);
            }
            list.Add(param);
        }

        var rawGroups = new List<object>();
        for (int i = 0; i < groupOrder.Count; i++)
            rawGroups.Add(Json.Obj("name", groupOrder[i], "params", grouped[groupOrder[i]]));
        return SanitizeGroups(rawGroups);
    }

    static string FindArchiveThumb(string themeDir, string themeId, object manifestThumb)
    {
        if (manifestThumb is string)
        {
            string val = ((string)manifestThumb).Trim();
            if (val.StartsWith("radial-gradient", StringComparison.OrdinalIgnoreCase) ||
                val.StartsWith("linear-gradient", StringComparison.OrdinalIgnoreCase) ||
                val.StartsWith("#", StringComparison.OrdinalIgnoreCase))
                return val;
            string rel = NormalizeArchiveRel(val);
            if (!string.IsNullOrEmpty(rel))
            {
                string cand = Path.Combine(themeDir, rel.Replace('/', Path.DirectorySeparatorChar));
                if (IsUnderDir(cand, themeDir) && File.Exists(cand))
                    return "url(__custom/" + themeId + "/" + rel + ") center / cover no-repeat";
            }
        }
        string[] names = new string[] { "thumb.webp", "thumb.png", "thumb.jpg", "thumb.svg", "preview.webp", "preview.png", "preview.jpg", "preview.svg" };
        for (int i = 0; i < names.Length; i++)
            if (File.Exists(Path.Combine(themeDir, names[i])))
                return "url(__custom/" + themeId + "/" + names[i] + ") center / cover no-repeat";

        return "radial-gradient(circle at 28% 35%, rgba(120,210,255,0.45) 0%, transparent 45%), radial-gradient(circle at 75% 70%, rgba(180,120,255,0.38) 0%, transparent 48%), linear-gradient(145deg, #1b2738 0%, #243b55 55%, #16222f 100%)";
    }

    static string MediaWrapperHtml(string mediaFile, bool isVideo, string title)
    {
        string safeTitle = WebUtility.HtmlEncode(title);
        string safeMedia = WebUtility.HtmlEncode(mediaFile);
        string tag = isVideo
            ? "<video id=\"media\" src=\"" + safeMedia + "\" autoplay loop muted playsinline></video>"
            : "<img id=\"media\" src=\"" + safeMedia + "\" alt=\"" + safeTitle + "\">";
        string defaults = isVideo ? "speed: 1" : "motion: 0.35";
        string sb = "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>" + safeTitle + "</title>\n"
            + "<style>\n"
            + "  * { box-sizing: border-box; margin: 0; padding: 0; }\n"
            + "  html, body { width: 100%; height: 100%; overflow: hidden; background: #0d1117; }\n"
            + "  #stage { position: fixed; inset: 0; overflow: hidden; }\n"
            + "  #media { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; transform-origin: center center; will-change: transform, filter; }\n"
            + "  #halo { position: fixed; inset: 0; width: 100%; height: 100%; pointer-events: none; }\n"
            + "  #ghosts { position: fixed; inset: 0; pointer-events: none; display: none; }\n"
            + "  body.preview #ghosts { display: block; }\n"
            + "  .ghost { position: absolute; display: flex; flex-direction: column; align-items: center; padding-top: 6px; opacity: .28; }\n"
            + "  .ghost i { width: 40px; height: 40px; border-radius: 9px; background: #fff; box-shadow: 0 2px 6px rgba(0,0,0,.35); }\n"
            + "  .ghost b { width: 46px; height: 6px; margin-top: 8px; border-radius: 3px; background: #fff; }\n"
            + "</style>\n</head>\n<body>\n"
            + "<div id=\"stage\">" + tag + "</div>\n<canvas id=\"halo\"></canvas>\n<script>\n"
            + "'use strict';\n"
            + "const QS = new URLSearchParams(location.search), WALL = QS.has('wallpaper'), PREVIEW = QS.has('preview');\n"
            + "const isVideo = " + (isVideo ? "true" : "false") + ";\n"
            + "const P = { brightness: 1, dimIcons: 0.28, " + defaults + ", adaptive: 1 };\n"
            + "const DEFAULTS = Object.assign({}, P);\n"
            + "const media = document.getElementById('media'), haloCv = document.getElementById('halo'), haloCtx = haloCv.getContext('2d');\n"
            + "let icons = [], paused = false, t0 = performance.now();\n"
            + "function resize() { const dpr = Math.min(window.devicePixelRatio || 1, 2); haloCv.width = Math.max(1, Math.round(innerWidth * dpr)); haloCv.height = Math.max(1, Math.round(innerHeight * dpr)); haloCtx.setTransform(dpr, 0, 0, dpr, 0, 0); drawHalo(); }\n"
            + "function drawHalo() {\n"
            + "  haloCtx.clearRect(0, 0, innerWidth, innerHeight);\n"
            + "  if (!P.adaptive || P.dimIcons <= 0.01 || !icons.length) return;\n"
            + "  const a = Math.min(0.75, Math.max(0, P.dimIcons)) * 0.65;\n"
            + "  for (const ic of icons) { const cx = ic.x + ic.w * 0.5, cy = ic.y + ic.h * 0.5, rad = Math.max(ic.w, ic.h) * 0.9, g = haloCtx.createRadialGradient(cx, cy, rad * 0.15, cx, cy, rad); g.addColorStop(0, 'rgba(8, 12, 18, ' + a.toFixed(3) + ')'); g.addColorStop(1, 'rgba(8, 12, 18, 0)'); haloCtx.fillStyle = g; haloCtx.beginPath(); haloCtx.arc(cx, cy, rad, 0, Math.PI * 2); haloCtx.fill(); }\n"
            + "}\n"
            + "function syncVisuals(now) {\n"
            + "  const b = Math.max(0.35, Math.min(1.5, P.brightness || 1)); media.style.filter = 'brightness(' + b.toFixed(2) + ')';\n"
            + "  if (isVideo) { media.playbackRate = Math.max(0.25, Math.min(2.0, P.speed || 1)); media.style.transform = 'scale(1)'; }\n"
            + "  else { const amt = Math.max(0, Math.min(1, P.motion || 0)); if (amt <= 0.01) media.style.transform = 'scale(1)'; else { const s = (now - t0) * 0.00035, sc = 1 + amt * (0.04 + 0.03 * (0.5 + 0.5 * Math.sin(s))), dx = Math.sin(s * 0.7) * amt * 10, dy = Math.cos(s * 0.53) * amt * 7; media.style.transform = 'translate(' + dx.toFixed(1) + 'px,' + dy.toFixed(1) + 'px) scale(' + sc.toFixed(4) + ')'; } }\n"
            + "}\n"
            + "function tick(now) { if (!paused) syncVisuals(now); requestAnimationFrame(tick); }\n"
            + "function applySettings(v) { for (const k in v) if (k in P && typeof v[k] === 'number') P[k] = v[k]; syncVisuals(performance.now()); drawHalo(); }\n"
            + "function drawGhosts() { const box = document.getElementById('ghosts'); if (!box) return; box.innerHTML = icons.map(ic => '<div class=\"ghost\" style=\"left:' + ic.x + 'px;top:' + ic.y + 'px;width:' + ic.w + 'px;height:' + ic.h + 'px\"><i></i><b></b></div>').join(''); }\n"
            + "function handleMsg(m) {\n"
            + "  if (!m) return;\n"
            + "  if (m.t === 'pause') { paused = PREVIEW ? false : !!m.v; if (isVideo) { if (paused) media.pause(); else media.play().catch(() => {}); } return; }\n"
            + "  if (m.t === 'settings') { applySettings(m.v || {}); return; }\n"
            + "  if (m.t === 'reset') { applySettings(Object.assign({}, DEFAULTS, m.v || {})); return; }\n"
            + "  if (m.t === 'icons') { const k = window.devicePixelRatio || 1; icons = m.vis ? (m.r || []).map(([l, t, r, b]) => ({ x: l / k, y: t / k, w: (r - l) / k, h: (b - t) / k })) : []; if (PREVIEW) drawGhosts(); drawHalo(); }\n"
            + "}\n"
            + "if (PREVIEW) { document.body.classList.add('preview'); const g = document.createElement('div'); g.id = 'ghosts'; document.body.appendChild(g); addEventListener('message', ev => { if (ev.source === parent) handleMsg(typeof ev.data === 'string' ? JSON.parse(ev.data) : ev.data); }); parent.postMessage(JSON.stringify({ t: 'ready' }), '*'); }\n"
            + "else if (WALL && window.chrome && window.chrome.webview) { window.chrome.webview.addEventListener('message', ev => handleMsg(JSON.parse(ev.data))); window.chrome.webview.postMessage('ready'); }\n"
            + "window.desklandSnapshot = function () {\n"
            + "  const cw = Math.max(320, innerWidth), ch = Math.max(180, innerHeight), c = document.createElement('canvas'); c.width = cw; c.height = ch; const ctx = c.getContext('2d'); ctx.fillStyle = '#0d1117'; ctx.fillRect(0, 0, cw, ch);\n"
            + "  try { const mw = isVideo ? (media.videoWidth || cw) : (media.naturalWidth || cw), mh = isVideo ? (media.videoHeight || ch) : (media.naturalHeight || ch), sc = Math.max(cw / mw, ch / mh), dw = mw * sc, dh = mh * sc; ctx.drawImage(media, (cw - dw) / 2, (ch - dh) / 2, dw, dh); } catch (_) {}\n"
            + "  ctx.drawImage(haloCv, 0, 0, cw, ch); const tw = 760, th = Math.round(tw * ch / cw), tc = document.createElement('canvas'); tc.width = tw; tc.height = th; tc.getContext('2d').drawImage(c, 0, 0, tw, th);\n"
            + "  return { canvas: c, thumb: tc.toDataURL('image/jpeg', 0.88) };\n"
            + "};\n"
            + "addEventListener('resize', resize); resize(); requestAnimationFrame(tick);\n"
            + "</script>\n</body>\n</html>\n";
        return sb;
    }

    static List<Dictionary<string, object>> MediaParams(bool isVideo)
    {
        var second = isVideo
            ? Json.Obj("key", "speed", "label", "Playback speed", "hint", "Speed multiplier for animated video playback", "min", 0.25, "max", 2.0, "step", 0.05, "default", 1.0, "format", "times")
            : Json.Obj("key", "motion", "label", "Gentle drift", "hint", "Slow ambient zoom and drift across the wallpaper", "min", 0.0, "max", 1.0, "step", 0.01, "default", 0.35, "format", "percent");

        var list = new List<Dictionary<string, object>>();
        list.Add(Json.Obj("name", "Appearance", "params", new object[] {
            Json.Obj("key", "brightness", "label", "Brightness", "hint", "Overall brightness of the imported wallpaper", "min", 0.4, "max", 1.4, "step", 0.01, "default", 1.0, "format", "percent"),
            second,
            Json.Obj("key", "dimIcons", "label", "Icon halo", "hint", "Soft dark backdrop behind desktop icons for legibility", "min", 0.0, "max", 1.0, "step", 0.01, "default", 0.28, "format", "percent")
        }));
        return list;
    }

    static string ShaderWrapperHtml(string title, string glslSource)
    {
        string safeTitle = WebUtility.HtmlEncode(title);
        string encodedGlsl = Json.Stringify(glslSource);
        return "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>" + safeTitle + "</title>\n"
            + "<style>html,body,canvas{margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#090c14;display:block;}</style>\n"
            + "</head>\n<body>\n<canvas id=\"gl\"></canvas>\n<script>\n(() => {\n"
            + "  const canvas = document.getElementById('gl');\n"
            + "  const gl = canvas.getContext('webgl', { preserveDrawingBuffer: true, antialias: false });\n"
            + "  if (!gl) return;\n"
            + "  const rawFrag = " + encodedGlsl + ";\n"
            + "  const hasMainImage = /void\\s+mainImage\\s*\\(/.test(rawFrag);\n"
            + "  const header = 'precision highp float;\\nuniform float u_time;\\nuniform vec2 u_resolution;\\nuniform vec2 u_mouse;\\nuniform float u_speed;\\nuniform float u_brightness;\\nuniform float u_audio;\\nuniform int u_iconCount;\\nuniform vec4 u_icons[32];\\nuniform vec3 iResolution;\\nuniform float iTime;\\nuniform vec4 iMouse;\\n';\n"
            + "  const stripped = rawFrag.replace(/^\\s*precision\\s+\\w+\\s+float\\s*;/gm, '');\n"
            + "  const fragSrc = header + stripped + (hasMainImage ? '\\nvoid main(){ vec4 c=vec4(0.0); mainImage(c, gl_FragCoord.xy); gl_FragColor=vec4(c.rgb*u_brightness, 1.0); }\\n' : '');\n"
            + "  const vertSrc = 'attribute vec2 a_pos; void main(){ gl_Position = vec4(a_pos, 0.0, 1.0); }';\n"
            + "  function compile(type, src){ const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) || 'shader compile error'); return s; }\n"
            + "  const prog = gl.createProgram();\n"
            + "  try { gl.attachShader(prog, compile(gl.VERTEX_SHADER, vertSrc)); gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, fragSrc)); gl.linkProgram(prog); }\n"
            + "  catch (err) { document.body.innerHTML = '<pre style=\"color:#f78;padding:20px;font:13px monospace\">' + String(err) + '</pre>'; return; }\n"
            + "  gl.useProgram(prog);\n"
            + "  const buf = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buf);\n"
            + "  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);\n"
            + "  const locPos = gl.getAttribLocation(prog, 'a_pos'); gl.enableVertexAttribArray(locPos); gl.vertexAttribPointer(locPos, 2, gl.FLOAT, false, 0, 0);\n"
            + "  const U = n => gl.getUniformLocation(prog, n);\n"
            + "  const uTime=U('u_time'), uRes=U('u_resolution'), uMouse=U('u_mouse'), uSpeed=U('u_speed'), uBright=U('u_brightness'), uAudio=U('u_audio'), uIconCount=U('u_iconCount'), uIcons=U('u_icons'), iRes=U('iResolution'), iTime=U('iTime'), iMouse=U('iMouse');\n"
            + "  const cfg = { speed: 1.0, brightness: 1.0 };\n"
            + "  let paused = false, mouseX = 0, mouseY = 0, audioRms = 0, icons = new Float32Array(128), iconCount = 0, simTime = 0, lastTs = performance.now();\n"
            + "  function resize(){ const dpr = Math.min(window.devicePixelRatio || 1, 1.5); canvas.width = Math.max(1, Math.floor(innerWidth * dpr)); canvas.height = Math.max(1, Math.floor(innerHeight * dpr)); gl.viewport(0, 0, canvas.width, canvas.height); }\n"
            + "  addEventListener('resize', resize); resize();\n"
            + "  function applyMsg(raw){ let m = raw; if (typeof m === 'string') { try { m = JSON.parse(m); } catch (_) { return; } } if (!m || typeof m !== 'object') return;\n"
            + "    if (m.t === 'settings' && m.v) { if (typeof m.v.speed === 'number') cfg.speed = m.v.speed; if (typeof m.v.brightness === 'number') cfg.brightness = m.v.brightness; }\n"
            + "    else if (m.t === 'reset') { cfg.speed = 1.0; cfg.brightness = 1.0; }\n"
            + "    else if (m.t === 'pause') { paused = !!m.v; }\n"
            + "    else if (m.t === 'pointer' && typeof m.x === 'number') { mouseX = m.x * (window.devicePixelRatio || 1); mouseY = canvas.height - m.y * (window.devicePixelRatio || 1); }\n"
            + "    else if (m.t === 'audioLevels' && typeof m.rms === 'number') { audioRms = m.rms; }\n"
            + "    else if (m.t === 'icons' && Array.isArray(m.r)) { iconCount = Math.min(32, m.r.length); for (let i = 0; i < iconCount; i++) { const r = m.r[i] || [0,0,0,0]; icons[i*4]=r[0]||0; icons[i*4+1]=r[1]||0; icons[i*4+2]=r[2]||0; icons[i*4+3]=r[3]||0; } }\n"
            + "  }\n"
            + "  addEventListener('message', e => applyMsg(e.data));\n"
            + "  if (window.chrome && window.chrome.webview) window.chrome.webview.addEventListener('message', e => applyMsg(e.data));\n"
            + "  function render(ts){ requestAnimationFrame(render); const dt = Math.min(0.1, Math.max(0, (ts - lastTs) / 1000)); lastTs = ts; if (paused) return; simTime += dt * cfg.speed;\n"
            + "    if (uTime) gl.uniform1f(uTime, simTime); if (iTime) gl.uniform1f(iTime, simTime); if (uRes) gl.uniform2f(uRes, canvas.width, canvas.height); if (iRes) gl.uniform3f(iRes, canvas.width, canvas.height, 1.0);\n"
            + "    if (uMouse) gl.uniform2f(uMouse, mouseX, mouseY); if (iMouse) gl.uniform4f(iMouse, mouseX, mouseY, 0, 0); if (uSpeed) gl.uniform1f(uSpeed, cfg.speed); if (uBright) gl.uniform1f(uBright, cfg.brightness); if (uAudio) gl.uniform1f(uAudio, audioRms);\n"
            + "    if (uIconCount) gl.uniform1i(uIconCount, iconCount); if (uIcons) gl.uniform4fv(uIcons, icons); gl.drawArrays(gl.TRIANGLES, 0, 6); }\n"
            + "  window.desklandSnapshot = () => canvas.toDataURL('image/png');\n"
            + "  requestAnimationFrame(render);\n"
            + "  if (new URLSearchParams(location.search).get('preview') === '1') parent.postMessage(JSON.stringify({ t: 'ready' }), '*');\n"
            + "  else if (window.chrome && window.chrome.webview) window.chrome.webview.postMessage('ready');\n"
            + "})();\n</script>\n</body>\n</html>\n";
    }

    public static Dictionary<string, object> ImportFile(string sourcePath, string pageDir)
    {
        if (string.IsNullOrEmpty(sourcePath) || !File.Exists(sourcePath))
            throw new FileNotFoundException("Selected file does not exist.");
        var fi = new FileInfo(sourcePath);
        if (fi.Length == 0) throw new InvalidOperationException("Selected file is empty.");

        string ext = (Path.GetExtension(sourcePath) ?? "").ToLowerInvariant();
        if (!IsSupportedExt(ext))
            throw new InvalidOperationException("Unsupported file type: " + (string.IsNullOrEmpty(ext) ? "none" : ext));
        if (!HasExt(ext, ArchiveExts) && fi.Length > MaxArchiveBytes)
            throw new InvalidOperationException("Selected file exceeds 150 MB size limit.");

        Directory.CreateDirectory(Dir);
        var current = Load();
        var taken = new HashSet<string>(StringComparer.Ordinal);
        var builtIn = Themes.LoadBuiltIn(pageDir);
        for (int i = 0; i < builtIn.Count; i++)
        {
            string tid = builtIn[i].ContainsKey("id") ? builtIn[i]["id"] as string : null;
            if (!string.IsNullOrEmpty(tid)) taken.Add(tid);
        }
        for (int i = 0; i < current.Count; i++)
        {
            string tid = current[i].ContainsKey("id") ? current[i]["id"] as string : null;
            if (!string.IsNullOrEmpty(tid)) taken.Add(tid);
        }

        string stem = Path.GetFileNameWithoutExtension(sourcePath);
        string themeId = UniqueId(Slugify(stem), taken);
        string themeDir = Path.Combine(Dir, themeId);
        Directory.CreateDirectory(themeDir);

        try
        {
            Dictionary<string, object> themeObj;
            if (HasExt(ext, ArchiveExts))
            {
                ZipArchive zf;
                try { zf = ZipFile.OpenRead(sourcePath); }
                catch (InvalidDataException) { throw new InvalidOperationException("Archive is corrupted or not a valid ZIP file."); }
                using (zf)
                {
                    var entries = SafeZipMembers(zf);
                    long remaining = MaxArchiveBytes;
                    for (int i = 0; i < entries.Count; i++)
                    {
                        string rel = string.Join(Path.DirectorySeparatorChar.ToString(), entries[i].Parts);
                        string dest = Path.GetFullPath(Path.Combine(themeDir, rel));
                        if (!IsUnderDir(dest, themeDir))
                            throw new InvalidOperationException("Unsafe archive entry path.");
                        Directory.CreateDirectory(Path.GetDirectoryName(dest));
                        using (Stream zin = entries[i].Entry.Open())
                        using (FileStream zout = File.Create(dest))
                            remaining -= CopyLimited(zin, zout, remaining);
                    }
                }

                var manifest = new Dictionary<string, object>();
                string[] manifestNames = new string[] { "manifest.json", "theme.json" };
                for (int i = 0; i < manifestNames.Length; i++)
                {
                    string mp = Path.Combine(themeDir, manifestNames[i]);
                    if (!File.Exists(mp)) continue;
                    Dictionary<string, object> parsed = null;
                    try { parsed = Json.Parse(File.ReadAllText(mp, Encoding.UTF8)) as Dictionary<string, object>; }
                    catch { parsed = null; }
                    if (parsed == null) throw new InvalidOperationException("Invalid " + manifestNames[i] + " in theme archive.");
                    manifest = parsed;
                    break;
                }
                Dictionary<string, object> livelyInfo = null;
                string livelyInfoPath = Path.Combine(themeDir, "LivelyInfo.json");
                if (File.Exists(livelyInfoPath))
                {
                    try { livelyInfo = Json.Parse(File.ReadAllText(livelyInfoPath, Encoding.UTF8)) as Dictionary<string, object>; }
                    catch { livelyInfo = null; }
                }
                Dictionary<string, object> livelyProps = null;
                string livelyPropsPath = Path.Combine(themeDir, "LivelyProperties.json");
                if (File.Exists(livelyPropsPath))
                {
                    try { livelyProps = Json.Parse(File.ReadAllText(livelyPropsPath, Encoding.UTF8)) as Dictionary<string, object>; }
                    catch { livelyProps = null; }
                }

                object entryPref = null;
                string[] entryKeys = new string[] { "entry", "page", "main", "file" };
                for (int i = 0; i < entryKeys.Length; i++)
                {
                    if (manifest.TryGetValue(entryKeys[i], out entryPref) && entryPref != null) break;
                }
                if (entryPref == null && livelyInfo != null) livelyInfo.TryGetValue("FileName", out entryPref);
                string entry = FindEntryHtml(themeDir, entryPref as string);
                string entryHtml = ValidateHtmlFile(Path.Combine(themeDir, entry.Replace('/', Path.DirectorySeparatorChar)));
                string htmlTitle = ExtractHtmlTitle(entryHtml, PrettyTitle(stem));

                object nameObj = null, descObj = null, adapObj = null, thumbObj = null, groupsObj = null, settingsObj = null;
                if (!manifest.TryGetValue("name", out nameObj) || nameObj == null) manifest.TryGetValue("title", out nameObj);
                if (nameObj == null && livelyInfo != null) livelyInfo.TryGetValue("Title", out nameObj);
                string tName = nameObj != null ? Convert.ToString(nameObj, CultureInfo.InvariantCulture) : htmlTitle;
                if (tName.Length > 60) tName = tName.Substring(0, 60);
                if (!manifest.TryGetValue("desc", out descObj) || descObj == null) manifest.TryGetValue("description", out descObj);
                if (descObj == null && livelyInfo != null) livelyInfo.TryGetValue("Desc", out descObj);
                string tDesc = descObj != null ? Convert.ToString(descObj, CultureInfo.InvariantCulture) : "Imported custom theme package.";
                bool adaptive = manifest.TryGetValue("adaptive", out adapObj) && adapObj is bool && (bool)adapObj;
                if (!manifest.TryGetValue("thumb", out thumbObj) || thumbObj == null)
                {
                    if (!manifest.TryGetValue("thumbnail", out thumbObj) || thumbObj == null)
                        manifest.TryGetValue("preview", out thumbObj);
                }
                if (thumbObj == null && livelyInfo != null) livelyInfo.TryGetValue("Thumbnail", out thumbObj);

                themeObj = Json.Obj(
                    "id", themeId,
                    "name", tName,
                    "desc", tDesc,
                    "page", "__custom/" + themeId + "/" + entry,
                    "entry", entry,
                    "status", "ready",
                    "product", "free",
                    "imported", true,
                    "adaptive", adaptive,
                    "thumb", FindArchiveThumb(themeDir, themeId, thumbObj));
                List<Dictionary<string, object>> groups = null;
                if (manifest.TryGetValue("groups", out groupsObj)) groups = SanitizeGroups(groupsObj);
                if ((groups == null || groups.Count == 0) && manifest.TryGetValue("settings", out settingsObj))
                    groups = GroupsFromSettings(settingsObj);
                if ((groups == null || groups.Count == 0) && livelyProps != null)
                    groups = GroupsFromSettings(livelyProps);
                if (groups != null && groups.Count > 0) themeObj["groups"] = groups;
            }
            else if (HasExt(ext, ShaderExts))
            {
                byte[] rawBytes = File.ReadAllBytes(sourcePath);
                if (rawBytes.Length == 0 || rawBytes.Length > 2 * 1024 * 1024 || Array.IndexOf(rawBytes, (byte)0) >= 0)
                    throw new InvalidOperationException("Shader file is empty, too large, or corrupted.");
                string glsl = Encoding.UTF8.GetString(rawBytes);
                string title = PrettyTitle(stem);
                if (title.Length > 60) title = title.Substring(0, 60);
                File.WriteAllText(Path.Combine(themeDir, "shader.glsl"), glsl, new UTF8Encoding(false));
                File.WriteAllText(Path.Combine(themeDir, "index.html"), ShaderWrapperHtml(title, glsl), new UTF8Encoding(false));
                var shaderGroups = new List<Dictionary<string, object>>();
                shaderGroups.Add(Json.Obj("name", "Shader", "params", new object[] {
                    Json.Obj("key", "speed", "label", "Speed", "min", 0.1, "max", 3.0, "step", 0.05, "default", 1.0, "format", "times"),
                    Json.Obj("key", "brightness", "label", "Brightness", "min", 0.4, "max", 1.4, "step", 0.01, "default", 1.0, "format", "percent")
                }));
                themeObj = Json.Obj(
                    "id", themeId,
                    "name", title,
                    "desc", "Imported GLSL WebGL fragment shader.",
                    "page", "__custom/" + themeId + "/index.html",
                    "entry", "index.html",
                    "status", "ready",
                    "product", "free",
                    "imported", true,
                    "adaptive", true,
                    "thumb", "radial-gradient(circle at 28% 35%, rgba(168,95,255,0.52) 0%, transparent 50%), radial-gradient(circle at 75% 68%, rgba(72,210,255,0.42) 0%, transparent 48%), linear-gradient(150deg, #131126 0%, #1d2642 55%, #111829 100%)",
                    "groups", shaderGroups);
            }
            else if (HasExt(ext, HtmlExts))
            {
                string rawHtml = ValidateHtmlFile(sourcePath);
                File.WriteAllText(Path.Combine(themeDir, "index.html"), rawHtml, new UTF8Encoding(false));
                string title = ExtractHtmlTitle(rawHtml, PrettyTitle(stem));
                themeObj = Json.Obj(
                    "id", themeId,
                    "name", title,
                    "desc", "Imported interactive HTML wallpaper.",
                    "page", "__custom/" + themeId + "/index.html",
                    "entry", "index.html",
                    "status", "ready",
                    "product", "free",
                    "imported", true,
                    "adaptive", false,
                    "thumb", "radial-gradient(circle at 30% 35%, rgba(98,195,255,0.45) 0%, transparent 48%), radial-gradient(circle at 75% 70%, rgba(120,235,190,0.35) 0%, transparent 45%), linear-gradient(150deg, #162231 0%, #243e54 55%, #172738 100%)");
            }
            else
            {
                bool isVideo = HasExt(ext, VideoExts);
                string mediaName = "media" + ext;
                using (FileStream min = File.OpenRead(sourcePath))
                using (FileStream mout = File.Create(Path.Combine(themeDir, mediaName)))
                    CopyLimited(min, mout, MaxArchiveBytes);
                string title = PrettyTitle(stem);
                File.WriteAllText(Path.Combine(themeDir, "index.html"), MediaWrapperHtml(mediaName, isVideo, title), new UTF8Encoding(false));
                string thumb = isVideo
                    ? "radial-gradient(circle at 32% 38%, rgba(255,170,95,0.45) 0%, transparent 48%), linear-gradient(150deg, #1e2130 0%, #342f48 55%, #1b1d29 100%)"
                    : "url(__custom/" + themeId + "/" + mediaName + ") center / cover no-repeat";
                themeObj = Json.Obj(
                    "id", themeId,
                    "name", title,
                    "desc", isVideo ? "Imported animated video wallpaper." : "Imported custom image wallpaper.",
                    "page", "__custom/" + themeId + "/index.html",
                    "entry", "index.html",
                    "status", "ready",
                    "product", "free",
                    "imported", true,
                    "adaptive", true,
                    "thumb", thumb,
                    "groups", MediaParams(isVideo));
            }

            current.Add(themeObj);
            Save(current);
            return themeObj;
        }
        catch
        {
            try { Directory.Delete(themeDir, true); } catch { }
            throw;
        }
    }

    public static bool Remove(string themeId)
    {
        if (string.IsNullOrEmpty(themeId)) return false;
        var current = Load();
        var kept = new List<Dictionary<string, object>>();
        for (int i = 0; i < current.Count; i++)
        {
            string tid = current[i].ContainsKey("id") ? current[i]["id"] as string : null;
            if (tid != themeId) kept.Add(current[i]);
        }
        if (kept.Count == current.Count) return false;
        Save(kept);
        string safeName = Path.GetFileName(themeId);
        if (!string.IsNullOrEmpty(safeName) && safeName == themeId)
        {
            string themeDir = Path.Combine(Dir, safeName);
            if (IsUnderDir(themeDir, Dir) && Directory.Exists(themeDir))
                try { Directory.Delete(themeDir, true); } catch { }
        }
        return true;
    }

    public static bool ResolveFile(string relPath, out string fullPath, out string mime)
    {
        fullPath = null;
        mime = "application/octet-stream";
        string norm = NormalizeArchiveRel(relPath);
        if (string.IsNullOrEmpty(norm) || !norm.StartsWith("__custom/", StringComparison.Ordinal)) return false;
        string sub = norm.Substring("__custom/".Length);
        if (string.IsNullOrEmpty(sub)) return false;

        string decoded = Uri.UnescapeDataString(sub);
        string safeSub = NormalizeArchiveRel(decoded);
        if (string.IsNullOrEmpty(safeSub)) return false;

        string cand = Path.Combine(Dir, safeSub.Replace('/', Path.DirectorySeparatorChar));
        if (!IsUnderDir(cand, Dir) || !File.Exists(cand)) return false;
        fullPath = cand;
        mime = MimeForExtension(Path.GetExtension(cand));
        return true;
    }

    public static string MimeForExtension(string ext)
    {
        switch ((ext ?? "").ToLowerInvariant())
        {
            case ".html":
            case ".htm": return "text/html; charset=utf-8";
            case ".js":
            case ".mjs": return "application/javascript; charset=utf-8";
            case ".css": return "text/css; charset=utf-8";
            case ".json": return "application/json; charset=utf-8";
            case ".svg": return "image/svg+xml";
            case ".png": return "image/png";
            case ".jpg":
            case ".jpeg": return "image/jpeg";
            case ".webp": return "image/webp";
            case ".gif": return "image/gif";
            case ".avif": return "image/avif";
            case ".bmp": return "image/bmp";
            case ".mp4": return "video/mp4";
            case ".webm": return "video/webm";
            case ".mkv": return "video/x-matroska";
            case ".mov": return "video/quicktime";
            case ".mp3": return "audio/mpeg";
            case ".wav": return "audio/wav";
            case ".ogg": return "audio/ogg";
            case ".woff": return "font/woff";
            case ".woff2": return "font/woff2";
            default: return "application/octet-stream";
        }
    }
}
