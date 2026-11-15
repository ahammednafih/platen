package com.kanamlabs.platen

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeoutOrNull
import java.net.Inet4Address

data class Found(val name: String, val base: String)

suspend fun findScanner(context: Context, timeoutMs: Long): Found? {
    val nsd = context.getSystemService(NsdManager::class.java)
    val found = CompletableDeferred<Found>()
    val listener = object : NsdManager.DiscoveryListener {
        override fun onServiceFound(info: NsdServiceInfo) {
            resolve(nsd, info) { device -> found.complete(device) }
        }

        override fun onStartDiscoveryFailed(type: String, code: Int) {
            found.completeExceptionally(IllegalStateException("can't search the network (error $code)"))
        }

        override fun onDiscoveryStarted(type: String) {}

        override fun onDiscoveryStopped(type: String) {}

        override fun onServiceLost(info: NsdServiceInfo) {}

        override fun onStopDiscoveryFailed(type: String, code: Int) {}
    }
    nsd.discoverServices("_uscan._tcp", NsdManager.PROTOCOL_DNS_SD, listener)
    return try {
        withTimeoutOrNull(timeoutMs) { found.await() }
    } finally {
        runCatching { nsd.stopServiceDiscovery(listener) }
    }
}

@Suppress("DEPRECATION")
private fun resolve(nsd: NsdManager, service: NsdServiceInfo, done: (Found) -> Unit) {
    nsd.resolveService(service, object : NsdManager.ResolveListener {
        override fun onServiceResolved(info: NsdServiceInfo) {
            val addresses = if (Build.VERSION.SDK_INT >= 34) info.hostAddresses else listOfNotNull(info.host)
            val ip = addresses.firstOrNull { it is Inet4Address }?.hostAddress ?: return
            val rs = info.attributes["rs"]?.decodeToString()?.trim('/') ?: "eSCL"
            val name = info.attributes["ty"]?.decodeToString() ?: info.serviceName
            done(Found(name, "http://$ip:${info.port}/$rs"))
        }

        override fun onResolveFailed(info: NsdServiceInfo, code: Int) {}
    })
}

suspend fun findScanners(context: Context, timeoutMs: Long): List<Found> {
    val nsd = context.getSystemService(NsdManager::class.java)
    val found = java.util.concurrent.ConcurrentHashMap<String, Found>()
    val listener = object : NsdManager.DiscoveryListener {
        override fun onServiceFound(info: NsdServiceInfo) {
            resolve(nsd, info) { f -> found[f.base] = f }
        }

        override fun onStartDiscoveryFailed(type: String, code: Int) {}

        override fun onDiscoveryStarted(type: String) {}

        override fun onDiscoveryStopped(type: String) {}

        override fun onServiceLost(info: NsdServiceInfo) {}

        override fun onStopDiscoveryFailed(type: String, code: Int) {}
    }
    nsd.discoverServices("_uscan._tcp", NsdManager.PROTOCOL_DNS_SD, listener)
    try {
        kotlinx.coroutines.delay(timeoutMs)
    } finally {
        runCatching { nsd.stopServiceDiscovery(listener) }
    }
    return found.values.sortedBy { it.name }
}
