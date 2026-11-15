package com.kanamlabs.platen

import android.content.Context
import android.print.PageRange
import android.print.PrintAttributes
import android.print.PrinterCapabilitiesInfo
import android.print.PrinterId
import android.print.PrinterInfo
import android.printservice.PrintJob
import android.printservice.PrintService
import android.printservice.PrinterDiscoverySession
import com.kanamlabs.platen.core.PlatenException
import com.kanamlabs.platen.core.PrintProgress
import com.kanamlabs.platen.core.Quality
import com.kanamlabs.platen.core.Scale
import com.kanamlabs.platen.core.Subset
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import com.kanamlabs.platen.core.PrintJob as CoreJob
import com.kanamlabs.platen.core.PrintOpts as CoreOpts

class PlatenPrintService : PrintService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val platen get() = (application as PlatenApp).platen

    override fun onCreatePrinterDiscoverySession(): PrinterDiscoverySession = Session()

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    override fun onRequestCancelPrintJob(job: PrintJob) {
        platen.stopSending()
        job.cancel()
    }

    override fun onPrintJobQueued(job: PrintJob) {
        val base = job.info.printerId?.localId
        if (base == null) {
            job.fail("No printer chosen")
            return
        }
        job.start()
        scope.launch {
            val result = withContext(Dispatchers.IO) {
                runCatching {
                    val pdf = File(cacheDir, "print-service/${System.nanoTime()}.pdf").apply { parentFile?.mkdirs() }
                    job.document.data?.use { fd -> android.os.ParcelFileDescriptor.AutoCloseInputStream(fd).use { input -> pdf.outputStream().use { input.copyTo(it) } } }
                        ?: error("The app sent nothing to print")
                    val attrs = job.info.attributes
                    val caps = platen.printCapsOn(base)
                    val media = attrs.mediaSize?.id?.takeIf { it in caps.media } ?: caps.mediaDefault
                    val opts = CoreOpts(
                        copies = job.info.copies.coerceAtLeast(1).toUInt(),
                        color = attrs.colorMode != PrintAttributes.COLOR_MODE_MONOCHROME,
                        media = media,
                        scaling = "fit",
                        quality = Quality.NORMAL,
                        mediaType = null,
                        borderless = false,
                    )
                    val coreJob = CoreJob(listOf(pdf.path), ranges(job.info.pages), Subset.ALL, false, 1u, Scale.Fit, true, false, opts)
                    platen.printOn(base, coreJob, object : PrintProgress {
                        override fun progress(done: UInt, total: UInt) {
                            scope.launch { if (!job.isCancelled) job.setProgress(done.toFloat() / total.toFloat().coerceAtLeast(1f)) }
                        }
                    })
                    pdf.delete()
                }
            }
            when {
                job.isCancelled -> Unit
                result.isSuccess -> job.complete()
                else -> job.fail((result.exceptionOrNull() as? PlatenException)?.message ?: result.exceptionOrNull()?.message ?: "Printing failed")
            }
        }
    }

    private fun ranges(pages: Array<PageRange>?): String {
        if (pages == null || pages.any { it == PageRange.ALL_PAGES }) return ""
        return pages.joinToString(", ") { if (it.start == it.end) "${it.start + 1}" else "${it.start + 1}-${it.end + 1}" }
    }

    private inner class Session : PrinterDiscoverySession() {
        override fun onStartPrinterDiscovery(priorityList: MutableList<PrinterId>) {
            val saved = savedPrinters(this@PlatenPrintService)
            addPrinters(saved.map { info(it.base, it.label, null) })
            if (saved.isEmpty()) {
                scope.launch {
                    val found = findScanners(this@PlatenPrintService, 5_000)
                    addPrinters(found.map { info(it.base, it.name, null) })
                }
            }
        }

        override fun onStartPrinterStateTracking(printerId: PrinterId) {
            val base = printerId.localId
            val label = printers.firstOrNull { it.id == printerId }?.name ?: base
            scope.launch {
                val caps = withContext(Dispatchers.IO) { runCatching { platen.printCapsOn(base) }.getOrNull() }
                addPrinters(listOf(info(base, label, caps)))
            }
        }

        override fun onStopPrinterDiscovery() {}

        override fun onValidatePrinters(printerIds: MutableList<PrinterId>) {}

        override fun onStopPrinterStateTracking(printerId: PrinterId) {}

        override fun onDestroy() {}

        private fun info(base: String, label: String, caps: com.kanamlabs.platen.core.PrintCaps?): PrinterInfo {
            val id = generatePrinterId(base)
            val builder = PrinterInfo.Builder(id, label, PrinterInfo.STATUS_IDLE)
            if (caps != null) {
                val c = PrinterCapabilitiesInfo.Builder(id)
                caps.media.filter { !it.startsWith("custom_") }.mapNotNull { kw -> mediaSize(kw) }.forEach { m -> c.addMediaSize(m, m.id == caps.mediaDefault) }
                c.addResolution(PrintAttributes.Resolution("300", "300 dpi", 300, 300), true)
                c.setColorModes(
                    PrintAttributes.COLOR_MODE_MONOCHROME or (if (caps.color) PrintAttributes.COLOR_MODE_COLOR else 0),
                    if (caps.color) PrintAttributes.COLOR_MODE_COLOR else PrintAttributes.COLOR_MODE_MONOCHROME,
                )
                c.setMinMargins(PrintAttributes.Margins.NO_MARGINS)
                builder.setCapabilities(c.build())
            }
            return builder.build()
        }
    }

    private fun mediaSize(kw: String): PrintAttributes.MediaSize? {
        val dims = Regex("""([\d.]+)x([\d.]+)(mm|in)$""").find(kw) ?: return null
        val (w, h, unit) = dims.destructured
        val toMils = if (unit == "in") 1000f else 1000f / 25.4f
        return PrintAttributes.MediaSize(kw, paperName(kw), (w.toFloat() * toMils).toInt(), (h.toFloat() * toMils).toInt())
    }
}

fun savedPrinters(context: Context): List<SavedPrinter> = loadPrinters(context.getSharedPreferences("platen", Context.MODE_PRIVATE))
