package com.kanamlabs.platen

import android.view.HapticFeedbackConstants
import android.view.View
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.Icon
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.foundation.layout.Spacer
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

object Haptics {
    fun confirm(view: View) = view.performHapticFeedback(HapticFeedbackConstants.CONFIRM)
    fun reject(view: View) = view.performHapticFeedback(HapticFeedbackConstants.REJECT)
    fun tick(view: View) = view.performHapticFeedback(HapticFeedbackConstants.SEGMENT_TICK)
    fun click(view: View) = view.performHapticFeedback(HapticFeedbackConstants.CONTEXT_CLICK)
}

@Composable
fun rememberTick(): () -> Unit {
    val view = LocalView.current
    return remember(view) { { Haptics.tick(view) } }
}

@Composable
fun rememberClick(): () -> Unit {
    val view = LocalView.current
    return remember(view) { { Haptics.click(view) } }
}

@Composable
fun GlassBed(modifier: Modifier = Modifier, shape: Shape = RoundedCornerShape(28.dp), content: @Composable BoxScope.() -> Unit) {
    val c = MaterialTheme.colorScheme
    Box(modifier.clip(shape).background(c.surfaceContainer).border(1.dp, c.outlineVariant, shape)) { content() }
}

@Composable
fun Paper(modifier: Modifier = Modifier, fresh: Boolean = false, content: @Composable () -> Unit) {
    val drop = remember { Animatable(if (fresh) 0f else 1f) }
    LaunchedEffect(Unit) { drop.animateTo(1f, spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessLow)) }
    Box(
        modifier
            .graphicsLayer {
                val s = 0.92f + 0.08f * drop.value
                scaleX = s
                scaleY = s
                alpha = drop.value.coerceIn(0f, 1f)
                translationY = (1f - drop.value) * -24.dp.toPx()
            }
            .shadow(2.dp, RoundedCornerShape(4.dp), ambientColor = Color.Black.copy(alpha = 0.18f), spotColor = Color.Black.copy(alpha = 0.22f))
            .clip(RoundedCornerShape(4.dp))
            .background(Color.White)
            .border(0.5.dp, Color.Black.copy(alpha = 0.08f), RoundedCornerShape(4.dp)),
    ) { content() }
}

@Composable
fun Section(title: String? = null, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        title?.let { Text(it, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 4.dp)) }
        Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.surfaceContainerLow, tonalElevation = 0.dp) {
            Column(Modifier.fillMaxWidth().padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp), content = content)
        }
    }
}

@Composable
fun BusyPill(label: String, modifier: Modifier = Modifier, trailing: @Composable () -> Unit = {}) {
    Surface(shape = MaterialTheme.shapes.extraLarge, color = MaterialTheme.colorScheme.inverseSurface, contentColor = MaterialTheme.colorScheme.inverseOnSurface, modifier = modifier) {
        Row(Modifier.padding(start = 8.dp, end = 8.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            LoadingIndicator(Modifier.size(36.dp), color = MaterialTheme.colorScheme.inverseOnSurface)
            Text(label, style = MaterialTheme.typography.labelLarge, modifier = Modifier.weight(1f, fill = false).padding(end = 4.dp))
            trailing()
        }
    }
}

enum class Glyph { One, TwoInOne, FourInOne, IdCard, Long, Poster, Empty }

@Composable
fun SheetGlyph(kind: Glyph, active: Boolean, modifier: Modifier = Modifier, width: Dp = 40.dp) {
    val ink = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f)
    val edge = MaterialTheme.colorScheme.outline
    val blank = MaterialTheme.colorScheme.surfaceContainerLowest.takeIf { MaterialTheme.colorScheme.surface.luminance() < 0.3f }?.let { MaterialTheme.colorScheme.surfaceContainerHigh } ?: Color.White
    Canvas(modifier.size(width, width * 1.36f)) {
        val corner = CornerRadius(3.dp.toPx())
        drawRoundRect(if (kind == Glyph.Empty) blank else Color.White, cornerRadius = corner)
        if (kind == Glyph.Empty) {
            drawRoundRect(edge, cornerRadius = corner, style = Stroke(1.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f))))
            return@Canvas
        }
        drawRoundRect(edge, cornerRadius = corner, style = Stroke(1.dp.toPx()))
        val pad = size.width * 0.14f
        val w = size.width - pad * 2
        val h = size.height - pad * 2
        when (kind) {
            Glyph.One -> lines(ink, Offset(pad, pad), Size(w, h), 6)
            Glyph.TwoInOne -> {
                val half = (w - pad * 0.6f) / 2
                lines(ink, Offset(pad, pad), Size(half, h * 0.62f), 4)
                lines(ink, Offset(pad + half + pad * 0.6f, pad), Size(half, h * 0.62f), 4)
            }
            Glyph.FourInOne -> {
                val half = (w - pad * 0.6f) / 2
                val tall = (h - pad * 0.6f) / 2
                for (row in 0..1) for (col in 0..1) {
                    lines(ink, Offset(pad + col * (half + pad * 0.6f), pad + row * (tall + pad * 0.6f)), Size(half, tall * 0.8f), 3)
                }
            }
            Glyph.Poster -> {
                lines(ink.copy(alpha = 0.35f), Offset(pad, pad), Size(w, h), 6)
                val third = w / 3f
                val tall = h / 3f
                for (i in 1..2) {
                    drawLine(ink, Offset(pad + i * third, pad), Offset(pad + i * third, pad + h), 1.5.dp.toPx())
                    drawLine(ink, Offset(pad, pad + i * tall), Offset(pad + w, pad + i * tall), 1.5.dp.toPx())
                }
            }
            Glyph.IdCard -> {
                val cw = w * 0.72f
                val ch = cw * 0.63f
                drawRoundRect(ink, Offset(pad + (w - cw) / 2, pad), Size(cw, ch), CornerRadius(2.dp.toPx()))
                drawRoundRect(ink.copy(alpha = 0.6f), Offset(pad + (w - cw) / 2, pad + ch + pad * 0.7f), Size(cw, ch), CornerRadius(2.dp.toPx()))
            }
            Glyph.Long -> {
                lines(ink, Offset(pad + w * 0.18f, pad), Size(w * 0.64f, h), 9)
            }
            Glyph.Empty -> Unit
        }
    }
}

private fun DrawScope.lines(color: Color, at: Offset, area: Size, count: Int) {
    val gap = area.height / (count * 1.6f)
    val bar = gap * 0.75f
    for (i in 0 until count) {
        val w = if (i == count - 1) area.width * 0.6f else area.width
        drawRoundRect(color, Offset(at.x, at.y + i * (bar + gap)), Size(w, bar), CornerRadius(bar / 2))
    }
}

fun inkColor(name: String, dark: Boolean): Color {
    val n = name.lowercase()
    return when {
        "cyan" in n -> Color(0xFF00A6D6)
        "magenta" in n -> Color(0xFFD6247D)
        "yellow" in n -> Color(0xFFF2C200)
        dark -> Color(0xFF8E9199)
        else -> Color(0xFF2A2B2E)
    }
}

@Composable
fun InkTank(name: String, level: Int, modifier: Modifier = Modifier) {
    val fill = remember { Animatable(0f) }
    LaunchedEffect(level) { fill.animateTo(level.coerceIn(0, 100) / 100f, spring(stiffness = Spring.StiffnessVeryLow)) }
    val color = inkColor(name, MaterialTheme.colorScheme.surface.luminance() < 0.3f)
    val track = MaterialTheme.colorScheme.surfaceContainerHigh
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Canvas(Modifier.size(40.dp, 96.dp)) {
            val r = CornerRadius(12.dp.toPx())
            drawRoundRect(track, cornerRadius = r)
            val h = size.height * fill.value
            drawRoundRect(color, Offset(0f, size.height - h), Size(size.width, h), r)
        }
        Text("${level.coerceAtLeast(0)}%", style = MaterialTheme.typography.labelLarge)
        Text(name.replaceFirstChar { it.uppercase() }, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
    }
}

@Composable
fun Swatch(color: Color, selected: Boolean, modifier: Modifier = Modifier) {
    val ring = MaterialTheme.colorScheme.onSurface
    Canvas(modifier.size(40.dp)) {
        drawCircle(color, radius = size.minDimension / 2 - 4.dp.toPx())
        if (selected) drawCircle(ring, radius = size.minDimension / 2 - 1.dp.toPx(), style = Stroke(2.dp.toPx()))
    }
}

@Composable
fun Stepper(label: String, value: Int, range: IntRange, onChange: (Int) -> Unit) {
    val tick = rememberTick()
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, Modifier.weight(1f), style = MaterialTheme.typography.titleSmall)
        FilledTonalIconButton(
            onClick = { tick(); onChange(value - 1) },
            enabled = value > range.first,
            shapes = IconButtonDefaults.shapes(),
        ) { Text("−", style = MaterialTheme.typography.titleLarge) }
        AnimatedContent(value, transitionSpec = {
            val up = targetState > initialState
            (slideInVertically { if (up) it else -it } + fadeIn()) togetherWith (slideOutVertically { if (up) -it else it } + fadeOut())
        }, label = "stepper") { v ->
            Text("$v", style = MaterialTheme.typography.headlineSmall, textAlign = TextAlign.Center, modifier = Modifier.width(56.dp))
        }
        FilledTonalIconButton(
            onClick = { tick(); onChange(value + 1) },
            enabled = value < range.last,
            shapes = IconButtonDefaults.shapes(),
        ) { Text("+", style = MaterialTheme.typography.titleLarge) }
    }
}

@Composable
fun Sheet(onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = MaterialTheme.colorScheme.surface,
    ) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).padding(bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(16.dp), content = content)
    }
}

@Composable
fun LowInk(state: AppState) {
    val low = state.status?.printer?.ink?.filter { it.level in 0..15 }.orEmpty()
    AnimatedVisibility(low.isNotEmpty(), enter = fadeIn() + expandVertically(), exit = fadeOut() + shrinkVertically()) {
        Surface(shape = MaterialTheme.shapes.large, color = MaterialTheme.colorScheme.errorContainer, contentColor = MaterialTheme.colorScheme.onErrorContainer) {
            Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                low.forEach { Box(Modifier.size(12.dp).clip(RoundedCornerShape(6.dp)).background(inkColor(it.name, false))) }
                Text(
                    low.joinToString(" and ") { "${it.name.replaceFirstChar { c -> c.uppercase() }} (${it.level}%)" } + if (low.size == 1) " ink is low" else " inks are low",
                    style = MaterialTheme.typography.titleSmall,
                )
            }
        }
    }
}

@Composable
fun PrinterSheet(model: AppModel, state: AppState, onSettings: () -> Unit, onDismiss: () -> Unit) {
    val tick = rememberTick()
    var renaming by remember { mutableStateOf<SavedPrinter?>(null) }
    Sheet(onDismiss) {
        Text("Printers", style = MaterialTheme.typography.headlineSmall)
        if (state.printers.isEmpty()) Text("No printers saved yet. Look for one on this Wi-Fi.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        state.printers.forEach { p ->
            val current = p.base == state.current
            Surface(
                onClick = { if (!current) { tick(); model.usePrinter(p); onDismiss() } },
                shape = MaterialTheme.shapes.large,
                color = if (current) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerLow,
            ) {
                Row(Modifier.fillMaxWidth().padding(start = 16.dp, top = 8.dp, bottom = 8.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(p.label, style = MaterialTheme.typography.titleMedium, maxLines = 1)
                        Text(
                            listOfNotNull(p.model.takeIf { p.nickname != null }, p.base.substringAfter("//").substringBefore('/').substringBefore(':')).joinToString(", "),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                        )
                    }
                    if (current) Text("In use", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(horizontal = 8.dp))
                    IconButton(onClick = { renaming = p }, shapes = IconButtonDefaults.shapes()) {
                        Icon(androidx.compose.material.icons.Icons.Default.Edit, contentDescription = "Rename ${p.label}")
                    }
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            androidx.compose.material3.FilledTonalButton(onClick = { tick(); model.searchPrinters() }, enabled = !state.searching, shapes = androidx.compose.material3.ButtonDefaults.shapes()) {
                Text("Look for printers")
            }
            if (state.searching) LoadingIndicator(Modifier.size(32.dp))
            Spacer(Modifier.weight(1f))
            androidx.compose.material3.TextButton(onClick = onSettings, shapes = androidx.compose.material3.ButtonDefaults.shapes()) { Text("Printer settings") }
        }
    }
    renaming?.let { p -> NameDialog("Rename printer", p.nickname ?: p.model, onDismiss = { renaming = null }) { model.renamePrinter(p, it); renaming = null } }
}
