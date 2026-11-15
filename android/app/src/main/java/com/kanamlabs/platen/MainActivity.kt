package com.kanamlabs.platen

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import java.io.File

enum class Tab(val label: String, val icon: Int) {
    Scan("Scan", R.drawable.ic_scan),
    Copy("Copy", R.drawable.ic_copy),
    Print("Print", R.drawable.ic_print),
    Files("Files", R.drawable.ic_folder),
}

class MainActivity : ComponentActivity() {
    private val model: AppModel by viewModels()
    private val incoming = mutableStateOf<List<File>>(emptyList())

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        if (savedInstanceState == null) receive(intent)
        setContent {
            val state by model.state.collectAsStateWithLifecycle()
            PlatenTheme(state.accent) {
                AppRoot(model, state, incoming.value) { incoming.value = emptyList() }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        receive(intent)
    }

    private fun receive(intent: Intent) {
        val uris: List<Uri> = when (intent.action) {
            Intent.ACTION_VIEW -> listOfNotNull(intent.data)
            Intent.ACTION_SEND -> listOfNotNull(intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java))
            Intent.ACTION_SEND_MULTIPLE -> intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java).orEmpty()
            else -> emptyList()
        }
        if (uris.isEmpty()) return
        model.work("Opening", { uris.map { model.saveFolder.copyToCache(it) } }) { incoming.value = it }
    }
}

@Composable
fun AppRoot(model: AppModel, state: AppState, incoming: List<File>, onIncomingTaken: () -> Unit) {
    val view = LocalView.current
    val snackbar = remember { SnackbarHostState() }
    var tab by rememberSaveable { mutableStateOf(Tab.Scan) }
    var showDevice by rememberSaveable { mutableStateOf(false) }
    var printFiles by remember { mutableStateOf<List<File>>(emptyList()) }
    var shrinkFile by remember { mutableStateOf<Pair<File, String?>?>(null) }

    LaunchedEffect(Unit) { model.outcomes.collect { ok -> if (ok) Haptics.confirm(view) else Haptics.reject(view) } }
    LaunchedEffect(incoming) {
        if (incoming.isEmpty()) return@LaunchedEffect
        printFiles = incoming
        tab = Tab.Print
        onIncomingTaken()
    }
    LaunchedEffect(state.message) {
        state.message?.let {
            snackbar.showSnackbar(it)
            model.messageShown()
        }
    }

    if (showDevice) {
        BackHandler { showDevice = false }
        DeviceScreen(model, state, onBack = { showDevice = false })
        return
    }
    shrinkFile?.let { (file, folder) ->
        BackHandler { shrinkFile = null }
        ShrinkScreen(model, file, folder, onClose = { shrinkFile = null })
        return
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        topBar = {
            TopAppBar(
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.surface),
                title = {
                    Text(
                        if (tab == Tab.Scan) state.session?.name ?: "Platen" else tab.label,
                        style = MaterialTheme.typography.headlineSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                subtitle = {
                    Text(
                        when {
                            state.finding -> "Looking for the printer…"
                            state.device != null -> state.device.name
                            else -> "No printer found"
                        },
                        maxLines = 1,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                },
                actions = {
                    if (tab == Tab.Scan) ScanActions(model, state)
                    IconButton(onClick = { showDevice = true }, shapes = IconButtonDefaults.shapes()) {
                        Icon(Icons.Default.Settings, contentDescription = "Printer and settings")
                    }
                },
            )
        },
        bottomBar = {
            NavigationBar(containerColor = MaterialTheme.colorScheme.surface) {
                Tab.entries.forEach { t ->
                    NavigationBarItem(
                        selected = tab == t,
                        onClick = {
                            if (tab != t) Haptics.tick(view)
                            tab = t
                        },
                        icon = { Icon(painterResource(t.icon), contentDescription = null) },
                        label = { Text(t.label) },
                    )
                }
            }
        },
        snackbarHost = { SnackbarHost(snackbar, Modifier.padding(bottom = if (tab == Tab.Scan && state.session != null) 96.dp else 0.dp)) },
    ) { padding ->
        AnimatedContent(tab, transitionSpec = { fadeIn() togetherWith fadeOut() }, label = "tab") { t ->
            when (t) {
                Tab.Scan -> ScanTab(model, state, padding, onPrint = { printFiles = listOf(it); tab = Tab.Print })
                Tab.Copy -> CopyTab(model, state, padding)
                Tab.Print -> PrintTab(model, state, padding, printFiles, onFiles = { printFiles = it })
                Tab.Files -> FilesTab(
                    model, state, padding,
                    onPrint = { printFiles = listOf(it); tab = Tab.Print },
                    onShrink = { file, folder -> shrinkFile = file to folder },
                )
            }
        }
    }
}
