package com.kanamlabs.platen

import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import android.provider.MediaStore
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import java.io.File

data class SavedFile(val name: String, val uri: Uri, val size: Long, val modified: Long, val mime: String, val folder: String)

class SaveFolder(private val context: Context) {
    private val prefs = context.getSharedPreferences("platen", Context.MODE_PRIVATE)
    private val resolver = context.contentResolver

    var tree: Uri?
        get() = prefs.getString("saveTree", null)?.let(Uri::parse)
        set(value) {
            tree?.let { old -> runCatching { resolver.releasePersistableUriPermission(old, FLAGS) } }
            value?.let { resolver.takePersistableUriPermission(it, FLAGS) }
            prefs.edit().putString("saveTree", value?.toString()).apply()
        }

    val label: String
        get() = tree?.let { DocumentsContract.getTreeDocumentId(it).substringAfter(':').ifEmpty { "Storage" } } ?: DEFAULT

    fun save(file: File, subfolder: String?): Uri {
        val mime = mimeOf(file.name)
        val uri = tree?.let { createInTree(it, subfolder, file.name, mime) } ?: createInDocuments(subfolder, file.name, mime)
        resolver.openOutputStream(uri)?.use { out -> file.inputStream().use { it.copyTo(out) } } ?: error("Can't write to ${file.name}")
        if (tree == null) resolver.update(uri, ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }, null, null)
        return uri
    }

    fun list(): List<SavedFile> = (tree?.let(::listTree) ?: listDocuments()).sortedByDescending { it.modified }

    fun delete(file: SavedFile) {
        if (tree != null) DocumentsContract.deleteDocument(resolver, file.uri) else resolver.delete(file.uri, null, null)
    }

    fun copyToCache(uri: Uri, fallbackName: String = "file"): File {
        val name = displayName(uri) ?: fallbackName
        val dir = File(context.cacheDir, "picked/${System.nanoTime()}").apply { mkdirs() }
        val out = File(dir, name.replace('/', '_'))
        resolver.openInputStream(uri)?.use { input -> out.outputStream().use { input.copyTo(it) } } ?: error("Can't open $name")
        return out
    }

    private fun displayName(uri: Uri): String? =
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c -> if (c.moveToFirst()) c.getString(0) else null }

    private fun createInDocuments(subfolder: String?, name: String, mime: String): Uri {
        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, name)
            put(MediaStore.MediaColumns.MIME_TYPE, mime)
            put(MediaStore.MediaColumns.RELATIVE_PATH, "$DEFAULT/" + (subfolder?.let { "$it/" } ?: ""))
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }
        return resolver.insert(FILES, values) ?: error("Can't save $name to $DEFAULT")
    }

    private fun listDocuments(): List<SavedFile> {
        val cols = arrayOf(
            MediaStore.MediaColumns._ID, MediaStore.MediaColumns.DISPLAY_NAME, MediaStore.MediaColumns.SIZE,
            MediaStore.MediaColumns.DATE_MODIFIED, MediaStore.MediaColumns.MIME_TYPE, MediaStore.MediaColumns.RELATIVE_PATH,
        )
        val out = mutableListOf<SavedFile>()
        resolver.query(FILES, cols, "${MediaStore.MediaColumns.RELATIVE_PATH} LIKE ?", arrayOf("$DEFAULT/%"), null)?.use { c ->
            while (c.moveToNext()) {
                val folder = c.getString(5).removePrefix("$DEFAULT/").trimEnd('/')
                out += SavedFile(c.getString(1), Uri.withAppendedPath(FILES, c.getLong(0).toString()), c.getLong(2), c.getLong(3) * 1000, c.getString(4) ?: "", folder)
            }
        }
        return out
    }

    private fun createInTree(tree: Uri, subfolder: String?, name: String, mime: String): Uri {
        var parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        if (subfolder != null) {
            parent = children(tree, parent).firstOrNull { it.name == subfolder && it.mime == Document.MIME_TYPE_DIR }?.uri
                ?: DocumentsContract.createDocument(resolver, parent, Document.MIME_TYPE_DIR, subfolder)
                ?: error("Can't make the folder $subfolder")
        }
        return DocumentsContract.createDocument(resolver, parent, mime, name) ?: error("Can't save $name")
    }

    private fun listTree(tree: Uri): List<SavedFile> {
        val root = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        return children(tree, root).flatMap { entry ->
            if (entry.mime == Document.MIME_TYPE_DIR) children(tree, entry.uri).filter { it.mime != Document.MIME_TYPE_DIR }.map { it.copy(folder = entry.name) }
            else listOf(entry)
        }
    }

    private fun children(tree: Uri, parent: Uri): List<SavedFile> {
        val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getDocumentId(parent))
        val cols = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_SIZE, Document.COLUMN_LAST_MODIFIED, Document.COLUMN_MIME_TYPE)
        val out = mutableListOf<SavedFile>()
        resolver.query(uri, cols, null, null, null)?.use { c ->
            while (c.moveToNext()) {
                out += SavedFile(c.getString(1), DocumentsContract.buildDocumentUriUsingTree(tree, c.getString(0)), c.getLong(2), c.getLong(3), c.getString(4) ?: "", "")
            }
        }
        return out
    }

    companion object {
        const val DEFAULT = "Documents/Platen"
        private const val FLAGS = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        private val FILES = MediaStore.Files.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)

        fun mimeOf(name: String): String =
            MimeTypeMap.getSingleton().getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase()) ?: "application/octet-stream"
    }
}
