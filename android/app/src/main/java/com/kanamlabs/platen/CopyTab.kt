package com.kanamlabs.platen

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Surface
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.kanamlabs.platen.core.Area
import com.kanamlabs.platen.core.PlatenException
import com.kanamlabs.platen.core.PrintJob
import com.kanamlabs.platen.core.PrintOpts
import com.kanamlabs.platen.core.PrintProgress
import com.kanamlabs.platen.core.Quality
import com.kanamlabs.platen.core.Scale
import com.kanamlabs.platen.core.SizeId
import com.kanamlabs.platen.core.Subset
import com.kanamlabs.platen.core.TopHalf
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private enum class CopyKind(val title: String, val short: String, val glyph: Glyph, val steps: List<Pair<String, String>>) {
    One("Copy", "One page, one sheet", Glyph.One, listOf("Put the page face down on the glass, top edge against the far edge." to "Copy")),
    TwoInOne(
        "2 in 1", "Two pages side by side", Glyph.TwoInOne,
        listOf("Put the first page face down on the glass. It goes on the left of the sheet." to "Scan first page", "Put the second page face down on the glass. The copy prints right after this scan." to "Scan second page"),
    ),
    FourInOne(
        "4 in 1", "Four pages on one sheet", Glyph.FourInOne,
        listOf(
            "Put the first page face down on the glass. It goes top left on the sheet." to "Scan page 1",
            "Put the second page face down on the glass. It goes top right." to "Scan page 2",
            "Put the third page face down on the glass. It goes bottom left." to "Scan page 3",
            "Put the fourth page face down on the glass. The copy prints right after this scan." to "Scan page 4",
        ),
    ),
    IdCard(
        "ID card", "Both sides, actual size", Glyph.IdCard,
        listOf("Put the card face down in the top half of the glass, a little away from the corner." to "Scan front", "Turn the card over and put it back in the same spot. The copy prints right after this scan." to "Scan back"),
    ),
    Long(
        "Long page", "Taller than the glass", Glyph.Long,
        listOf("Put the top of the page face down on the glass, top edge against the far edge." to "Scan top half", "Turn the page around so the bottom is on the glass. Leave a few centimetres of overlap." to "Scan bottom half"),
    ),
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CopyTab(model: AppModel, state: AppState, padding: PaddingValues) {
    val scope = rememberCoroutineScope()
    var kind by rememberSaveable { mutableStateOf(CopyKind.One) }
    var original by rememberSaveable { mutableStateOf(SizeId.A4) }
    var copies by rememberSaveable { mutableIntStateOf(1) }
    var color by rememberSaveable { mutableStateOf(false) }
    var density by rememberSaveable { mutableIntStateOf(0) }
    var background by rememberSaveable { mutableIntStateOf(0) }
    var media by rememberSaveable { mutableStateOf(state.printCaps?.mediaDefault ?: "iso_a4_210x297mm") }
    var quality by rememberSaveable { mutableStateOf(Quality.NORMAL) }
    var step by remember { mutableIntStateOf(0) }
    val scans = remember { mutableListOf<String>() }
    var top by remember { mutableStateOf<TopHalf?>(null) }
    var busy by remember { mutableStateOf<String?>(null) }
    var log by remember { mutableStateOf<String?>(null) }

    DisposableEffect(Unit) { onDispose { model.platen.copyReset() } }

    fun reset() {
        step = 0
        scans.clear()
        top = null
        model.platen.copyReset()
    }

    fun go() = scope.launch {
        log = null
        busy = "Scanning"
        try {
            withContext(Dispatchers.IO) {
                val path = when {
                    kind == CopyKind.Long && step == 0 -> model.platen.copyLongTop(color).also { top = it }.path
                    kind == CopyKind.Long -> model.platen.copyLongBottom(top!!, color)
                    else -> model.platen.copyScan(if (kind == CopyKind.IdCard) Area.HalfTop else Area.Size(original), color)
                }
                scans += path
            }
            if (step < kind.steps.size - 1) {
                step++
                return@launch
            }
            var files = if (kind == CopyKind.Long) listOf(scans.last()) else scans.toList()
            withContext(Dispatchers.IO) {
                if (kind == CopyKind.IdCard) {
                    busy = "Placing front and back"
                    files = listOf(model.platen.copyIdCard(scans[0], scans[1]))
                }
                if (density != 0 || background != 0) {
                    busy = "Adjusting"
                    files = model.platen.copyTone(files, density, background.toUByte())
                }
                busy = "Printing"
                val scale = when (kind) {
                    CopyKind.One -> Scale.Actual
                    CopyKind.IdCard -> Scale.Actual
                    else -> Scale.Fit
                }
                val job = PrintJob(files, "", Subset.ALL, false, when (kind) { CopyKind.TwoInOne -> 2u; CopyKind.FourInOne -> 4u; else -> 1u }, scale, true, false, PrintOpts(copies.toUInt(), color, media, "fit", quality))
                model.platen.print(job, object : PrintProgress {
                    override fun progress(done: UInt, total: UInt) {
                        busy = "Printing $done of $total"
                    }
                })
            }
            log = "Sent $copies ${if (copies == 1) "copy" else "copies"} to the printer."
            reset()
        } catch (e: PlatenException) {
            log = if (e.message == "stopped") "Stopped. Nothing else will print." else e.message
        } finally {
            busy = null
        }
    }

    val tick = rememberTick()
    Column(Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
        LowInk(state)
        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
            CopyKind.entries.chunked(2).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    row.forEach { k ->
                        val on = kind == k
                        Surface(
                            onClick = { if (busy == null && kind != k) { tick(); kind = k; log = null; reset() } },
                            shape = MaterialTheme.shapes.large,
                            color = if (on) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerLow,
                            border = if (on) BorderStroke(2.dp, MaterialTheme.colorScheme.primary) else null,
                            modifier = Modifier.weight(1f),
                        ) {
                            Row(Modifier.padding(14.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                                SheetGlyph(k.glyph, active = on, width = 30.dp)
                                Column {
                                    Text(k.title, style = MaterialTheme.typography.titleMedium)
                                    Text(k.short, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2)
                                }
                            }
                        }
                    }
                    if (row.size == 1) Spacer(Modifier.weight(1f))
                }
            }
        }

        GlassBed(Modifier.fillMaxWidth()) {
            Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                if (kind.steps.size > 1) {
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        kind.steps.indices.forEach { i ->
                            Box(Modifier.size(width = if (i == step) 28.dp else 10.dp, height = 10.dp).clip(CircleShape).background(if (i <= step) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline))
                        }
                    }
                }
                AnimatedContent(step to kind, transitionSpec = { fadeIn() togetherWith fadeOut() }, label = "step") { (st, k) ->
                    Text(k.steps[st].first, style = MaterialTheme.typography.titleLarge)
                }
                AnimatedContent(busy, transitionSpec = { fadeIn() togetherWith fadeOut() }, label = "busy") { b ->
                    if (b != null) {
                        BusyPill("$b…") {
                            TextButton(onClick = { scope.launch(Dispatchers.IO) { runCatching { model.platen.stopJob() } } }, shapes = ButtonDefaults.shapes()) {
                                Text("Stop", color = MaterialTheme.colorScheme.inverseOnSurface)
                            }
                        }
                    } else {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                            Button(
                                onClick = { go() },
                                enabled = state.device != null,
                                shapes = ButtonDefaults.shapes(),
                                contentPadding = ButtonDefaults.contentPaddingFor(ButtonDefaults.MediumContainerHeight),
                                modifier = Modifier.weight(1f).heightIn(ButtonDefaults.MediumContainerHeight),
                            ) { Text(kind.steps[step].second, style = ButtonDefaults.textStyleFor(ButtonDefaults.MediumContainerHeight)) }
                            if (step > 0) TextButton(onClick = { reset() }, shapes = ButtonDefaults.shapes()) { Text("Start over") }
                        }
                    }
                }
                log?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant) }
            }
        }

        Section("Copy") {
            if (kind == CopyKind.One || kind == CopyKind.TwoInOne || kind == CopyKind.FourInOne) {
                Labeled("Original size") {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        (state.caps?.sizes?.map { it.id to it.name } ?: listOf(SizeId.A4 to "A4", SizeId.LETTER to "Letter", SizeId.A5 to "A5")).forEach { (id, name) ->
                            FilterChip(selected = original == id, onClick = { tick(); original = id }, label = { Text(name) })
                        }
                    }
                }
            }
            Stepper("Copies", copies, 1..99) { copies = it }
            Labeled("Colour") { Choice(listOf(false to "Black and white", true to "Colour"), color, { color = it }) }
            Labeled("Density: ${if (density > 0) "+$density darker" else if (density < 0) "$density lighter" else "normal"}") {
                Slider(density.toFloat(), { v -> val d = Math.round(v); if (d != density) tick(); density = d }, valueRange = -3f..3f, steps = 5)
            }
            Labeled("Background") { Choice(listOf(0 to "Keep", 1 to "Low", 2 to "Medium", 3 to "High"), background, { background = it }) }
        }
        Section("Paper") { PaperAndQuality(state, media, { media = it }, quality, { quality = it }) }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PaperAndQuality(state: AppState, media: String, onMedia: (String) -> Unit, quality: Quality, onQuality: (Quality) -> Unit) {
    val papers = (state.printCaps?.media ?: listOf("iso_a4_210x297mm", "na_letter_8.5x11in")).filter { !it.startsWith("custom_") }
    val qualities = state.printCaps?.qualities?.takeIf { it.isNotEmpty() } ?: Quality.entries
    Labeled("Paper") {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            papers.forEach { p -> FilterChip(selected = media == p, onClick = { onMedia(p) }, label = { Text(paperName(p)) }) }
        }
    }
    Labeled("Quality") { Choice(qualities.map { it to it.name.lowercase().replaceFirstChar { c -> c.uppercase() } }, quality, onQuality, Modifier.fillMaxWidth()) }
}
