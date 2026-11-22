using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
#if HAS_WINRT_SMTC
using Windows.Foundation;
using Windows.Media.Control;
using Windows.Storage.Streams;
#endif

// 100% local Windows System Media Transport Controls (SMTC) bridge:
// Reads currently playing media (Spotify, Brave/Chrome YouTube, VLC, Apple Music, etc.)
// and forwards Play/Pause/Next/Prev/Seek commands directly to the active system session.
// Zero network access, zero external packages.
sealed class MediaBridge : IDisposable
{
    public event Action<string> StateChanged;

#if HAS_WINRT_SMTC
    GlobalSystemMediaTransportControlsSessionManager manager;
    Timer pollTimer;
    int polling;
    string lastPayload = "";
    string lastArtKey = "";
    string lastArtDataUrl = "";
#endif
    volatile bool disposed;

    [DllImport("user32.dll")]
    static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);

    const byte VK_MEDIA_NEXT_TRACK = 0xB0;
    const byte VK_MEDIA_PREV_TRACK = 0xB1;
    const byte VK_MEDIA_PLAY_PAUSE = 0xB3;
    const uint KEYEVENTF_KEYUP = 0x0002;

    static void TapMediaKey(byte vk)
    {
        try
        {
            keybd_event(vk, 0, 0, UIntPtr.Zero);
            keybd_event(vk, 0, KEYEVENTF_KEYUP, UIntPtr.Zero);
        }
        catch { }
    }

#if HAS_WINRT_SMTC
    static Task<T> AwaitOp<T>(IAsyncOperation<T> op)
    {
        var tcs = new TaskCompletionSource<T>();
        if (op == null) { tcs.TrySetResult(default(T)); return tcs.Task; }
        if (op.Status == AsyncStatus.Completed)
        {
            try { tcs.TrySetResult(op.GetResults()); }
            catch (Exception ex) { tcs.TrySetException(ex); }
            return tcs.Task;
        }
        op.Completed = delegate(IAsyncOperation<T> info, AsyncStatus status)
        {
            if (status == AsyncStatus.Completed)
            {
                try { tcs.TrySetResult(info.GetResults()); }
                catch (Exception ex) { tcs.TrySetException(ex); }
            }
            else if (status == AsyncStatus.Canceled) tcs.TrySetCanceled();
            else tcs.TrySetException(info.ErrorCode ?? new Exception("WinRT async error"));
        };
        return tcs.Task;
    }

    public void Start()
    {
        if (disposed || pollTimer != null) return;
        Task.Run(delegate { InitAsync(); });
        pollTimer = new Timer(delegate { PollTick(); }, null, 600, 1000);
    }

    async void InitAsync()
    {
        try
        {
            manager = await AwaitOp(GlobalSystemMediaTransportControlsSessionManager.RequestAsync());
            if (!disposed) PollTick();
        }
        catch { }
    }

    void PollTick()
    {
        if (disposed || Interlocked.Exchange(ref polling, 1) != 0) return;
        Task.Run(async delegate
        {
            try { await ReadAndEmitAsync(); }
            catch { }
            finally { Interlocked.Exchange(ref polling, 0); }
        });
    }

    public void RequestRefresh()
    {
        lastPayload = "";
        PollTick();
    }

    async Task ReadAndEmitAsync()
    {
        if (disposed) return;
        if (manager == null)
        {
            try { manager = await AwaitOp(GlobalSystemMediaTransportControlsSessionManager.RequestAsync()); }
            catch { return; }
        }
        if (manager == null || disposed) return;

        GlobalSystemMediaTransportControlsSession session = null;
        try { session = manager.GetCurrentSession(); } catch { }

        if (session == null)
        {
            Emit(Json.Stringify(Json.Obj("t", "mediaState", "active", false)));
            return;
        }

        string title = "";
        string artist = "";
        string album = "";
        string artUrl = "";
        bool playing = false;
        double posSec = 0;
        double durSec = 0;

        try
        {
            var pb = session.GetPlaybackInfo();
            if (pb != null)
            {
                playing = pb.PlaybackStatus == GlobalSystemMediaTransportControlsSessionPlaybackStatus.Playing;
            }
        }
        catch { }

        try
        {
            var tl = session.GetTimelineProperties();
            if (tl != null)
            {
                double startS = tl.StartTime.TotalSeconds;
                double endS = tl.EndTime.TotalSeconds;
                double rawPos = tl.Position.TotalSeconds;
                durSec = Math.Max(0, endS - startS);
                posSec = Math.Max(0, rawPos - startS);
                if (playing && tl.LastUpdatedTime != DateTimeOffset.MinValue)
                {
                    double elapsed = (DateTimeOffset.UtcNow - tl.LastUpdatedTime.ToUniversalTime()).TotalSeconds;
                    if (elapsed > 0 && elapsed < 3600) posSec += elapsed;
                }
                if (durSec > 0 && posSec > durSec) posSec = durSec;
            }
        }
        catch { }

        try
        {
            var props = await AwaitOp(session.TryGetMediaPropertiesAsync());
            if (props != null)
            {
                title = (props.Title ?? "").Trim();
                artist = (props.Artist ?? "").Trim();
                album = (props.AlbumTitle ?? "").Trim();
                string artKey = title + "|" + artist + "|" + album;
                if (artKey == lastArtKey && !string.IsNullOrEmpty(lastArtDataUrl))
                {
                    artUrl = lastArtDataUrl;
                }
                else
                {
                    lastArtKey = artKey;
                    lastArtDataUrl = "";
                    if (props.Thumbnail != null)
                    {
                        artUrl = await ReadThumbnailDataUrlAsync(props.Thumbnail);
                        lastArtDataUrl = artUrl;
                    }
                }
            }
        }
        catch { }

        bool active = !string.IsNullOrEmpty(title) || !string.IsNullOrEmpty(artist) || playing;
        string json = Json.Stringify(Json.Obj(
            "t", "mediaState",
            "active", active,
            "playing", playing,
            "title", title,
            "artist", artist,
            "album", album,
            "pos", Math.Round(posSec, 1),
            "dur", Math.Round(durSec, 1),
            "art", artUrl
        ));
        Emit(json);
    }

    static async Task<string> ReadThumbnailDataUrlAsync(IRandomAccessStreamReference thumbRef)
    {
        try
        {
            using (var stream = await AwaitOp(thumbRef.OpenReadAsync()))
            {
                if (stream == null || stream.Size == 0 || stream.Size > 512 * 1024) return "";
                uint size = (uint)stream.Size;
                byte[] bytes = new byte[size];
                using (var reader = new DataReader(stream))
                {
                    await AwaitOp(reader.LoadAsync(size));
                    reader.ReadBytes(bytes);
                }
                string mime = "image/jpeg";
                if (bytes.Length >= 4 && bytes[0] == 0x89 && bytes[1] == 0x50) mime = "image/png";
                else if (bytes.Length >= 4 && bytes[0] == 0x52 && bytes[1] == 0x49) mime = "image/webp";
                return "data:" + mime + ";base64," + Convert.ToBase64String(bytes);
            }
        }
        catch { return ""; }
    }

    void Emit(string payload)
    {
        if (disposed || payload == lastPayload) return;
        lastPayload = payload;
        var cb = StateChanged;
        if (cb != null)
        {
            try { cb(payload); } catch { }
        }
    }

    public void HandleCommand(string action, double seekSec)
    {
        if (disposed || string.IsNullOrEmpty(action)) return;
        Task.Run(async delegate
        {
            try
            {
                if (manager == null)
                    manager = await AwaitOp(GlobalSystemMediaTransportControlsSessionManager.RequestAsync());
                var session = manager != null ? manager.GetCurrentSession() : null;
                if (action == "toggle")
                {
                    if (session != null) await AwaitOp(session.TryTogglePlayPauseAsync());
                    else TapMediaKey(VK_MEDIA_PLAY_PAUSE);
                }
                else if (action == "next")
                {
                    if (session != null) await AwaitOp(session.TrySkipNextAsync());
                    else TapMediaKey(VK_MEDIA_NEXT_TRACK);
                }
                else if (action == "prev")
                {
                    if (session != null) await AwaitOp(session.TrySkipPreviousAsync());
                    else TapMediaKey(VK_MEDIA_PREV_TRACK);
                }
                else if (action == "seek" && session != null && seekSec >= 0)
                {
                    var tl = session.GetTimelineProperties();
                    long baseTicks = tl != null ? tl.StartTime.Ticks : 0;
                    await AwaitOp(session.TryChangePlaybackPositionAsync(baseTicks + TimeSpan.FromSeconds(seekSec).Ticks));
                }
                await Task.Delay(180);
                lastPayload = "";
                await ReadAndEmitAsync();
            }
            catch
            {
                if (action == "toggle") TapMediaKey(VK_MEDIA_PLAY_PAUSE);
                else if (action == "next") TapMediaKey(VK_MEDIA_NEXT_TRACK);
                else if (action == "prev") TapMediaKey(VK_MEDIA_PREV_TRACK);
            }
        });
    }

    public void Dispose()
    {
        disposed = true;
        if (pollTimer != null)
        {
            try { pollTimer.Dispose(); } catch { }
            pollTimer = null;
        }
    }
#else
    public void Start() { }
    public void RequestRefresh() { }
    public void HandleCommand(string action, double seekSec)
    {
        if (disposed || string.IsNullOrEmpty(action)) return;
        if (action == "toggle") TapMediaKey(VK_MEDIA_PLAY_PAUSE);
        else if (action == "next") TapMediaKey(VK_MEDIA_NEXT_TRACK);
        else if (action == "prev") TapMediaKey(VK_MEDIA_PREV_TRACK);
    }
    public void Dispose() { disposed = true; }
#endif
}
