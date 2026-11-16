package com.kanamlabs.platen

import android.graphics.Bitmap
import android.util.Size
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Compress
import androidx.compose.material.icons.outlined.Print
import androidx.compose.material.icons.outlined.Share
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import java.text.DateFormat
import java.util.Date

@Composable
fun FilesTab(model: AppModel, state: AppState, padding: PaddingValues, onPrint: (File) -> Unit, onShrink: (File, String?) -> Unit) {
    val context = LocalContext.current
    LaunchedEffect(Unit) { model.loadFiles() }
    val pickPdf = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) model.work("Opening", { model.saveFolder.copyToCache(uri, "file.pdf") }) { onShrink(it, null) }
    }

    LazyVerticalGrid(
        GridCells.Adaptive(360.dp),
        Modifier.fillMaxSize().padding(padding),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        item(span = { GridItemSpan(maxLineSpan) }) {
            Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("Saved in", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(state.folder, style = MaterialTheme.typography.titleMedium)
                }
                FilledTonalButton(onClick = { pickPdf.launch(arrayOf("application/pdf")) }, shapes = ButtonDefaults.shapes()) {
                    Icon(painterResource(R.drawable.ic_shrink), contentDescription = null, Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Shrink a PDF")
                }
            }
        }
        if (state.files.isEmpty()) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
                    GlassBed(Modifier.readable()) {
                        Column(Modifier.fillMaxWidth().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp)) {
                            SheetGlyph(Glyph.Empty, active = false, width = 64.dp)
                            Text("Nothing saved yet", style = MaterialTheme.typography.titleLarge)
                            Text("Export a scan and its PDFs and images land here.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
                        }
                    }
                }
            }
        }
        items(state.files, key = { it.uri.toString() }) { f ->
            var menu by remember { mutableStateOf(false) }
            val printable = f.mime == "application/pdf" || f.mime.startsWith("image/")
            val thumb by produceState<Bitmap?>(null, f.uri) {
                value = withContext(Dispatchers.IO) {
                    runCatching {
                        if (f.mime == "application/pdf") AndroidPdf.previewUri(context, f.uri, 160) else context.contentResolver.loadThumbnail(f.uri, Size(160, 220), null)
                    }.getOrNull()
                }
            }
            Surface(onClick = { openUri(context, f.uri, f.mime) }, shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow, modifier = Modifier.animateItem()) {
                Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    Paper(Modifier.size(44.dp, 60.dp)) {
                        thumb?.let { Image(it.asImageBitmap(), null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize()) }
                            ?: Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                                Text(f.name.substringAfterLast('.', "").uppercase(), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                    }
                    Column(Modifier.weight(1f)) {
                        Text(f.name, style = MaterialTheme.typography.titleSmall, maxLines = 2)
                        Text(
                            listOfNotNull(f.folder.ifEmpty { null }, size(f.size), DateFormat.getDateInstance(DateFormat.MEDIUM).format(Date(f.modified))).joinToString(", "),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                        )
                    }
                    Box {
                        IconButton(onClick = { menu = true }, shapes = IconButtonDefaults.shapes()) { Icon(Icons.Default.MoreVert, contentDescription = "More for ${f.name}") }
                        DropdownMenu(menu, { menu = false }) {
                            DropdownMenuItem({ Text("Share") }, { menu = false; shareUri(context, f.uri, f.mime) }, leadingIcon = { Icon(Icons.Outlined.Share, null) })
                            if (printable) DropdownMenuItem({ Text("Print") }, {
                                menu = false
                                model.work("Opening", { model.saveFolder.copyToCache(f.uri, f.name) }, onPrint)
                            }, leadingIcon = { Icon(Icons.Outlined.Print, null) })
                            if (f.mime == "application/pdf") DropdownMenuItem({ Text("Shrink") }, {
                                menu = false
                                model.work("Opening", { model.saveFolder.copyToCache(f.uri, f.name) }) { onShrink(it, f.folder.ifEmpty { null }) }
                            }, leadingIcon = { Icon(Icons.Outlined.Compress, null) })
                            DropdownMenuItem({ Text("Delete", color = MaterialTheme.colorScheme.error) }, { menu = false; model.deleteFile(f) }, leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) })
                        }
                    }
                }
            }
        }
    }
}
