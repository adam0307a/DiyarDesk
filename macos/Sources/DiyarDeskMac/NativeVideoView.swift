import Cocoa
import AVFoundation
import DiyarDeskCore

/// Zero-WKWebView hardware-accelerated video wallpaper view powered by AVQueuePlayer + AVPlayerLooper.
final class NativeVideoView: NSView {
    private let player: AVQueuePlayer
    private var looper: AVPlayerLooper?
    private let playerLayer: AVPlayerLayer
    private let dimLayer = CALayer()
    private var statusObservation: NSKeyValueObservation?
    private var currentURL: URL
    private var speed: Float = 1.0
    private var paused: Bool = false
    private var tornDown: Bool = false

    var onFailure: ((String) -> Void)?

    init(frame: NSRect, videoURL: URL, speed: Double, brightness: Double) {
        self.currentURL = videoURL
        self.player = AVQueuePlayer()
        self.playerLayer = AVPlayerLayer(player: self.player)
        super.init(frame: frame)

        self.wantsLayer = true
        self.layer?.backgroundColor = NSColor.black.cgColor

        self.player.isMuted = true
        self.player.volume = 0.0
        self.player.automaticallyWaitsToMinimizeStalling = false
        self.player.actionAtItemEnd = .none

        self.playerLayer.videoGravity = .resizeAspectFill
        self.playerLayer.backgroundColor = NSColor.black.cgColor
        self.layer?.addSublayer(self.playerLayer)

        self.dimLayer.backgroundColor = NSColor.black.cgColor
        self.dimLayer.opacity = 0.0
        self.layer?.addSublayer(self.dimLayer)

        loadVideo(url: videoURL)
        updateParams(speed: speed, brightness: brightness)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func matchesURL(_ url: URL) -> Bool {
        return currentURL == url
    }

    private func loadVideo(url: URL) {
        statusObservation?.invalidate()
        looper?.disableLooping()
        player.removeAllItems()
        currentURL = url

        let asset = AVURLAsset(url: url, options: [AVURLAssetPreferPreciseDurationAndTimingKey: true])
        let templateItem = AVPlayerItem(asset: asset)
        looper = AVPlayerLooper(player: player, templateItem: templateItem)

        statusObservation = templateItem.observe(\.status, options: [.new]) { [weak self] item, _ in
            guard let self = self, !self.tornDown else { return }
            if item.status == .failed {
                let err = item.error?.localizedDescription ?? "AVPlayerItem failed"
                DispatchQueue.main.async {
                    guard !self.tornDown else { return }
                    self.onFailure?(err)
                }
            }
        }

        if !paused {
            player.play()
            player.rate = speed
        }
    }

    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        playerLayer.frame = bounds
        dimLayer.frame = bounds
        CATransaction.commit()
    }

    func updateParams(speed newSpeed: Double, brightness newBrightness: Double) {
        self.speed = NativeGlSupport.clampVideoSpeed(newSpeed)
        if !paused {
            player.rate = self.speed
        }
        let style = NativeGlSupport.brightnessOverlayStyle(newBrightness)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        dimLayer.backgroundColor = (style.isWhite ? NSColor.white : NSColor.black).cgColor
        dimLayer.opacity = Float(style.alpha)
        CATransaction.commit()
    }

    func setPaused(_ pause: Bool) {
        guard !tornDown, paused != pause else { return }
        paused = pause
        if pause {
            player.pause()
        } else {
            player.play()
            player.rate = speed
        }
    }

    func teardown() {
        guard !tornDown else { return }
        tornDown = true
        statusObservation?.invalidate()
        statusObservation = nil
        player.pause()
        looper?.disableLooping()
        looper = nil
        player.removeAllItems()
        playerLayer.player = nil
        playerLayer.removeFromSuperlayer()
        dimLayer.removeFromSuperlayer()
        removeFromSuperview()
    }

    deinit {
        if !tornDown {
            statusObservation?.invalidate()
            looper?.disableLooping()
            player.pause()
        }
    }
}
