package com.kanamlabs.platen

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.zIndex
import androidx.compose.foundation.selection.toggleable
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingToolbarDefaults
import androidx.compose.material3.HorizontalFloatingToolbar
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.kanamlabs.platen.core.ColorMode
import com.kanamlabs.platen.core.ExportOpts
import com.kanamlabs.platen.core.Fill
import com.kanamlabs.platen.core.Page
import com.kanamlabs.platen.core.SizeId
import java.io.File
import java.text.DateFormat
import java.util.Date

@Composable
fun ScanActions(model: AppModel, state: AppState) {
    val s = state.session ?: return
    var menu by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }
    IconButton(onClick = model::undo, enabled = s.canUndo && state.busy == null, shapes = IconButtonDefaults.shapes()) {
        Icon(painterResource(R.drawable.ic_undo), contentDescription = "Undo")
    }
    IconButton(onClick = model::redo, enabled = s.canRedo && state.busy == null, shapes = IconButtonDefaults.shapes()) {
        Icon(painterResource(R.drawable.ic_redo), contentDescription = "Redo")
    }
    Box {
        IconButton(onClick = { menu = true }, shapes = IconButtonDefaults.shapes()) { Icon(Icons.Default.MoreVert, contentDescription = "More") }
        DropdownMenu(menu, { menu = false }) {
            DropdownMenuItem({ Text("Rename") }, { menu = false; renaming = true })
            DropdownMenuItem({ Text("Close scan") }, { menu = false; model.closeScan() })
        }
    }
    if (renaming) NameDialog("Rename scan", s.name, onDismiss = { renaming = false }) { model.rename(it); renaming = false }
}

@Composable
fun ScanTab(model: AppModel, state: AppState, padding: PaddingValues, onPrint: (File) -> Unit) {
    if (state.session == null) StartPane(model, state, padding) else SessionPane(model, state, padding, onPrint)
}

@Composable
private fun StartPane(model: AppModel, state: AppState, padding: PaddingValues) {
    var name by rememberSaveable { mutableStateOf("") }
    LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        item {
            GlassBed(Modifier.readable().padding(bottom = 8.dp)) {
                Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(20.dp)) {
                    SheetGlyph(Glyph.Empty, active = false, width = 88.dp, modifier = Modifier.padding(top = 8.dp))
                    Text("Put a page on the glass", style = MaterialTheme.typography.headlineSmall, textAlign = TextAlign.Center)
                    OutlinedTextField(name, { name = it }, placeholder = { Text("Name this scan (optional)") }, singleLine = true, shape = MaterialTheme.shapes.medium, modifier = Modifier.fillMaxWidth())
                    Button(
                        onClick = { model.newScan(name.ifBlank { null }); name = "" },
                        shapes = ButtonDefaults.shapes(),
                        contentPadding = ButtonDefaults.contentPaddingFor(ButtonDefaults.MediumContainerHeight),
                        modifier = Modifier.fillMaxWidth().heightIn(ButtonDefaults.MediumContainerHeight),
                    ) {
                        Icon(painterResource(R.drawable.ic_scan), contentDescription = null, Modifier.size(ButtonDefaults.iconSizeFor(ButtonDefaults.MediumContainerHeight)))
                        Spacer(Modifier.width(ButtonDefaults.iconSpacingFor(ButtonDefaults.MediumContainerHeight)))
                        Text("Start a scan", style = ButtonDefaults.textStyleFor(ButtonDefaults.MediumContainerHeight))
                    }
                }
            }
        }
        if (state.drafts.isNotEmpty()) {
            item { Text("Pick up where you left off", style = MaterialTheme.typography.titleMedium, modifier = Modifier.readable().padding(start = 4.dp, top = 8.dp)) }
            items(state.drafts, key = { it.name }) { d -> DraftRow(model, d.name, d.pages.toInt(), d.modified.toLong(), Modifier.readable()) }
        }
    }
}

@Composable
private fun DraftRow(model: AppModel, name: String, pages: Int, modified: Long, modifier: Modifier = Modifier) {
    val cover = remember(name, pages) {
        File(model.scansDir, "$name/pages").listFiles()?.filter { it.extension == "jpg" || it.extension == "png" }?.minByOrNull { it.lastModified() }?.path
    }
    Surface(onClick = { model.newScan(name) }, modifier = modifier, shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Box(Modifier.size(52.dp, 70.dp), contentAlignment = Alignment.Center) {
                if (cover != null) Paper(Modifier.fillMaxSize()) { FileImage(cover, 160, null, Modifier.fillMaxSize()) }
                else SheetGlyph(Glyph.Empty, active = false, width = 52.dp)
            }
            Column(Modifier.weight(1f)) {
                Text(name, style = MaterialTheme.typography.titleMedium, maxLines = 1)
                Text(
                    "${if (pages == 1) "1 page" else "$pages pages"}, ${DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(modified * 1000))}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(onClick = { model.deleteDraft(name) }, shapes = IconButtonDefaults.shapes()) {
                Icon(Icons.Default.Delete, contentDescription = "Delete draft $name", tint = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable
private fun SessionPane(model: AppModel, state: AppState, padding: PaddingValues, onPrint: (File) -> Unit) {
    val session = state.session ?: return
    val click = rememberClick()
    var viewing by remember { mutableStateOf<Int?>(null) }
    var tools by remember { mutableStateOf<Int?>(null) }
    var showSettings by remember { mutableStateOf(false) }
    var showExport by remember { mutableStateOf(false) }

    Box(Modifier.fillMaxSize().padding(padding)) {
        Column(Modifier.fillMaxSize()) {
            SettingsChips(state, onClick = { showSettings = true })
            GlassBed(Modifier.weight(1f).fillMaxWidth(), shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp)) {
                if (session.pages.isEmpty()) {
                    Column(Modifier.align(Alignment.Center).padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
                        SheetGlyph(Glyph.Empty, active = false, width = 72.dp)
                        Text("Put a page face down on the glass, then tap Scan.", style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                } else {
                    val grid = rememberLazyGridState()
                    var dragFrom by remember { mutableStateOf<Int?>(null) }
                    var dragTo by remember { mutableStateOf<Int?>(null) }
                    var dragOffset by remember { mutableStateOf(Offset.Zero) }
                    val tick = rememberTick()
                    fun target(from: Int): Int? {
                        val items = grid.layoutInfo.visibleItemsInfo
                        val start = items.firstOrNull { it.index == from } ?: return null
                        val point = Offset(start.offset.x + start.size.width / 2f, start.offset.y + start.size.height / 2f) + dragOffset
                        return items.firstOrNull { point.x >= it.offset.x && point.x <= it.offset.x + it.size.width && point.y >= it.offset.y && point.y <= it.offset.y + it.size.height }?.index
                    }
                    LazyVerticalGrid(
                        columns = GridCells.Adaptive(if (isWide()) 200.dp else 140.dp),
                        state = grid,
                        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 120.dp),
                        horizontalArrangement = Arrangement.spacedBy(20.dp),
                        verticalArrangement = Arrangement.spacedBy(20.dp),
                    ) {
                        itemsIndexed(session.pages, key = { _, p -> p.file }) { i, p ->
                            val lifted = dragFrom == i
                            PageSheet(
                                i, p, fresh = p.file == state.fresh, dropHere = dragTo == i && dragFrom != i,
                                onOpen = { viewing = i }, onTools = { click(); tools = i },
                                modifier = (if (lifted) Modifier.zIndex(1f) else Modifier.animateItem())
                                    .graphicsLayer {
                                        if (lifted) {
                                            translationX = dragOffset.x
                                            translationY = dragOffset.y
                                            scaleX = 1.06f
                                            scaleY = 1.06f
                                        }
                                    }
                                    .pointerInput(p.file) {
                                        detectDragGesturesAfterLongPress(
                                            onDragStart = { dragFrom = i; dragTo = i; dragOffset = Offset.Zero; tick() },
                                            onDrag = { change, amount ->
                                                change.consume()
                                                dragOffset += amount
                                                val t = dragFrom?.let(::target)
                                                if (t != null && t != dragTo) { dragTo = t; tick() }
                                            },
                                            onDragEnd = {
                                                val from = dragFrom
                                                val to = dragTo
                                                if (from != null && to != null && from != to) { model.movePage(from, to); click() }
                                                dragFrom = null; dragTo = null; dragOffset = Offset.Zero
                                            },
                                            onDragCancel = { dragFrom = null; dragTo = null; dragOffset = Offset.Zero },
                                        )
                                    },
                            )
                        }
                    }
                }
            }
        }
        Column(Modifier.align(Alignment.BottomCenter).padding(bottom = 16.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            AnimatedVisibility(state.busy != null, enter = fadeIn() + expandVertically(), exit = fadeOut() + shrinkVertically()) {
                BusyPill("${state.busy ?: ""}…", Modifier.padding(horizontal = 16.dp)) {
                    if (state.busy?.startsWith("Scanning") == true) {
                        TextButton(onClick = { model.cancelScan() }, shapes = ButtonDefaults.shapes()) { Text("Cancel", color = MaterialTheme.colorScheme.inverseOnSurface) }
                    }
                }
            }
            AnimatedVisibility(state.top != null && state.busy == null, enter = fadeIn() + expandVertically(), exit = fadeOut() + shrinkVertically()) {
                Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.primaryContainer, modifier = Modifier.padding(horizontal = 16.dp)) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Text("Top half done", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onPrimaryContainer)
                        Text("Turn the page around so the bottom is on the glass, bottom edge against the far edge. Leave a few centimetres of overlap.", style = MaterialTheme.typography.bodyMedium)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(onClick = model::scanBottom, shapes = ButtonDefaults.shapes(), modifier = Modifier.weight(1f)) { Text("Scan bottom half") }
                            TextButton(onClick = model::cancelLong, shapes = ButtonDefaults.shapes()) { Text("Cancel") }
                        }
                    }
                }
            }
            val idle = state.busy == null && state.top == null
            HorizontalFloatingToolbar(
                expanded = true,
                floatingActionButton = {
                    FloatingToolbarDefaults.VibrantFloatingActionButton(
                        onClick = { if (idle && state.device != null) model.scan() },
                        containerColor = MaterialTheme.colorScheme.primary,
                        contentColor = MaterialTheme.colorScheme.onPrimary,
                    ) {
                        Icon(painterResource(R.drawable.ic_scan), contentDescription = "Scan a page")
                    }
                },
                colors = FloatingToolbarDefaults.standardFloatingToolbarColors(toolbarContainerColor = MaterialTheme.colorScheme.surfaceContainerHighest),
            ) {
                ToolbarButton(R.drawable.ic_tune, "Scan settings") { showSettings = true }
                ToolbarButton(R.drawable.ic_long_page, "Scan a long page", enabled = idle && state.device != null) { model.scanTop() }
                ToolbarButton(R.drawable.ic_export, "Export", enabled = session.pages.isNotEmpty()) { showExport = true }
                ToolbarButton(R.drawable.ic_print, "Print", enabled = session.pages.isNotEmpty() && idle) {
                    model.work("Making a PDF", { File(model.platen.scanToPdf()) }, onPrint)
                }
            }
        }
    }

    viewing?.takeIf { it < session.pages.size }?.let { i -> PageViewer(session.pages.map { it.path }, i, onClose = { viewing = null }) }
    tools?.takeIf { it < session.pages.size }?.let { i ->
        PageTools(model, session.pages[i], i, session.pages.size, onView = { tools = null; viewing = i }, onDismiss = { tools = null })
    }
    if (showSettings) ScanSettingsSheet(model, state, onDismiss = { showSettings = false })
    if (showExport) ExportSheet(model, onDismiss = { showExport = false })
}

@Composable
private fun ToolbarButton(icon: Int, description: String, enabled: Boolean = true, onClick: () -> Unit) {
    IconButton(onClick = onClick, enabled = enabled, shapes = IconButtonDefaults.shapes()) { Icon(painterResource(icon), contentDescription = description) }
}

@Composable
private fun SettingsChips(state: AppState, onClick: () -> Unit) {
    val s = state.settings
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        AssistChip(onClick = onClick, label = { Text("${s.dpi} dpi") })
        AssistChip(onClick = onClick, label = { Text(modeLabel(s.mode)) })
        AssistChip(onClick = onClick, label = { Text(sizeName(state, s.size)) })
        if (s.fill != Fill.OFF) AssistChip(onClick = onClick, label = { Text(if (s.fill == Fill.WHITE) "Fill edges white" else "Fill edges paper colour") })
    }
}

@Composable
private fun PageSheet(index: Int, p: Page, fresh: Boolean, dropHere: Boolean, onOpen: () -> Unit, onTools: () -> Unit, modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Surface(
            onClick = onOpen,
            color = androidx.compose.ui.graphics.Color.Transparent,
            border = if (dropHere) BorderStroke(3.dp, MaterialTheme.colorScheme.primary) else null,
            shape = RoundedCornerShape(6.dp),
        ) {
            Paper(Modifier.fillMaxWidth().aspectRatio(p.w.toFloat() / p.h.toFloat()), fresh = fresh) {
                FileImage(p.path, 420, "Page ${index + 1}", Modifier.fillMaxSize())
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("${index + 1}", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(start = 2.dp))
            Text(
                "  ${p.dpi} dpi, ${dur(p.secs)}",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
                maxLines = 1,
            )
            IconButton(onClick = onTools, shapes = IconButtonDefaults.shapes(), modifier = Modifier.size(36.dp)) {
                Icon(Icons.Default.MoreVert, contentDescription = "Edit page ${index + 1}", Modifier.size(20.dp))
            }
        }
    }
}

fun modeLabel(m: ColorMode) = when (m) {
    ColorMode.COLOR -> "Colour"
    ColorMode.GRAY -> "Gray"
    ColorMode.BLACK_WHITE -> "Black and white"
}

fun sizeName(state: AppState, id: SizeId) = state.caps?.sizes?.firstOrNull { it.id == id }?.name ?: id.name.lowercase().replaceFirstChar { it.uppercase() }

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ScanSettingsSheet(model: AppModel, state: AppState, onDismiss: () -> Unit) {
    val s = state.settings
    val tick = rememberTick()
    var saving by remember { mutableStateOf(false) }
    val resolutions = state.caps?.resolutions?.takeIf { it.isNotEmpty() } ?: listOf(150u, 300u, 600u)
    val modes = state.caps?.modes?.takeIf { it.isNotEmpty() } ?: ColorMode.entries
    Sheet(onDismiss) {
        run {
            Text("Scan settings", style = MaterialTheme.typography.headlineSmall)
            Labeled("Presets") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    state.presets.forEach { p ->
                        if (p.builtin) {
                            FilterChip(selected = p.settings == s, onClick = { tick(); model.setSettings(p.settings) }, label = { Text(p.name) })
                        } else {
                            InputChip(selected = p.settings == s, onClick = { tick(); model.setSettings(p.settings) }, label = { Text(p.name) })
                        }
                    }
                    AssistChip(onClick = { saving = true }, label = { Text("Save these settings…") })
                }
                val own = state.presets.filter { !it.builtin }
                if (own.isNotEmpty()) {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        own.forEach { p -> TextButton(onClick = { model.deletePreset(p.name) }, shapes = ButtonDefaults.shapes()) { Text("Delete ${p.name}") } }
                    }
                }
            }
            Labeled("Resolution") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    resolutions.forEach { r -> FilterChip(selected = s.dpi == r, onClick = { tick(); model.setSettings(s.copy(dpi = r)) }, label = { Text("$r dpi") }) }
                }
                if (s.dpi >= 1200u) Text("A full page at 1200 dpi takes over two minutes.", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Labeled("Colour") { Choice(modes.map { it to modeLabel(it) }, s.mode, { model.setSettings(s.copy(mode = it)) }) }
            Labeled("Original size") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    (state.caps?.sizes?.map { it.id to it.name } ?: listOf(SizeId.A4 to "A4", SizeId.LETTER to "Letter", SizeId.A5 to "A5", SizeId.A6 to "A6")).forEach { (id, name) ->
                        FilterChip(selected = s.size == id, onClick = { tick(); model.setSettings(s.copy(size = id)) }, label = { Text(name) })
                    }
                }
            }
            Labeled("Torn edges and punch holes") {
                Choice(listOf(Fill.OFF to "Leave", Fill.WHITE to "White", Fill.PAPER to "Paper colour"), s.fill, { model.setSettings(s.copy(fill = it)) })
            }
        }
    }
    if (saving) NameDialog("Save preset", "", onDismiss = { saving = false }) { model.savePreset(it); saving = false }
}

@Composable
private fun ExportSheet(model: AppModel, onDismiss: () -> Unit) {
    val saved = remember { model.platen.exportOpts() }
    var opts by remember { mutableStateOf(saved) }
    var limit by remember { mutableStateOf(saved.maxMb?.toString() ?: "") }
    Sheet(onDismiss) {
        run {
            Text("Export", style = MaterialTheme.typography.headlineSmall)
            Text("Saves to ${model.saveFolder.label}", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Section {
                Check("Colour PDF", "Full quality, as scanned", opts.color) { opts = opts.copy(color = it) }
                Check("Share-size PDF", "Smaller, for email and messages", opts.share) { opts = opts.copy(share = it) }
                Check("Print-ready PDF", "Clean black on white", opts.print) { opts = opts.copy(print = it) }
                Check("Images", "One JPEG or PNG per page", opts.images) { opts = opts.copy(images = it) }
            }
            OutlinedTextField(
                limit, { limit = it.filter { c -> c.isDigit() || c == '.' } },
                label = { Text("Keep each PDF under this many MB") },
                singleLine = true,
                shape = MaterialTheme.shapes.medium,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                modifier = Modifier.fillMaxWidth(),
            )
            Toggle("Close the scan afterwards", opts.cleanup) { opts = opts.copy(cleanup = it) }
            Spacer(Modifier.height(4.dp))
            Button(
                onClick = {
                    model.export(ExportOpts(opts.color, opts.share, opts.print, opts.images, opts.cleanup, limit.toFloatOrNull()))
                    onDismiss()
                },
                enabled = opts.color || opts.share || opts.print || opts.images,
                shapes = ButtonDefaults.shapes(),
                contentPadding = ButtonDefaults.contentPaddingFor(ButtonDefaults.MediumContainerHeight),
                modifier = Modifier.fillMaxWidth().heightIn(ButtonDefaults.MediumContainerHeight),
            ) { Text("Export", style = ButtonDefaults.textStyleFor(ButtonDefaults.MediumContainerHeight)) }
        }
    }
}

@Composable
fun Check(label: String, detail: String? = null, checked: Boolean, onChange: (Boolean) -> Unit) {
    val tick = rememberTick()
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clip(MaterialTheme.shapes.small).toggleable(checked, role = Role.Checkbox) { tick(); onChange(it) }.padding(vertical = 4.dp),
    ) {
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.titleSmall)
            detail?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
        Checkbox(checked, null)
    }
}

@Composable
fun Toggle(label: String, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    val tick = rememberTick()
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.fillMaxWidth().clip(MaterialTheme.shapes.small).toggleable(checked, enabled = enabled, role = Role.Switch) { tick(); onChange(it) }.padding(vertical = 4.dp),
    ) {
        Text(label, Modifier.weight(1f), style = MaterialTheme.typography.titleSmall, color = if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant)
        Switch(checked, null, enabled = enabled)
    }
}

@Composable
fun NameDialog(title: String, initial: String, onDismiss: () -> Unit, onDone: (String) -> Unit) {
    var name by remember { mutableStateOf(TextFieldValue(initial, TextRange(0, initial.length))) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                name, { name = it },
                singleLine = true,
                shape = MaterialTheme.shapes.medium,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (name.text.isNotBlank()) onDone(name.text.trim()) }),
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
            )
        },
        confirmButton = { TextButton(onClick = { onDone(name.text.trim()) }, enabled = name.text.isNotBlank()) { Text("Save") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
