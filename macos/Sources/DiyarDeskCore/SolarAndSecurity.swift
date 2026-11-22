import Foundation

public enum DiyarSecurity {
    public static let scheme = "diyardesk"
    public static let appHost = "app"
    public static let customHost = "custom"

    public static let cspHeader: String =
        "default-src 'self' diyardesk://app diyardesk://custom data: blob:; " +
        "script-src 'self' diyardesk://app diyardesk://custom 'unsafe-inline' 'unsafe-eval' data: blob:; " +
        "style-src 'self' diyardesk://app diyardesk://custom 'unsafe-inline' data: blob:; " +
        "img-src 'self' diyardesk://app diyardesk://custom data: blob:; " +
        "media-src 'self' diyardesk://app diyardesk://custom data: blob:; " +
        "font-src 'self' diyardesk://app diyardesk://custom data: blob:; " +
        "connect-src 'self' diyardesk://app diyardesk://custom data: blob:; " +
        "frame-src 'self' diyardesk://app diyardesk://custom data: blob:; " +
        "object-src 'none'; base-uri 'self'; form-action 'none';"

    public static let archiveAllowedExts: Set<String> = [
        "html", "htm", "css", "js", "mjs", "json",
        "glsl", "vert", "frag", "wgsl", "wasm",
        "txt", "md", "xml", "csv", "map",
        "png", "jpg", "jpeg", "webp", "gif", "svg", "avif", "bmp", "ico",
        "mp4", "webm", "mkv", "mov", "ogv",
        "mp3", "wav", "ogg", "opus", "m4a", "aac", "flac",
        "woff", "woff2", "ttf", "otf", "eot",
        "obj", "mtl", "gltf", "glb", "bin", "hdr", "exr", "ktx", "ktx2", "dds", "ply", "stl", "fbx"
    ]

    public static let archiveAllowedBasenames: Set<String> = [
        "license", "notice", "readme", "copying", "authors", "changelog"
    ]

    public static let imageExts: Set<String> = ["png", "jpg", "jpeg", "webp", "gif", "svg", "avif", "bmp"]
    public static let videoExts: Set<String> = ["mp4", "webm", "mkv", "mov"]
    public static let shaderExts: Set<String> = ["glsl", "frag"]
    public static let htmlExts: Set<String> = ["html", "htm"]
    public static let archiveExts: Set<String> = ["zip", "deskland"]

    public static let maxArchiveBytes: Int64 = 150 * 1024 * 1024
    public static let maxArchiveFiles: Int = 2000

    public static func isAllowedLocalURL(_ url: URL?) -> Bool {
        guard let url = url, let s = url.scheme?.lowercased() else { return false }
        if s == "about" || s == "data" || s == "blob" { return true }
        if s == scheme {
            let h = (url.host ?? "").lowercased()
            return h == appHost || h == customHost
        }
        return false
    }

    public static func isAllowedArchiveMember(_ fileName: String) -> Bool {
        let base = (fileName as NSString).lastPathComponent
        if base.isEmpty { return false }
        let ext = (base as NSString).pathExtension.lowercased()
        if !ext.isEmpty {
            return archiveAllowedExts.contains(ext)
        }
        return archiveAllowedBasenames.contains(base.lowercased())
    }

    public static func isSafeRelativePath(_ rawPath: String) -> Bool {
        let normalized = rawPath.replacingOccurrences(of: "\\", with: "/")
        if normalized.hasPrefix("/") || normalized.contains(":") { return false }
        let parts = normalized.split(separator: "/")
        for part in parts {
            if part == ".." { return false }
        }
        return !parts.isEmpty
    }

    public static func sanitizeColor(_ raw: Any?) -> String {
        let s = (raw as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let sixRegex = try? NSRegularExpression(pattern: "^#[0-9a-fA-F]{6}$")
        let threeRegex = try? NSRegularExpression(pattern: "^#[0-9a-fA-F]{3}$")
        let range = NSRange(location: 0, length: s.utf16.count)
        if sixRegex?.firstMatch(in: s, options: [], range: range) != nil {
            return s.lowercased()
        }
        if threeRegex?.firstMatch(in: s, options: [], range: range) != nil {
            let chars = Array(s.lowercased())
            return "#\(chars[1])\(chars[1])\(chars[2])\(chars[2])\(chars[3])\(chars[3])"
        }
        return "#4a90e2"
    }

    public static func mimeType(forPath path: String) -> String {
        switch (path as NSString).pathExtension.lowercased() {
        case "html", "htm": return "text/html; charset=utf-8"
        case "js", "mjs": return "application/javascript; charset=utf-8"
        case "css": return "text/css; charset=utf-8"
        case "json": return "application/json; charset=utf-8"
        case "svg": return "image/svg+xml"
        case "png": return "image/png"
        case "jpg", "jpeg": return "image/jpeg"
        case "webp": return "image/webp"
        case "gif": return "image/gif"
        case "avif": return "image/avif"
        case "bmp": return "image/bmp"
        case "ico": return "image/x-icon"
        case "mp3": return "audio/mpeg"
        case "ogg": return "audio/ogg"
        case "wav": return "audio/wav"
        case "mp4": return "video/mp4"
        case "webm": return "video/webm"
        case "mov": return "video/quicktime"
        case "mkv": return "video/x-matroska"
        case "woff2": return "font/woff2"
        case "woff": return "font/woff"
        case "ttf": return "font/ttf"
        case "wasm": return "application/wasm"
        case "glsl", "frag", "vert": return "text/plain; charset=utf-8"
        default: return "application/octet-stream"
        }
    }
}

public enum SolarHelper {
    public static func computeEnvironment(hour: Double? = nil, darkMode: Bool = false, solarSync: Bool = false) -> [String: Any] {
        let h: Double
        if let provided = hour {
            h = provided.truncatingRemainder(dividingBy: 24.0)
        } else {
            let comps = Calendar.current.dateComponents([.hour, .minute], from: Date())
            h = Double(comps.hour ?? 12) + Double(comps.minute ?? 0) / 60.0
        }
        let normH = h < 0 ? h + 24.0 : h
        let phase: String
        if normH >= 5.0 && normH < 8.0 { phase = "dawn" }
        else if normH >= 8.0 && normH < 17.0 { phase = "day" }
        else if normH >= 17.0 && normH < 20.0 { phase = "dusk" }
        else { phase = "night" }
        let rawDl = sin(((normH - 6.0) / 12.0) * Double.pi)
        let daylight = max(0.0, min(1.0, rawDl))
        return [
            "t": "environment",
            "hour": (normH * 100.0).rounded() / 100.0,
            "phase": phase,
            "daylight": (daylight * 1000.0).rounded() / 1000.0,
            "darkMode": darkMode,
            "solarSync": solarSync
        ]
    }
}
