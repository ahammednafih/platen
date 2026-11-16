package com.kanamlabs.platen

import android.graphics.Bitmap
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.ArrowForward
import androidx.compose.material.icons.automirrored.outlined.RotateLeft
import androidx.compose.material.icons.automirrored.outlined.RotateRight
import androidx.compose.material.icons.outlined.AutoFixHigh
import androidx.compose.material.icons.outlined.Autorenew
import androidx.compose.material.icons.outlined.BrightnessMedium
import androidx.compose.material.icons.outlined.Contrast
import androidx.compose.material.icons.outlined.Crop
import androidx.compose.material.icons.outlined.CropFree
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.FormatColorFill
import androidx.compose.material.icons.outlined.InvertColors
import androidx.compose.material.icons.outlined.Palette
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.Straighten
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import kotlinx.coroutines.launch
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import com.kanamlabs.platen.core.Edit
import com.kanamlabs.platen.core.Page
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

@Composable
fun PageTools(model: AppModel, page: Page, index: Int, count: Int, onView: () -> Unit, onDismiss: () -> Unit) {
    var cropping by remember { mutableStateOf(false) }
    var levels by remember { mutableStateOf(false) }
    val act = { block: () -> Unit -> block(); onDismiss() }
    Sheet(onDismiss) {
        Text("Page ${index + 1}", style = MaterialTheme.typography.headlineSmall)
        OptionGroup("Page") {
            OptionRow(Icons.Outlined.Visibility, "View", onClick = onView)
            OptionRow(Icons.Outlined.Refresh, "Retake") { act { model.scan(replace = index) } }
            OptionRow(Icons.AutoMirrored.Outlined.ArrowBack, "Move earlier", enabled = index > 0) { act { model.movePage(index, index - 1) } }
            OptionRow(Icons.AutoMirrored.Outlined.ArrowForward, "Move later", enabled = index < count - 1) { act { model.movePage(index, index + 1) } }
            OptionRow(Icons.Outlined.Delete, "Delete", destructive = true) { act { model.deletePage(index) } }
        }
        OptionGroup("Turn and trim") {
            OptionRow(Icons.AutoMirrored.Outlined.RotateLeft, "Rotate left") { act { model.edit(index, Edit.RotateLeft) } }
            OptionRow(Icons.AutoMirrored.Outlined.RotateRight, "Rotate right") { act { model.edit(index, Edit.RotateRight) } }
            OptionRow(Icons.Outlined.Autorenew, "Rotate 180°") { act { model.edit(index, Edit.Rotate180) } }
            OptionRow(Icons.Outlined.Straighten, "Straighten") { act { model.edit(index, Edit.Straighten) } }
            OptionRow(Icons.Outlined.Crop, "Trim to paper") { act { model.edit(index, Edit.Crop) } }
            OptionRow(Icons.Outlined.CropFree, "Crop…") { cropping = true }
        }
        OptionGroup("Clean up") {
            OptionRow(Icons.Outlined.BrightnessMedium, "Brightness and contrast…") { levels = true }
            OptionRow(Icons.Outlined.AutoFixHigh, "Remove specks") { act { model.edit(index, Edit.Despeckle(page.dpi)) } }
            OptionRow(Icons.Outlined.FormatColorFill, "Fill torn edges") { act { model.fill(index) } }
        }
        OptionGroup("Colour") {
            OptionRow(Icons.Outlined.Contrast, "Gray") { act { model.edit(index, Edit.Gray) } }
            OptionRow(Icons.Outlined.InvertColors, "Black and white") { act { model.edit(index, Edit.BlackWhite) } }
            if (page.color != null) OptionRow(Icons.Outlined.Palette, "Back to colour") { act { model.edit(index, Edit.Colour) } }
        }
    }
    if (cropping) CropDialog(model, page, index, onDismiss = { cropping = false }, onDone = { cropping = false; onDismiss() })
    if (levels) LevelsDialog(model, page, index, onDismiss = { levels = false }, onDone = { levels = false; onDismiss() })
}

@Composable
private fun pageBitmap(page: Page): Bitmap? = produceState<Bitmap?>(null, page.path) { value = withContext(Dispatchers.IO) { thumbnail(page.path, 1200) } }.value

private val CropSizes = listOf("A4" to (210f to 297f), "A5" to (148f to 210f), "A6" to (105f to 148f), "B5" to (176f to 250f), "Letter" to (216f to 279f), "Photo 10×15" to (102f to 152f), "Card" to (86f to 54f))

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun CropDialog(model: AppModel, page: Page, index: Int, onDismiss: () -> Unit, onDone: () -> Unit) {
    val bitmap = pageBitmap(page)
    val tick = rememberTick()
    var rect by remember { mutableStateOf(floatArrayOf(0f, 0f, 1f, 1f)) }
    var picked by remember { mutableStateOf("Whole page") }
    var note by remember { mutableStateOf<String?>(null) }
    val pageW = page.w.toFloat() * 25.4f / page.dpi.toFloat()
    val pageH = page.h.toFloat() * 25.4f / page.dpi.toFloat()
    val auto = {
        picked = "Auto"
        note = null
        model.work(null, { model.platen.paperBounds(index.toUInt()) }) { b ->
            if (b != null) rect = floatArrayOf(b.x, b.y, b.x + b.w, b.y + b.h)
            else note = "No dark border around the paper to find. Pick a size or drag the corners."
        }
    }
    LaunchedEffect(index) { auto() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Crop") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    FilterChip(selected = picked == "Whole page", onClick = { tick(); picked = "Whole page"; note = null; rect = floatArrayOf(0f, 0f, 1f, 1f) }, label = { Text("Whole page") })
                    FilterChip(selected = picked == "Auto", onClick = { tick(); auto() }, label = { Text("Auto") })
                    CropSizes.filter { (_, s) -> s.first <= pageW + 2 && s.second <= pageH + 2 }.forEach { (name, s) ->
                        FilterChip(
                            selected = picked == name,
                            onClick = {
                                tick()
                                picked = name
                                note = null
                                rect = floatArrayOf(0f, 0f, (s.first / pageW).coerceAtMost(1f), (s.second / pageH).coerceAtMost(1f))
                            },
                            label = { Text(name) },
                        )
                    }
                }
                note?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                Box(Modifier.fillMaxWidth().aspectRatio(page.w.toFloat() / page.h.toFloat())) {
                    bitmap?.let { Image(it.asImageBitmap(), null, contentScale = ContentScale.FillBounds, modifier = Modifier.fillMaxSize()) }
                    CropOverlay(rect) { rect = it; picked = "Custom" }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = {
                val (l, t, r, b) = rect.toList()
                model.edit(index, Edit.CropTo(l, t, r - l, b - t))
                onDone()
            }, enabled = !(rect[0] == 0f && rect[1] == 0f && rect[2] == 1f && rect[3] == 1f)) { Text("Crop") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
fun CropOverlay(rect: FloatArray, onChange: (FloatArray) -> Unit) {
    val accent = MaterialTheme.colorScheme.primary
    var grabbed by remember { mutableStateOf(-1) }
    var current by remember { mutableStateOf(rect) }
    current = rect
    Canvas(
        Modifier.fillMaxSize().pointerInput(Unit) {
            detectDragGestures(
                onDragStart = { at ->
                    val (l, t, r, b) = current.toList()
                    val corners = listOf(Offset(l, t), Offset(r, t), Offset(l, b), Offset(r, b)).map { Offset(it.x * size.width, it.y * size.height) }
                    val near = corners.indexOfFirst { (it - at).getDistance() < 64f }
                    grabbed = if (near >= 0) near else 4
                },
                onDragEnd = { grabbed = -1 },
            ) { change, drag ->
                change.consume()
                val dx = drag.x / size.width
                val dy = drag.y / size.height
                val r = current.copyOf()
                when (grabbed) {
                    0 -> { r[0] += dx; r[1] += dy }
                    1 -> { r[2] += dx; r[1] += dy }
                    2 -> { r[0] += dx; r[3] += dy }
                    3 -> { r[2] += dx; r[3] += dy }
                    4 -> {
                        val mx = dx.coerceIn(-r[0], 1f - r[2])
                        val my = dy.coerceIn(-r[1], 1f - r[3])
                        r[0] += mx; r[2] += mx; r[1] += my; r[3] += my
                    }
                }
                r[0] = r[0].coerceIn(0f, r[2] - 0.05f)
                r[1] = r[1].coerceIn(0f, r[3] - 0.05f)
                r[2] = r[2].coerceIn(r[0] + 0.05f, 1f)
                r[3] = r[3].coerceIn(r[1] + 0.05f, 1f)
                onChange(r)
            }
        },
    ) {
        val (l, t, r, b) = current.toList()
        val tl = Offset(l * size.width, t * size.height)
        val box = Size((r - l) * size.width, (b - t) * size.height)
        val shade = Color.Black.copy(alpha = 0.45f)
        drawRect(shade, Offset.Zero, Size(size.width, tl.y))
        drawRect(shade, Offset(0f, tl.y + box.height), Size(size.width, size.height - tl.y - box.height))
        drawRect(shade, Offset(0f, tl.y), Size(tl.x, box.height))
        drawRect(shade, Offset(tl.x + box.width, tl.y), Size(size.width - tl.x - box.width, box.height))
        drawRect(accent, tl, box, style = Stroke(width = 3.dp.toPx()))
        listOf(tl, Offset(tl.x + box.width, tl.y), Offset(tl.x, tl.y + box.height), Offset(tl.x + box.width, tl.y + box.height)).forEach {
            drawCircle(accent, 9.dp.toPx(), it)
        }
    }
}

@Composable
private fun LevelsDialog(model: AppModel, page: Page, index: Int, onDismiss: () -> Unit, onDone: () -> Unit) {
    val bitmap = pageBitmap(page)
    var brightness by remember { mutableFloatStateOf(1f) }
    var contrast by remember { mutableFloatStateOf(1f) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Brightness and contrast") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                bitmap?.let {
                    Image(
                        it.asImageBitmap(), null,
                        colorFilter = ColorFilter.colorMatrix(levels(brightness, contrast)),
                        contentScale = ContentScale.Fit,
                        modifier = Modifier.fillMaxWidth().aspectRatio(page.w.toFloat() / page.h.toFloat()),
                    )
                }
                Text("Brightness ${Math.round(brightness * 100)}%")
                Slider(brightness, { brightness = it }, valueRange = 0.5f..1.5f)
                Text("Contrast ${Math.round(contrast * 100)}%")
                Slider(contrast, { contrast = it }, valueRange = 0.5f..2f)
                Row(horizontalArrangement = Arrangement.End, modifier = Modifier.fillMaxWidth()) {
                    OutlinedButton(onClick = { brightness = 1f; contrast = 1f }, shapes = ButtonDefaults.shapes()) { Text("Reset") }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { model.edit(index, Edit.Levels(brightness, contrast)); onDone() }, enabled = brightness != 1f || contrast != 1f) { Text("Apply") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

private fun levels(b: Float, c: Float): ColorMatrix {
    val offset = (0.5f - 0.5f * c) * 255f
    val k = b * c
    return ColorMatrix(floatArrayOf(k, 0f, 0f, 0f, offset, 0f, k, 0f, 0f, offset, 0f, 0f, k, 0f, offset, 0f, 0f, 0f, 1f, 0f))
}

private val PhotoAspects = listOf("Free" to null, "4×6" to (2f / 3f), "5×7" to (5f / 7f), "Square" to 1f)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ImageCropDialog(file: java.io.File, onDismiss: () -> Unit, onDone: (java.io.File) -> Unit) {
    val tick = rememberTick()
    val preview = produceState<Bitmap?>(null, file) { value = withContext(Dispatchers.IO) { thumbnail(file.path, 1400) } }.value
    var rect by remember { mutableStateOf(floatArrayOf(0f, 0f, 1f, 1f)) }
    var aspect by remember { mutableStateOf("Free") }
    var saving by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Crop before printing") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    PhotoAspects.forEach { (name, ratio) ->
                        FilterChip(selected = aspect == name, onClick = {
                            tick()
                            aspect = name
                            val b = preview ?: return@FilterChip
                            rect = if (ratio == null) floatArrayOf(0f, 0f, 1f, 1f) else centred(b.width.toFloat(), b.height.toFloat(), ratio)
                        }, label = { Text(name) })
                    }
                }
                preview?.let { b ->
                    Box(Modifier.fillMaxWidth().aspectRatio(b.width.toFloat() / b.height.toFloat())) {
                        Image(b.asImageBitmap(), null, contentScale = ContentScale.FillBounds, modifier = Modifier.fillMaxSize())
                        CropOverlay(rect) { rect = it; aspect = "Free" }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(enabled = !saving && preview != null, onClick = {
                saving = true
                scope.launch {
                    val out = withContext(Dispatchers.IO) { cropImage(file, rect) }
                    saving = false
                    if (out != null) onDone(out)
                }
            }) { Text(if (saving) "Cropping…" else "Crop") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

private fun centred(w: Float, h: Float, ratio: Float): FloatArray {
    val target = if (w < h) ratio else 1f / ratio
    return if (w / h > target) {
        val cw = h * target / w
        floatArrayOf((1f - cw) / 2f, 0f, (1f + cw) / 2f, 1f)
    } else {
        val ch = w / target / h
        floatArrayOf(0f, (1f - ch) / 2f, 1f, (1f + ch) / 2f)
    }
}

private fun cropImage(file: java.io.File, rect: FloatArray): java.io.File? = runCatching {
    val decoder = android.graphics.BitmapRegionDecoder.newInstance(file.path)
    val (l, t, r, b) = rect.toList()
    val region = android.graphics.Rect((l * decoder.width).toInt(), (t * decoder.height).toInt(), (r * decoder.width).toInt(), (b * decoder.height).toInt())
    val bitmap = decoder.decodeRegion(region, null)
    decoder.recycle()
    val out = java.io.File(file.parentFile, file.nameWithoutExtension + "_cropped.jpg")
    out.outputStream().use { bitmap.compress(Bitmap.CompressFormat.JPEG, 95, it) }
    bitmap.recycle()
    out
}.getOrNull()
