package com.diyardesk.tablet

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class LanSyncClientTest {

    @Test
    fun allowsOnlyPrivateLanHosts() {
        assertTrue(LanSyncClient.isPrivateLanHost("192.168.1.42"))
        assertTrue(LanSyncClient.isPrivateLanHost("10.0.0.5"))
        assertTrue(LanSyncClient.isPrivateLanHost("172.16.0.1"))
        assertTrue(LanSyncClient.isPrivateLanHost("172.31.255.254"))
        assertTrue(LanSyncClient.isPrivateLanHost("127.0.0.1"))

        assertFalse(LanSyncClient.isPrivateLanHost("8.8.8.8"))
        assertFalse(LanSyncClient.isPrivateLanHost("172.32.0.1"))
        assertFalse(LanSyncClient.isPrivateLanHost("192.169.1.1"))
        assertFalse(LanSyncClient.isPrivateLanHost("example.com"))
        assertFalse(LanSyncClient.isPrivateLanHost("999.1.1.1"))
    }

    @Test
    fun parsesPairingUriAndHostPort() {
        val fromUri = LanSyncClient.parsePairingInput("diyardesk://pair?host=192.168.1.12&port=51234&pin=4821")
        assertNotNull(fromUri)
        assertEquals("192.168.1.12", fromUri!!.host)
        assertEquals(51234, fromUri.port)
        assertEquals("4821", fromUri.pin)

        val fromHostPort = LanSyncClient.parsePairingInput("10.0.1.9:55100", "9012")
        assertNotNull(fromHostPort)
        assertEquals("10.0.1.9", fromHostPort!!.host)
        assertEquals(55100, fromHostPort.port)
        assertEquals("9012", fromHostPort.pin)

        assertNull(LanSyncClient.parsePairingInput("diyardesk://pair?host=8.8.8.8&port=51234&pin=4821"))
        assertNull(LanSyncClient.parsePairingInput("192.168.1.12:80", "4821"))
        assertNull(LanSyncClient.parsePairingInput("192.168.1.12:51234", "12"))
    }
}
