package com.kanamlabs.platen

import android.app.Activity
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.window.layout.FoldingFeature
import androidx.window.layout.WindowInfoTracker
import kotlin.math.roundToInt

data class Hinge(val top: Int, val bottom: Int)

val LocalHinge = staticCompositionLocalOf<Hinge?> { null }

@Composable
fun rememberTabletopHinge(activity: Activity): Hinge? {
    val tracker = remember(activity) { WindowInfoTracker.getOrCreate(activity).windowLayoutInfo(activity) }
    val info by tracker.collectAsStateWithLifecycle(null)
    val fold = info?.displayFeatures?.filterIsInstance<FoldingFeature>()?.firstOrNull {
        it.state == FoldingFeature.State.HALF_OPENED && it.orientation == FoldingFeature.Orientation.HORIZONTAL
    }
    return fold?.bounds?.let { Hinge(it.top, it.bottom) }
}

@Composable
fun isTabletop() = LocalHinge.current != null

@Composable
fun TabletopSplit(modifier: Modifier = Modifier, top: @Composable BoxScope.() -> Unit, bottom: @Composable BoxScope.() -> Unit) {
    val hinge = LocalHinge.current ?: return
    var origin by remember { mutableIntStateOf(0) }
    Layout(
        content = {
            Box(Modifier.padding(bottom = 12.dp), content = top)
            Box(Modifier.padding(top = 12.dp), content = bottom)
        },
        modifier = modifier.onGloballyPositioned { origin = it.positionInWindow().y.roundToInt() },
    ) { (upper, lower), c ->
        val split = (hinge.top - origin).coerceIn(0, c.maxHeight)
        val below = (hinge.bottom - origin).coerceIn(split, c.maxHeight)
        val t = upper.measure(Constraints.fixed(c.maxWidth, split))
        val b = lower.measure(Constraints.fixed(c.maxWidth, c.maxHeight - below))
        layout(c.maxWidth, c.maxHeight) {
            t.place(0, 0)
            b.place(0, below)
        }
    }
}
