package com.kanamlabs.platen

import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

@Composable
fun <T> Choice(options: List<Pair<T, String>>, selected: T, onSelect: (T) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val tick = rememberTick()
    Row(modifier, horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
        options.forEachIndexed { i, (value, label) ->
            ToggleButton(
                checked = selected == value,
                onCheckedChange = { if (selected != value) tick(); onSelect(value) },
                enabled = enabled,
                shapes = when (i) {
                    0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                    options.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                    else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                },
                colors = ToggleButtonDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerHighest),
                contentPadding = PaddingValues(horizontal = 4.dp),
                modifier = Modifier.weight(1f).semantics { role = Role.RadioButton },
            ) { Text(label, maxLines = 1) }
        }
    }
}

@Composable
fun Labeled(label: String, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(label, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        content()
    }
}

@Composable
fun FileImage(path: String, width: Int, description: String?, modifier: Modifier = Modifier, page: Int = 0) {
    val bitmap by produceState<Bitmap?>(null, path, page, width) {
        value = withContext(Dispatchers.IO) { runCatching { load(File(path), page, width) }.getOrNull() }
    }
    Box(modifier, contentAlignment = Alignment.Center) {
        bitmap?.let { Image(it.asImageBitmap(), description, contentScale = ContentScale.Fit, modifier = Modifier.fillMaxWidth()) }
    }
}

fun load(file: File, page: Int, width: Int): Bitmap? =
    if (file.extension.equals("pdf", true)) AndroidPdf.preview(file, page, width) else thumbnail(file.path, width)

fun thumbnail(path: String, width: Int): Bitmap? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(path, bounds)
    var sample = 1
    while (bounds.outWidth / (sample * 2) >= width) sample *= 2
    return BitmapFactory.decodeFile(path, BitmapFactory.Options().apply { inSampleSize = sample })
}

fun shareFile(context: Context, file: File) = shareUri(context, FileProvider.getUriForFile(context, "${context.packageName}.files", file), SaveFolder.mimeOf(file.name))

fun shareUri(context: Context, uri: Uri, mime: String) {
    val send = Intent(Intent.ACTION_SEND).setType(mime).putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    context.startActivity(Intent.createChooser(send, null))
}

fun openUri(context: Context, uri: Uri, mime: String) {
    val view = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    runCatching { context.startActivity(view) }
}

fun size(bytes: Long): String = if (bytes >= 1_000_000) "%.1f MB".format(bytes / 1e6) else "${maxOf(1, Math.round(bytes / 1e3))} KB"

fun dur(secs: Float): String {
    val s = Math.round(secs)
    return when {
        s < 60 -> "${s}s"
        s < 3600 -> "${s / 60}m ${"%02d".format(s % 60)}s"
        else -> "${s / 3600}h ${"%02d".format((s % 3600) / 60)}m"
    }
}

fun paperName(kw: String): String {
    val named = mapOf(
        "iso_a4" to "A4", "iso_a5" to "A5", "iso_a6" to "A6", "na_letter" to "Letter", "na_legal" to "Legal", "jis_b5" to "B5",
        "na_executive" to "Executive", "iso_dl" to "DL envelope", "iso_c5" to "C5 envelope", "na_number-10" to "No. 10 envelope",
        "na_monarch" to "Monarch envelope", "na_foolscap" to "Foolscap", "na_oficio" to "Oficio", "om_india-legal" to "India legal",
        "na_index-4x6" to "Photo 4×6 in", "na_5x7" to "Photo 5×7 in", "oe_photo-l" to "Photo 3.5×5 in", "na_index-5x8" to "Card 5×8 in",
    )
    val key = kw.split("_").take(2).joinToString("_")
    val dims = kw.substringAfterLast('_')
    return named[key] ?: dims.replace("x", " × ").replace(Regex("(mm|in)$"), " $1")
}

val Padding = PaddingValues(16.dp)

fun Modifier.screenPadding() = padding(horizontal = 16.dp)

fun Modifier.readable() = widthIn(max = 600.dp).fillMaxWidth()

@Composable
fun isWide() = LocalConfiguration.current.screenWidthDp >= 600

enum class ScaleMode(val label: String) { Fit("Fit to sheet"), Actual("Actual size"), Percent("Percent") }

fun scaleOf(mode: ScaleMode, percent: Int): com.kanamlabs.platen.core.Scale = when (mode) {
    ScaleMode.Fit -> com.kanamlabs.platen.core.Scale.Fit
    ScaleMode.Actual -> com.kanamlabs.platen.core.Scale.Actual
    ScaleMode.Percent -> com.kanamlabs.platen.core.Scale.Percent(percent.toUInt())
}

@Composable
fun ScalePicker(mode: ScaleMode, percent: Int, onMode: (ScaleMode) -> Unit, onPercent: (Int) -> Unit) {
    Labeled("Size") {
        Choice(ScaleMode.entries.map { it to it.label }, mode, onMode)
        if (mode == ScaleMode.Percent) PercentStepper(percent, onPercent)
    }
}

@Composable
private fun PercentStepper(percent: Int, onPercent: (Int) -> Unit) {
    Stepper("Scale, %", percent, 25..400) { onPercent(if (it > percent) (percent / 5 + 1) * 5 else ((percent - 1) / 5) * 5) }
}

fun paperTypeName(kw: String): String = when (kw) {
    "stationery" -> "Plain"
    "stationery-inkjet" -> "Inkjet"
    "photographic-glossy" -> "Glossy photo"
    "photographic-matte" -> "Matte photo"
    "photographic" -> "Photo"
    "photographic-high-gloss" -> "High-gloss photo"
    "photographic-semi-gloss" -> "Semi-gloss photo"
    "envelope" -> "Envelope"
    "labels" -> "Labels"
    "cardstock" -> "Card"
    else -> if (kw.startsWith("com.") || kw.startsWith("org.")) "Printer's photo paper" else kw.replace('-', ' ').replaceFirstChar { it.uppercase() }
}
