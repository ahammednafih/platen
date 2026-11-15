package com.kanamlabs.platen

import android.graphics.Bitmap
import android.graphics.Color
import android.os.ParcelFileDescriptor
import com.kanamlabs.platen.core.PlatenException
import com.kanamlabs.platen.core.RenderedPage
import java.io.File
import android.graphics.pdf.PdfRenderer as SystemPdfRenderer
import com.kanamlabs.platen.core.PdfRenderer as CorePdfRenderer

object AndroidPdf : CorePdfRenderer {
    private const val MAX_PIXELS = 60_000_000L

    override fun render(path: String, index: UInt, dpi: UInt): RenderedPage {
        val bitmap = page(File(path), index.toInt()) { _, _ -> dpi.toFloat() / 72f }
        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        val rgb = ByteArray(pixels.size * 3)
        for (i in pixels.indices) {
            val p = pixels[i]
            rgb[i * 3] = (p shr 16).toByte()
            rgb[i * 3 + 1] = (p shr 8).toByte()
            rgb[i * 3 + 2] = p.toByte()
        }
        return RenderedPage(bitmap.width.toUInt(), bitmap.height.toUInt(), rgb).also { bitmap.recycle() }
    }

    fun previewUri(context: android.content.Context, uri: android.net.Uri, width: Int): Bitmap? =
        context.contentResolver.openFileDescriptor(uri, "r")?.use { fd ->
            SystemPdfRenderer(fd).use { pdf ->
                pdf.openPage(0).use { page ->
                    val s = width.toFloat() / page.width
                    val bitmap = Bitmap.createBitmap(width, (page.height * s).toInt().coerceAtLeast(1), Bitmap.Config.ARGB_8888)
                    bitmap.eraseColor(Color.WHITE)
                    page.render(bitmap, null, null, SystemPdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
                    bitmap
                }
            }
        }

    fun pageCount(file: File): Int = open(file) { it.pageCount }

    fun preview(file: File, index: Int, width: Int): Bitmap = page(file, index) { w, _ -> width.toFloat() / w }

    private fun page(file: File, index: Int, scale: (Int, Int) -> Float): Bitmap = open(file) { pdf ->
        if (index !in 0 until pdf.pageCount) throw PlatenException.Failed("No page ${index + 1} in ${file.name}")
        pdf.openPage(index).use { page ->
            val s = scale(page.width, page.height)
            val w = (page.width * s).toInt().coerceAtLeast(1)
            val h = (page.height * s).toInt().coerceAtLeast(1)
            if (w.toLong() * h > MAX_PIXELS) throw PlatenException.Failed("That page is too large to print at this resolution")
            val bitmap = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
            bitmap.eraseColor(Color.WHITE)
            page.render(bitmap, null, null, SystemPdfRenderer.Page.RENDER_MODE_FOR_PRINT)
            bitmap
        }
    }

    private fun <T> open(file: File, use: (SystemPdfRenderer) -> T): T =
        ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY).use { fd -> SystemPdfRenderer(fd).use(use) }
}
