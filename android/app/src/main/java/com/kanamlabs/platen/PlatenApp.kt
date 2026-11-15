package com.kanamlabs.platen

import android.app.Application
import com.kanamlabs.platen.core.Platen
import java.io.File

class PlatenApp : Application() {
    val platen: Platen by lazy {
        Platen(
            config = File(filesDir, "config").apply { mkdirs() }.path,
            cache = cacheDir.path,
            scans = File(filesDir, "scans").apply { mkdirs() }.path,
            renderer = AndroidPdf,
        )
    }

    val saveFolder by lazy { SaveFolder(this) }

    val secrets by lazy { Secrets(this) }
}
