package com.kanamlabs.platen

import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

@Composable
fun PageViewer(pages: List<String>, start: Int, onClose: () -> Unit) {
    val pager = rememberPagerState(start) { pages.size }
    var zoomed by remember { mutableStateOf(false) }
    ViewerFrame(onClose, pager = {
        HorizontalPager(pager, userScrollEnabled = !zoomed, beyondViewportPageCount = 1, key = { pages[it] }) { i ->
            ZoomableImage({ thumbnail(pages[i], ViewerCache.WIDTH) }, pages[i], onZoomChange = { if (i == pager.currentPage) zoomed = it })
        }
    }) {
        Text("Page ${pager.currentPage + 1} of ${pages.size}", style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { liveRegion = LiveRegionMode.Polite })
    }
}

@Composable
private fun ViewerFrame(onClose: () -> Unit, pager: @Composable () -> Unit, footer: @Composable () -> Unit) {
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        val label: @Composable () -> Unit = {
            Surface(shape = MaterialTheme.shapes.extraLarge, color = MaterialTheme.colorScheme.surfaceContainerHigh) { footer() }
        }
        Box(Modifier.fillMaxSize().background(Color.Black)) {
            if (isTabletop()) {
                TabletopSplit(Modifier.fillMaxSize(), top = { Box(Modifier.statusBarsPadding()) { pager() } }, bottom = { Box(Modifier.align(Alignment.Center)) { label() } })
            } else {
                pager()
                Box(Modifier.align(Alignment.BottomCenter).safeDrawingPadding().padding(16.dp)) { label() }
            }
            FilledTonalIconButton(
                onClick = onClose,
                shapes = IconButtonDefaults.shapes(),
                modifier = Modifier.align(Alignment.TopEnd).safeDrawingPadding().padding(8.dp),
            ) { Icon(Icons.Default.Close, contentDescription = "Close") }
        }
    }
}

@Composable
fun ZoomableImage(load: suspend () -> Bitmap?, key: Any, onZoomChange: (Boolean) -> Unit) {
    val bitmap by produceState(ViewerCache.get(key.toString()), key) {
        if (value == null) value = withContext(Dispatchers.IO) { load()?.also { ViewerCache.put(key.toString(), it) } }
    }
    val shown by animateFloatAsState(if (bitmap != null) 1f else 0f, tween(180), label = "fade")
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    fun zoomTo(s: Float, o: Offset) {
        scale = s
        offset = if (s <= 1f) Offset.Zero else o
        onZoomChange(s > 1f)
    }
    Box(
        Modifier
            .fillMaxSize()
            .pointerInput(Unit) {
                detectTapGestures(onDoubleTap = { tap ->
                    if (scale > 1f) zoomTo(1f, Offset.Zero) else zoomTo(2.5f, (Offset(size.width / 2f, size.height / 2f) - tap) * 1.5f)
                })
            }
            .pointerInput(Unit) {
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    do {
                        val event = awaitPointerEvent()
                        val fingers = event.changes.count { it.pressed }
                        if (fingers > 1 || scale > 1f) {
                            val zoom = event.calculateZoom()
                            val pan = event.calculatePan()
                            zoomTo((scale * zoom).coerceIn(1f, 8f), offset + pan)
                            event.changes.forEach { if (it.positionChanged()) it.consume() }
                        }
                    } while (event.changes.any { it.pressed })
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        val b = bitmap
        if (b == null) {
            Box(Modifier.fillMaxWidth(0.86f).aspectRatio(0.707f).background(Color.White.copy(alpha = 0.08f)))
        } else {
            Image(
                b.asImageBitmap(),
                contentDescription = null,
                contentScale = ContentScale.Fit,
                modifier = Modifier.fillMaxSize().graphicsLayer {
                    alpha = shown
                    scaleX = scale
                    scaleY = scale
                    translationX = offset.x
                    translationY = offset.y
                },
            )
        }
    }
}

@Composable
fun PdfCompare(files: List<String>, start: Int, onClose: () -> Unit) {
    var shown by remember { mutableStateOf(start) }
    val count by produceState(0, files) { value = withContext(Dispatchers.IO) { files.minOf { runCatching { AndroidPdf.pageCount(java.io.File(it)) }.getOrDefault(0) } } }
    val pager = rememberPagerState { count }
    var zoomed by remember { mutableStateOf(false) }
    ViewerFrame(onClose, pager = {
        HorizontalPager(pager, userScrollEnabled = !zoomed, beyondViewportPageCount = 1) { i ->
            ZoomableImage({ AndroidPdf.preview(java.io.File(files[shown]), i, ViewerCache.WIDTH) }, "$shown/$i", onZoomChange = { if (i == pager.currentPage) zoomed = it })
        }
    }) {
        Column(Modifier.padding(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Choice(listOf(0 to "Original", 1 to "Shrunk"), shown, { shown = it }, Modifier.width(260.dp))
            Text("Page ${pager.currentPage + 1} of $count", style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(top = 8.dp).semantics { liveRegion = LiveRegionMode.Polite })
        }
    }
}

@Composable
fun FilePagesViewer(pages: List<Pair<java.io.File, Int>>, start: Int, onClose: () -> Unit) {
    val pager = rememberPagerState(start) { pages.size }
    var zoomed by remember { mutableStateOf(false) }
    ViewerFrame(onClose, pager = {
        HorizontalPager(pager, userScrollEnabled = !zoomed, beyondViewportPageCount = 1) { i ->
            val (file, page) = pages[i]
            ZoomableImage({ load(file, page, ViewerCache.WIDTH) }, "${file.path}/$page", onZoomChange = { if (i == pager.currentPage) zoomed = it })
        }
    }) {
        Text("Page ${pager.currentPage + 1} of ${pages.size}", style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { liveRegion = LiveRegionMode.Polite })
    }
}

@Composable
fun SheetsViewer(count: Int, start: Int, job: Any, render: (Int) -> android.graphics.Bitmap?, onClose: () -> Unit) {
    val pager = rememberPagerState(start) { count }
    var zoomed by remember { mutableStateOf(false) }
    ViewerFrame(onClose, pager = {
        HorizontalPager(pager, userScrollEnabled = !zoomed, beyondViewportPageCount = 1) { i ->
            ZoomableImage({ render(i) }, "sheet/${job.hashCode()}/$i", onZoomChange = { if (i == pager.currentPage) zoomed = it })
        }
    }) {
        Text("Sheet ${pager.currentPage + 1} of $count", style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { liveRegion = LiveRegionMode.Polite })
    }
}

object ViewerCache {
    const val WIDTH = 1600
    private val cache = object : android.util.LruCache<String, Bitmap>((Runtime.getRuntime().maxMemory() / 8).toInt()) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount
    }

    fun get(key: String): Bitmap? = cache.get(key)

    fun put(key: String, bitmap: Bitmap) {
        cache.put(key, bitmap)
    }
}
