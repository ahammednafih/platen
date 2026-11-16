package com.kanamlabs.platen

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.ui.draw.rotate
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.RadioButton
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Surface
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.kanamlabs.platen.core.DetailGroup
import com.kanamlabs.platen.core.Ink
import com.kanamlabs.platen.core.Inks
import com.kanamlabs.platen.core.Strength
import com.kanamlabs.platen.core.PlatenException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@Composable
fun DeviceScreen(model: AppModel, state: AppState, onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    val host = state.device?.host?.substringBefore(':')
    var details by remember { mutableStateOf<List<DetailGroup>?>(null) }
    var detailsOpen by remember { mutableStateOf(true) }
    var loading by remember { mutableStateOf(false) }
    var askPassword by remember { mutableStateOf(false) }
    var wrong by remember { mutableStateOf(false) }
    var password by remember { mutableStateOf("") }
    val tick = rememberTick()
    var renaming by remember { mutableStateOf<SavedPrinter?>(null) }
    var afterUnlock by remember { mutableStateOf<(() -> Unit)?>(null) }
    var inks by rememberSaveable { mutableStateOf(Inks.BLACK) }
    var strength by rememberSaveable { mutableStateOf(Strength.NORMAL) }
    var confirming by remember { mutableStateOf<HeadAction?>(null) }
    var lowInks by remember { mutableStateOf<List<Ink>>(emptyList()) }
    var headBusy by remember { mutableStateOf(false) }
    var headNote by remember { mutableStateOf<String?>(null) }
    val chooseFolder = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri -> if (uri != null) model.setFolder(uri) }

    fun load(typed: String?) {
        val h = host ?: return
        val pw = typed ?: model.secrets.password(h)
        if (pw == null) {
            askPassword = true
            return
        }
        loading = true
        scope.launch {
            try {
                details = withContext(Dispatchers.IO) { model.platen.deviceDetails(pw) }
                detailsOpen = true
                if (typed != null) model.secrets.savePassword(h, typed)
                askPassword = false
                wrong = false
                afterUnlock?.invoke()
                afterUnlock = null
            } catch (e: PlatenException) {
                if (e.message == "wrong-password") {
                    askPassword = true
                    wrong = true
                } else {
                    model.say(e.message ?: "Can't read the printer's details")
                }
            } finally {
                loading = false
            }
        }
    }

    fun runHead(action: HeadAction) {
        val h = host ?: return
        val pw = model.secrets.password(h)
        if (pw == null) {
            afterUnlock = { runHead(action) }
            askPassword = true
            return
        }
        headBusy = true
        headNote = null
        scope.launch {
            try {
                withContext(Dispatchers.IO) { if (action == HeadAction.Clean) model.platen.cleanHead(pw, inks, strength) else model.platen.nozzleCheck(pw) }
                headNote = when (action) {
                    HeadAction.Clean -> "Cleaning. The printer is busy for a minute or more; print a nozzle check afterwards to see if it helped."
                    HeadAction.Check -> "Printing. Every dotted line in the four colour blocks should be there with no gaps. If any are missing, clean that colour."
                }
            } catch (e: PlatenException) {
                if (e.message == "wrong-password") {
                    afterUnlock = { runHead(action) }
                    askPassword = true
                    wrong = true
                } else {
                    model.say(e.message ?: "The printer didn't take it")
                }
            } finally {
                headBusy = false
            }
        }
    }

    fun ask(action: HeadAction) {
        scope.launch {
            lowInks = if (action == HeadAction.Clean) withContext(Dispatchers.IO) { runCatching { model.platen.cleaningLowInks(inks) }.getOrDefault(emptyList()) } else emptyList()
            confirming = action
        }
    }

    LaunchedEffect(Unit) { model.refreshDevice() }
    LaunchedEffect(inks) { if (inks != Inks.ALL && strength == Strength.SPECIAL) strength = Strength.NORMAL }

    confirming?.let { action ->
        val low = lowInks.isNotEmpty()
        AlertDialog(
            onDismissRequest = { confirming = null },
            icon = if (action == HeadAction.Clean) { { Icon(Icons.Default.Warning, contentDescription = null, tint = if (low) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.secondary) } } else null,
            title = { Text(if (action == HeadAction.Clean) "Clean the print head?" else "Print a nozzle check?") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (action == HeadAction.Check) {
                        Text("Prints one sheet. Load A4 or Letter plain paper in the main tray.")
                    } else {
                        Text(
                            "Cleaning uses ink" + when (strength) {
                                Strength.NORMAL -> ""
                                Strength.SPECIAL -> ", and Special uses the most"
                                else -> ", and ${strength.label} uses more than Normal"
                            } + ". The printer can't print or copy until it's done, about a minute.",
                        )
                        if (low) {
                            Text(
                                lowInks.joinToString(", ") { "${it.name} is at ${it.level}%" } + ". Cleaning with little ink left can damage the print head. Refill first.",
                                color = MaterialTheme.colorScheme.error,
                            )
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(
                    onClick = { confirming = null; runHead(action) },
                    colors = if (low) ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error) else ButtonDefaults.textButtonColors(),
                ) { Text(if (action == HeadAction.Check) "Print" else if (low) "Clean anyway" else "Start cleaning") }
            },
            dismissButton = { TextButton(onClick = { confirming = null }) { Text("Cancel") } },
        )
    }

    renaming?.let { p ->
        NameDialog("Rename printer", p.nickname ?: p.model, onDismiss = { renaming = null }) { model.renamePrinter(p, it); renaming = null }
    }

    if (askPassword) {
        var visible by remember { mutableStateOf(false) }
        Sheet({ askPassword = false; wrong = false }) {
            run {
                Text("Printer password", style = MaterialTheme.typography.headlineSmall)
                Text("The admin password for the printer's web page. It's kept encrypted on this phone.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                OutlinedTextField(
                    password, { password = it; wrong = false },
                    label = { Text("Password") },
                    supportingText = if (wrong) { { Text("That password didn't work. Try again.") } } else null,
                    isError = wrong,
                    singleLine = true,
                    shape = MaterialTheme.shapes.medium,
                    visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    trailingIcon = { TextButton(onClick = { visible = !visible }) { Text(if (visible) "Hide" else "Show") } },
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(
                    onClick = { load(password) },
                    enabled = password.isNotEmpty() && !loading,
                    shapes = ButtonDefaults.shapes(),
                    contentPadding = ButtonDefaults.contentPaddingFor(ButtonDefaults.MediumContainerHeight),
                    modifier = Modifier.fillMaxWidth().heightIn(ButtonDefaults.MediumContainerHeight),
                ) {
                    if (loading) LoadingIndicator(Modifier.size(28.dp), color = MaterialTheme.colorScheme.onPrimary)
                    else Text("Unlock", style = ButtonDefaults.textStyleFor(ButtonDefaults.MediumContainerHeight))
                }
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Printer and settings") },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
            )
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
            LowInk(state)
            GlassBed(Modifier.fillMaxWidth()) {
                Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    Column {
                        Text(state.device?.name ?: "No printer", style = MaterialTheme.typography.headlineSmall)
                        Text(
                            state.status?.let { s -> listOfNotNull("Scanner ${s.scanner.lowercase()}", s.printer?.let { "printer ${it.state.lowercase()}" }).joinToString(", ") }
                                ?: state.device?.host ?: "Make sure it's on and this phone is on the same Wi-Fi.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        state.status?.printer?.reasons?.filter { it != "none" }?.takeIf { it.isNotEmpty() }?.let {
                            Text(it.joinToString().replace('-', ' '), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                        }
                    }
                    state.status?.printer?.ink?.takeIf { it.isNotEmpty() }?.let { inks ->
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly) {
                            inks.forEach { InkTank(it.name, it.level) }
                        }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilledTonalButton(onClick = { model.refreshDevice() }, shapes = ButtonDefaults.shapes()) { Text("Refresh") }
                    }
                }
            }

            Section("Printers") {
                state.printers.forEach { p ->
                    val current = p.base == state.current
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        RadioButton(selected = current, onClick = { if (!current) { tick(); model.usePrinter(p) } })
                        Column(Modifier.weight(1f)) {
                            Text(p.label, style = MaterialTheme.typography.titleMedium, maxLines = 1)
                            if (p.nickname != null) Text(p.model, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                            Text(p.base.substringAfter("//").substringBefore('/').substringBefore(':'), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                        }
                        IconButton(onClick = { renaming = p }, shapes = IconButtonDefaults.shapes()) { Icon(Icons.Default.Edit, contentDescription = "Rename ${p.label}") }
                        IconButton(onClick = { model.forgetPrinter(p) }, shapes = IconButtonDefaults.shapes()) {
                            Icon(Icons.Default.Delete, contentDescription = "Forget ${p.label}", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
                if (state.printers.isEmpty()) Text("No printers saved yet.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    FilledTonalButton(onClick = { tick(); model.searchPrinters() }, enabled = !state.searching, shapes = ButtonDefaults.shapes()) { Text("Look for printers") }
                    if (state.searching) LoadingIndicator(Modifier.size(32.dp))
                }
            }

            Surface(
                onClick = { tick(); if (details == null) load(null) else detailsOpen = !detailsOpen },
                enabled = host != null && !loading,
                shape = MaterialTheme.shapes.large,
                color = MaterialTheme.colorScheme.surfaceContainerLow,
            ) {
                Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    Surface(shape = MaterialTheme.shapes.medium, color = MaterialTheme.colorScheme.primaryContainer, modifier = Modifier.size(44.dp)) {
                        Box(contentAlignment = Alignment.Center) {
                            if (loading) LoadingIndicator(Modifier.size(32.dp))
                            else Icon(Icons.Default.Info, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimaryContainer)
                        }
                    }
                    Column(Modifier.weight(1f)) {
                        Text("Counters, maintenance and network", style = MaterialTheme.typography.titleMedium)
                        Text(
                            when {
                                loading -> "Reading the printer's pages…"
                                details == null -> "Read from the printer's own web page"
                                detailsOpen -> "Tap to hide"
                                else -> "Tap to show"
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    val turn by animateFloatAsState(if (details != null && detailsOpen) 90f else 0f, label = "chevron")
                    Icon(
                        Icons.AutoMirrored.Filled.KeyboardArrowRight,
                        contentDescription = if (details != null && detailsOpen) "Hide details" else "Show details",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.rotate(turn),
                    )
                }
            }
            AnimatedVisibility(details != null && detailsOpen, enter = fadeIn() + expandVertically(), exit = fadeOut() + shrinkVertically()) {
                Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
                    details.orEmpty().forEach { g ->
                        Section(g.title.ifBlank { null }) {
                            g.rows.forEachIndexed { i, r ->
                                if (i > 0 && !r.sub) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                                DetailLine(r.label, r.value, r.sub)
                            }
                        }
                    }
                    if (host != null) {
                        TextButton(onClick = { model.secrets.forget(host); details = null }, shapes = ButtonDefaults.shapes()) { Text("Forget the printer password") }
                    }
                }
            }

            Section("Print head") {
                Text(
                    "For faint, streaky or missing colours. Print a nozzle check first and clean only the colours with gaps.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                FilledTonalButton(onClick = { tick(); ask(HeadAction.Check) }, enabled = host != null && !headBusy, shapes = ButtonDefaults.shapes()) { Text("Print nozzle check") }
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                Labeled("Inks") { Choice(listOf(Inks.BLACK to "Black", Inks.COLOUR to "Colour", Inks.ALL to "All"), inks, { inks = it }, enabled = !headBusy) }
                Labeled("Strength") {
                    Choice(
                        listOfNotNull(Strength.NORMAL to "Normal", Strength.STRONG to "Strong", Strength.STRONGEST to "Strongest", (Strength.SPECIAL to "Special").takeIf { inks == Inks.ALL }),
                        strength, { strength = it }, enabled = !headBusy,
                    )
                }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    FilledTonalButton(onClick = { tick(); ask(HeadAction.Clean) }, enabled = host != null && !headBusy, shapes = ButtonDefaults.shapes()) { Text("Clean print head") }
                    if (headBusy) LoadingIndicator(Modifier.size(32.dp))
                }
                headNote?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }
            }

            Section("Print from other apps") {
                Text(
                    "Turn on Platen in Android's print settings, then choose your printer from the Print option in any app, like Docs, Word, Chrome or Photos.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                val context = androidx.compose.ui.platform.LocalContext.current
                FilledTonalButton(onClick = { runCatching { context.startActivity(android.content.Intent(android.provider.Settings.ACTION_PRINT_SETTINGS)) } }, shapes = ButtonDefaults.shapes()) {
                    Text("Open print settings")
                }
            }

            Section("Where scans are saved") {
                Text(state.folder, style = MaterialTheme.typography.titleMedium)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilledTonalButton(onClick = { chooseFolder.launch(null) }, shapes = ButtonDefaults.shapes()) { Text("Choose folder") }
                    if (state.folder != SaveFolder.DEFAULT) TextButton(onClick = { model.setFolder(null) }, shapes = ButtonDefaults.shapes()) { Text("Use ${SaveFolder.DEFAULT}") }
                }
            }

            Section("Drafts") {
                Text("Delete unfinished scans you haven't touched for a while. Exported files in your scans folder are never removed.", style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Choice(listOf(0 to "Keep all", 30 to "After 30 days", 90 to "After 90 days"), state.keepDays, model::setKeepDays)
            }

            Section("Accent colour") {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Accent.entries.forEach { a ->
                        val dark = isSystemInDarkTheme()
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            Surface(onClick = { tick(); model.setAccent(a) }, shape = CircleShape, color = Color.Transparent) {
                                Swatch(if (a == Accent.Wallpaper) MaterialTheme.colorScheme.primary.takeIf { state.accent == a } ?: (if (dark) a.dark else a.light) else if (dark) a.dark else a.light, selected = state.accent == a)
                            }
                        }
                    }
                }
                Text(state.accent.label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

private enum class HeadAction { Check, Clean }

private val Strength.label get() = name.lowercase().replaceFirstChar { it.uppercase() }

@Composable
private fun DetailLine(label: String, value: String, sub: Boolean) {
    val numeric = Regex("""^[\d,]+%?( .*)?$""").matches(value.trim())
    Row(Modifier.fillMaxWidth().padding(start = if (sub) 16.dp else 0.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            label,
            Modifier.weight(1f),
            style = if (sub) MaterialTheme.typography.bodyMedium else MaterialTheme.typography.titleSmall,
            color = if (sub) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
        )
        Text(
            value.ifBlank { "—" },
            style = if (numeric && !sub) MaterialTheme.typography.titleLarge else MaterialTheme.typography.bodyMedium,
            color = if (sub) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
            textAlign = TextAlign.End,
        )
    }
}
