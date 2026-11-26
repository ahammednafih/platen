package com.kanamlabs.platen

import android.app.Application
import android.content.Context
import android.net.Uri
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.kanamlabs.platen.core.Caps
import com.kanamlabs.platen.core.Device
import com.kanamlabs.platen.core.DeviceStatus
import com.kanamlabs.platen.core.Draft
import com.kanamlabs.platen.core.Edit
import com.kanamlabs.platen.core.ExportOpts
import com.kanamlabs.platen.core.PageResult
import com.kanamlabs.platen.core.PlatenException
import com.kanamlabs.platen.core.Preset
import com.kanamlabs.platen.core.PrintCaps
import com.kanamlabs.platen.core.SessionView
import com.kanamlabs.platen.core.Settings
import com.kanamlabs.platen.core.TopHalf
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.io.IOException

data class SavedPrinter(val base: String, val model: String, val nickname: String? = null) {
    val label get() = nickname ?: model
}

data class AppState(
    val device: Device? = null,
    val finding: Boolean = false,
    val status: DeviceStatus? = null,
    val caps: Caps? = null,
    val printCaps: PrintCaps? = null,
    val settings: Settings,
    val presets: List<Preset> = emptyList(),
    val session: SessionView? = null,
    val drafts: List<Draft> = emptyList(),
    val top: TopHalf? = null,
    val busy: String? = null,
    val message: String? = null,
    val files: List<SavedFile> = emptyList(),
    val folder: String = SaveFolder.DEFAULT,
    val accent: Accent = Accent.Ink,
    val fresh: String? = null,
    val printers: List<SavedPrinter> = emptyList(),
    val current: String? = null,
    val searching: Boolean = false,
    val keepDays: Int = 0,
)

class AppModel(app: Application) : AndroidViewModel(app) {
    private val platenApp = app as PlatenApp
    val platen = platenApp.platen
    val saveFolder = platenApp.saveFolder
    val secrets = platenApp.secrets
    val scansDir = File(app.filesDir, "scans")
    private val prefs = app.getSharedPreferences("platen", Context.MODE_PRIVATE)
    private val _state = MutableStateFlow(
        AppState(
            settings = platen.settings(), presets = platen.presets(), drafts = platen.drafts(), folder = saveFolder.label,
            accent = prefs.getString("accent", null)?.let { a -> Accent.entries.firstOrNull { it.name == a } } ?: Accent.Ink,
            printers = loadPrinters(app.getSharedPreferences("platen", Context.MODE_PRIVATE)),
            current = app.getSharedPreferences("platen", Context.MODE_PRIVATE).getString("base", null),
            keepDays = app.getSharedPreferences("platen", Context.MODE_PRIVATE).getInt("keepDays", 0),
        ),
    )
    val state: StateFlow<AppState> = _state
    val outcomes = MutableSharedFlow<Boolean>(extraBufferCapacity = 4)

    fun setAccent(a: Accent) {
        prefs.edit().putString("accent", a.name).apply()
        _state.update { it.copy(accent = a) }
    }

    init {
        find()
        cleanOldDrafts()
    }

    fun setKeepDays(days: Int) {
        prefs.edit().putInt("keepDays", days).apply()
        _state.update { it.copy(keepDays = days) }
        cleanOldDrafts()
    }

    private fun cleanOldDrafts() {
        val days = _state.value.keepDays
        if (days <= 0) return
        val cutoff = System.currentTimeMillis() / 1000 - days * 86_400L
        work(null, {
            platen.drafts().filter { it.modified.toLong() < cutoff }.forEach { runCatching { platen.deleteDraft(it.name) } }
            platen.drafts()
        }) { d -> _state.update { it.copy(drafts = d) } }
    }

    fun find() {
        if (_state.value.finding) return
        _state.update { it.copy(finding = true) }
        viewModelScope.launch {
            try {
                val device = withContext(Dispatchers.IO) { connectSaved() ?: connectFound() }
                _state.update { it.copy(device = device) }
                refreshDevice()
            } catch (e: Exception) {
                _state.update { it.copy(message = e.message, device = null) }
            } finally {
                _state.update { it.copy(finding = false) }
            }
        }
    }

    private fun connectSaved(): Device? {
        val base = prefs.getString("base", null) ?: return null
        val label = _state.value.printers.firstOrNull { it.base == base }?.label ?: prefs.getString("name", null) ?: base
        return runCatching { platen.connect(base, label) }.getOrNull()
    }

    private suspend fun connectFound(): Device {
        val found = findScanner(getApplication(), 8_000) ?: throw IOException("No scanner found. Is it on, and is this phone on the same Wi-Fi?")
        remember(listOf(found))
        val device = platen.connect(found.base, labelOf(found.base, found.name))
        choose(found.base)
        return device
    }

    private fun labelOf(base: String, model: String) = _state.value.printers.firstOrNull { it.base == base }?.label ?: model

    private fun choose(base: String) {
        prefs.edit().putString("base", base).apply()
        _state.update { it.copy(current = base) }
    }

    private fun remember(found: List<Found>) {
        val known = _state.value.printers.associateBy { it.base }.toMutableMap()
        found.forEach { f -> known[f.base] = known[f.base]?.copy(model = f.name) ?: SavedPrinter(f.base, f.name) }
        savePrinters(known.values.sortedBy { it.label.lowercase() })
    }

    private fun savePrinters(list: List<SavedPrinter>) {
        val json = org.json.JSONArray(list.map { p -> org.json.JSONObject().put("base", p.base).put("model", p.model).put("nickname", p.nickname ?: org.json.JSONObject.NULL) })
        prefs.edit().putString("printers", json.toString()).apply()
        _state.update { it.copy(printers = list) }
    }

    fun searchPrinters() {
        if (_state.value.searching) return
        _state.update { it.copy(searching = true) }
        viewModelScope.launch {
            val found = findScanners(getApplication(), 5_000)
            remember(found)
            _state.update { it.copy(searching = false) }
            if (found.isEmpty()) say("No printers found. Check they're on and on the same Wi-Fi.")
        }
    }

    fun usePrinter(p: SavedPrinter) = work("Connecting", { platen.connect(p.base, p.label) }) { d ->
        choose(p.base)
        _state.update { it.copy(device = d, status = null, caps = null, printCaps = null) }
        refreshDevice()
    }

    fun renamePrinter(p: SavedPrinter, nickname: String) {
        val nick = nickname.trim().ifEmpty { null }
        savePrinters(_state.value.printers.map { if (it.base == p.base) it.copy(nickname = nick) else it }.sortedBy { it.label.lowercase() })
        if (_state.value.current == p.base) _state.update { s -> s.copy(device = s.device?.copy(name = nick ?: p.model)) }
    }

    fun forgetPrinter(p: SavedPrinter) {
        savePrinters(_state.value.printers.filter { it.base != p.base })
        if (_state.value.current == p.base) {
            prefs.edit().remove("base").apply()
            _state.update { it.copy(current = null) }
        }
    }

    fun refreshDevice() = viewModelScope.launch(Dispatchers.IO) {
        val status = runCatching { platen.status() }.getOrNull()
        val caps = runCatching { platen.capabilities() }.getOrNull()
        val printCaps = runCatching { platen.printCaps() }.getOrNull()
        _state.update { it.copy(status = status ?: it.status, caps = caps ?: it.caps, printCaps = printCaps ?: it.printCaps) }
    }

    fun <T> work(label: String?, block: () -> T, done: (T) -> Unit = {}) {
        if (label != null && _state.value.busy != null) return
        if (label != null) _state.update { it.copy(busy = label) }
        viewModelScope.launch {
            try {
                done(withContext(Dispatchers.IO) { block() })
                if (label != null) outcomes.tryEmit(true)
            } catch (e: PlatenException) {
                if (e.message != "stopped") fail(e.message)
            } catch (e: IOException) {
                fail(e.message)
            } catch (e: IllegalStateException) {
                fail(e.message)
            } finally {
                if (label != null) _state.update { it.copy(busy = null) }
            }
        }
    }

    fun say(message: String) = _state.update { it.copy(message = message.replaceFirstChar { c -> c.uppercase() }) }

    private fun fail(message: String?) {
        outcomes.tryEmit(false)
        say(message ?: "Something went wrong")
    }

    fun messageShown() = _state.update { it.copy(message = null) }

    fun busy(label: String?) = _state.update { it.copy(busy = label) }

    private fun show(v: SessionView?) = _state.update { it.copy(session = v, drafts = platen.drafts()) }

    private fun placed(r: PageResult) {
        _state.update { it.copy(fresh = r.session.pages.getOrNull(r.index.toInt())?.file) }
        show(r.session)
        if (r.note.isNotBlank()) say(r.note)
    }

    fun newScan(name: String? = null) = work(null, { platen.openSession(name) }, ::show)

    fun closeScan() {
        platen.closeSession()
        show(null)
    }

    fun deleteDraft(name: String) = work(null, { platen.deleteDraft(name) }) { d -> _state.update { it.copy(drafts = d) } }

    fun rename(name: String) = work(null, { platen.renameSession(name) }, ::show)

    fun scan(replace: Int? = null) = work("Scanning", { platen.scanPage(replace?.toUInt(), null) }, ::placed)

    fun importFiles(uris: List<Uri>) = work("Adding pages", { platen.importFiles(uris.map { saveFolder.copyToCache(it).path }, null) }, ::placed)

    fun scanTop() = work("Scanning the top half", { platen.scanTop() }) { t -> _state.update { it.copy(top = t) } }

    fun scanBottom() {
        val top = _state.value.top ?: return
        work("Scanning the bottom half and joining", { platen.scanBottom(top, null, null) }) { r ->
            _state.update { it.copy(top = null) }
            placed(r)
        }
    }

    fun cancelLong() = _state.update { it.copy(top = null) }

    fun cancelScan() = viewModelScope.launch(Dispatchers.IO) { platen.cancelScan() }

    fun edit(index: Int, edit: Edit) = work("Working", { platen.editPage(index.toUInt(), edit) }, ::placed)

    fun fill(index: Int) = work("Filling edges", { platen.fillPage(index.toUInt()) }, ::placed)

    fun deletePage(index: Int) = work(null, { platen.deletePage(index.toUInt()) }, ::show)

    fun movePage(from: Int, to: Int) = work(null, { platen.movePage(from.toUInt(), to.toUInt()) }, ::show)

    fun undo() = work(null, { platen.undo() }, ::show)

    fun redo() = work(null, { platen.redo() }, ::show)

    fun setSettings(s: Settings) {
        platen.setSettings(s)
        _state.update { it.copy(settings = s) }
    }

    fun savePreset(name: String) = work(null, { platen.savePreset(name) }) { p -> _state.update { it.copy(presets = p) } }

    fun deletePreset(name: String) = work(null, { platen.deletePreset(name) }) { p -> _state.update { it.copy(presets = p) } }

    fun export(opts: ExportOpts, pages: String) {
        val folder = _state.value.session?.name
        work("Exporting", {
            val out = platen.export(opts, pages)
            out.files.forEach { saveFolder.save(File(it), folder) }
            out
        }) { out ->
            if (out.closed) show(null)
            say("Saved ${out.files.size} ${if (out.files.size == 1) "file" else "files"} to ${saveFolder.label}")
            loadFiles()
        }
    }

    fun loadFiles() = work(null, { saveFolder.list() }) { f -> _state.update { it.copy(files = f) } }

    fun deleteFile(f: SavedFile) = work(null, { saveFolder.delete(f); saveFolder.list() }) { l -> _state.update { it.copy(files = l) } }

    fun setFolder(tree: Uri?) {
        saveFolder.tree = tree
        _state.update { it.copy(folder = saveFolder.label) }
        loadFiles()
    }

    fun saveFile(file: File, subfolder: String?, then: () -> Unit = {}) = work("Saving", { saveFolder.save(file, subfolder) }) {
        say("Saved ${file.name} to ${saveFolder.label}")
        loadFiles()
        then()
    }
}

internal fun loadPrinters(prefs: android.content.SharedPreferences): List<SavedPrinter> {
    val raw = prefs.getString("printers", null)
    if (raw == null) {
        val base = prefs.getString("base", null) ?: return emptyList()
        return listOf(SavedPrinter(base, prefs.getString("name", null) ?: base))
    }
    val arr = org.json.JSONArray(raw)
    return (0 until arr.length()).map { i ->
        val o = arr.getJSONObject(i)
        SavedPrinter(o.getString("base"), o.getString("model"), o.optString("nickname").takeIf { !o.isNull("nickname") && it.isNotEmpty() })
    }
}
