package com.diyardesk.tablet

import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URI
import java.net.URL

/**
 * Cloud-free local Wi-Fi (LAN) companion client.
 * Connects strictly to user-entered RFC1918 private LAN addresses with a 4-digit PIN.
 */
object LanSyncClient {

    data class PairingTarget(val host: String, val port: Int, val pin: String)

    private val IPV4_REGEX = Regex("^(\\d{1,3})\\.(\\d{1,3})\\.(\\d{1,3})\\.(\\d{1,3})$")
    private val PIN_REGEX = Regex("^\\d{4}$")

    /** Returns true only for RFC1918 private LAN IPv4 addresses (or loopback for local tests). */
    fun isPrivateLanHost(host: String): Boolean {
        val m = IPV4_REGEX.matchEntire(host.trim()) ?: return false
        val a = m.groupValues[1].toIntOrNull() ?: return false
        val b = m.groupValues[2].toIntOrNull() ?: return false
        val c = m.groupValues[3].toIntOrNull() ?: return false
        val d = m.groupValues[4].toIntOrNull() ?: return false
        if (a !in 0..255 || b !in 0..255 || c !in 0..255 || d !in 0..255) return false
        return when {
            a == 10 -> true
            a == 172 && b in 16..31 -> true
            a == 192 && b == 168 -> true
            a == 127 && b == 0 && c == 0 && d == 1 -> true
            else -> false
        }
    }

    fun parsePairingInput(rawInput: String, fallbackPin: String = ""): PairingTarget? {
        val trimmed = rawInput.trim()
        if (trimmed.isEmpty()) return null
        if (trimmed.startsWith("diyardesk://", ignoreCase = true)) {
            val uri = try { URI(trimmed) } catch (_: Exception) { return null }
            val params = (uri.rawQuery ?: "").split('&')
                .mapNotNull { kv ->
                    val eq = kv.indexOf('=')
                    if (eq <= 0) null else kv.substring(0, eq) to kv.substring(eq + 1)
                }.toMap()
            val host = params["host"]?.trim() ?: return null
            val port = params["port"]?.toIntOrNull() ?: return null
            val pin = (params["pin"] ?: fallbackPin).trim()
            return validateTarget(host, port, pin)
        }
        val colon = trimmed.lastIndexOf(':')
        if (colon <= 0 || colon >= trimmed.length - 1) return null
        val host = trimmed.substring(0, colon).trim()
        val port = trimmed.substring(colon + 1).trim().toIntOrNull() ?: return null
        return validateTarget(host, port, fallbackPin.trim())
    }

    private fun validateTarget(host: String, port: Int, pin: String): PairingTarget? {
        if (!isPrivateLanHost(host)) return null
        if (port !in 1024..65535) return null
        if (!PIN_REGEX.matches(pin)) return null
        return PairingTarget(host, port, pin)
    }

    fun fetchPairInfo(target: PairingTarget): JSONObject {
        require(isPrivateLanHost(target.host)) { "Only private LAN IP addresses are allowed" }
        val url = URL("http://${target.host}:${target.port}/pair/info")
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = "GET"
            connectTimeout = 4000
            readTimeout = 8000
            setRequestProperty("X-Diyar-Pin", target.pin)
        }
        try {
            val code = conn.responseCode
            if (code == 403) throw SecurityException("Invalid pairing PIN")
            if (code == 410) throw IllegalStateException("Pairing session expired on desktop")
            if (code != 200) throw IllegalStateException("Desktop returned HTTP $code")
            val body = conn.inputStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
            return JSONObject(body)
        } finally {
            conn.disconnect()
        }
    }

    fun downloadCustomTheme(
        target: PairingTarget,
        themeId: String,
        importer: CustomThemeImporter
    ): JSONObject {
        require(isPrivateLanHost(target.host)) { "Only private LAN IP addresses are allowed" }
        require(themeId.startsWith("custom-") && !themeId.contains('/') && !themeId.contains("..")) {
            "Invalid custom theme id"
        }
        val url = URL("http://${target.host}:${target.port}/pair/theme/$themeId.deskland")
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = "GET"
            connectTimeout = 4000
            readTimeout = 30000
            setRequestProperty("X-Diyar-Pin", target.pin)
        }
        try {
            val code = conn.responseCode
            if (code == 403) throw SecurityException("Invalid pairing PIN")
            if (code != 200) throw IllegalStateException("Theme download failed (HTTP $code)")
            return conn.inputStream.use { stream ->
                importer.importStream(stream, "$themeId.deskland")
            }
        } finally {
            conn.disconnect()
        }
    }

    fun pushHandoff(
        target: PairingTarget,
        activeTheme: String,
        params: JSONObject?,
        impulseState: JSONObject?
    ): JSONObject {
        require(isPrivateLanHost(target.host)) { "Only private LAN IP addresses are allowed" }
        val payload = JSONObject().apply {
            put("activeTheme", activeTheme)
            if (params != null) put("params", params)
            if (impulseState != null) put("impulseState", impulseState)
        }
        val bytes = payload.toString().toByteArray(Charsets.UTF_8)
        require(bytes.size <= 262_144) { "Handoff payload too large" }
        val url = URL("http://${target.host}:${target.port}/pair/handoff")
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = "POST"
            doOutput = true
            connectTimeout = 4000
            readTimeout = 8000
            setRequestProperty("X-Diyar-Pin", target.pin)
            setRequestProperty("Content-Type", "application/json; charset=utf-8")
            setFixedLengthStreamingMode(bytes.size)
        }
        try {
            conn.outputStream.use { it.write(bytes) }
            val code = conn.responseCode
            if (code == 403) throw SecurityException("Invalid pairing PIN")
            if (code == 410) throw IllegalStateException("Pairing session expired on desktop")
            if (code != 200) throw IllegalStateException("Desktop returned HTTP $code")
            val body = conn.inputStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
            return JSONObject(body)
        } finally {
            conn.disconnect()
        }
    }
}
