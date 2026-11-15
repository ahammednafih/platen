package com.kanamlabs.platen

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.kanamlabs.platen.core.ShrinkResult
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

private val Limits = listOf(0.5f to "½ MB", 1f to "1 MB", 2f to "2 MB", 5f to "5 MB")

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ShrinkScreen(model: AppModel, file: File, folder: String?, onClose: () -> Unit) {
    val context = LocalContext.current
    val state by model.state.collectAsStateWithLifecycle()
    val tick = rememberTick()
    var count by remember { mutableIntStateOf(0) }
    var skip by remember { mutableStateOf(setOf<Int>()) }
    var limit by remember { mutableStateOf<Float?>(2f) }
    var custom by remember { mutableStateOf(false) }
    var customText by remember { mutableStateOf("") }
    var result by remember { mutableStateOf<ShrinkResult?>(null) }
    var comparing by remember { mutableStateOf(false) }
    LaunchedEffect(file) { count = withContext(Dispatchers.IO) { runCatching { AndroidPdf.pageCount(file) }.getOrDefault(0) } }

    val target = if (custom) customText.toFloatOrNull() else limit
    fun discard() {
        result?.let { File(it.out).delete() }
        result = null
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        topBar = {
            TopAppBar(
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.surface),
                title = { Text("Shrink", style = MaterialTheme.typography.headlineSmall) },
                subtitle = { Text(file.name, maxLines = 1, color = MaterialTheme.colorScheme.onSurfaceVariant) },
                navigationIcon = { IconButton(onClick = { discard(); onClose() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
            )
        },
        bottomBar = {
            Surface(color = MaterialTheme.colorScheme.surface) {
                Box(Modifier.fillMaxWidth().navigationBarsPadding().padding(16.dp)) {
                    AnimatedContent(Triple(state.busy, result, target), transitionSpec = { fadeIn() togetherWith fadeOut() }, label = "shrink") { (busy, r, t) ->
                        when {
                            busy != null -> BusyPill("$busy…", Modifier.fillMaxWidth())
                            r != null -> ResultBar(
                                r,
                                onSave = {
                                    val out = File(r.out)
                                    model.saveFile(out, folder) { out.delete(); result = null; onClose() }
                                },
                                onShare = { shareFile(context, File(r.out)) },
                                onCompare = { comparing = true },
                                onDiscard = ::discard,
                            )
                            else -> Button(
                                onClick = { model.work("Shrinking", { model.platen.shrinkPdf(file.path, t, skip.map { (it + 1).toUInt() }) }) { result = it } },
                                enabled = (t != null || skip.isNotEmpty()) && skip.size < count,
                                shapes = ButtonDefaults.shapes(),
                                contentPadding = ButtonDefaults.contentPaddingFor(ButtonDefaults.MediumContainerHeight),
                                modifier = Modifier.fillMaxWidth().heightIn(ButtonDefaults.MediumContainerHeight),
                            ) {
                                Text(
                                    when {
                                        t == null -> "Leave out ${plural(skip.size, "page")}"
                                        skip.isEmpty() -> "Shrink under ${fmt(t)} MB"
                                        else -> "Shrink and leave out ${plural(skip.size, "page")}"
                                    },
                                    style = ButtonDefaults.textStyleFor(ButtonDefaults.MediumContainerHeight),
                                )
                            }
                        }
                    }
                }
            }
        },
    ) { padding ->
        LazyVerticalGrid(
            columns = GridCells.Adaptive(104.dp),
            modifier = Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Section("Keep it under") {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Limits.forEach { (mb, label) ->
                            FilterChip(selected = !custom && limit == mb, onClick = { tick(); custom = false; limit = mb; discard() }, label = { Text(label) })
                        }
                        FilterChip(selected = custom, onClick = { tick(); custom = true; discard() }, label = { Text("Other") })
                        FilterChip(selected = !custom && limit == null, onClick = { tick(); custom = false; limit = null; discard() }, label = { Text("No limit") })
                    }
                    if (custom) {
                        OutlinedTextField(
                            customText, { customText = it.filter { c -> c.isDigit() || c == '.' }; discard() },
                            label = { Text("Megabytes") },
                            singleLine = true,
                            shape = MaterialTheme.shapes.medium,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }
                }
            }
            item(span = { GridItemSpan(maxLineSpan) }) {
                Text(
                    if (skip.isEmpty()) "Tap a page to leave it out" else "${plural(count - skip.size, "page")} kept, ${skip.size} left out",
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, top = 8.dp),
                )
            }
            items((0 until count).toList()) { i ->
                val out = i in skip
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Surface(onClick = { tick(); skip = if (out) skip - i else skip + i; discard() }, color = Color.Transparent) {
                        Paper(Modifier.fillMaxWidth().aspectRatio(0.72f).graphicsLayer { alpha = if (out) 0.3f else 1f }) {
                            FileImage(file.path, 240, "Page ${i + 1}", Modifier.fillMaxSize(), page = i)
                        }
                    }
                    Text(
                        if (out) "Left out" else "${i + 1}",
                        style = MaterialTheme.typography.labelLarge,
                        color = if (out) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                    )
                }
            }
        }
    }
    if (comparing) result?.let { PdfCompare(listOf(file.path, it.out), 0, onClose = { comparing = false }) }
}

@Composable
private fun ResultBar(r: ShrinkResult, onSave: () -> Unit, onShare: () -> Unit, onCompare: () -> Unit, onDiscard: () -> Unit) {
    val saved = if (r.before > 0u) 100 - (r.after.toLong() * 100 / r.before.toLong()) else 0
    Surface(shape = RoundedCornerShape(28.dp), color = MaterialTheme.colorScheme.primaryContainer) {
        Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(size(r.after.toLong()), style = MaterialTheme.typography.headlineMedium, color = MaterialTheme.colorScheme.onPrimaryContainer)
                Text("from ${size(r.before.toLong())}", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(bottom = 4.dp))
            }
            Text(
                if (r.fits) "$saved% smaller, ${plural(r.pages.toInt(), "page")}" else "As small as it gets without losing pages, ${plural(r.pages.toInt(), "page")}",
                style = MaterialTheme.typography.bodyMedium,
                color = if (r.fits) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.error,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Button(onClick = onSave, shapes = ButtonDefaults.shapes(), modifier = Modifier.weight(1f)) { Text("Save") }
                FilledTonalButton(onClick = onShare, shapes = ButtonDefaults.shapes()) { Text("Share") }
                FilledTonalButton(onClick = onCompare, shapes = ButtonDefaults.shapes()) { Text("Compare") }
            }
            TextButton(onClick = onDiscard, shapes = ButtonDefaults.shapes()) { Text("Try a different size") }
        }
    }
}

fun plural(n: Int, word: String) = if (n == 1) "1 $word" else "$n ${word}s"

private fun fmt(mb: Float) = if (mb % 1f == 0f) mb.toInt().toString() else mb.toString()
