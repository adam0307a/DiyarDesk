// DiyarDesk · Windows Shell IFolderView COM Desktop Icon Reader
//
// Reads desktop icon positions and display names via documented Windows Shell
// COM interfaces (IShellWindows -> IServiceProvider -> IShellBrowser ->
// IFolderView + IShellFolder + StrRetToBSTR) without OpenProcess,
// VirtualAllocEx, or ReadProcessMemory on explorer.exe.
//
// C# 5 compatible (.NET Framework 4.x csc.exe).

using System;
using System.Collections;
using System.Collections.Generic;
using System.Drawing;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

class IconSnapshotEntry
{
    public string Key;
    public string Name;
    public int X;
    public int Y;
}

class ShellIconReader : IDisposable
{
    const int SwcDesktop = 8;
    const int SwfoNeedDispatch = 1;
    const uint SvgioAllView = 0x2;
    const uint ShgdnNormal = 0x0;
    const uint ShgdnForParsing = 0x8000;
    const uint SvsiPositionItem = 0x00000002;
    const int LvmGetItemSpacing = 0x1033;
    const int StrRetBytes = 272; // sizeof(STRRET) on x64 is 272 bytes (uint + pad + 260-byte union)

    static readonly Guid ClsidShellWindows = new Guid("9BA05972-F6A8-11CF-A442-00A0C90A8F39");
    static readonly Guid SidSTopLevelBrowser = new Guid("4C96BE40-915C-11CF-99D3-00AA004AE837");
    static readonly Guid IidIShellBrowser = new Guid("000214E2-0000-0000-C000-000000000046");
    static readonly Guid IidIShellFolder = new Guid("000214E6-0000-0000-C000-000000000046");

    [ComImport]
    [Guid("85CB6900-4D95-11CF-960C-0080C7F4EE85")]
    [InterfaceType(ComInterfaceType.InterfaceIsDual)]
    interface IShellWindows
    {
        [DispId(0x60020000)] int Count { get; }
        [DispId(0x00000000)] [return: MarshalAs(UnmanagedType.IDispatch)] object Item([In] object index);
        [DispId(-4)] [return: MarshalAs(UnmanagedType.IUnknown)] object _NewEnum();
        [DispId(0x60020003)] void Register([In, MarshalAs(UnmanagedType.IDispatch)] object pid, [In] int hwnd, [In] int swClass, out int plCookie);
        [DispId(0x60020004)] void RegisterPending([In] int lThreadId, [In] ref object pvarloc, [In] ref object pvarlocRoot, [In] int swClass, out int plCookie);
        [DispId(0x60020005)] void Revoke([In] int lCookie);
        [DispId(0x60020006)] void OnNavigate([In] int lCookie, [In] ref object pvarLoc);
        [DispId(0x60020007)] void OnActivated([In] int lCookie, [In] bool fActive);
        [DispId(0x60020008)] [return: MarshalAs(UnmanagedType.IDispatch)] object FindWindowSW([In] ref object pvarLoc, [In] ref object pvarLocRoot, [In] int swClass, out int pHWND, [In] int swfwOptions);
    }

    [ComImport]
    [Guid("6D5140C1-7436-11CE-8034-00AA006009FA")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IShellServiceProvider
    {
        [PreserveSig]
        int QueryService(ref Guid guidService, ref Guid riid, [MarshalAs(UnmanagedType.Interface)] out object ppvObject);
    }

    [ComImport]
    [Guid("000214E2-0000-0000-C000-000000000046")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IShellBrowser
    {
        [PreserveSig] int GetWindow(out IntPtr phwnd);
        [PreserveSig] int ContextSensitiveHelp(bool fEnterMode);
        [PreserveSig] int InsertMenusSB(IntPtr hmenuShared, IntPtr lpMenuWidths);
        [PreserveSig] int SetMenuSB(IntPtr hmenuShared, IntPtr holemenuRes, IntPtr hwndActiveObject);
        [PreserveSig] int RemoveMenusSB(IntPtr hmenuShared);
        [PreserveSig] int SetStatusTextSB([MarshalAs(UnmanagedType.LPWStr)] string pszStatusText);
        [PreserveSig] int EnableModelessSB(bool fEnable);
        [PreserveSig] int TranslateAcceleratorSB(IntPtr pmsg, ushort wID);
        [PreserveSig] int BrowseObject(IntPtr pidl, uint wFlags);
        [PreserveSig] int GetViewStateStream(uint grfMode, out IntPtr ppStrm);
        [PreserveSig] int GetControlWindow(uint id, out IntPtr lphwnd);
        [PreserveSig] int SendControlMsg(uint id, uint uMsg, IntPtr wParam, IntPtr lParam, out IntPtr pret);
        [PreserveSig] int QueryActiveShellView([MarshalAs(UnmanagedType.Interface)] out object ppshv);
    }

    [ComImport]
    [Guid("CDE725B0-CCC9-4519-917E-325D72FAB4CE")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IFolderView
    {
        [PreserveSig] int GetCurrentViewMode(out uint pViewMode);
        [PreserveSig] int SetCurrentViewMode(uint viewMode);
        [PreserveSig] int GetFolder(ref Guid riid, [MarshalAs(UnmanagedType.Interface)] out object ppv);
        [PreserveSig] int Item(int iItemIndex, out IntPtr ppidl);
        [PreserveSig] int ItemCount(uint uFlags, out int pcItems);
        [PreserveSig] int Items(uint uFlags, ref Guid riid, out IntPtr ppv);
        [PreserveSig] int GetSelectionMarkedItem(out int piItem);
        [PreserveSig] int GetFocusedItem(out int piItem);
        [PreserveSig] int GetItemPosition(IntPtr pidl, out Native.POINT ppt);
        [PreserveSig] int GetSpacing(ref Native.POINT ppt);
        [PreserveSig] int GetDefaultSpacing(ref Native.POINT ppt);
        [PreserveSig] int GetAutoArrange();
        [PreserveSig] int SelectItem(int iItem, uint dwFlags);
        [PreserveSig] int SelectAndPositionItems(uint cidl, IntPtr apidl, IntPtr apt, uint dwFlags);
    }

    [ComImport]
    [Guid("1AF3A467-214F-4298-908E-06B03E0B39F9")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IFolderView2
    {
        // IFolderView (14 methods)
        [PreserveSig] int GetCurrentViewMode(out uint pViewMode);
        [PreserveSig] int SetCurrentViewMode(uint viewMode);
        [PreserveSig] int GetFolder(ref Guid riid, [MarshalAs(UnmanagedType.Interface)] out object ppv);
        [PreserveSig] int Item(int iItemIndex, out IntPtr ppidl);
        [PreserveSig] int ItemCount(uint uFlags, out int pcItems);
        [PreserveSig] int Items(uint uFlags, ref Guid riid, out IntPtr ppv);
        [PreserveSig] int GetSelectionMarkedItem(out int piItem);
        [PreserveSig] int GetFocusedItem(out int piItem);
        [PreserveSig] int GetItemPosition(IntPtr pidl, out Native.POINT ppt);
        [PreserveSig] int GetSpacing(ref Native.POINT ppt);
        [PreserveSig] int GetDefaultSpacing(ref Native.POINT ppt);
        [PreserveSig] int GetAutoArrange();
        [PreserveSig] int SelectItem(int iItem, uint dwFlags);
        [PreserveSig] int SelectAndPositionItems(uint cidl, IntPtr apidl, IntPtr apt, uint dwFlags);
        // IFolderView2
        [PreserveSig] int SetGroupBy(IntPtr key, bool fAscending);
        [PreserveSig] int GetGroupBy(IntPtr pkey, out bool pfAscending);
        [PreserveSig] int SetViewProperty(IntPtr pidl, IntPtr propkey, IntPtr propvar);
        [PreserveSig] int GetViewProperty(IntPtr pidl, IntPtr propkey, IntPtr ppropvar);
        [PreserveSig] int SetTileViewProperties(IntPtr pidl, [MarshalAs(UnmanagedType.LPWStr)] string pszPropList);
        [PreserveSig] int SetExtendedTileViewProperties(IntPtr pidl, [MarshalAs(UnmanagedType.LPWStr)] string pszPropList);
        [PreserveSig] int SetText(int iType, [MarshalAs(UnmanagedType.LPWStr)] string pwszText);
        [PreserveSig] int SetCurrentFolderFlags(uint dwMask, uint dwFlags);
        [PreserveSig] int GetCurrentFolderFlags(out uint pdwFlags);
        [PreserveSig] int GetSortColumnCount(out int pcColumns);
        [PreserveSig] int SetSortColumns(IntPtr rgSortColumns, int cColumns);
        [PreserveSig] int GetSortColumns(IntPtr rgSortColumns, int cColumns);
        [PreserveSig] int GetItem(int iItem, ref Guid riid, out IntPtr ppv);
        [PreserveSig] int GetVisibleItem(int iStart, bool fPrevious, out int piItem);
        [PreserveSig] int GetSelectedItem(int iStart, out int piItem);
        [PreserveSig] int GetSelection(bool fNoneImpliesFolder, out IntPtr ppsia);
        [PreserveSig] int GetSelectionState(IntPtr pidl, out uint pdwFlags);
        [PreserveSig] int InvokeVerbOnSelection([MarshalAs(UnmanagedType.LPStr)] string pszVerb);
        [PreserveSig] int SetViewModeAndIconSize(int uViewMode, int iImageSize);
        [PreserveSig] int GetViewModeAndIconSize(out int puViewMode, out int piImageSize);
    }

    [ComImport]
    [Guid("000214E6-0000-0000-C000-000000000046")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IShellFolder
    {
        [PreserveSig] int ParseDisplayName(IntPtr hwnd, IntPtr pbc, [MarshalAs(UnmanagedType.LPWStr)] string pszDisplayName, out uint pchEaten, out IntPtr ppidl, ref uint pdwAttributes);
        [PreserveSig] int EnumObjects(IntPtr hwnd, uint grfFlags, out IntPtr ppenumIDList);
        [PreserveSig] int BindToObject(IntPtr pidl, IntPtr pbc, ref Guid riid, out IntPtr ppv);
        [PreserveSig] int BindToStorage(IntPtr pidl, IntPtr pbc, ref Guid riid, out IntPtr ppv);
        [PreserveSig] int CompareIDs(IntPtr lParam, IntPtr pidl1, IntPtr pidl2);
        [PreserveSig] int CreateViewObject(IntPtr hwndOwner, ref Guid riid, out IntPtr ppv);
        [PreserveSig] int GetAttributesOf(uint cidl, IntPtr apidl, ref uint rgfInOut);
        [PreserveSig] int GetUIObjectOf(IntPtr hwndOwner, uint cidl, IntPtr apidl, ref Guid riid, IntPtr rgfReserved, out IntPtr ppv);
        [PreserveSig] int GetDisplayNameOf(IntPtr pidl, uint uFlags, IntPtr pName);
        [PreserveSig] int SetNameOf(IntPtr hwnd, IntPtr pidl, [MarshalAs(UnmanagedType.LPWStr)] string pszName, uint uFlags, out IntPtr ppidlOut);
    }

    [DllImport("shlwapi.dll", CharSet = CharSet.Unicode)]
    static extern int StrRetToBSTR(IntPtr pstr, IntPtr pidl, out IntPtr pbstr);

    IFolderView folderView;
    IShellFolder shellFolder;
    IntPtr boundListView;
    IntPtr strRetBuf;
    bool loggedComActive;
    bool loggedFallback;

    bool EnsureBound(IntPtr listView)
    {
        if (folderView != null && boundListView == listView && listView != IntPtr.Zero && Native.IsWindow(listView))
            return true;

        ReleaseCom();

        object swObj = null;
        object dispObj = null;
        object browserObj = null;
        object viewObj = null;
        try
        {
            Type swType = Type.GetTypeFromCLSID(ClsidShellWindows);
            if (swType == null) return false;
            swObj = Activator.CreateInstance(swType);
            IShellWindows sw = swObj as IShellWindows;
            if (sw == null) return false;

            object loc = 0; // CSIDL_DESKTOP
            object empty = Type.Missing;
            int hwnd;
            dispObj = sw.FindWindowSW(ref loc, ref empty, SwcDesktop, out hwnd, SwfoNeedDispatch);
            IShellServiceProvider sp = dispObj as IShellServiceProvider;
            if (sp == null) return false;

            Guid sid = SidSTopLevelBrowser;
            Guid iidBrowser = IidIShellBrowser;
            if (sp.QueryService(ref sid, ref iidBrowser, out browserObj) != 0 || browserObj == null)
                return false;

            IShellBrowser browser = browserObj as IShellBrowser;
            if (browser == null) return false;
            if (browser.QueryActiveShellView(out viewObj) != 0 || viewObj == null)
                return false;

            IFolderView fv = viewObj as IFolderView;
            if (fv == null) return false;

            Guid iidFolder = IidIShellFolder;
            object folderObj;
            IShellFolder sf = null;
            if (fv.GetFolder(ref iidFolder, out folderObj) == 0 && folderObj != null)
                sf = folderObj as IShellFolder;

            folderView = fv;
            shellFolder = sf;
            boundListView = listView;
            viewObj = null; // ownership transferred to folderView
            return true;
        }
        catch
        {
            ReleaseCom();
            return false;
        }
        finally
        {
            SafeRelease(viewObj);
            SafeRelease(browserObj);
            SafeRelease(dispObj);
            SafeRelease(swObj);
        }
    }

    string ReadDisplayName(IntPtr pidl)
    {
        return ReadItemName(pidl, ShgdnNormal);
    }

    string ReadItemName(IntPtr pidl, uint flags)
    {
        if (shellFolder == null || pidl == IntPtr.Zero) return "";
        if (strRetBuf == IntPtr.Zero) strRetBuf = Marshal.AllocCoTaskMem(StrRetBytes);
        // Zero header so StrRetToBSTR never reads stale fields
        Marshal.WriteInt32(strRetBuf, 0, 0);
        Marshal.WriteIntPtr(strRetBuf, IntPtr.Size, IntPtr.Zero);
        if (shellFolder.GetDisplayNameOf(pidl, flags, strRetBuf) != 0) return "";
        IntPtr bstr = IntPtr.Zero;
        try
        {
            if (StrRetToBSTR(strRetBuf, pidl, out bstr) != 0 || bstr == IntPtr.Zero) return "";
            return Marshal.PtrToStringBSTR(bstr) ?? "";
        }
        finally
        {
            if (bstr != IntPtr.Zero) Marshal.FreeBSTR(bstr);
        }
    }

    void QueryIconMetrics(IntPtr listView, out int boxW, out int boxH, out int iconSize)
    {
        boxW = 76;
        boxH = 86;
        iconSize = 48;
        int spaceW = 0, spaceH = 0;
        try
        {
            if (folderView != null)
            {
                var sp = new Native.POINT();
                if (folderView.GetSpacing(ref sp) == 0 && sp.X >= 32 && sp.X <= 512 && sp.Y >= 32 && sp.Y <= 512)
                {
                    spaceW = sp.X;
                    spaceH = sp.Y;
                }
                IFolderView2 fv2 = folderView as IFolderView2;
                if (fv2 != null)
                {
                    int vm, imgSz;
                    if (fv2.GetViewModeAndIconSize(out vm, out imgSz) == 0 && imgSz >= 16 && imgSz <= 256)
                    {
                        // IFolderView2 returns logical icon size; scale by system DPI when spacing is physical.
                        uint dpi = 96;
                        try { dpi = Native.GetDpiForSystem(); } catch { dpi = 96; }
                        if (dpi < 96 || dpi > 480) dpi = 96;
                        int scaled = (int)Math.Round(imgSz * (dpi / 96.0));
                        // Clamp within cell spacing so horizontal centering never inverts.
                        iconSize = (spaceW > 0 && scaled <= spaceW) ? scaled : imgSz;
                    }
                }
            }
            if ((spaceW == 0 || spaceH == 0) && listView != IntPtr.Zero)
            {
                int packed = (int)Native.SendMessage(listView, LvmGetItemSpacing, IntPtr.Zero, IntPtr.Zero).ToInt64();
                int sw = packed & 0xFFFF;
                int sh = (packed >> 16) & 0xFFFF;
                if (sw >= 32 && sw <= 512 && sh >= 32 && sh <= 512)
                {
                    spaceW = sw;
                    spaceH = sh;
                }
            }
            if (spaceW >= 32 && spaceH >= 32)
            {
                boxW = Math.Max(iconSize + 12, spaceW - 4);
                boxH = Math.Max(iconSize + 24, spaceH - 8);
            }
        }
        catch { }
    }

    public bool TryRead(IntPtr listView, out List<DesktopIcon> items)
    {
        items = null;
        if (listView == IntPtr.Zero || !Native.IsWindow(listView)) return false;
        if (!EnsureBound(listView))
        {
            if (!loggedFallback)
            {
                loggedFallback = true;
                Log.Write("IconReader: IFolderView COM unavailable, falling back to SysListView32");
            }
            return false;
        }

        try
        {
            int count;
            if (folderView.ItemCount(SvgioAllView, out count) != 0 || count < 0)
            {
                ReleaseCom();
                return false;
            }

            var origin = new Native.POINT();
            Native.ClientToScreen(listView, ref origin);

            int boxW, boxH, iconSize;
            QueryIconMetrics(listView, out boxW, out boxH, out iconSize);
            // GetItemPosition returns the top-left of the icon glyph (LVIR_ICON), which is
            // horizontally centered inside the cell; shift Left symmetrically so the box
            // aligns with LVIR_BOUNDS (icon + label).
            int xShift = (iconSize - boxW) / 2;

            var list = new List<DesktopIcon>(count);
            for (int i = 0; i < count; i++)
            {
                IntPtr pidl = IntPtr.Zero;
                try
                {
                    if (folderView.Item(i, out pidl) != 0 || pidl == IntPtr.Zero)
                        continue;

                    Native.POINT pt;
                    if (folderView.GetItemPosition(pidl, out pt) != 0)
                        continue;

                    var r = new Native.RECT();
                    r.Left = origin.X + pt.X + xShift;
                    r.Top = origin.Y + pt.Y;
                    r.Right = r.Left + boxW;
                    r.Bottom = r.Top + boxH;

                    var icon = new DesktopIcon();
                    icon.Rect = r;
                    icon.Name = ReadDisplayName(pidl);
                    list.Add(icon);
                }
                finally
                {
                    if (pidl != IntPtr.Zero) Marshal.FreeCoTaskMem(pidl);
                }
            }

            if (!loggedComActive)
            {
                loggedComActive = true;
                Log.Write("IconReader: IFolderView COM active (" + list.Count + " icons, zero cross-process memory)");
            }
            items = list;
            return true;
        }
        catch (Exception ex)
        {
            ReleaseCom();
            if (!loggedFallback)
            {
                loggedFallback = true;
                Log.Write("IconReader: IFolderView COM error (" + ex.Message + "), falling back to SysListView32");
            }
            return false;
        }
    }

    static void SafeRelease(object comObj)
    {
        if (comObj != null && Marshal.IsComObject(comObj))
        {
            try { Marshal.ReleaseComObject(comObj); } catch { }
        }
    }

    void ReleaseCom()
    {
        SafeRelease(shellFolder);
        shellFolder = null;
        SafeRelease(folderView);
        folderView = null;
        boundListView = IntPtr.Zero;
    }

    public bool TryCapturePositions(IntPtr listView, out List<IconSnapshotEntry> entries, out string err)
    {
        entries = null;
        err = null;
        if (listView == IntPtr.Zero || !Native.IsWindow(listView) || !EnsureBound(listView))
        {
            err = "unavailable";
            return false;
        }
        try
        {
            int count;
            if (folderView.ItemCount(SvgioAllView, out count) != 0 || count < 0)
            {
                err = "unavailable";
                ReleaseCom();
                return false;
            }
            var list = new List<IconSnapshotEntry>(count);
            for (int i = 0; i < count; i++)
            {
                IntPtr pidl = IntPtr.Zero;
                try
                {
                    if (folderView.Item(i, out pidl) != 0 || pidl == IntPtr.Zero) continue;
                    Native.POINT pt;
                    if (folderView.GetItemPosition(pidl, out pt) != 0) continue;
                    string name = ReadItemName(pidl, ShgdnNormal);
                    string key = ReadItemName(pidl, ShgdnForParsing);
                    if (string.IsNullOrEmpty(key)) key = name;
                    if (string.IsNullOrEmpty(key) && string.IsNullOrEmpty(name)) continue;
                    var item = new IconSnapshotEntry();
                    item.Key = key;
                    item.Name = name;
                    item.X = pt.X;
                    item.Y = pt.Y;
                    list.Add(item);
                }
                finally
                {
                    if (pidl != IntPtr.Zero) Marshal.FreeCoTaskMem(pidl);
                }
            }
            entries = list;
            return true;
        }
        catch (Exception ex)
        {
            err = ex.Message;
            ReleaseCom();
            return false;
        }
    }

    public bool TryRestorePositions(IntPtr listView, List<IconSnapshotEntry> savedEntries, int curW, int curH, out int restoredCount, out string err)
    {
        restoredCount = 0;
        err = null;
        if (savedEntries == null || savedEntries.Count == 0)
        {
            err = "none";
            return false;
        }
        if (listView == IntPtr.Zero || !Native.IsWindow(listView) || !EnsureBound(listView))
        {
            err = "unavailable";
            return false;
        }
        try
        {
            if (folderView.GetAutoArrange() == 0)
            {
                err = "autoArrange";
                return false;
            }
            int count;
            if (folderView.ItemCount(SvgioAllView, out count) != 0 || count < 0)
            {
                err = "unavailable";
                ReleaseCom();
                return false;
            }

            var byKey = new Dictionary<string, IconSnapshotEntry>(StringComparer.OrdinalIgnoreCase);
            var byName = new Dictionary<string, IconSnapshotEntry>(StringComparer.OrdinalIgnoreCase);
            for (int i = 0; i < savedEntries.Count; i++)
            {
                var e = savedEntries[i];
                if (e == null) continue;
                if (!string.IsNullOrEmpty(e.Key) && !byKey.ContainsKey(e.Key)) byKey[e.Key] = e;
                if (!string.IsNullOrEmpty(e.Name) && !byName.ContainsKey(e.Name)) byName[e.Name] = e;
            }

            var origin = new Native.POINT();
            Native.ClientToScreen(listView, ref origin);
            Rectangle vs = SystemInformation.VirtualScreen;
            int minX = vs.Left - origin.X;
            int minY = vs.Top - origin.Y;
            int maxX = Math.Max(minX + 32, vs.Right - origin.X - 64);
            int maxY = Math.Max(minY + 32, vs.Bottom - origin.Y - 80);
            if (SystemInformation.MonitorCount <= 1)
            {
                minX = 0;
                minY = 0;
                maxX = Math.Max(64, curW - 64);
                maxY = Math.Max(64, curH - 80);
            }

            var pidls = new List<IntPtr>(count);
            var pts = new List<Native.POINT>(count);
            try
            {
                for (int i = 0; i < count; i++)
                {
                    IntPtr pidl = IntPtr.Zero;
                    if (folderView.Item(i, out pidl) != 0 || pidl == IntPtr.Zero) continue;
                    string key = ReadItemName(pidl, ShgdnForParsing);
                    string name = ReadItemName(pidl, ShgdnNormal);
                    IconSnapshotEntry match = null;
                    if (!string.IsNullOrEmpty(key)) byKey.TryGetValue(key, out match);
                    if (match == null && !string.IsNullOrEmpty(name)) byName.TryGetValue(name, out match);
                    if (match == null)
                    {
                        Marshal.FreeCoTaskMem(pidl);
                        continue;
                    }
                    var pt = new Native.POINT();
                    pt.X = Math.Max(minX, Math.Min(maxX, match.X));
                    pt.Y = Math.Max(minY, Math.Min(maxY, match.Y));
                    pidls.Add(pidl);
                    pts.Add(pt);
                }

                if (pidls.Count == 0)
                {
                    restoredCount = 0;
                    return true;
                }

                int ptSize = Marshal.SizeOf(typeof(Native.POINT));
                IntPtr apidl = IntPtr.Zero;
                IntPtr apt = IntPtr.Zero;
                try
                {
                    apidl = Marshal.AllocHGlobal(pidls.Count * IntPtr.Size);
                    apt = Marshal.AllocHGlobal(pts.Count * ptSize);
                    for (int k = 0; k < pidls.Count; k++)
                    {
                        Marshal.WriteIntPtr(apidl, k * IntPtr.Size, pidls[k]);
                        Marshal.StructureToPtr(pts[k], IntPtr.Add(apt, k * ptSize), false);
                    }
                    int hr = folderView.SelectAndPositionItems((uint)pidls.Count, apidl, apt, SvsiPositionItem);
                    if (hr != 0)
                    {
                        err = "COM 0x" + hr.ToString("X8", CultureInfo.InvariantCulture);
                        return false;
                    }
                    restoredCount = pidls.Count;
                    return true;
                }
                finally
                {
                    if (apidl != IntPtr.Zero) Marshal.FreeHGlobal(apidl);
                    if (apt != IntPtr.Zero) Marshal.FreeHGlobal(apt);
                }
            }
            finally
            {
                for (int k = 0; k < pidls.Count; k++)
                {
                    if (pidls[k] != IntPtr.Zero) Marshal.FreeCoTaskMem(pidls[k]);
                }
            }
        }
        catch (Exception ex)
        {
            err = ex.Message;
            ReleaseCom();
            return false;
        }
    }

    public void Dispose()
    {
        ReleaseCom();
        if (strRetBuf != IntPtr.Zero)
        {
            Marshal.FreeCoTaskMem(strRetBuf);
            strRetBuf = IntPtr.Zero;
        }
    }
}

static class DesktopIconSnapshotStore
{
    static readonly string FilePath = Path.Combine(Log.DataDir, "icon-snapshots.json");

    public static string CurrentResolutionKey()
    {
        Rectangle b = Screen.PrimaryScreen.Bounds;
        return b.Width.ToString(CultureInfo.InvariantCulture) + "x" + b.Height.ToString(CultureInfo.InvariantCulture);
    }

    static Dictionary<string, object> LoadRoot()
    {
        try
        {
            if (File.Exists(FilePath))
            {
                var d = Json.Parse(File.ReadAllText(FilePath, Encoding.UTF8)) as Dictionary<string, object>;
                if (d != null) return d;
            }
        }
        catch { }
        return new Dictionary<string, object>();
    }

    static Dictionary<string, object> GetMap(Dictionary<string, object> root, string key)
    {
        object raw;
        if (root != null && root.TryGetValue(key, out raw) && raw is Dictionary<string, object>)
            return (Dictionary<string, object>)raw;
        return new Dictionary<string, object>();
    }

    static object[] AsArr(object raw)
    {
        var arr = raw as object[];
        if (arr != null) return arr;
        var al = raw as ArrayList;
        return al != null ? al.ToArray() : null;
    }

    public static bool SaveCurrent(out int count, out string resKey, out string err)
    {
        count = 0;
        resKey = CurrentResolutionKey();
        err = null;
        IntPtr lv = IconReader.FindListView();
        List<IconSnapshotEntry> captured;
        using (var sr = new ShellIconReader())
        {
            if (!sr.TryCapturePositions(lv, out captured, out err) || captured == null)
                return false;
        }

        var list = new List<object>(captured.Count);
        for (int i = 0; i < captured.Count; i++)
        {
            var e = captured[i];
            list.Add(Json.Obj("key", e.Key, "name", e.Name, "x", e.X, "y", e.Y));
        }

        var root = LoadRoot();
        var snaps = GetMap(root, "snapshots");
        snaps[resKey] = Json.Obj(
            "savedAt", DateTime.Now.ToString("yyyy-MM-dd HH:mm", CultureInfo.InvariantCulture),
            "count", captured.Count,
            "items", list.ToArray());
        root["version"] = 1;
        root["latestKey"] = resKey;
        root["snapshots"] = snaps;

        try
        {
            Directory.CreateDirectory(Log.DataDir);
            File.WriteAllText(FilePath, Json.Stringify(root), new UTF8Encoding(false));
            count = captured.Count;
            return true;
        }
        catch (Exception ex)
        {
            err = ex.Message;
            return false;
        }
    }

    public static bool RestoreForCurrentScreen(out int restored, out int totalSaved, out string resKey, out string err)
    {
        restored = 0;
        totalSaved = 0;
        resKey = CurrentResolutionKey();
        err = null;

        var root = LoadRoot();
        var snaps = GetMap(root, "snapshots");
        object snapObj = null;
        if (!snaps.TryGetValue(resKey, out snapObj))
        {
            object latestObj;
            string lk = root.TryGetValue("latestKey", out latestObj) ? (latestObj as string) : null;
            if (!string.IsNullOrEmpty(lk) && snaps.TryGetValue(lk, out snapObj))
                resKey = lk;
        }

        var snapDict = snapObj as Dictionary<string, object>;
        if (snapDict == null)
        {
            err = "none";
            return false;
        }

        object rawItems;
        var itemsArr = snapDict.TryGetValue("items", out rawItems) ? AsArr(rawItems) : null;
        if (itemsArr == null || itemsArr.Length == 0)
        {
            err = "none";
            return false;
        }

        var entries = new List<IconSnapshotEntry>(itemsArr.Length);
        for (int i = 0; i < itemsArr.Length; i++)
        {
            var d = itemsArr[i] as Dictionary<string, object>;
            if (d == null) continue;
            var e = new IconSnapshotEntry();
            e.Key = d.ContainsKey("key") ? (d["key"] as string ?? "") : "";
            e.Name = d.ContainsKey("name") ? (d["name"] as string ?? "") : "";
            try
            {
                e.X = d.ContainsKey("x") ? Convert.ToInt32(d["x"], CultureInfo.InvariantCulture) : 0;
                e.Y = d.ContainsKey("y") ? Convert.ToInt32(d["y"], CultureInfo.InvariantCulture) : 0;
                entries.Add(e);
            }
            catch { }
        }
        totalSaved = entries.Count;
        if (entries.Count == 0)
        {
            err = "none";
            return false;
        }

        Rectangle scr = Screen.PrimaryScreen.Bounds;
        IntPtr lv = IconReader.FindListView();
        using (var sr = new ShellIconReader())
        {
            return sr.TryRestorePositions(lv, entries, scr.Width, scr.Height, out restored, out err);
        }
    }

    public static Dictionary<string, object> GetStatus()
    {
        string cur = CurrentResolutionKey();
        var root = LoadRoot();
        var snaps = GetMap(root, "snapshots");
        bool exact = false;
        string rk = cur;
        object snapObj = null;
        if (snaps.TryGetValue(cur, out snapObj) && snapObj is Dictionary<string, object>)
        {
            exact = true;
        }
        else
        {
            object latestObj;
            string lk = root.TryGetValue("latestKey", out latestObj) ? (latestObj as string) : null;
            if (!string.IsNullOrEmpty(lk) && snaps.TryGetValue(lk, out snapObj) && snapObj is Dictionary<string, object>)
                rk = lk;
        }

        var snapDict = snapObj as Dictionary<string, object>;
        bool has = snapDict != null;
        int cnt = 0;
        string ts = "";
        if (has)
        {
            object cv, tv;
            if (snapDict.TryGetValue("count", out cv))
                try { cnt = Convert.ToInt32(cv, CultureInfo.InvariantCulture); } catch { }
            if (snapDict.TryGetValue("savedAt", out tv) && tv is string)
                ts = (string)tv;
        }
        return Json.Obj("hasSnapshot", has, "count", cnt, "resKey", rk, "savedAt", ts, "exactMatch", exact);
    }
}
