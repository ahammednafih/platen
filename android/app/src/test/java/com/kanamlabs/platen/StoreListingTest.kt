package com.kanamlabs.platen

import android.provider.Settings
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import com.github.takahirom.roborazzi.captureRoboImage
import com.kanamlabs.platen.core.Device
import com.kanamlabs.platen.core.DeviceStatus
import com.kanamlabs.platen.core.Ink
import com.kanamlabs.platen.core.PrinterStatus
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import java.io.File

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36])
class StoreListingTest {
    @get:Rule val compose = createComposeRule()

    private val app get() = RuntimeEnvironment.getApplication() as PlatenApp

    @Before fun onlyWhenAsked() {
        assumeTrue(System.getProperty("storeListing") != null)
        // Robolectric reports an animator scale of 0, which stalls Compose animations inside a capture
        Settings.Global.putFloat(app.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f)
    }

    @Test @Config(qualifiers = PHONE) fun phoneScan() = shot("phoneScreenshots/1_scan")
    @Test @Config(qualifiers = PHONE) fun phoneTools() = shot("phoneScreenshots/2_tools") { click("Edit page 1") }
    @Test @Config(qualifiers = PHONE) fun phoneCrop() = shot("phoneScreenshots/3_crop") { click("Edit page 1"); clickText("Crop…") }
    @Test @Config(qualifiers = PHONE) fun phoneCopy() = shot("phoneScreenshots/4_copy") { clickText("Copy") }
    @Test @Config(qualifiers = PHONE) fun phonePrint() = shot("phoneScreenshots/5_print", print = true)
    @Test @Config(qualifiers = PHONE) fun phoneDevice() = shot("phoneScreenshots/6_device") { click("Printer and settings") }

    @Test @Config(qualifiers = TABLET) fun tabletScan() = shot("tabletScreenshots/1_scan")
    @Test @Config(qualifiers = TABLET) fun tabletCopy() = shot("tabletScreenshots/2_copy") { clickText("Copy") }
    @Test @Config(qualifiers = TABLET) fun tabletPrint() = shot("tabletScreenshots/3_print", print = true)
    @Test @Config(qualifiers = TABLET) fun tabletDevice() = shot("tabletScreenshots/4_device") { click("Printer and settings") }

    @Test @Config(qualifiers = UNFOLDED) fun foldableScan() = shot("foldableScreenshots/1_scan")
    @Test @Config(qualifiers = UNFOLDED) fun foldableCopy() = shot("foldableScreenshots/2_copy") { clickText("Copy") }
    @Test @Config(qualifiers = UNFOLDED) fun foldablePrint() = shot("foldableScreenshots/3_print", print = true)
    @Test @Config(qualifiers = UNFOLDED) fun foldableDevice() = shot("foldableScreenshots/4_device") { click("Printer and settings") }
    @Test @Config(qualifiers = UNFOLDED_WIDE) fun foldableWideScan() = shot("foldableScreenshots/5_scan_wide")
    @Test @Config(qualifiers = FOLDED) fun foldedScan() = shot("foldableScreenshots/6_scan_folded")

    private fun shot(name: String, print: Boolean = false, steps: () -> Unit = {}) {
        val model = AppModel(app)
        val pages = fixtures()
        val state = scene(model, pages)
        compose.setContent {
            PlatenTheme(Accent.Ink) {
                AppRoot(model, state, if (print) pages.take(2) else emptyList()) {}
            }
        }
        settle()
        steps()
        settle()
        compose.onRoot().captureRoboImage("build/store-listing/$name.png")
    }

    private fun scene(model: AppModel, pages: List<File>): AppState {
        app.platen.drafts().forEach { app.platen.deleteDraft(it.name) }
        app.platen.openSession("Country loaf")
        val session = app.platen.importFiles(listOf(pages[0], pages[1], pages[0]).map { it.path }, null).session
        return model.state.value.copy(
            device = Device(name = "Office printer", host = "192.168.1.20"),
            finding = false,
            status = DeviceStatus(
                scanner = "Idle",
                printer = PrinterStatus(
                    model = "Inkjet all-in-one",
                    state = "Idle",
                    reasons = emptyList(),
                    ink = listOf(Ink("Magenta", 82), Ink("Cyan", 74), Ink("Yellow", 90), Ink("Black", 61)),
                ),
            ),
            session = session,
            printers = listOf(SavedPrinter(base = "http://192.168.1.20", model = "Inkjet all-in-one", nickname = "Office printer")),
            current = "http://192.168.1.20",
            message = null,
            busy = null,
        )
    }

    private fun fixtures(): List<File> = listOf("country-loaf.jpg", "variations.jpg").map { name ->
        File(app.cacheDir, name).apply { StoreListingTest::class.java.getResourceAsStream("/store/$name")!!.use { input -> outputStream().use { input.copyTo(it) } } }
    }

    private fun click(description: String) = compose.onNodeWithContentDescription(description).performClick().also { settle() }

    private fun clickText(text: String) = compose.onNodeWithText(text).performClick().also { settle() }

    private fun settle() = repeat(15) {
        compose.mainClock.advanceTimeBy(100)
        Thread.sleep(40)
        compose.waitForIdle()
    }

    private companion object {
        const val PHONE = "w411dp-h821dp-420dpi"
        const val TABLET = "w1280dp-h800dp-xhdpi"
        const val UNFOLDED = "w720dp-h720dp-xxhdpi"
        const val UNFOLDED_WIDE = "w960dp-h720dp-xhdpi"
        const val FOLDED = "w400dp-h640dp-xxhdpi"
    }
}
