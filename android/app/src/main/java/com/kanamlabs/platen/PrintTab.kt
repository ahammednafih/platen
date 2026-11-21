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
import android.graphics.BitmapFactory
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Spacer
import androidx.compose.material3.Surface
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Crop
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.ListItem
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import com.kanamlabs.platen.core.Plan
import com.kanamlabs.platen.core.PlatenException
import com.kanamlabs.platen.core.PrintJob
import com.kanamlabs.platen.core.PrintOpts
import com.kanamlabs.platen.core.PrintProgress
import com.kanamlabs.platen.core.Quality
import com.kanamlabs.platen.core.Scale
import com.kanamlabs.platen.core.Subset
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

private enum class Tone(val label: String) { Colour("Colour"), Gray("Gray"), Clean("B&W") }

private data class SrcPage(val file: File, val index: Int)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PrintTab(model: AppModel, state: AppState, padding: PaddingValues, files: List<File>, onFiles: (List<File>) -> Unit) {
    val scope = rememberCoroutineScope()
    var pages by remember { mutableStateOf<List<SrcPage>>(emptyList()) }
    var sel by remember { mutableStateOf<List<Boolean>>(emptyList()) }
    var ranges by remember { mutableStateOf("") }
    var nup by remember { mutableIntStateOf(1) }
    var scaleMode by remember { mutableStateOf(ScaleMode.Fit) }
    var percent by remember { mutableIntStateOf(100) }
    val scale = scaleOf(scaleMode, percent)
    var reverse by remember { mutableStateOf(false) }
    var copies by remember { mutableIntStateOf(1) }
    var collate by remember { mutableStateOf(true) }
    var tone by remember { mutableStateOf(Tone.Colour) }
    var media by remember { mutableStateOf(state.printCaps?.mediaDefault ?: "iso_a4_210x297mm") }
    var quality by remember { mutableStateOf(Quality.NORMAL) }
    var mediaType by remember { mutableStateOf<String?>(null) }
    var borderless by remember { mutableStateOf(false) }
    var cropping by remember { mutableStateOf<File?>(null) }
    var viewing by remember { mutableStateOf<Int?>(null) }
    var viewingSheet by remember { mutableStateOf<Int?>(null) }
    val click = rememberClick()
    var twoSided by remember { mutableStateOf(false) }
    var plan by remember { mutableStateOf<Plan?>(null) }
    var sheets by remember { mutableStateOf<List<Bitmap?>>(emptyList()) }
    var busy by remember { mutableStateOf<String?>(null) }
    var flip by remember { mutableStateOf(false) }
    var log by remember { mutableStateOf<String?>(null) }

    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isNotEmpty()) model.work("Opening", { uris.map { model.saveFolder.copyToCache(it) } }) { onFiles(files + it) }
    }

    fun job(subset: Subset = Subset.ALL, rev: Boolean = reverse) = PrintJob(
        files.map { it.path }, ranges, subset, rev, nup.toUInt(), scale, collate, tone == Tone.Clean,
        PrintOpts(copies.toUInt(), tone == Tone.Colour, media, "fit", quality, mediaType, borderless),
    )

    LaunchedEffect(files) {
        pages = withContext(Dispatchers.IO) {
            files.flatMap { f -> (0 until runCatching { model.platen.pageCount(f.path).toInt() }.getOrDefault(1)).map { SrcPage(f, it) } }
        }
        sel = pages.map { true }
        ranges = ""
    }
    LaunchedEffect(files, ranges, nup, scale, reverse, tone, media, borderless) {
        if (files.isEmpty()) {
            plan = null
            sheets = emptyList()
            return@LaunchedEffect
        }
        delay(250)
        val j = job()
        plan = withContext(Dispatchers.IO) { runCatching { model.platen.printPlan(j) }.getOrNull() }
        val count = plan?.order?.size?.coerceAtMost(8) ?: 0
        sheets = withContext(Dispatchers.IO) {
            (0 until count).map { i ->
                runCatching { model.platen.printPreview(j, i.toUInt(), 40u) }.getOrNull()?.let { BitmapFactory.decodeByteArray(it, 0, it.size) }
            }
        }
    }

    fun setSel(s: List<Boolean>) {
        sel = s
        ranges = if (s.all { it }) "" else toRanges(s)
    }

    fun send(j: PrintJob, label: String, then: () -> Unit = {}) = scope.launch {
        busy = label
        log = null
        try {
            val n = withContext(Dispatchers.IO) {
                model.platen.print(j, object : PrintProgress {
                    override fun progress(done: UInt, total: UInt) {
                        busy = "$label $done of $total"
                    }
                })
            }
            log = "Sent $n ${if (n == 1u) "job" else "jobs"} to the printer."
            then()
        } catch (e: PlatenException) {
            log = if (e.message == "stopped") "Stopped. Nothing else will print." else e.message
        } finally {
            busy = null
        }
    }

    val tick = rememberTick()
    if (files.isEmpty()) {
        Column(Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(20.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.readable()) { LowInk(state) }
            GlassBed(Modifier.readable()) {
                Column(Modifier.fillMaxWidth().padding(28.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    SheetGlyph(Glyph.Empty, active = false, width = 72.dp)
                    Text("Print PDFs and photos", style = MaterialTheme.typography.headlineSmall, modifier = Modifier.semantics { heading() })
                    Text("Pick pages by tapping them, put several on a sheet, or print both sides by hand.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
                    Button(
                        onClick = { pick.launch(arrayOf("application/pdf", "image/jpeg", "image/png")) },
                        shapes = ButtonDefaults.shapes(),
                        contentPadding = ButtonDefaults.contentPaddingFor(ButtonDefaults.MediumContainerHeight),
                        modifier = Modifier.heightIn(ButtonDefaults.MediumContainerHeight),
                    ) { Text("Choose files", style = ButtonDefaults.textStyleFor(ButtonDefaults.MediumContainerHeight)) }
                }
            }
        }
    } else {
        val source: @Composable ColumnScope.() -> Unit = {
            Section("Files") {
                files.forEachIndexed { i, f ->
                    val view = { pages.indexOfFirst { it.file == f }.takeIf { it >= 0 }?.let { viewing = it } }
                    var menu by remember { mutableStateOf(false) }
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        Row(
                            Modifier.weight(1f).clip(MaterialTheme.shapes.small).clickable(onClickLabel = "View ${f.name}") { view() }.padding(vertical = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Paper(Modifier.size(36.dp, 48.dp)) { FileImage(f.path, 120, null, Modifier.fillMaxSize()) }
                            Text(f.name, Modifier.weight(1f), style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
                        }
                        Box {
                            IconButton(onClick = { menu = true }, shapes = IconButtonDefaults.shapes()) { Icon(Icons.Default.MoreVert, contentDescription = "More for ${f.name}") }
                            DropdownMenu(menu, { menu = false }) {
                                DropdownMenuItem({ Text("View") }, { menu = false; view() }, leadingIcon = { Icon(Icons.Outlined.Visibility, null) })
                                if (i > 0) DropdownMenuItem({ Text("Move up") }, { menu = false; onFiles(files.move(i, i - 1)) }, leadingIcon = { Icon(Icons.Default.ArrowUpward, null) })
                                if (i < files.size - 1) DropdownMenuItem({ Text("Move down") }, { menu = false; onFiles(files.move(i, i + 1)) }, leadingIcon = { Icon(Icons.Default.ArrowDownward, null) })
                                if (!f.extension.equals("pdf", true)) DropdownMenuItem({ Text("Crop") }, { menu = false; cropping = f }, leadingIcon = { Icon(Icons.Default.Crop, null) })
                            }
                        }
                        IconButton(onClick = { onFiles(files - f) }, shapes = IconButtonDefaults.shapes()) { Icon(Icons.Default.Close, contentDescription = "Remove ${f.name}") }
                    }
                }
                TextButton(onClick = { pick.launch(arrayOf("application/pdf", "image/jpeg", "image/png")) }, shapes = ButtonDefaults.shapes()) { Text("Add more") }
            }

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Pages, ${sel.count { it }} of ${pages.size}. Tap to pick, hold to view", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 4.dp))
                GlassBed(Modifier.fillMaxWidth()) {
                    LazyRow(contentPadding = PaddingValues(16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                        itemsIndexed(pages) { i, p ->
                            val on = sel.getOrElse(i) { true }
                            Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                Box(
                                    Modifier.combinedClickable(
                                        onClick = { tick(); setSel(sel.toMutableList().also { it[i] = !on }) },
                                        onClickLabel = if (on) "Leave out" else "Print",
                                        onLongClick = { click(); viewing = i },
                                        onLongClickLabel = "View page ${i + 1}",
                                    ).semantics { role = Role.Checkbox; toggleableState = ToggleableState(on); stateDescription = if (on) "Will print" else "Left out" },
                                ) {
                                    Paper(Modifier.size(84.dp, 116.dp).graphicsLayer { alpha = if (on) 1f else 0.35f }) {
                                        FileImage(p.file.path, 200, "Page ${i + 1}", Modifier.fillMaxSize(), page = p.index)
                                    }
                                }
                                Text("${i + 1}", style = MaterialTheme.typography.labelLarge, color = if (on) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                    FilterChip(selected = sel.all { it }, onClick = { tick(); setSel(pages.map { true }) }, label = { Text("All") })
                    FilterChip(selected = sel == pages.indices.map { it % 2 == 0 }, onClick = { tick(); setSel(pages.indices.map { it % 2 == 0 }) }, label = { Text("Odd") })
                    FilterChip(selected = sel == pages.indices.map { it % 2 == 1 }, onClick = { tick(); setSel(pages.indices.map { it % 2 == 1 }) }, label = { Text("Even") })
                }
                OutlinedTextField(ranges, { text ->
                    ranges = text
                    fromRanges(text, pages.size)?.let { sel = it }
                }, label = { Text("Pages, like 1-3, 6") }, singleLine = true, shape = MaterialTheme.shapes.medium, modifier = Modifier.fillMaxWidth())
            }

            plan?.let { p ->
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        "${p.order.size} ${if (p.order.size == 1) "sheet" else "sheets"}${if (copies > 1) ", $copies copies" else ""}. Tap one to see it bigger",
                        style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 4.dp),
                    )
                    LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp), contentPadding = PaddingValues(horizontal = 4.dp, vertical = 6.dp)) {
                        itemsIndexed(sheets) { i, b ->
                            Paper(
                                Modifier.size(80.dp, 113.dp).combinedClickable(
                                    onClick = { click(); viewingSheet = i },
                                    onLongClick = { click(); viewingSheet = i },
                                    onClickLabel = "View sheet ${i + 1}",
                                ).semantics { contentDescription = "Sheet ${i + 1}" },
                            ) {
                                b?.let { Image(it.asImageBitmap(), null, contentScale = ContentScale.Fit, modifier = Modifier.fillMaxSize()) }
                            }
                        }
                    }
                }
            }
        }
        val settings: @Composable ColumnScope.() -> Unit = {
            Section("Layout") {
                Labeled("Pages per sheet") { Choice(listOf(1, 2, 4, 6, 9, 16).map { it to "$it" }, nup, { nup = it }) }
                ScalePicker(scaleMode, percent, { scaleMode = it }, { percent = it })
                Toggle("Reverse order", reverse) { reverse = it }
                Toggle("Two-sided by hand", twoSided) { twoSided = it }
            }
            Section("Copies and colour") {
                Stepper("Copies", copies, 1..99) { copies = it }
                Toggle("Whole sets (1 2 3, 1 2 3)", collate, enabled = copies > 1) { collate = it }
                Labeled("Colour") { Choice(Tone.entries.map { it to it.label }, tone, { tone = it }) }
            }
            Section("Paper") {
                PaperAndQuality(state, media, { media = it }, quality, { quality = it }, mediaType, { mediaType = it })
                if (state.printCaps?.borderless == true) Toggle("Borderless (to the edges of the paper)", borderless) { borderless = it }
            }
            log?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }) }
        }
        val printBar: @Composable () -> Unit = {
            AnimatedContent(busy, transitionSpec = { fadeIn() togetherWith fadeOut() }, label = "print") { b ->
                if (b != null) {
                    BusyPill("$b…", Modifier.fillMaxWidth()) {
                        TextButton(onClick = { scope.launch(Dispatchers.IO) { runCatching { model.platen.stopJob() } } }, shapes = ButtonDefaults.shapes()) {
                            Text("Stop", color = MaterialTheme.colorScheme.inverseOnSurface)
                        }
                    }
                } else {
                    Button(
                        onClick = { if (twoSided) send(job(Subset.ODD), "Printing fronts") { flip = true } else send(job(), "Printing") },
                        enabled = state.device != null && plan != null && sel.any { it },
                        shapes = ButtonDefaults.shapes(),
                        contentPadding = ButtonDefaults.contentPaddingFor(ButtonDefaults.MediumContainerHeight),
                        modifier = Modifier.fillMaxWidth().heightIn(ButtonDefaults.MediumContainerHeight),
                    ) {
                        Icon(painterResource(R.drawable.ic_print), contentDescription = null, Modifier.size(ButtonDefaults.iconSizeFor(ButtonDefaults.MediumContainerHeight)))
                        Spacer(Modifier.width(ButtonDefaults.iconSpacingFor(ButtonDefaults.MediumContainerHeight)))
                        Text(
                            when {
                                twoSided -> "Print fronts"
                                copies > 1 -> "Print $copies copies"
                                else -> "Print"
                            },
                            style = ButtonDefaults.textStyleFor(ButtonDefaults.MediumContainerHeight),
                        )
                    }
                }
            }
        }
        if (isTabletop()) {
            TabletopSplit(
                Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp),
                top = {
                    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(20.dp)) {
                        LowInk(state)
                        source()
                    }
                },
                bottom = {
                    Row(Modifier.fillMaxSize(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                        Column(Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState()).padding(bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) { settings() }
                        Box(Modifier.weight(1f).fillMaxHeight(), contentAlignment = Alignment.Center) { printBar() }
                    }
                },
            )
        } else if (isWide()) {
            Row(Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
                Column(Modifier.weight(1f).fillMaxHeight().verticalScroll(rememberScrollState()).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
                    LowInk(state)
                    source()
                }
                Column(Modifier.weight(1f).fillMaxHeight()) {
                    Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(bottom = 16.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) { settings() }
                    Box(Modifier.padding(vertical = 12.dp)) { printBar() }
                }
            }
        } else {
            Box(Modifier.fillMaxSize().padding(padding)) {
                Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 104.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
                    LowInk(state)
                    source()
                    settings()
                }
                Surface(color = MaterialTheme.colorScheme.surface, modifier = Modifier.align(Alignment.BottomCenter).fillMaxWidth()) {
                    Box(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 12.dp)) { printBar() }
                }
            }
        }
    }

    viewing?.takeIf { it < pages.size }?.let { start ->
        FilePagesViewer(pages.map { it.file to it.index }, start, onClose = { viewing = null })
    }

    viewingSheet?.let { start ->
        val j = job()
        SheetsViewer(plan?.order?.size ?: 0, start, j, render = { slot ->
            runCatching { model.platen.printPreview(j, slot.toUInt(), 120u) }.getOrNull()?.let { BitmapFactory.decodeByteArray(it, 0, it.size) }
        }, onClose = { viewingSheet = null })
    }

    cropping?.let { f ->
        ImageCropDialog(f, onDismiss = { cropping = null }) { out -> onFiles(files.map { if (it == f) out else it }); cropping = null }
    }

    if (flip) {
        AlertDialog(
            onDismissRequest = { flip = false },
            title = { Text("Flip the stack") },
            text = { Text("Take the printed sheets, turn the stack over without changing its order, and put it back in the tray.") },
            confirmButton = { TextButton(onClick = { flip = false; send(job(Subset.EVEN, !reverse), "Printing backs") }) { Text("Print backs") } },
            dismissButton = { TextButton(onClick = { flip = false }) { Text("Skip") } },
        )
    }
}

private fun <T> List<T>.move(from: Int, to: Int) = toMutableList().apply { add(to, removeAt(from)) }

private fun toRanges(s: List<Boolean>): String {
    val out = mutableListOf<String>()
    var i = 0
    while (i < s.size) {
        if (!s[i]) { i++; continue }
        var j = i
        while (j + 1 < s.size && s[j + 1]) j++
        out += if (i == j) "${i + 1}" else "${i + 1}-${j + 1}"
        i = j + 1
    }
    return out.joinToString(", ")
}

private fun fromRanges(text: String, n: Int): List<Boolean>? {
    if (text.isBlank()) return List(n) { true }
    val sel = MutableList(n) { false }
    for (part in text.split(',').map { it.trim() }.filter { it.isNotEmpty() }) {
        val bits = part.split('-').map { it.trim() }
        val a = bits[0].toIntOrNull() ?: return null
        val b = if (bits.size > 1) bits[1].ifEmpty { "$n" }.toIntOrNull() ?: return null else a
        for (k in a..b) if (k in 1..n) sel[k - 1] = true
    }
    return sel
}
