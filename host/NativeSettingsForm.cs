using System;
using System.Collections;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows.Forms;
using Microsoft.Win32;

// Pure native WinForms + live Rust OpenGL 3.3 Settings window (zero WebView2).
// Used whenever Rust OpenGL mode is active so DiyarDesk spawns 0 msedgewebview2.exe processes.
class NativeSettingsForm : Form
{
    readonly string pageDir;
    readonly HostContext host;
    readonly ToolTip tip = new ToolTip();
    readonly Random rng = new Random();

    // Theme palette (auto-synced with Windows Light/Dark mode)
    bool darkTheme;
    Color colBg, colSurface, colCard, colCardHover, colBorder, colText, colMuted, colAccent, colAccentSoft, colInputBg, colBadgeBg;

    [DllImport("uxtheme.dll", CharSet = CharSet.Unicode)]
    static extern int SetWindowTheme(IntPtr hwnd, string pszSubAppName, string pszSubIdList);

    // Layout & controls
    Panel topPanel, leftPanel, rightPanel, previewHostWrap, previewFrame, paramsWrap;
    FlowLayoutPanel topControlsFlow, themeGrid;
    Label brandLabel, engineBadge, titleLabel, subtitleLabel, themeNameLabel, themeDescLabel, previewFootLabel, savedLabel;
    TextBox searchBox;
    CheckBox chkRustGl, chkLowMode, chkDisableAutoPause, chkSolarSync, chkWindowAware, chkAutostart, chkAdaptive;
    ComboBox cmbLang, cmbDisplay, cmbPlaylist;
    Button btnSwitchWebView, btnDiag, btnPreviewPause, btnReseed, btnResetAll;
    Font fontBrand, fontTitle, fontSub, fontGroup, fontBody, fontSmall, fontBold;

    // Live Rust OpenGL preview
    NativeGlHost previewGl;
    bool previewPaused;
    string lastIconsJson = "";

    // Data
    List<Dictionary<string, object>> allThemes = new List<Dictionary<string, object>>();
    readonly List<ThemeCardControl> themeCards = new List<ThemeCardControl>();
    bool suppressEvents;
    System.Windows.Forms.Timer savedFlashTimer;
    System.Windows.Forms.Timer statusTimer;

    // Audio UI refs (if active theme has an audio param)
    Label lblAudioName;
    Button btnAudioPlay;

    static readonly Regex HexColorRx = new Regex(@"#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})\b", RegexOptions.Compiled);
    static readonly Regex RgbColorRx = new Regex(@"rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)", RegexOptions.Compiled);
    static readonly Regex ThumbUrlRx = new Regex(@"url\(\s*['""]?([^'""\)\?]+)", RegexOptions.Compiled | RegexOptions.IgnoreCase);
    readonly Dictionary<string, Bitmap> thumbCache = new Dictionary<string, Bitmap>(StringComparer.OrdinalIgnoreCase);

    public NativeSettingsForm(string pageDir, HostContext host)
    {
        this.pageDir = pageDir;
        this.host = host;

        Text = "DiyarDesk";
        AccessibleName = "DiyarDesk";
        ShowIcon = true;
        Icon = AppIcon.Create();
        StartPosition = FormStartPosition.CenterScreen;
        DoubleBuffered = true;

        float s = Native.GetDpiForSystem() / 96f;
        if (s < 1f) s = 1f;
        ClientSize = new Size((int)(1160 * s), (int)(820 * s));
        MinimumSize = new Size((int)(920 * s), (int)(660 * s));

        fontBrand = new Font("Segoe UI", 13f, FontStyle.Bold);
        fontTitle = new Font("Segoe UI", 14f, FontStyle.Bold);
        fontSub = new Font("Segoe UI", 9.5f, FontStyle.Regular);
        fontGroup = new Font("Segoe UI", 9.5f, FontStyle.Bold);
        fontBody = new Font("Segoe UI", 9f, FontStyle.Regular);
        fontSmall = new Font("Segoe UI", 8.25f, FontStyle.Regular);
        fontBold = new Font("Segoe UI", 9f, FontStyle.Bold);

        tip.AutoPopDelay = 8000;
        tip.InitialDelay = 300;
        tip.ReshowDelay = 120;
        tip.ShowAlways = true;

        DetectSystemPalette();
        BuildUi(s);
        LoadThemes();
        ApplyLocalizedTexts();
        SyncGlobalControlsFromSettings();
        RebuildThemeCards();
        SelectThemeUi(Settings.Theme);

        savedFlashTimer = new System.Windows.Forms.Timer();
        savedFlashTimer.Interval = 1800;
        savedFlashTimer.Tick += delegate
        {
            savedFlashTimer.Stop();
            if (!IsDisposed && savedLabel != null)
            {
                savedLabel.Text = I18n.Ui("autosave");
                savedLabel.ForeColor = colMuted;
            }
        };

        statusTimer = new System.Windows.Forms.Timer();
        statusTimer.Interval = 1200;
        statusTimer.Tick += delegate { UpdateEngineBadge(); };
        statusTimer.Start();
    }

    int S(int px)
    {
        float s = Native.GetDpiForSystem() / 96f;
        if (s < 1f) s = 1f;
        return (int)Math.Round(px * s);
    }

    void DetectSystemPalette()
    {
        bool light = true;
        try
        {
            object v = Registry.GetValue(@"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "AppsUseLightTheme", 1);
            light = v == null || Convert.ToInt32(v) != 0;
        }
        catch { }
        darkTheme = !light;
        if (darkTheme)
        {
            colBg = Color.FromArgb(20, 22, 26);
            colSurface = Color.FromArgb(26, 29, 35);
            colCard = Color.FromArgb(34, 38, 46);
            colCardHover = Color.FromArgb(44, 50, 61);
            colBorder = Color.FromArgb(56, 62, 74);
            colText = Color.FromArgb(236, 240, 246);
            colMuted = Color.FromArgb(148, 158, 174);
            colAccent = Color.FromArgb(56, 189, 248);
            colAccentSoft = Color.FromArgb(30, 58, 76);
            colInputBg = Color.FromArgb(28, 32, 39);
            colBadgeBg = Color.FromArgb(22, 64, 48);
        }
        else
        {
            colBg = Color.FromArgb(245, 247, 250);
            colSurface = Color.FromArgb(255, 255, 255);
            colCard = Color.FromArgb(255, 255, 255);
            colCardHover = Color.FromArgb(242, 246, 252);
            colBorder = Color.FromArgb(220, 226, 235);
            colText = Color.FromArgb(22, 26, 32);
            colMuted = Color.FromArgb(100, 112, 128);
            colAccent = Color.FromArgb(2, 132, 199);
            colAccentSoft = Color.FromArgb(224, 242, 254);
            colInputBg = Color.FromArgb(250, 251, 253);
            colBadgeBg = Color.FromArgb(220, 252, 231);
        }
        BackColor = colBg;
        ForeColor = colText;
    }

    void ThemeTitleBar()
    {
        if (!IsHandleCreated) return;
        int dark = darkTheme ? 1 : 0;
        int color = darkTheme ? 0x001A1614 : 0x00FAF7F5; // COLORREF 0x00BBGGRR
        try
        {
            Native.DwmSetWindowAttribute(Handle, 20, ref dark, 4);
            Native.DwmSetWindowAttribute(Handle, 35, ref color, 4);
        }
        catch { }
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        ThemeTitleBar();
    }

    protected override void WndProc(ref Message m)
    {
        if (m.Msg == 0x001A /* WM_SETTINGCHANGE */)
        {
            bool wasDark = darkTheme;
            DetectSystemPalette();
            ThemeTitleBar();
            if (wasDark != darkTheme) ApplyPaletteToTree();
        }
        else if (m.Msg == 0x02E0 /* WM_DPICHANGED */ && m.LParam != IntPtr.Zero)
        {
            try
            {
                var suggested = (Native.RECT)Marshal.PtrToStructure(m.LParam, typeof(Native.RECT));
                Native.SetWindowPos(Handle, IntPtr.Zero, suggested.Left, suggested.Top,
                    suggested.Right - suggested.Left, suggested.Bottom - suggested.Top,
                    Native.SWP_NOACTIVATE | Native.SWP_NOZORDER);
            }
            catch { }
        }
        base.WndProc(ref m);
    }

    void BuildUi(float s)
    {
        // --- Top header bar ---
        topPanel = new Panel();
        topPanel.Dock = DockStyle.Top;
        topPanel.AutoSize = true;
        topPanel.AutoSizeMode = AutoSizeMode.GrowAndShrink;
        topPanel.Padding = new Padding(S(20), S(12), S(20), S(10));
        topPanel.BackColor = colSurface;

        var topHeaderRow = new Panel();
        topHeaderRow.Dock = DockStyle.Top;
        topHeaderRow.Height = S(36);
        topHeaderRow.BackColor = Color.Transparent;

        var headerLeft = new FlowLayoutPanel();
        headerLeft.Dock = DockStyle.Left;
        headerLeft.AutoSize = true;
        headerLeft.WrapContents = false;
        headerLeft.BackColor = Color.Transparent;

        brandLabel = new Label();
        brandLabel.Text = "DiyarDesk";
        brandLabel.Font = fontBrand;
        brandLabel.AutoSize = true;
        brandLabel.Margin = new Padding(0, S(3), S(12), 0);

        engineBadge = new Label();
        engineBadge.AutoSize = true;
        engineBadge.Font = fontSmall;
        engineBadge.Padding = new Padding(S(8), S(3), S(8), S(3));
        engineBadge.Margin = new Padding(0, S(5), S(8), 0);

        headerLeft.Controls.Add(brandLabel);
        headerLeft.Controls.Add(engineBadge);

        var headerRight = new FlowLayoutPanel();
        headerRight.Dock = DockStyle.Right;
        headerRight.AutoSize = true;
        headerRight.WrapContents = false;
        headerRight.FlowDirection = FlowDirection.RightToLeft;
        headerRight.BackColor = Color.Transparent;

        btnDiag = MakeGhostButton("Diagnostics");
        btnDiag.Margin = new Padding(S(8), S(3), 0, 0);
        btnDiag.Click += delegate { host.ExportDiagnosticsFromNative(this); };

        btnSwitchWebView = MakeGhostButton("WebView2 (OpenGL Off)");
        btnSwitchWebView.Font = fontBold;
        btnSwitchWebView.Margin = new Padding(S(8), S(3), 0, 0);
        btnSwitchWebView.Click += delegate { host.SetRustOpenGlFromUi(false); };

        headerRight.Controls.Add(btnDiag);
        headerRight.Controls.Add(btnSwitchWebView);

        topHeaderRow.Controls.Add(headerLeft);
        topHeaderRow.Controls.Add(headerRight);

        topControlsFlow = new FlowLayoutPanel();
        topControlsFlow.Dock = DockStyle.Top;
        topControlsFlow.AutoSize = true;
        topControlsFlow.AutoSizeMode = AutoSizeMode.GrowAndShrink;
        topControlsFlow.WrapContents = true;
        topControlsFlow.Padding = new Padding(0, S(8), 0, 0);
        topControlsFlow.BackColor = Color.Transparent;

        chkRustGl = MakeCheckBox("Rust OpenGL");
        chkRustGl.Font = fontBold;
        chkRustGl.CheckedChanged += delegate
        {
            if (suppressEvents) return;
            host.SetRustOpenGlFromUi(chkRustGl.Checked);
        };

        cmbDisplay = MakeCombo(S(155));
        cmbDisplay.SelectedIndexChanged += delegate
        {
            if (suppressEvents) return;
            string[] modes = { "primary", "independent", "span", "duplicate" };
            if (cmbDisplay.SelectedIndex >= 0 && cmbDisplay.SelectedIndex < modes.Length)
                host.SetDisplayModeFromUi(modes[cmbDisplay.SelectedIndex]);
        };

        cmbPlaylist = MakeCombo(S(145));
        cmbPlaylist.SelectedIndexChanged += delegate
        {
            if (suppressEvents) return;
            int[] mins = { 0, 15, 30, 60, 180 };
            if (cmbPlaylist.SelectedIndex >= 0 && cmbPlaylist.SelectedIndex < mins.Length)
            {
                int iv = mins[cmbPlaylist.SelectedIndex];
                host.SetPlaylistFromUi(iv > 0, iv > 0 ? iv : 30);
            }
        };

        cmbLang = MakeCombo(S(165));
        cmbLang.SelectedIndexChanged += delegate
        {
            if (suppressEvents) return;
            if (cmbLang.SelectedIndex == 0) host.SetLangFromUi("auto");
            else if (cmbLang.SelectedIndex > 0 && cmbLang.SelectedIndex - 1 < I18n.Supported.Length)
                host.SetLangFromUi(I18n.Supported[cmbLang.SelectedIndex - 1]);
        };

        chkSolarSync = MakeCheckBox("Solar sync");
        chkSolarSync.CheckedChanged += delegate
        {
            if (suppressEvents) return;
            host.SetSolarSyncFromUi(chkSolarSync.Checked);
            if (previewGl != null && previewGl.IsRunning)
                previewGl.PostJson(EnvironmentHelper.BuildSolarMessage(chkSolarSync.Checked));
        };

        chkWindowAware = MakeCheckBox("Window-aware");
        chkWindowAware.CheckedChanged += delegate
        {
            if (suppressEvents) return;
            host.SetWindowAwareFromUi(chkWindowAware.Checked);
        };

        chkLowMode = MakeCheckBox("Eco mode");
        chkLowMode.CheckedChanged += delegate
        {
            if (suppressEvents) return;
            host.SetLowModeFromUi(chkLowMode.Checked);
            if (previewGl != null && previewGl.IsRunning)
            {
                previewGl.SetLowMode(chkLowMode.Checked);
                previewGl.PostJson(Json.Stringify(Json.Obj("t", "lowMode", "v", chkLowMode.Checked)));
            }
        };

        chkDisableAutoPause = MakeCheckBox("Disable auto-pause");
        chkDisableAutoPause.CheckedChanged += delegate
        {
            if (suppressEvents) return;
            host.SetDisableAutoPauseFromUi(chkDisableAutoPause.Checked);
        };

        chkAutostart = MakeCheckBox("Autostart");
        chkAutostart.CheckedChanged += delegate
        {
            if (suppressEvents) return;
            host.SetAutoStartFromUi(chkAutostart.Checked);
        };

        topControlsFlow.Controls.Add(chkRustGl);
        topControlsFlow.Controls.Add(cmbDisplay);
        topControlsFlow.Controls.Add(cmbPlaylist);
        topControlsFlow.Controls.Add(cmbLang);
        topControlsFlow.Controls.Add(chkSolarSync);
        topControlsFlow.Controls.Add(chkWindowAware);
        topControlsFlow.Controls.Add(chkLowMode);
        topControlsFlow.Controls.Add(chkDisableAutoPause);
        topControlsFlow.Controls.Add(chkAutostart);

        var topBorder = new Panel();
        topBorder.Dock = DockStyle.Bottom;
        topBorder.Height = 1;
        topBorder.BackColor = colBorder;

        topPanel.Controls.Add(topControlsFlow);
        topPanel.Controls.Add(topHeaderRow);
        topPanel.Controls.Add(topBorder);
        topPanel.Resize += delegate
        {
            int w = Math.Max(S(320), topPanel.ClientSize.Width - topPanel.Padding.Horizontal);
            topControlsFlow.MaximumSize = new Size(w, 0);
        };

        // --- Bottom status bar ---
        var bottomPanel = new Panel();
        bottomPanel.Dock = DockStyle.Bottom;
        bottomPanel.Height = S(36);
        bottomPanel.Padding = new Padding(S(20), S(8), S(20), S(8));
        bottomPanel.BackColor = colSurface;

        var bottomBorder = new Panel();
        bottomBorder.Dock = DockStyle.Top;
        bottomBorder.Height = 1;
        bottomBorder.BackColor = colBorder;

        savedLabel = new Label();
        savedLabel.Dock = DockStyle.Fill;
        savedLabel.Font = fontSmall;
        savedLabel.ForeColor = colMuted;
        savedLabel.TextAlign = ContentAlignment.MiddleLeft;

        bottomPanel.Controls.Add(savedLabel);
        bottomPanel.Controls.Add(bottomBorder);

        // --- Main two-column split ---
        var bodyPanel = new Panel();
        bodyPanel.Dock = DockStyle.Fill;
        bodyPanel.Padding = new Padding(S(16), S(12), S(16), S(12));
        bodyPanel.BackColor = colBg;

        // Right column (Preview + Parameters)
        rightPanel = new Panel();
        rightPanel.Dock = DockStyle.Right;
        rightPanel.Width = S(440);
        rightPanel.Padding = new Padding(S(14), 0, 0, 0);
        rightPanel.BackColor = colBg;

        // Preview card
        var previewCard = new Panel();
        previewCard.Dock = DockStyle.Top;
        previewCard.AutoSize = true;
        previewCard.AutoSizeMode = AutoSizeMode.GrowAndShrink;
        previewCard.Padding = new Padding(S(14));
        previewCard.BackColor = colSurface;

        var prevHead = new Panel();
        prevHead.Dock = DockStyle.Top;
        prevHead.Height = S(28);
        prevHead.BackColor = Color.Transparent;

        themeNameLabel = new Label();
        themeNameLabel.Dock = DockStyle.Fill;
        themeNameLabel.Font = fontTitle;
        themeNameLabel.AutoEllipsis = true;
        themeNameLabel.TextAlign = ContentAlignment.MiddleLeft;

        chkAdaptive = MakeCheckBox("Adaptive");
        chkAdaptive.Dock = DockStyle.Right;
        chkAdaptive.Margin = new Padding(0);
        chkAdaptive.CheckedChanged += delegate
        {
            if (suppressEvents) return;
            OnParamChanged("adaptive", chkAdaptive.Checked ? 1 : 0);
        };

        prevHead.Controls.Add(themeNameLabel);
        prevHead.Controls.Add(chkAdaptive);

        themeDescLabel = new Label();
        themeDescLabel.Dock = DockStyle.Top;
        themeDescLabel.Font = fontSmall;
        themeDescLabel.ForeColor = colMuted;
        themeDescLabel.AutoSize = true;
        themeDescLabel.MaximumSize = new Size(S(392), S(54));
        themeDescLabel.Padding = new Padding(0, S(4), 0, S(8));

        previewHostWrap = new Panel();
        previewHostWrap.Dock = DockStyle.Top;
        previewHostWrap.Height = S(224);
        previewHostWrap.Padding = new Padding(1);
        previewHostWrap.BackColor = colBorder;

        previewFrame = new Panel();
        previewFrame.Dock = DockStyle.Fill;
        previewFrame.BackColor = Color.FromArgb(14, 18, 24);
        previewHostWrap.Controls.Add(previewFrame);

        previewFootLabel = new Label();
        previewFootLabel.Dock = DockStyle.Top;
        previewFootLabel.Height = S(22);
        previewFootLabel.Font = fontSmall;
        previewFootLabel.ForeColor = colMuted;
        previewFootLabel.TextAlign = ContentAlignment.MiddleLeft;
        previewFootLabel.Padding = new Padding(0, S(4), 0, 0);

        var prevActions = new FlowLayoutPanel();
        prevActions.Dock = DockStyle.Top;
        prevActions.AutoSize = true;
        prevActions.AutoSizeMode = AutoSizeMode.GrowAndShrink;
        prevActions.Padding = new Padding(0, S(6), 0, 0);
        prevActions.BackColor = Color.Transparent;

        btnPreviewPause = MakeGhostButton("Pause preview");
        btnPreviewPause.Click += delegate { TogglePreviewPause(); };

        btnReseed = MakeGhostButton("Refresh");
        btnReseed.Click += delegate
        {
            int seed = rng.Next(1, 999999);
            OnParamChanged("seed", seed);
        };

        btnResetAll = MakeGhostButton("Reset all");
        btnResetAll.Click += delegate
        {
            host.ResetThemeFromNative(Settings.Theme);
            RebuildParamControls(FindTheme(Settings.Theme));
            SyncPreviewThemeState();
            FlashSaved();
        };

        prevActions.Controls.Add(btnPreviewPause);
        prevActions.Controls.Add(btnReseed);
        prevActions.Controls.Add(btnResetAll);

        previewCard.Controls.Add(prevActions);
        previewCard.Controls.Add(previewFootLabel);
        previewCard.Controls.Add(previewHostWrap);
        previewCard.Controls.Add(themeDescLabel);
        previewCard.Controls.Add(prevHead);

        var spacer = new Panel();
        spacer.Dock = DockStyle.Top;
        spacer.Height = S(10);
        spacer.BackColor = Color.Transparent;

        // Scrollable Parameters panel
        paramsWrap = new Panel();
        paramsWrap.Dock = DockStyle.Fill;
        paramsWrap.AutoScroll = true;
        paramsWrap.Padding = new Padding(S(14), S(10), S(14), S(14));
        paramsWrap.BackColor = colSurface;

        rightPanel.Controls.Add(paramsWrap);
        rightPanel.Controls.Add(spacer);
        rightPanel.Controls.Add(previewCard);

        // Left column (Theme Collection)
        leftPanel = new Panel();
        leftPanel.Dock = DockStyle.Fill;
        leftPanel.BackColor = colBg;

        var leftHead = new Panel();
        leftHead.Dock = DockStyle.Top;
        leftHead.Height = S(58);
        leftHead.Padding = new Padding(0, 0, S(6), S(8));
        leftHead.BackColor = Color.Transparent;

        titleLabel = new Label();
        titleLabel.AutoSize = true;
        titleLabel.Font = fontTitle;
        titleLabel.Location = new Point(0, 0);

        subtitleLabel = new Label();
        subtitleLabel.AutoSize = true;
        subtitleLabel.Font = fontSub;
        subtitleLabel.ForeColor = colMuted;
        subtitleLabel.Location = new Point(0, S(26));

        searchBox = new TextBox();
        searchBox.Width = S(200);
        searchBox.Font = fontBody;
        searchBox.Anchor = AnchorStyles.Top | AnchorStyles.Right;
        searchBox.BorderStyle = BorderStyle.FixedSingle;
        searchBox.TextChanged += delegate { FilterThemeCards(searchBox.Text); };

        leftHead.Resize += delegate
        {
            searchBox.Location = new Point(Math.Max(S(240), leftHead.ClientSize.Width - searchBox.Width - S(4)), S(12));
        };

        leftHead.Controls.Add(titleLabel);
        leftHead.Controls.Add(subtitleLabel);
        leftHead.Controls.Add(searchBox);

        themeGrid = new FlowLayoutPanel();
        themeGrid.Dock = DockStyle.Fill;
        themeGrid.AutoScroll = true;
        themeGrid.WrapContents = true;
        themeGrid.BackColor = colBg;
        themeGrid.Padding = new Padding(0, S(4), S(4), S(12));

        leftPanel.Controls.Add(themeGrid);
        leftPanel.Controls.Add(leftHead);

        bodyPanel.Controls.Add(leftPanel);
        bodyPanel.Controls.Add(rightPanel);

        Controls.Add(bodyPanel);
        Controls.Add(bottomPanel);
        Controls.Add(topPanel);

        ApplyPaletteToTree();
    }

    CheckBox MakeCheckBox(string text)
    {
        var cb = new CheckBox();
        cb.Text = text;
        cb.AutoSize = true;
        cb.Font = fontBody;
        cb.Margin = new Padding(S(4), S(5), S(12), S(4));
        cb.Cursor = Cursors.Hand;
        return cb;
    }

    ComboBox MakeCombo(int width)
    {
        var cb = new ComboBox();
        cb.DropDownStyle = ComboBoxStyle.DropDownList;
        cb.Width = width;
        cb.Font = fontBody;
        cb.FlatStyle = FlatStyle.Flat;
        cb.Margin = new Padding(S(2), S(3), S(10), S(3));
        return cb;
    }

    Button MakeGhostButton(string text)
    {
        var b = new Button();
        b.Text = text;
        b.AutoSize = true;
        b.FlatStyle = FlatStyle.Flat;
        b.FlatAppearance.BorderSize = 1;
        b.Font = fontSmall;
        b.Padding = new Padding(S(8), S(3), S(8), S(3));
        b.Margin = new Padding(0, 0, S(8), 0);
        b.Cursor = Cursors.Hand;
        return b;
    }

    void ApplyNativeScrollTheme(Control c)
    {
        if (c == null || c.IsDisposed) return;
        Action apply = delegate
        {
            if (c.IsDisposed || !c.IsHandleCreated) return;
            try
            {
                if (c is ComboBox)
                    SetWindowTheme(c.Handle, darkTheme ? "DarkMode_CFD" : "CFD", null);
                else
                    SetWindowTheme(c.Handle, darkTheme ? "DarkMode_Explorer" : "Explorer", null);
            }
            catch { }
        };
        if (c.IsHandleCreated) apply();
        else c.HandleCreated += delegate { apply(); };
    }

    void ApplyPaletteToTree()
    {
        BackColor = colBg;
        ForeColor = colText;
        ApplyPaletteRecursive(this);
        ApplyNativeScrollTheme(paramsWrap);
        ApplyNativeScrollTheme(themeGrid);
        UpdateEngineBadge();
        if (btnSwitchWebView != null)
        {
            btnSwitchWebView.BackColor = colAccentSoft;
            btnSwitchWebView.ForeColor = colAccent;
            btnSwitchWebView.FlatAppearance.BorderColor = colAccent;
            btnSwitchWebView.FlatAppearance.MouseOverBackColor = darkTheme ? Color.FromArgb(40, 76, 98) : Color.FromArgb(196, 231, 252);
        }
        for (int i = 0; i < themeCards.Count; i++)
            themeCards[i].SetPalette(colCard, colCardHover, colBorder, colText, colMuted, colAccent, colAccentSoft);
    }

    void ApplyPaletteRecursive(Control root)
    {
        foreach (Control c in root.Controls)
        {
            if (c == previewFrame || c is GlSurfaceControl || c is ThemeCardControl) continue;
            if (c is ModernSliderControl)
            {
                ((ModernSliderControl)c).SetPalette(colSurface, colBorder, colAccent, colText);
                continue;
            }
            if (c == topPanel || c == paramsWrap || (c.Parent == rightPanel && c is Panel && c.BackColor != Color.Transparent && c.Height != S(10)))
            {
                if (c != previewHostWrap) c.BackColor = colSurface;
            }
            else if (c == leftPanel || c == themeGrid || c == rightPanel)
            {
                c.BackColor = colBg;
            }
            else if (c == previewHostWrap)
            {
                c.BackColor = colBorder;
            }

            if (c is Button && c != btnSwitchWebView)
            {
                var b = (Button)c;
                b.BackColor = colCard;
                b.ForeColor = colText;
                b.FlatAppearance.BorderColor = colBorder;
                b.FlatAppearance.MouseOverBackColor = colCardHover;
            }
            else if (c is TextBox || c is ComboBox)
            {
                c.BackColor = colInputBg;
                c.ForeColor = colText;
                if (c is ComboBox) ApplyNativeScrollTheme(c);
            }
            else if (c is CheckBox)
            {
                c.ForeColor = colText;
            }
            else if (c is Label)
            {
                if (c != engineBadge && c != subtitleLabel && c != themeDescLabel && c != previewFootLabel && c != savedLabel)
                    c.ForeColor = colText;
                else if (c != engineBadge)
                    c.ForeColor = colMuted;
            }

            if (c.HasChildren) ApplyPaletteRecursive(c);
        }
    }

    void UpdateEngineBadge()
    {
        if (IsDisposed || engineBadge == null) return;
        int wvCount = WebViewProcessGuard.OwnWebView2ProcessCount();
        engineBadge.Text = wvCount == 0
            ? "Rust OpenGL 3.3 · 0 WebView2"
            : ("Rust OpenGL 3.3 · WebView2: " + wvCount);
        engineBadge.BackColor = colBadgeBg;
        engineBadge.ForeColor = darkTheme ? Color.FromArgb(134, 239, 172) : Color.FromArgb(21, 128, 61);
    }

    void LoadThemes()
    {
        allThemes.Clear();
        var builtIn = Themes.LoadBuiltIn(pageDir);
        for (int i = 0; i < builtIn.Count; i++)
        {
            var th = builtIn[i];
            string id = th.ContainsKey("id") ? th["id"] as string : null;
            string status = th.ContainsKey("status") ? th["status"] as string : null;
            if (!string.IsNullOrEmpty(id) && string.Equals(status, "ready", StringComparison.OrdinalIgnoreCase) && WallpaperForm.IsNativeGlTheme(id))
            {
                allThemes.Add(th);
            }
        }
    }

    Dictionary<string, object> FindTheme(string id)
    {
        for (int i = 0; i < allThemes.Count; i++)
        {
            if (string.Equals(allThemes[i]["id"] as string, id, StringComparison.OrdinalIgnoreCase))
                return allThemes[i];
        }
        return allThemes.Count > 0 ? allThemes[0] : null;
    }

    public void ApplyLocalizedTexts()
    {
        suppressEvents = true;
        try
        {
            titleLabel.Text = I18n.Ui("collectionTitle");
            subtitleLabel.Text = I18n.Ui("collectionSubtitle");
            chkRustGl.Text = I18n.Ui("rustOpenGl") + " (" + I18n.Ui("rustOpenGlFast") + ")";
            tip.SetToolTip(chkRustGl, I18n.Ui("rustOpenGlTitle"));
            btnSwitchWebView.Text = I18n.Code == "tr"
                ? "WebView2'ye geç (OpenGL kapat)"
                : (I18n.Code == "zh-CN" ? "切回 WebView2（关闭 OpenGL）" : "Switch to WebView2 (Disable OpenGL)");
            tip.SetToolTip(btnSwitchWebView, I18n.Ui("rustOpenGlTitle"));
            chkSolarSync.Text = I18n.Ui("solarSync");
            tip.SetToolTip(chkSolarSync, I18n.Ui("solarSyncTitle"));
            chkWindowAware.Text = I18n.Ui("windowAware");
            tip.SetToolTip(chkWindowAware, I18n.Ui("windowAwareTitle"));
            chkLowMode.Text = I18n.Ui("lowMode");
            tip.SetToolTip(chkLowMode, I18n.Ui("lowModeTitle"));
            chkDisableAutoPause.Text = I18n.Ui("disableAutoPause");
            tip.SetToolTip(chkDisableAutoPause, I18n.Ui("disableAutoPauseTitle"));
            chkAutostart.Text = I18n.Ui("autostart");
            tip.SetToolTip(chkAutostart, I18n.Ui("autostartTitle"));
            chkAdaptive.Text = I18n.Ui("adaptive");
            btnDiag.Text = I18n.Ui("exportDiag");
            tip.SetToolTip(btnDiag, I18n.Ui("exportDiagTitle"));
            btnReseed.Text = I18n.Ui("refresh");
            tip.SetToolTip(btnReseed, I18n.Ui("refreshTitle"));
            btnResetAll.Text = I18n.Ui("resetAll");
            tip.SetToolTip(btnResetAll, I18n.Ui("resetAllTitle"));
            btnPreviewPause.Text = previewPaused ? "▶ " + I18n.Ui("livePreview") : "⏸ " + I18n.Ui("livePreview");
            savedLabel.Text = I18n.Ui("autosave");
            previewFootLabel.Text = "● " + I18n.Ui("livePreview") + " (Rust OpenGL 3.3)";

            cmbDisplay.Items.Clear();
            cmbDisplay.Items.AddRange(new object[] {
                I18n.Ui("displayPrimary"),
                I18n.Ui("displayIndependent"),
                I18n.Ui("displaySpan"),
                I18n.Ui("displayDuplicate")
            });

            cmbPlaylist.Items.Clear();
            cmbPlaylist.Items.AddRange(new object[] {
                I18n.Ui("playlistOff"),
                I18n.Ui("playlist15"),
                I18n.Ui("playlist30"),
                I18n.Ui("playlist60"),
                I18n.Ui("playlist180")
            });

            cmbLang.Items.Clear();
            cmbLang.Items.Add(I18n.LangDisplayName("auto"));
            for (int i = 0; i < I18n.Supported.Length; i++)
                cmbLang.Items.Add(I18n.LangDisplayName(I18n.Supported[i]));
        }
        finally { suppressEvents = false; }

        SyncGlobalControlsFromSettings();
        RebuildThemeCards();
        SelectThemeUi(Settings.Theme);
    }

    public void SyncGlobalControlsFromSettings()
    {
        suppressEvents = true;
        try
        {
            chkRustGl.Checked = Settings.RustOpenGl;
            chkLowMode.Checked = Settings.LowMode;
            chkDisableAutoPause.Checked = Settings.DisableAutoPause;
            chkSolarSync.Checked = Settings.SolarSync;
            chkWindowAware.Checked = Settings.WindowAware;
            chkAutostart.Checked = host.IsAutoStartEnabled();

            string[] modes = { "primary", "independent", "span", "duplicate" };
            int dIdx = Array.IndexOf(modes, Settings.DisplayMode);
            if (dIdx >= 0 && dIdx < cmbDisplay.Items.Count) cmbDisplay.SelectedIndex = dIdx;

            int pIdx = 0;
            if (Settings.PlaylistEnabled)
            {
                if (Settings.PlaylistIntervalMin <= 15) pIdx = 1;
                else if (Settings.PlaylistIntervalMin <= 30) pIdx = 2;
                else if (Settings.PlaylistIntervalMin <= 60) pIdx = 3;
                else pIdx = 4;
            }
            if (pIdx < cmbPlaylist.Items.Count) cmbPlaylist.SelectedIndex = pIdx;

            int lIdx = 0;
            if (Settings.Lang != "auto")
            {
                int found = Array.IndexOf(I18n.Supported, Settings.Lang);
                if (found >= 0) lIdx = found + 1;
            }
            if (lIdx < cmbLang.Items.Count) cmbLang.SelectedIndex = lIdx;
        }
        finally { suppressEvents = false; }
    }

    static Color[] ParseThumbGradient(string thumb)
    {
        var cols = new List<Color>();
        if (!string.IsNullOrEmpty(thumb))
        {
            foreach (Match m in HexColorRx.Matches(thumb))
            {
                string h = m.Groups[1].Value;
                try
                {
                    if (h.Length == 3)
                        h = new string(new char[] { h[0], h[0], h[1], h[1], h[2], h[2] });
                    int r = int.Parse(h.Substring(0, 2), NumberStyles.HexNumber);
                    int g = int.Parse(h.Substring(2, 2), NumberStyles.HexNumber);
                    int b = int.Parse(h.Substring(4, 2), NumberStyles.HexNumber);
                    cols.Add(Color.FromArgb(r, g, b));
                }
                catch { }
                if (cols.Count >= 3) break;
            }
            if (cols.Count < 2)
            {
                foreach (Match m in RgbColorRx.Matches(thumb))
                {
                    try
                    {
                        int r = Math.Min(255, int.Parse(m.Groups[1].Value));
                        int g = Math.Min(255, int.Parse(m.Groups[2].Value));
                        int b = Math.Min(255, int.Parse(m.Groups[3].Value));
                        cols.Add(Color.FromArgb(r, g, b));
                    }
                    catch { }
                    if (cols.Count >= 3) break;
                }
            }
        }
        if (cols.Count == 0) return new Color[] { Color.FromArgb(30, 64, 110), Color.FromArgb(56, 189, 248) };
        if (cols.Count == 1) return new Color[] { cols[0], ControlPaint.Light(cols[0], 0.25f) };
        return cols.ToArray();
    }

    Bitmap LoadThumbBitmap(string id, string thumbCss)
    {
        string relPath = null;
        if (!string.IsNullOrEmpty(thumbCss))
        {
            var m = ThumbUrlRx.Match(thumbCss);
            if (m.Success) relPath = m.Groups[1].Value.Trim();
        }
        string fullPath = null;
        if (!string.IsNullOrEmpty(relPath) && !relPath.StartsWith("__custom/", StringComparison.Ordinal))
        {
            string candidate = Path.Combine(pageDir, relPath.Replace('/', Path.DirectorySeparatorChar));
            if (File.Exists(candidate)) fullPath = candidate;
        }
        if (fullPath == null && !string.IsNullOrEmpty(id))
        {
            string fallbackWebp = Path.Combine(Path.Combine(pageDir, "thumbs"), id + ".webp");
            if (File.Exists(fallbackWebp)) fullPath = fallbackWebp;
        }
        if (string.IsNullOrEmpty(fullPath)) return null;

        Bitmap cached;
        if (thumbCache.TryGetValue(fullPath, out cached)) return cached;

        Bitmap bmp = DecodeThumbScaled(fullPath, 320);
        thumbCache[fullPath] = bmp;
        return bmp;
    }

    static Bitmap DecodeThumbScaled(string filePath, int maxWidth)
    {
#if HAS_WIC_WEBP
        try
        {
            using (var fs = new FileStream(filePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
            {
                var dec = System.Windows.Media.Imaging.BitmapDecoder.Create(
                    fs,
                    System.Windows.Media.Imaging.BitmapCreateOptions.None,
                    System.Windows.Media.Imaging.BitmapCacheOption.OnLoad);
                if (dec.Frames.Count > 0)
                {
                    System.Windows.Media.Imaging.BitmapSource src = dec.Frames[0];
                    if (src.PixelWidth > maxWidth && src.PixelWidth > 0)
                    {
                        double sc = (double)maxWidth / src.PixelWidth;
                        src = new System.Windows.Media.Imaging.TransformedBitmap(src, new System.Windows.Media.ScaleTransform(sc, sc));
                    }
                    var fmt = new System.Windows.Media.Imaging.FormatConvertedBitmap(src, System.Windows.Media.PixelFormats.Bgra32, null, 0);
                    int w = fmt.PixelWidth;
                    int h = fmt.PixelHeight;
                    var bmp = new Bitmap(w, h, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
                    var data = bmp.LockBits(new Rectangle(0, 0, w, h), System.Drawing.Imaging.ImageLockMode.WriteOnly, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
                    try
                    {
                        fmt.CopyPixels(System.Windows.Int32Rect.Empty, data.Scan0, data.Stride * h, data.Stride);
                    }
                    finally
                    {
                        bmp.UnlockBits(data);
                    }
                    return bmp;
                }
            }
        }
        catch { }
#endif
        try
        {
            using (var img = Image.FromFile(filePath))
                return new Bitmap(img);
        }
        catch { return null; }
    }

    void RebuildThemeCards()
    {
        themeGrid.SuspendLayout();
        themeGrid.Controls.Clear();
        themeCards.Clear();

        int cardW = S(194);
        int cardH = S(132);
        string activeBadge = I18n.Ui("badgeOn");
        string readyBadge = I18n.Ui("badgeReady");

        for (int i = 0; i < allThemes.Count; i++)
        {
            var th = allThemes[i];
            string id = th["id"] as string;
            string rawName = th.ContainsKey("name") ? th["name"] as string : id;
            string name = I18n.ThemeName(id, rawName);
            string thumb = th.ContainsKey("thumb") ? th["thumb"] as string : "";
            Color[] grad = ParseThumbGradient(thumb);
            Bitmap thumbBmp = LoadThumbBitmap(id, thumb);

            var card = new ThemeCardControl(id, name, grad, thumbBmp, activeBadge, readyBadge, fontBold, fontSmall);
            card.Size = new Size(cardW, cardH);
            card.Margin = new Padding(0, 0, S(10), S(10));
            card.SetPalette(colCard, colCardHover, colBorder, colText, colMuted, colAccent, colAccentSoft);
            card.SetSelected(string.Equals(id, Settings.Theme, StringComparison.OrdinalIgnoreCase));
            string capturedId = id;
            card.Click += delegate
            {
                if (!string.Equals(Settings.Theme, capturedId, StringComparison.OrdinalIgnoreCase))
                {
                    host.SwitchThemeFromNative(capturedId);
                }
                SelectThemeUi(capturedId);
            };
            themeCards.Add(card);
            themeGrid.Controls.Add(card);
        }

        FilterThemeCards(searchBox != null ? searchBox.Text : "");
        themeGrid.ResumeLayout();
    }

    void FilterThemeCards(string query)
    {
        string q = (query ?? "").Trim().ToLowerInvariant();
        themeGrid.SuspendLayout();
        for (int i = 0; i < themeCards.Count; i++)
        {
            var c = themeCards[i];
            bool match = string.IsNullOrEmpty(q)
                || c.ThemeTitle.ToLowerInvariant().Contains(q)
                || c.ThemeId.ToLowerInvariant().Contains(q);
            c.Visible = match;
        }
        themeGrid.ResumeLayout();
    }

    public void SelectThemeUi(string themeId)
    {
        var th = FindTheme(themeId);
        if (th == null) return;
        string id = th["id"] as string;

        for (int i = 0; i < themeCards.Count; i++)
            themeCards[i].SetSelected(string.Equals(themeCards[i].ThemeId, id, StringComparison.OrdinalIgnoreCase));

        string rawName = th.ContainsKey("name") ? th["name"] as string : id;
        string rawDesc = th.ContainsKey("desc") ? th["desc"] as string : "";
        themeNameLabel.Text = I18n.ThemeName(id, rawName);
        themeDescLabel.Text = I18n.ThemeDesc(id, rawDesc);

        bool supportsAdaptive = th.ContainsKey("adaptive") && th["adaptive"] is bool && (bool)th["adaptive"];
        suppressEvents = true;
        try
        {
            chkAdaptive.Visible = supportsAdaptive;
            if (supportsAdaptive)
            {
                var p = Settings.For(id);
                object av;
                bool isOn = !p.TryGetValue("adaptive", out av) || Convert.ToInt32(av) != 0;
                chkAdaptive.Checked = isOn;
            }
        }
        finally { suppressEvents = false; }

        RebuildParamControls(th);
        SyncPreviewThemeState();
    }

    static object[] AsArray(object raw)
    {
        var arr = raw as object[];
        if (arr != null) return arr;
        var al = raw as ArrayList;
        return al != null ? al.ToArray() : null;
    }

    static string FormatNumericValue(Dictionary<string, object> p, double val)
    {
        string fmt = p.ContainsKey("format") ? p["format"] as string : null;
        string unit = p.ContainsKey("unit") ? p["unit"] as string : null;
        double step = p.ContainsKey("step") ? Convert.ToDouble(p["step"]) : 1.0;
        if (fmt == "times") return "×" + val.ToString("0.00", CultureInfo.InvariantCulture);
        if (fmt == "percent") return Math.Round(val * 100.0).ToString(CultureInfo.InvariantCulture) + "%";
        bool isInt = Math.Abs(step - Math.Round(step)) < 1e-6;
        string num = isInt ? Math.Round(val).ToString(CultureInfo.InvariantCulture) : val.ToString("0.00", CultureInfo.InvariantCulture);
        return string.IsNullOrEmpty(unit) ? num : (num + " " + unit);
    }

    void RebuildParamControls(Dictionary<string, object> theme)
    {
        paramsWrap.SuspendLayout();
        paramsWrap.Controls.Clear();
        lblAudioName = null;
        btnAudioPlay = null;

        if (theme == null)
        {
            paramsWrap.ResumeLayout();
            return;
        }
        string tid = theme["id"] as string;
        object rawGroups;
        var groups = theme.TryGetValue("groups", out rawGroups) ? AsArray(rawGroups) : null;
        if (groups == null || groups.Length == 0)
        {
            var empty = new Label();
            empty.Text = I18n.Ui("noParams");
            empty.Font = fontBody;
            empty.ForeColor = colMuted;
            empty.AutoSize = true;
            paramsWrap.Controls.Add(empty);
            paramsWrap.ResumeLayout();
            return;
        }

        var saved = Settings.For(tid);
        int y = S(4);
        int fullW = Math.Max(S(320), paramsWrap.ClientSize.Width - S(44));

        for (int gi = 0; gi < groups.Length; gi++)
        {
            var g = groups[gi] as Dictionary<string, object>;
            if (g == null) continue;
            string gName = I18n.ThemeGroup(tid, gi, g.ContainsKey("name") ? g["name"] as string : "");

            var lblG = new Label();
            lblG.Text = gName;
            lblG.Font = fontGroup;
            lblG.ForeColor = colAccent;
            lblG.AutoSize = true;
            lblG.Location = new Point(0, y);
            paramsWrap.Controls.Add(lblG);
            y += S(24);

            object rawParams;
            var plist = g.TryGetValue("params", out rawParams) ? AsArray(rawParams) : null;
            if (plist == null) continue;

            for (int pi = 0; pi < plist.Length; pi++)
            {
                var p = plist[pi] as Dictionary<string, object>;
                if (p == null || !p.ContainsKey("key")) continue;
                string key = p["key"] as string;
                string pType = p.ContainsKey("type") ? (p["type"] as string ?? "") : "";
                string label = I18n.ThemeParamLabel(tid, key, p.ContainsKey("label") ? p["label"] as string : key);
                string hint = I18n.ThemeParamHint(tid, key, p.ContainsKey("hint") ? p["hint"] as string : "");
                object defVal = p.ContainsKey("default") ? p["default"] : 0;

                var row = new Panel();
                row.Location = new Point(0, y);
                row.Width = fullW;
                row.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
                row.BackColor = Color.Transparent;

                var lblP = new Label();
                lblP.Text = label;
                lblP.Font = fontBody;
                lblP.ForeColor = colText;
                lblP.AutoSize = true;
                lblP.Location = new Point(0, 0);
                if (!string.IsNullOrEmpty(hint)) tip.SetToolTip(lblP, hint);
                row.Controls.Add(lblP);

                if (pType == "audio")
                {
                    string defName = p.ContainsKey("defaultName") ? (p["defaultName"] as string) : I18n.Ui("audioNone");
                    string curName = saved.ContainsKey("audioName") && !string.IsNullOrEmpty(saved["audioName"] as string) ? saved["audioName"] as string : defName;
                    bool hasAudio = (saved.ContainsKey("audioFile") && !string.IsNullOrEmpty(saved["audioFile"] as string))
                        || (p.ContainsKey("defaultSrc") && !string.IsNullOrEmpty(p["defaultSrc"] as string));
                    bool playing = hasAudio && saved.ContainsKey("audioPlay") && Convert.ToInt32(saved["audioPlay"]) == 1;

                    lblAudioName = new Label();
                    lblAudioName.Text = curName;
                    lblAudioName.Font = fontSmall;
                    lblAudioName.ForeColor = colMuted;
                    lblAudioName.AutoEllipsis = true;
                    lblAudioName.TextAlign = ContentAlignment.MiddleRight;
                    lblAudioName.Bounds = new Rectangle(fullW / 2, 0, fullW / 2, S(18));
                    lblAudioName.Anchor = AnchorStyles.Top | AnchorStyles.Right;
                    row.Controls.Add(lblAudioName);

                    var btnPick = MakeGhostButton(I18n.Ui("audioPick"));
                    btnPick.Location = new Point(0, S(22));
                    btnPick.Click += delegate { host.PickAudioFromNative(tid, this); };

                    btnAudioPlay = MakeGhostButton(playing ? I18n.Ui("audioPause") : I18n.Ui("audioPlay"));
                    btnAudioPlay.Location = new Point(S(110), S(22));
                    btnAudioPlay.Enabled = hasAudio;
                    btnAudioPlay.Click += delegate
                    {
                        var cur = Settings.For(tid);
                        int next = (cur.ContainsKey("audioPlay") && Convert.ToInt32(cur["audioPlay"]) == 1) ? 0 : 1;
                        OnParamChanged("audioPlay", next);
                        btnAudioPlay.Text = next == 1 ? I18n.Ui("audioPause") : I18n.Ui("audioPlay");
                    };

                    row.Controls.Add(btnPick);
                    row.Controls.Add(btnAudioPlay);
                    row.Height = S(58);
                }
                else if (p.ContainsKey("options"))
                {
                    var opts = AsArray(p["options"]);
                    var vals = p.ContainsKey("values") ? AsArray(p["values"]) : null;
                    object curVal = saved.ContainsKey(key) ? saved[key] : defVal;

                    var cmb = MakeCombo(fullW - S(34));
                    cmb.Location = new Point(0, S(21));
                    cmb.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
                    cmb.BackColor = colInputBg;
                    cmb.ForeColor = colText;
                    ApplyNativeScrollTheme(cmb);
                    if (!string.IsNullOrEmpty(hint)) tip.SetToolTip(cmb, hint);

                    int selIdx = 0;
                    if (opts != null)
                    {
                        for (int oi = 0; oi < opts.Length; oi++)
                        {
                            string oLabel = I18n.ThemeParamOption(tid, key, oi, Convert.ToString(opts[oi]));
                            cmb.Items.Add(oLabel);
                            object ov = (vals != null && oi < vals.Length) ? vals[oi] : (object)oi;
                            if (ValuesEqual(curVal, ov)) selIdx = oi;
                        }
                    }
                    if (cmb.Items.Count > 0) cmb.SelectedIndex = selIdx;

                    string capKey = key;
                    var capVals = vals;
                    object capDef = defVal;
                    cmb.SelectedIndexChanged += delegate
                    {
                        if (suppressEvents || cmb.SelectedIndex < 0) return;
                        object v = (capVals != null && cmb.SelectedIndex < capVals.Length) ? capVals[cmb.SelectedIndex] : (object)cmb.SelectedIndex;
                        OnParamChanged(capKey, v);
                    };

                    var btnUndo = MakeResetBtn(fullW - S(28), S(20));
                    btnUndo.Click += delegate
                    {
                        OnParamChanged(capKey, capDef);
                        suppressEvents = true;
                        try
                        {
                            for (int oi = 0; oi < cmb.Items.Count; oi++)
                            {
                                object ov = (capVals != null && oi < capVals.Length) ? capVals[oi] : (object)oi;
                                if (ValuesEqual(capDef, ov)) { cmb.SelectedIndex = oi; break; }
                            }
                        }
                        finally { suppressEvents = false; }
                    };

                    row.Controls.Add(cmb);
                    row.Controls.Add(btnUndo);
                    row.Height = S(52);
                }
                else if (pType == "color")
                {
                    string curHex = Convert.ToString(saved.ContainsKey(key) ? saved[key] : defVal);
                    if (string.IsNullOrEmpty(curHex)) curHex = "#4a90e2";
                    Color curColor = ColorTranslator.FromHtml(curHex);

                    var btnColor = new Button();
                    btnColor.Bounds = new Rectangle(0, S(21), S(60), S(24));
                    btnColor.FlatStyle = FlatStyle.Flat;
                    btnColor.BackColor = curColor;
                    btnColor.FlatAppearance.BorderColor = colBorder;

                    var lblHex = new Label();
                    lblHex.Text = curHex.ToUpperInvariant();
                    lblHex.Font = fontSmall;
                    lblHex.ForeColor = colMuted;
                    lblHex.Location = new Point(S(70), S(25));
                    lblHex.AutoSize = true;

                    string capKey = key;
                    string capDefHex = Convert.ToString(defVal);
                    btnColor.Click += delegate
                    {
                        using (var dlg = new ColorDialog())
                        {
                            dlg.Color = btnColor.BackColor;
                            dlg.FullOpen = true;
                            if (dlg.ShowDialog(this) == DialogResult.OK)
                            {
                                string hex = string.Format("#{0:x2}{1:x2}{2:x2}", dlg.Color.R, dlg.Color.G, dlg.Color.B);
                                btnColor.BackColor = dlg.Color;
                                lblHex.Text = hex.ToUpperInvariant();
                                OnParamChanged(capKey, hex);
                            }
                        }
                    };

                    var btnUndo = MakeResetBtn(fullW - S(28), S(20));
                    btnUndo.Click += delegate
                    {
                        try { btnColor.BackColor = ColorTranslator.FromHtml(capDefHex); } catch { }
                        lblHex.Text = capDefHex.ToUpperInvariant();
                        OnParamChanged(capKey, capDefHex);
                    };

                    row.Controls.Add(btnColor);
                    row.Controls.Add(lblHex);
                    row.Controls.Add(btnUndo);
                    row.Height = S(52);
                }
                else if (p.ContainsKey("min") && p.ContainsKey("max"))
                {
                    double min = Convert.ToDouble(p["min"]);
                    double max = Convert.ToDouble(p["max"]);
                    double step = p.ContainsKey("step") ? Convert.ToDouble(p["step"]) : 1.0;
                    if (step <= 0) step = 1.0;
                    double defNum = Convert.ToDouble(defVal);
                    double curNum = saved.ContainsKey(key) ? Convert.ToDouble(saved[key]) : defNum;
                    if (curNum < min) curNum = min;
                    if (curNum > max) curNum = max;
                    int steps = Math.Max(1, (int)Math.Round((max - min) / step));

                    var lblVal = new Label();
                    lblVal.Text = FormatNumericValue(p, curNum);
                    lblVal.Font = fontBold;
                    lblVal.ForeColor = colText;
                    lblVal.TextAlign = ContentAlignment.MiddleRight;
                    lblVal.Bounds = new Rectangle(fullW - S(128), 0, S(96), S(18));
                    lblVal.Anchor = AnchorStyles.Top | AnchorStyles.Right;

                    var tb = new ModernSliderControl();
                    tb.Minimum = 0;
                    tb.Maximum = steps;
                    tb.Bounds = new Rectangle(0, S(20), fullW - S(34), S(24));
                    tb.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
                    tb.SetPalette(colSurface, colBorder, colAccent, colText);
                    tb.Value = Math.Max(0, Math.Min(steps, (int)Math.Round((curNum - min) / step)));
                    if (!string.IsNullOrEmpty(hint)) tip.SetToolTip(tb, hint);

                    string capKey = key;
                    var capP = p;
                    double capMin = min, capStep = step, capDef = defNum;
                    int capSteps = steps;
                    bool isIntStep = Math.Abs(step - Math.Round(step)) < 1e-6;

                    tb.ValueChanged += delegate
                    {
                        double val = capMin + tb.Value * capStep;
                        val = Math.Round(val, 4);
                        lblVal.Text = FormatNumericValue(capP, val);
                        OnParamChanged(capKey, isIntStep ? (object)(int)Math.Round(val) : (object)val);
                    };

                    tb.DoubleClick += delegate
                    {
                        tb.Value = Math.Max(0, Math.Min(capSteps, (int)Math.Round((capDef - capMin) / capStep)));
                        lblVal.Text = FormatNumericValue(capP, capDef);
                        OnParamChanged(capKey, isIntStep ? (object)(int)Math.Round(capDef) : (object)capDef);
                    };

                    var btnUndo = MakeResetBtn(fullW - S(28), S(19));
                    btnUndo.Click += delegate
                    {
                        tb.Value = Math.Max(0, Math.Min(capSteps, (int)Math.Round((capDef - capMin) / capStep)));
                        lblVal.Text = FormatNumericValue(capP, capDef);
                        OnParamChanged(capKey, isIntStep ? (object)(int)Math.Round(capDef) : (object)capDef);
                    };

                    row.Controls.Add(lblVal);
                    row.Controls.Add(tb);
                    row.Controls.Add(btnUndo);
                    row.Height = S(48);
                }
                else
                {
                    row.Height = S(24);
                }

                paramsWrap.Controls.Add(row);
                y += row.Height + S(4);
            }
            y += S(8);
        }

        paramsWrap.ResumeLayout();
    }

    Button MakeResetBtn(int x, int y)
    {
        var b = new Button();
        b.Text = "↺";
        b.Bounds = new Rectangle(x, y, S(26), S(24));
        b.Anchor = AnchorStyles.Top | AnchorStyles.Right;
        b.FlatStyle = FlatStyle.Flat;
        b.FlatAppearance.BorderSize = 1;
        b.FlatAppearance.BorderColor = colBorder;
        b.BackColor = colCard;
        b.ForeColor = colMuted;
        b.Font = fontSmall;
        b.Cursor = Cursors.Hand;
        return b;
    }

    static bool ValuesEqual(object a, object b)
    {
        if (a == null && b == null) return true;
        if (a == null || b == null) return false;
        if (a is string || b is string)
            return string.Equals(Convert.ToString(a), Convert.ToString(b), StringComparison.OrdinalIgnoreCase);
        try { return Math.Abs(Convert.ToDouble(a) - Convert.ToDouble(b)) < 1e-6; }
        catch { return a.Equals(b); }
    }

    void OnParamChanged(string key, object value)
    {
        host.SetParamFromNative(Settings.Theme, key, value);
        if (previewGl != null && previewGl.IsRunning)
        {
            previewGl.PostJson(Json.Stringify(Json.Obj("t", "settings", "v", Json.Obj(key, value))));
        }
        FlashSaved();
    }

    void FlashSaved()
    {
        if (IsDisposed || savedLabel == null) return;
        savedLabel.Text = I18n.Ui("saved");
        savedLabel.ForeColor = colAccent;
        savedFlashTimer.Stop();
        savedFlashTimer.Start();
    }

    public void NotifyAudioPicked(string name, int play)
    {
        if (IsDisposed) return;
        if (lblAudioName != null) lblAudioName.Text = name ?? "";
        if (btnAudioPlay != null)
        {
            btnAudioPlay.Enabled = true;
            btnAudioPlay.Text = play == 1 ? I18n.Ui("audioPause") : I18n.Ui("audioPlay");
        }
    }

    public void NotifyAudioPlayChanged(int play)
    {
        if (IsDisposed || btnAudioPlay == null) return;
        btnAudioPlay.Text = play == 1 ? I18n.Ui("audioPause") : I18n.Ui("audioPlay");
    }

    // --- Live Rust OpenGL 3.3 Preview ---
    protected override void OnShown(EventArgs e)
    {
        base.OnShown(e);
        StartPreviewGl();
        UpdateEngineBadge();
    }

    void StartPreviewGl()
    {
        if (IsDisposed || previewGl != null || !NativeGlHost.IsAvailable()) return;
        float dpi = Native.GetDpiForSystem() / 96f;
        if (dpi < 1f) dpi = 1f;
        Log.Write("NativeSettingsForm: Rust OpenGL önizleme başlatılıyor (" + previewFrame.Width + "x" + previewFrame.Height + ")");
        var gl = new NativeGlHost();
        if (gl.Start(previewFrame, dpi))
        {
            previewGl = gl;
            if (gl.Surface != null)
            {
                gl.Surface.MouseMove += delegate(object s, MouseEventArgs me) { if (previewGl != null) previewGl.DispatchMouse(0, me.X / dpi, me.Y / dpi); };
                gl.Surface.MouseDown += delegate(object s, MouseEventArgs me) { if (previewGl != null && me.Button == MouseButtons.Left) previewGl.DispatchMouse(1, me.X / dpi, me.Y / dpi); };
                gl.Surface.MouseUp += delegate(object s, MouseEventArgs me) { if (previewGl != null && me.Button == MouseButtons.Left) previewGl.DispatchMouse(2, me.X / dpi, me.Y / dpi); };
            }
            SyncPreviewThemeState();
        }
        else
        {
            Log.Write("NativeSettingsForm: Rust OpenGL önizleme başlatılamadı; statik önizleme yedek modunda");
            gl.Dispose();
        }
    }

    void SyncPreviewThemeState()
    {
        if (previewGl == null || !previewGl.IsRunning) return;
        string tid = Settings.Theme ?? "papercut";
        previewGl.SetLowMode(Settings.LowMode);
        previewGl.PostJson(Json.Stringify(Json.Obj("t", "lowMode", "v", Settings.LowMode)));
        previewGl.PostJson(Json.Stringify(Json.Obj("t", "theme", "id", tid)));
        previewGl.PostJson(EnvironmentHelper.BuildSolarMessage(Settings.SolarSync));
        previewGl.PostJson(Json.Stringify(Json.Obj("t", "settings", "v", Settings.For(tid))));
        PushScaledIconsToPreview(string.IsNullOrEmpty(lastIconsJson) ? host.GetLastIconsJson() : lastIconsJson);
        previewGl.SetPaused(previewPaused || WindowState == FormWindowState.Minimized);
    }

    public void OnIconsChanged(string desktopIconsJson)
    {
        lastIconsJson = desktopIconsJson ?? "";
        if (!IsDisposed && previewGl != null && previewGl.IsRunning)
            PushScaledIconsToPreview(lastIconsJson);
    }

    void PushScaledIconsToPreview(string desktopIconsJson)
    {
        if (previewGl == null || !previewGl.IsRunning || previewFrame == null) return;
        int pw = Math.Max(64, previewFrame.ClientSize.Width);
        int ph = Math.Max(64, previewFrame.ClientSize.Height);
        Rectangle scr = Screen.PrimaryScreen.Bounds;
        double sx = scr.Width > 0 ? (double)pw / scr.Width : 0.25;
        double sy = scr.Height > 0 ? (double)ph / scr.Height : 0.25;

        var scaled = new List<object>();
        bool vis = true;
        if (!string.IsNullOrEmpty(desktopIconsJson))
        {
            try
            {
                var root = Json.Parse(desktopIconsJson) as Dictionary<string, object>;
                if (root != null)
                {
                    if (root.ContainsKey("vis") && root["vis"] is bool) vis = (bool)root["vis"];
                    object rawR;
                    var arr = root.TryGetValue("r", out rawR) ? AsArray(rawR) : null;
                    if (vis && arr != null)
                    {
                        for (int i = 0; i < arr.Length; i++)
                        {
                            var rc = AsArray(arr[i]);
                            if (rc == null || rc.Length < 4) continue;
                            int l = (int)Math.Round(Convert.ToDouble(rc[0]) * sx);
                            int t = (int)Math.Round(Convert.ToDouble(rc[1]) * sy);
                            int r = (int)Math.Round(Convert.ToDouble(rc[2]) * sx);
                            int b = (int)Math.Round(Convert.ToDouble(rc[3]) * sy);
                            if (r > l + 2 && b > t + 2)
                                scaled.Add(new object[] { l, t, r, b });
                        }
                    }
                }
            }
            catch { }
        }
        // If user hid desktop icons, send a small sample cluster to preview so adaptive themes stay expressive
        if (scaled.Count == 0)
        {
            vis = true;
            int iw = Math.Max(18, pw / 16);
            int ih = Math.Max(22, ph / 8);
            int gap = Math.Max(6, pw / 48);
            for (int row = 0; row < 3; row++)
                scaled.Add(new object[] { gap, gap + row * (ih + gap), gap + iw, gap + row * (ih + gap) + ih });
            scaled.Add(new object[] { gap * 2 + iw, gap, gap * 2 + iw * 2, gap + ih });
        }
        previewGl.PostJson(Json.Stringify(Json.Obj("t", "icons", "vis", vis, "r", scaled)));
    }

    void TogglePreviewPause()
    {
        previewPaused = !previewPaused;
        if (previewGl != null && previewGl.IsRunning) previewGl.SetPaused(previewPaused);
        btnPreviewPause.Text = previewPaused ? "▶ " + I18n.Ui("livePreview") : "⏸ " + I18n.Ui("livePreview");
    }

    protected override void OnResize(EventArgs e)
    {
        base.OnResize(e);
        if (previewGl != null && previewGl.IsRunning)
        {
            previewGl.SetPaused(previewPaused || WindowState == FormWindowState.Minimized);
            if (WindowState != FormWindowState.Minimized)
                PushScaledIconsToPreview(lastIconsJson);
        }
    }

    protected override void OnFormClosed(FormClosedEventArgs e)
    {
        if (statusTimer != null) { statusTimer.Stop(); statusTimer.Dispose(); statusTimer = null; }
        if (savedFlashTimer != null) { savedFlashTimer.Stop(); savedFlashTimer.Dispose(); savedFlashTimer = null; }
        if (previewGl != null)
        {
            try { previewGl.Dispose(); } catch { }
            previewGl = null;
        }
        foreach (var kv in thumbCache)
        {
            try { if (kv.Value != null) kv.Value.Dispose(); } catch { }
        }
        thumbCache.Clear();
        base.OnFormClosed(e);
    }

    // Custom rounded theme card with thumbnail image / gradient swatch & active highlight
    sealed class ThemeCardControl : Control
    {
        public readonly string ThemeId;
        public readonly string ThemeTitle;
        readonly Color[] gradColors;
        readonly Bitmap thumbImage;
        readonly string activeBadge, readyBadge;
        readonly Font titleFont, badgeFont;
        Color cBg, cHover, cBorder, cText, cMuted, cAccent, cAccentSoft;
        bool selected, hovered;

        public ThemeCardControl(string id, string title, Color[] grad, Bitmap thumbImage, string activeBadge, string readyBadge, Font titleFont, Font badgeFont)
        {
            ThemeId = id;
            ThemeTitle = title;
            gradColors = grad;
            this.thumbImage = thumbImage;
            this.activeBadge = activeBadge;
            this.readyBadge = readyBadge;
            this.titleFont = titleFont;
            this.badgeFont = badgeFont;
            DoubleBuffered = true;
            Cursor = Cursors.Hand;
        }

        public void SetPalette(Color bg, Color hover, Color border, Color text, Color muted, Color accent, Color accentSoft)
        {
            cBg = bg; cHover = hover; cBorder = border; cText = text; cMuted = muted; cAccent = accent; cAccentSoft = accentSoft;
            Invalidate();
        }

        public void SetSelected(bool sel)
        {
            if (selected == sel) return;
            selected = sel;
            Invalidate();
        }

        protected override void OnMouseEnter(EventArgs e) { hovered = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { hovered = false; Invalidate(); base.OnMouseLeave(e); }

        static GraphicsPath RoundedRect(Rectangle r, int radius)
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

        protected override void OnPaint(PaintEventArgs e)
        {
            var g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;
            g.Clear(Parent != null ? Parent.BackColor : cBg);

            var rect = new Rectangle(1, 1, Width - 3, Height - 3);
            using (var path = RoundedRect(rect, 10))
            {
                using (var fill = new SolidBrush(selected ? cAccentSoft : (hovered ? cHover : cBg)))
                    g.FillPath(fill, path);

                // Top thumbnail / gradient preview strip
                int stripH = (int)(Height * 0.54);
                var stripRect = new Rectangle(rect.X, rect.Y, rect.Width, stripH);
                g.SetClip(path);
                if (thumbImage != null)
                {
                    g.InterpolationMode = InterpolationMode.HighQualityBicubic;
                    float imgRatio = (float)thumbImage.Width / Math.Max(1, thumbImage.Height);
                    float boxRatio = (float)stripRect.Width / Math.Max(1, stripRect.Height);
                    RectangleF srcRect;
                    if (imgRatio > boxRatio)
                    {
                        float sw = thumbImage.Height * boxRatio;
                        srcRect = new RectangleF((thumbImage.Width - sw) * 0.5f, 0, sw, thumbImage.Height);
                    }
                    else
                    {
                        float sh = thumbImage.Width / boxRatio;
                        srcRect = new RectangleF(0, (thumbImage.Height - sh) * 0.5f, thumbImage.Width, sh);
                    }
                    g.DrawImage(thumbImage, stripRect, srcRect, GraphicsUnit.Pixel);
                }
                else
                {
                    Color c1 = gradColors.Length > 0 ? gradColors[0] : cAccent;
                    Color c2 = gradColors.Length > 1 ? gradColors[1] : c1;
                    using (var lg = new LinearGradientBrush(stripRect, c1, c2, 25f))
                    {
                        if (gradColors.Length >= 3)
                        {
                            var cb = new ColorBlend();
                            cb.Colors = new Color[] { gradColors[0], gradColors[1], gradColors[2] };
                            cb.Positions = new float[] { 0f, 0.55f, 1f };
                            lg.InterpolationColors = cb;
                        }
                        g.FillRectangle(lg, stripRect);
                    }
                }
                g.ResetClip();

                using (var pen = new Pen(selected ? cAccent : cBorder, selected ? 2f : 1f))
                    g.DrawPath(pen, path);

                // Badge text
                string badge = selected ? activeBadge : readyBadge;
                var badgeRect = new Rectangle(rect.X + 10, stripH + 6, rect.Width - 20, 16);
                TextRenderer.DrawText(g, badge, badgeFont, badgeRect, selected ? cAccent : cMuted, TextFormatFlags.Left | TextFormatFlags.EndEllipsis);

                // Theme title
                var titleRect = new Rectangle(rect.X + 10, stripH + 22, rect.Width - 20, rect.Bottom - (stripH + 24));
                TextRenderer.DrawText(g, ThemeTitle, titleFont, titleRect, cText, TextFormatFlags.Left | TextFormatFlags.WordBreak | TextFormatFlags.EndEllipsis);
            }
        }
    }

    // Custom dark/light-aware modern pill slider (replaces legacy white WinForms TrackBar)
    sealed class ModernSliderControl : Control
    {
        public event EventHandler ValueChanged;

        int minVal = 0, maxVal = 100, curVal = 0;
        Color cBg = Color.FromArgb(26, 29, 35);
        Color cTrack = Color.FromArgb(56, 62, 74);
        Color cAccent = Color.FromArgb(56, 189, 248);
        Color cThumb = Color.White;
        bool dragging, hovered;

        public ModernSliderControl()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            Cursor = Cursors.Hand;
            TabStop = true;
        }

        public int Minimum
        {
            get { return minVal; }
            set { minVal = value; if (curVal < minVal) curVal = minVal; Invalidate(); }
        }

        public int Maximum
        {
            get { return maxVal; }
            set { maxVal = Math.Max(minVal + 1, value); if (curVal > maxVal) curVal = maxVal; Invalidate(); }
        }

        public int Value
        {
            get { return curVal; }
            set
            {
                int clamped = Math.Max(minVal, Math.Min(maxVal, value));
                if (clamped == curVal) return;
                curVal = clamped;
                Invalidate();
            }
        }

        public void SetPalette(Color bg, Color track, Color accent, Color thumbRing)
        {
            cBg = bg;
            cTrack = track;
            cAccent = accent;
            cThumb = thumbRing;
            BackColor = bg;
            Invalidate();
        }

        void SetFromX(int x)
        {
            int pad = 9;
            int trackW = Math.Max(1, Width - pad * 2);
            double t = (double)(x - pad) / trackW;
            if (t < 0) t = 0;
            if (t > 1) t = 1;
            int next = minVal + (int)Math.Round(t * (maxVal - minVal));
            if (next != curVal)
            {
                curVal = next;
                Invalidate();
                var cb = ValueChanged;
                if (cb != null) cb(this, EventArgs.Empty);
            }
        }

        protected override void OnMouseEnter(EventArgs e) { hovered = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { hovered = false; Invalidate(); base.OnMouseLeave(e); }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            if (e.Button == MouseButtons.Left)
            {
                Focus();
                dragging = true;
                Capture = true;
                SetFromX(e.X);
            }
            base.OnMouseDown(e);
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            if (dragging) SetFromX(e.X);
            base.OnMouseMove(e);
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            if (dragging && e.Button == MouseButtons.Left)
            {
                dragging = false;
                Capture = false;
                Invalidate();
            }
            base.OnMouseUp(e);
        }

        protected override bool IsInputKey(Keys keyData)
        {
            if (keyData == Keys.Left || keyData == Keys.Right) return true;
            return base.IsInputKey(keyData);
        }

        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (e.KeyCode == Keys.Left || e.KeyCode == Keys.Right)
            {
                int delta = e.KeyCode == Keys.Right ? 1 : -1;
                int next = Math.Max(minVal, Math.Min(maxVal, curVal + delta));
                if (next != curVal)
                {
                    curVal = next;
                    Invalidate();
                    var cb = ValueChanged;
                    if (cb != null) cb(this, EventArgs.Empty);
                }
                e.Handled = true;
            }
            base.OnKeyDown(e);
        }

        static GraphicsPath PillPath(float x, float y, float w, float h)
        {
            var p = new GraphicsPath();
            if (w <= h)
            {
                p.AddEllipse(x, y, Math.Max(1f, w), Math.Max(1f, h));
                return p;
            }
            p.AddArc(x, y, h, h, 90, 180);
            p.AddArc(x + w - h, y, h, h, 270, 180);
            p.CloseFigure();
            return p;
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            var g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(cBg);

            float pad = 9f;
            float trackW = Math.Max(1f, Width - pad * 2f);
            float cy = Height * 0.5f;
            float trackH = 5f;
            float ratio = (maxVal > minVal) ? (float)(curVal - minVal) / (maxVal - minVal) : 0f;
            float thumbX = pad + ratio * trackW;

            // Background pill track
            using (var bgPath = PillPath(pad, cy - trackH * 0.5f, trackW, trackH))
            using (var br = new SolidBrush(cTrack))
                g.FillPath(br, bgPath);

            // Active accent fill
            float fillW = Math.Max(trackH, thumbX - pad);
            if (ratio > 0.001f)
            {
                using (var fillPath = PillPath(pad, cy - trackH * 0.5f, fillW, trackH))
                using (var br = new SolidBrush(cAccent))
                    g.FillPath(br, fillPath);
            }

            // Hover / drag soft glow halo
            if (hovered || dragging)
            {
                float haloR = dragging ? 10.5f : 9.5f;
                using (var haloBr = new SolidBrush(Color.FromArgb(dragging ? 70 : 42, cAccent)))
                    g.FillEllipse(haloBr, thumbX - haloR, cy - haloR, haloR * 2f, haloR * 2f);
            }

            // Circular thumb
            float r = dragging ? 7f : 6.2f;
            using (var thumbBr = new SolidBrush(cThumb))
                g.FillEllipse(thumbBr, thumbX - r, cy - r, r * 2f, r * 2f);
            using (var ringPen = new Pen(cAccent, 2f))
                g.DrawEllipse(ringPen, thumbX - r, cy - r, r * 2f, r * 2f);
        }
    }
}
