import XCTest
@testable import DiyarDeskCore

final class DiyarDeskMacTests: XCTestCase {
    func testSecurityPolicies() {
        XCTAssertTrue(DiyarSecurity.isAllowedLocalURL(URL(string: "diyardesk://app/settings.html")))
        XCTAssertTrue(DiyarSecurity.isAllowedLocalURL(URL(string: "diyardesk://custom/custom-demo/index.html")))
        XCTAssertFalse(DiyarSecurity.isAllowedLocalURL(URL(string: "https://example.com/tracker.js")))
        XCTAssertFalse(DiyarSecurity.isAllowedLocalURL(URL(string: "file:///etc/passwd")))

        XCTAssertTrue(DiyarSecurity.isAllowedArchiveMember("index.html"))
        XCTAssertTrue(DiyarSecurity.isAllowedArchiveMember("shader.glsl"))
        XCTAssertTrue(DiyarSecurity.isAllowedArchiveMember("LICENSE"))
        XCTAssertFalse(DiyarSecurity.isAllowedArchiveMember("dropper.sh"))
        XCTAssertFalse(DiyarSecurity.isAllowedArchiveMember("evil.dylib"))

        XCTAssertTrue(DiyarSecurity.isSafeRelativePath("assets/bg.png"))
        XCTAssertFalse(DiyarSecurity.isSafeRelativePath("../../etc/passwd"))
        XCTAssertFalse(DiyarSecurity.isSafeRelativePath("/abs/path.html"))

        XCTAssertEqual(DiyarSecurity.sanitizeColor("#0AF"), "#00aaff")
        XCTAssertEqual(DiyarSecurity.sanitizeColor("#123456"), "#123456")
        XCTAssertEqual(DiyarSecurity.sanitizeColor("bad"), "#4a90e2")
    }

    func testSolarPhaseCalculation() {
        let noon = SolarHelper.computeEnvironment(hour: 12.0, darkMode: false, solarSync: true)
        XCTAssertEqual(noon["phase"] as? String, "day")
        XCTAssertEqual(noon["daylight"] as? Double ?? 0, 1.0, accuracy: 0.01)

        let night = SolarHelper.computeEnvironment(hour: 23.0, darkMode: true, solarSync: true)
        XCTAssertEqual(night["phase"] as? String, "night")
        XCTAssertEqual(night["daylight"] as? Double ?? 1, 0.0, accuracy: 0.01)
    }

    func testThemeStoreSettingsAndShaderImport() throws {
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("diyardesk-mac-test-\(UUID().uuidString)")
        let proto = tmp.appendingPathComponent("prototype", isDirectory: true)
        try FileManager.default.createDirectory(at: proto, withIntermediateDirectories: true)
        try "{\"themes\":[{\"id\":\"papercut\",\"name\":\"Papercut\",\"page\":\"papercut.html\",\"status\":\"ready\"}]}".write(
            to: proto.appendingPathComponent("themes.json"),
            atomically: true,
            encoding: .utf8
        )
        defer { try? FileManager.default.removeItem(at: tmp) }

        let store = ThemeStore(dataDir: tmp.appendingPathComponent("data", isDirectory: true), prototypeDir: proto)
        store.settings.solarSync = true
        store.settings.windowAware = true
        store.settings.displayMode = "independent"
        store.save()

        let store2 = ThemeStore(dataDir: tmp.appendingPathComponent("data", isDirectory: true), prototypeDir: proto)
        XCTAssertTrue(store2.settings.solarSync)
        XCTAssertTrue(store2.settings.windowAware)
        XCTAssertEqual(store2.settings.displayMode, "independent")

        let fragURL = tmp.appendingPathComponent("aurora_test.frag")
        try "void mainImage(out vec4 c, in vec2 p){ c = vec4(p / u_resolution, 0.5, 1.0); }".write(
            to: fragURL,
            atomically: true,
            encoding: .utf8
        )
        let entry = try store2.importFile(at: fragURL)
        let id = try XCTUnwrap(entry["id"] as? String)
        XCTAssertTrue(id.hasPrefix("custom-"))
        let indexPath = store2.customThemesDir.appendingPathComponent(id).appendingPathComponent("index.html")
        let html = try String(contentsOf: indexPath, encoding: .utf8)
        XCTAssertTrue(html.contains("uniform vec4 u_icons[32];"))
        XCTAssertTrue(store2.removeCustomTheme(id: id))
    }
}
