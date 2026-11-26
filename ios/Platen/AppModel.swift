import Foundation
import Observation
import UIKit

struct SavedPrinter: Codable, Hashable, Identifiable {
    var base: String
    var model: String
    var nickname: String?
    var id: String { base }
    var label: String { nickname ?? model }
    var ip: String { base.components(separatedBy: "//").last?.components(separatedBy: "/").first?.components(separatedBy: ":").first ?? base }
}

enum AppTab: String, CaseIterable, Hashable {
    case scan = "Scan", copy = "Copy", print = "Print", files = "Files"

    var icon: String {
        switch self {
        case .scan: "doc.viewfinder"
        case .copy: "doc.on.doc"
        case .print: "printer"
        case .files: "folder"
        }
    }
}

@MainActor
@Observable
final class AppModel {
    let platen: Platen
    private let prefs = UserDefaults.standard

    var device: Device?
    var finding = false
    var status: DeviceStatus?
    var caps: Caps?
    var printCaps: PrintCaps?
    var settings: Settings
    var presets: [Preset]
    var session: SessionView?
    var drafts: [Draft]
    var top: TopHalf?
    var busy: String?
    var message: String?
    var files: [SavedFile] = []
    var accent: Accent
    var fresh: String?
    var printers: [SavedPrinter]
    var current: String?
    var searching = false
    var keepDays: Int
    var tab: AppTab = .scan
    var printFiles: [URL] = []
    var shrinking: (file: URL, folder: String?)?
    var outcome = 0
    var renamingScan = false
    var failure = 0

    init() {
        Storage.prepare()
        platen = Platen(config: Storage.config.path, cache: Storage.cache.path, scans: Storage.scans.path, renderer: PdfPages())
        settings = platen.settings()
        presets = platen.presets()
        drafts = platen.drafts()
        accent = Accent(rawValue: prefs.string(forKey: "accent") ?? "") ?? .ink
        printers = (prefs.data(forKey: "printers")).flatMap { try? JSONDecoder().decode([SavedPrinter].self, from: $0) } ?? []
        current = prefs.string(forKey: "base")
        keepDays = prefs.integer(forKey: "keepDays")
        find()
        cleanOldDrafts()
    }

    var scansDir: URL { Storage.scans }

    func setAccent(_ a: Accent) {
        prefs.set(a.rawValue, forKey: "accent")
        accent = a
    }

    func setKeepDays(_ days: Int) {
        prefs.set(days, forKey: "keepDays")
        keepDays = days
        cleanOldDrafts()
    }

    private func cleanOldDrafts() {
        guard keepDays > 0 else { return }
        let cutoff = UInt64(Date().timeIntervalSince1970) - UInt64(keepDays) * 86_400
        let p = platen
        work(nil, {
            for d in p.drafts() where d.modified < cutoff { _ = try? p.deleteDraft(name: d.name) }
            return p.drafts()
        }) { self.drafts = $0 }
    }

    // MARK: Printers

    func find() {
        guard !finding else { return }
        finding = true
        Task {
            defer { finding = false }
            if let d = await connectSaved() {
                device = d
                refreshDevice()
                return
            }
            guard let found = await Discovery.first(within: 8) else {
                device = nil
                say("No scanner found. Is it on, and is this phone on the same Wi-Fi?")
                return
            }
            remember([found])
            let label = labelOf(found.base, found.name)
            do {
                device = try await off { [platen] in try platen.connect(base: found.base, name: label) }
                choose(found.base)
                refreshDevice()
            } catch {
                say(error.text)
            }
        }
    }

    private func connectSaved() async -> Device? {
        guard let base = current else { return nil }
        let label = printers.first { $0.base == base }?.label ?? base
        return try? await off { [platen] in try platen.connect(base: base, name: label) }
    }

    private func labelOf(_ base: String, _ model: String) -> String {
        printers.first { $0.base == base }?.label ?? model
    }

    private func choose(_ base: String) {
        prefs.set(base, forKey: "base")
        current = base
    }

    private func remember(_ found: [Found]) {
        var known = Dictionary(uniqueKeysWithValues: printers.map { ($0.base, $0) })
        for f in found {
            known[f.base] = known[f.base].map { var p = $0; p.model = f.name; return p } ?? SavedPrinter(base: f.base, model: f.name)
        }
        savePrinters(Array(known.values))
    }

    private func savePrinters(_ list: [SavedPrinter]) {
        let sorted = list.sorted { $0.label.lowercased() < $1.label.lowercased() }
        prefs.set(try? JSONEncoder().encode(sorted), forKey: "printers")
        printers = sorted
    }

    func searchPrinters() {
        guard !searching else { return }
        searching = true
        Task {
            let found = await Discovery.all(within: 5)
            remember(found)
            searching = false
            if found.isEmpty { say("No printers found. Check they're on and on the same Wi-Fi.") }
        }
    }

    func usePrinter(_ p: SavedPrinter) {
        let platen = platen
        work("Connecting", { try platen.connect(base: p.base, name: p.label) }) { d in
            self.choose(p.base)
            self.device = d
            self.status = nil
            self.caps = nil
            self.printCaps = nil
            self.refreshDevice()
        }
    }

    func renamePrinter(_ p: SavedPrinter, _ nickname: String) {
        let nick = nickname.trimmingCharacters(in: .whitespaces)
        savePrinters(printers.map { $0.base == p.base ? SavedPrinter(base: p.base, model: p.model, nickname: nick.isEmpty ? nil : nick) : $0 })
        if current == p.base, let d = device { device = Device(name: nick.isEmpty ? p.model : nick, host: d.host) }
    }

    func forgetPrinter(_ p: SavedPrinter) {
        savePrinters(printers.filter { $0.base != p.base })
        if current == p.base {
            prefs.removeObject(forKey: "base")
            current = nil
        }
    }

    func refreshDevice() {
        let platen = platen
        Task {
            async let s = try? off { try platen.status() }
            async let c = try? off { try platen.capabilities() }
            async let pc = try? off { try platen.printCaps() }
            let (status, caps, printCaps) = await (s, c, pc)
            if let status { self.status = status }
            if let caps { self.caps = caps }
            if let printCaps { self.printCaps = printCaps }
        }
    }

    var host: String? { device?.host.components(separatedBy: ":").first }

    var lowInks: [Ink] { status?.printer?.ink.filter { (0...15).contains($0.level) } ?? [] }

    // MARK: Running work

    func work<T>(_ label: String?, _ block: @escaping @Sendable () throws -> T, done: @escaping (T) -> Void = { _ in }) {
        if label != nil, busy != nil { return }
        if let label { busy = label }
        Task {
            defer { if label != nil { busy = nil } }
            do {
                let value = try await off(block)
                done(value)
                if label != nil { outcome += 1 }
            } catch {
                if error.text != "stopped" { fail(error.text) }
            }
        }
    }

    func say(_ text: String) {
        message = text.prefix(1).uppercased() + text.dropFirst()
    }

    private func fail(_ text: String) {
        failure += 1
        say(text)
    }

    // MARK: Scanning

    private func show(_ v: SessionView?) {
        session = v
        drafts = platen.drafts()
    }

    private func placed(_ r: PageResult) {
        let i = Int(r.index)
        fresh = i < r.session.pages.count ? r.session.pages[i].file : nil
        show(r.session)
        if !r.note.trimmingCharacters(in: .whitespaces).isEmpty { say(r.note) }
    }

    func newScan(_ name: String? = nil) {
        let platen = platen
        work(nil, { try platen.openSession(name: name) }, done: show)
    }

    func closeScan() {
        platen.closeSession()
        show(nil)
    }

    func deleteDraft(_ name: String) {
        let platen = platen
        work(nil, { try platen.deleteDraft(name: name) }) { self.drafts = $0 }
    }

    func rename(_ name: String) {
        let platen = platen
        work(nil, { try platen.renameSession(name: name) }, done: show)
    }

    func scan(replace: Int? = nil) {
        let platen = platen
        work("Scanning", { try platen.scanPage(replace: replace.map(UInt32.init), at: nil) }, done: placed)
    }

    func importFiles(_ urls: [URL]) {
        let platen = platen
        work("Adding pages", { try platen.importFiles(paths: urls.map { try Storage.incoming($0).path }, at: nil) }, done: placed)
    }

    func importPhotos(_ photos: [Data]) {
        let platen = platen
        work("Adding pages", {
            let paths = try photos.enumerated().map { try Storage.incomingImage($1, name: "Photo \($0 + 1).jpg").path }
            return try platen.importFiles(paths: paths, at: nil)
        }, done: placed)
    }

    func scanTop() {
        let platen = platen
        work("Scanning the top half", { try platen.scanTop() }) { self.top = $0 }
    }

    func scanBottom() {
        guard let top else { return }
        let platen = platen
        work("Scanning the bottom half and joining", { try platen.scanBottom(top: top, replace: nil, at: nil) }) { r in
            self.top = nil
            self.placed(r)
        }
    }

    func cancelLong() { top = nil }

    func cancelScan() {
        let platen = platen
        Task.detached { platen.cancelScan() }
    }

    func edit(_ index: Int, _ edit: Edit) {
        let platen = platen
        work("Working", { try platen.editPage(index: UInt32(index), edit: edit) }, done: placed)
    }

    func fill(_ index: Int) {
        let platen = platen
        work("Filling edges", { try platen.fillPage(index: UInt32(index)) }, done: placed)
    }

    func deletePage(_ index: Int) {
        let platen = platen
        work(nil, { try platen.deletePage(index: UInt32(index)) }, done: show)
    }

    func movePage(_ from: Int, _ to: Int) {
        let platen = platen
        work(nil, { try platen.movePage(from: UInt32(from), to: UInt32(to)) }, done: show)
    }

    func undo() {
        let platen = platen
        work(nil, { try platen.undo() }, done: show)
    }

    func redo() {
        let platen = platen
        work(nil, { try platen.redo() }, done: show)
    }

    func setSettings(_ s: Settings) {
        platen.setSettings(settings: s)
        settings = s
    }

    func savePreset(_ name: String) {
        let platen = platen
        work(nil, { try platen.savePreset(name: name) }) { self.presets = $0 }
    }

    func deletePreset(_ name: String) {
        presets = platen.deletePreset(name: name)
    }

    func export(_ opts: ExportOpts, pages: String) {
        let platen = platen
        let folder = session?.name
        work("Exporting", {
            let out = try platen.export(opts: opts, pages: pages)
            for f in out.files { try Storage.save(URL(fileURLWithPath: f), into: folder) }
            return out
        }) { out in
            if out.closed { self.show(nil) }
            self.say("Saved \(plural(out.files.count, "file")) to \(Storage.label)")
            self.loadFiles()
        }
    }

    func printScan() {
        let platen = platen
        work("Making a PDF", { try platen.scanToPdf() }) { path in
            self.printFiles = [URL(fileURLWithPath: path)]
            self.tab = .print
        }
    }

    // MARK: Files

    func loadFiles() {
        work(nil, { Storage.list() }) { self.files = $0 }
    }

    func deleteFile(_ f: SavedFile) {
        work(nil, {
            try Storage.delete(f)
            return Storage.list()
        }) { self.files = $0 }
    }

    func saveFile(_ file: URL, into folder: String?, then: @escaping () -> Void = {}) {
        work("Saving", { try Storage.save(file, into: folder) }) { _ in
            self.say("Saved \(file.lastPathComponent) to \(Storage.label)")
            self.loadFiles()
            then()
        }
    }

    func open(_ urls: [URL]) {
        work("Opening", { try urls.map(Storage.incoming) }) { files in
            self.printFiles = files
            self.tab = .print
        }
    }

    func print(_ f: SavedFile) {
        work("Opening", { try Storage.incoming(f.url) }) { url in
            self.printFiles = [url]
            self.tab = .print
        }
    }

    func shrink(_ f: SavedFile) {
        work("Opening", { try Storage.incoming(f.url) }) { url in
            self.shrinking = (url, f.folder.isEmpty ? nil : f.folder)
        }
    }
}

func off<T>(_ block: @escaping @Sendable () throws -> T) async throws -> T {
    try await Task.detached(priority: .userInitiated) { try block() }.value
}

extension Error {
    var text: String {
        if let e = self as? PlatenError, case let .Failed(message) = e { return message }
        return localizedDescription
    }
}
