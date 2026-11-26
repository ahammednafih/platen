import PhotosUI
import SwiftUI
import UniformTypeIdentifiers

struct ScanActions: ToolbarContent {
    @Environment(AppModel.self) private var model

    var body: some ToolbarContent {
        if let s = model.session {
            ToolbarItemGroup(placement: .topBarLeading) {
                Button("Undo", systemImage: "arrow.uturn.backward") { model.undo() }
                    .disabled(!s.canUndo || model.busy != nil)
                Button("Redo", systemImage: "arrow.uturn.forward") { model.redo() }
                    .disabled(!s.canRedo || model.busy != nil)
            }
            ToolbarItem(placement: .topBarTrailing) {
                Menu("More", systemImage: "ellipsis") {
                    Button("Rename", systemImage: "pencil") { model.renamingScan = true }
                    Button("Close scan", systemImage: "xmark") { model.closeScan() }
                }
            }
        }
    }
}

struct ScanTab: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        @Bindable var model = model
        Group {
            if let s = model.session { SessionPane(session: s) } else { StartPane() }
        }
        .nameAlert("Rename scan", shown: $model.renamingScan, initial: model.session?.name ?? "") { model.rename($0) }
    }
}

private struct StartPane: View {
    @Environment(AppModel.self) private var model
    @State private var name = ""

    var body: some View {
        ScrollView {
            VStack(spacing: 14) {
                GlassBed {
                    VStack(spacing: 20) {
                        SheetGlyph(kind: .empty, width: 88).padding(.top, 8)
                        Text("Put a page on the glass").font(.title2.bold()).multilineTextAlignment(.center)
                        TextField("Name (optional)", text: $name)
                            .accessibilityLabel("Scan name, optional")
                            .textFieldStyle(.plain)
                            .padding(14)
                            .background(Color(.tertiarySystemFill), in: .rect(cornerRadius: 14))
                            .submitLabel(.go)
                            .onSubmit(start)
                        Button(action: start) {
                            Label("Start a scan", systemImage: "doc.viewfinder").font(.headline).frame(maxWidth: .infinity)
                        }
                        .prominent()
                        .controlSize(.large)
                    }
                    .padding(24)
                }
                .padding(.bottom, 8)
                if !model.drafts.isEmpty {
                    Text("Pick up where you left off").font(.headline).frame(maxWidth: .infinity, alignment: .leading).padding(.leading, 4).accessibilityAddTraits(.isHeader)
                    ForEach(model.drafts, id: \.name) { DraftRow(draft: $0) }
                }
            }
            .padding(16)
            .frame(maxWidth: 600)
            .frame(maxWidth: .infinity)
        }
        .scrollDismissesKeyboard(.interactively)
    }

    private func start() {
        let n = name.trimmingCharacters(in: .whitespaces)
        model.newScan(n.isEmpty ? nil : n)
        name = ""
    }
}

private struct DraftRow: View {
    @Environment(AppModel.self) private var model
    let draft: Draft
    @State private var cover: String?

    var body: some View {
        HStack(spacing: 16) {
            Button { model.newScan(draft.name) } label: {
                HStack(spacing: 16) {
                    Group {
                        if let cover {
                            Paper { FileImage(path: cover, width: 160, fill: true) }
                        } else {
                            SheetGlyph(kind: .empty, width: 52)
                        }
                    }
                    .frame(width: 52, height: 70)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(draft.name).font(.headline).lineLimit(2)
                        Text("\(plural(Int(draft.pages), "page")), \(Date(timeIntervalSince1970: TimeInterval(draft.modified)).formatted(date: .abbreviated, time: .shortened))")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 0)
                }
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilityHint("Opens the draft")
            Button("Delete draft \(draft.name)", systemImage: "trash", role: .destructive) { model.deleteDraft(draft.name) }
                .labelStyle(.iconOnly)
                .foregroundStyle(.secondary)
                .frame(minWidth: 44, minHeight: 44)
        }
        .padding(12)
        .background(Color(.secondarySystemBackground), in: .rect(cornerRadius: 20))
        .task(id: draft.pages) { cover = firstPage() }
    }

    private func firstPage() -> String? {
        let dir = model.scansDir.appendingPathComponent(draft.name).appendingPathComponent("pages")
        let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        return files.filter { ["jpg", "png"].contains($0.pathExtension) }
            .min { ((try? $0.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantFuture) < ((try? $1.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate) ?? .distantFuture) }?
            .path
    }
}

private struct SessionPane: View {
    @Environment(AppModel.self) private var model
    let session: SessionView
    @State private var viewing: Int?
    @State private var tools: Int?
    @State private var showSettings = false
    @State private var showExport = false
    @State private var dropTarget: Int?

    var body: some View {
        VStack(spacing: 0) {
            SettingsChips { showSettings = true }
            GlassBed(corner: 28) {
                if session.pages.isEmpty {
                    VStack(spacing: 16) {
                        SheetGlyph(kind: .empty, width: 72)
                        Text("Put a page face down on the glass, then tap Scan. PDFs and images from the phone can go in too.")
                            .font(.body).multilineTextAlignment(.center).foregroundStyle(.secondary)
                    }
                    .padding(32)
                    .frame(maxHeight: .infinity)
                } else {
                    ScrollView {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 140), spacing: 20)], spacing: 20) {
                            ForEach(Array(session.pages.enumerated()), id: \.element.file) { i, p in
                                PageCell(index: i, page: p, fresh: p.file == model.fresh, dropHere: dropTarget == i,
                                         onOpen: { viewing = i }, onTools: { tools = i },
                                         onMove: i > 0 || i < session.pages.count - 1 ? { to in model.movePage(i, to) } : nil,
                                         last: session.pages.count - 1)
                                    .draggable(p.file) {
                                        Paper { FileImage(path: p.path, width: 200) }.frame(width: 100, height: 140)
                                    }
                                    .dropDestination(for: String.self) { items, _ in
                                        guard let file = items.first, let from = session.pages.firstIndex(where: { $0.file == file }), from != i else { return false }
                                        model.movePage(from, i)
                                        return true
                                    } isTargeted: { on in
                                        if on { dropTarget = i } else if dropTarget == i { dropTarget = nil }
                                    }
                            }
                        }
                        .padding(20)
                        .padding(.bottom, 120)
                        .animation(.spring(duration: 0.35), value: session.pages.map(\.file))
                    }
                }
            }
            .ignoresSafeArea(edges: .bottom)
        }
        .overlay(alignment: .bottom) { Controls(showSettings: $showSettings, showExport: $showExport).padding(.bottom, 12) }
        .sensoryFeedback(.selection, trigger: dropTarget)
        .fullScreenCover(item: Binding(get: { viewing.map(Slot.init) }, set: { viewing = $0?.index })) { slot in
            PageViewer(sources: session.pages.map { .init(path: $0.path, page: 0) }, start: slot.index)
        }
        .sheet(item: Binding(get: { tools.map(Slot.init) }, set: { tools = $0?.index })) { slot in
            if slot.index < session.pages.count {
                PageTools(page: session.pages[slot.index], index: slot.index, count: session.pages.count, onView: {
                    tools = nil
                    viewing = slot.index
                })
            }
        }
        .sheet(isPresented: $showSettings) { ScanSettingsSheet() }
        .sheet(isPresented: $showExport) { ExportSheet() }
    }
}

struct Slot: Identifiable {
    let index: Int
    var id: Int { index }
}

private struct Controls: View {
    @Environment(AppModel.self) private var model
    @Binding var showSettings: Bool
    @Binding var showExport: Bool
    @State private var picking = false
    @State private var pickingPhotos = false
    @State private var photos: [PhotosPickerItem] = []
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .title3) private var side: CGFloat = 48

    var body: some View {
        let session = model.session
        let slide: AnyTransition = reduceMotion ? .opacity : .move(edge: .bottom).combined(with: .opacity)
        let idle = model.busy == nil && model.top == nil
        let hasPages = !(session?.pages.isEmpty ?? true)
        VStack(spacing: 12) {
            if let busy = model.busy {
                BusyPill(label: "\(busy)…") {
                    if busy.hasPrefix("Scanning") { Button("Cancel") { model.cancelScan() }.fontWeight(.semibold) }
                }
                .padding(.horizontal, 16)
                .transition(slide)
            }
            if model.top != nil, model.busy == nil {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Top half done").font(.headline)
                    Text("Turn the page around so the bottom is on the glass, bottom edge against the far edge. Leave a few centimetres of overlap.")
                        .font(.subheadline)
                    HStack {
                        Button("Scan bottom half") { model.scanBottom() }.prominent().frame(maxWidth: .infinity)
                        Button("Cancel") { model.cancelLong() }
                    }
                }
                .padding(16)
                .glassEffect(.regular, in: .rect(cornerRadius: 24))
                .padding(.horizontal, 16)
                .transition(slide)
                .onAppear { announce("Top half done. Turn the page around and scan the bottom half.") }
            }
            GlassEffectContainer {
                HStack(spacing: 4) {
                    tool("Scan settings", "slider.horizontal.3") { showSettings = true }
                    tool("Scan a long page", "arrow.up.and.down.text.horizontal", enabled: idle && model.device != nil) { model.scanTop() }
                    Menu {
                        Button("Files", systemImage: "folder") { picking = true }
                        Button("Photos", systemImage: "photo.on.rectangle") { pickingPhotos = true }
                    } label: {
                        Image(systemName: "doc.badge.plus").font(.title3).frame(width: side, height: side)
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(idle ? Color.primary : Color.secondary.opacity(0.5))
                    .disabled(!idle)
                    .accessibilityLabel("Add a PDF or image")
                    Button {
                        if idle, model.device != nil { model.scan() }
                    } label: {
                        Image(systemName: "doc.viewfinder").font(.title2.weight(.semibold)).frame(width: side * 4 / 3, height: side * 13 / 12)
                    }
                    .prominent(glass: true)
                    .disabled(!idle || model.device == nil)
                    .accessibilityLabel("Scan a page")
                    tool("Export", "square.and.arrow.down", enabled: hasPages) { showExport = true }
                    tool("Print", "printer", enabled: hasPages && idle) { model.printScan() }
                }
                .padding(6)
                .glassEffect(.regular, in: .capsule)
            }
        }
        .animation(.spring(duration: 0.3), value: model.busy)
        .animation(.spring(duration: 0.3), value: model.top != nil)
        .fileImporter(isPresented: $picking, allowedContentTypes: [.pdf, .jpeg, .png, .heic], allowsMultipleSelection: true) { result in
            if case let .success(urls) = result, !urls.isEmpty { model.importFiles(urls) }
        }
        .photosPicker(isPresented: $pickingPhotos, selection: $photos, maxSelectionCount: 20, matching: .images)
        .onChange(of: photos) { loadPhotos() }
    }

    private func loadPhotos() {
        let items = photos
        guard !items.isEmpty, model.busy == nil else { return }
        photos = []
        model.busy = "Adding pages"
        Task {
            var data: [Data] = []
            for item in items {
                if let d = try? await item.loadTransferable(type: Data.self) { data.append(d) }
            }
            model.busy = nil
            model.importPhotos(data)
        }
    }

    private func tool(_ label: String, _ icon: String, enabled: Bool = true, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: icon).font(.title3).frame(width: side, height: side) }
            .buttonStyle(.plain)
            .foregroundStyle(enabled ? Color.primary : Color.secondary.opacity(0.5))
            .disabled(!enabled)
            .accessibilityLabel(label)
    }
}

private struct SettingsChips: View {
    @Environment(AppModel.self) private var model
    let onTap: () -> Void

    var body: some View {
        let s = model.settings
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                chip("\(s.dpi) dpi")
                chip(modeLabel(s.mode))
                chip(sizeName(model.caps, s.size))
                if s.fill != .off { chip(s.fill == .white ? "Fill edges white" : "Fill edges paper colour") }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
    }

    private func chip(_ text: String) -> some View {
        Button(text, action: onTap)
            .font(.subheadline)
            .buttonStyle(.bordered)
            .buttonBorderShape(.capsule)
            .tint(.secondary)
            .foregroundStyle(.primary)
            .accessibilityHint("Opens scan settings")
    }
}

private struct PageCell: View {
    let index: Int
    let page: Page
    let fresh: Bool
    let dropHere: Bool
    let onOpen: () -> Void
    let onTools: () -> Void
    let onMove: ((Int) -> Void)?
    let last: Int

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button(action: onOpen) {
                Paper(fresh: fresh) { FileImage(path: page.path, width: 420) }
                    .aspectRatio(CGFloat(page.w) / CGFloat(max(page.h, 1)), contentMode: .fit)
                    .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Color.accentColor, lineWidth: dropHere ? 3 : 0).padding(-4))
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Page \(index + 1)")
            .accessibilityValue("\(page.dpi) dpi")
            .accessibilityActions {
                if let onMove, index > 0 { Button("Move earlier") { onMove(index - 1) } }
                if let onMove, index < last { Button("Move later") { onMove(index + 1) } }
            }
            HStack(spacing: 6) {
                Text("\(index + 1)").font(.headline).accessibilityHidden(true)
                Text("\(page.dpi) dpi, \(dur(page.secs))").font(.caption2).foregroundStyle(.secondary).lineLimit(1).accessibilityHidden(true)
                Spacer(minLength: 0)
                Button("Edit page \(index + 1)", systemImage: "ellipsis.circle", action: onTools)
                    .labelStyle(.iconOnly)
                    .font(.title3)
            }
        }
    }
}

private struct ScanSettingsSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var saving = false

    var body: some View {
        let s = model.settings
        let resolutions = model.caps?.resolutions.isEmpty == false ? model.caps!.resolutions : [150, 300, 600]
        let modes = model.caps?.modes.isEmpty == false ? model.caps!.modes : [.color, .gray, .blackWhite]
        NavigationStack {
            Form {
                Section("Presets") {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(model.presets, id: \.name) { p in
                                Button(p.name) { model.setSettings(p.settings) }
                                    .buttonStyle(.bordered)
                                    .buttonBorderShape(.capsule)
                                    .tint(p.settings == s ? .accentColor : .secondary)
                                    .accessibilityAddTraits(p.settings == s ? .isSelected : [])
                                    .contextMenu {
                                        if !p.builtin { Button("Delete \(p.name)", systemImage: "trash", role: .destructive) { model.deletePreset(p.name) } }
                                    }
                                    .accessibilityActions {
                                        if !p.builtin { Button("Delete \(p.name)") { model.deletePreset(p.name) } }
                                    }
                            }
                            Button("Save these…", systemImage: "plus") { saving = true }
                                .buttonStyle(.bordered)
                                .buttonBorderShape(.capsule)
                        }
                    }
                    if model.presets.contains(where: { !$0.builtin }) {
                        Text("Touch and hold your own preset to delete it.").font(.footnote).foregroundStyle(.secondary)
                    }
                }
                Section {
                    Picker("Resolution", selection: binding(\.dpi)) {
                        ForEach(resolutions, id: \.self) { Text("\($0) dpi").tag($0) }
                    }
                } footer: {
                    if s.dpi >= 1200 { Text("A full page at 1200 dpi takes over two minutes.") }
                }
                Section("Colour") {
                    Picker("Colour", selection: binding(\.mode)) {
                        ForEach(modes, id: \.self) { Text(modeLabel($0)).tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                }
                Section {
                    Picker("Original size", selection: binding(\.size)) {
                        ForEach(sizeChoices(model.caps), id: \.0) { Text($0.1).tag($0.0) }
                    }
                }
                Section("Torn edges and punch holes") {
                    Picker("Torn edges", selection: binding(\.fill)) {
                        Text("Leave").tag(Fill.off)
                        Text("White").tag(Fill.white)
                        Text("Paper colour").tag(Fill.paper)
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                }
            }
            .navigationTitle("Scan settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", systemImage: "checkmark") { dismiss() } } }
            .nameAlert("Save preset", shown: $saving, initial: "") { model.savePreset($0) }
        }
        .presentationDetents([.medium, .large])
    }

    private func binding<T>(_ key: WritableKeyPath<Settings, T>) -> Binding<T> {
        Binding(get: { model.settings[keyPath: key] }, set: { v in
            var s = model.settings
            s[keyPath: key] = v
            model.setSettings(s)
        })
    }
}

private struct ExportSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var opts: ExportOpts?
    @State private var limit = ""
    @State private var pages = ""

    var body: some View {
        let partial = !pages.trimmingCharacters(in: .whitespaces).isEmpty
        NavigationStack {
            Form {
                if let o = opts {
                    Section {
                        toggle("Colour PDF", "Full quality, as scanned", \.color)
                        toggle("Share-size PDF", "Smaller, for email and messages", \.share)
                        toggle("Print-ready PDF", "Clean black on white", \.print)
                        toggle("Images", "One JPEG or PNG per page", \.images)
                    } footer: {
                        Text("Saves to \(Storage.label)")
                    }
                    Section {
                        LabeledContent("Pages") {
                            TextField("All, or e.g. 1-3, 7", text: $pages).keyboardType(.numbersAndPunctuation).multilineTextAlignment(.trailing)
                        }
                        LabeledContent("Keep each PDF under") {
                            HStack(spacing: 4) {
                                TextField("No limit", text: $limit).keyboardType(.decimalPad).multilineTextAlignment(.trailing)
                                Text("MB").foregroundStyle(.secondary)
                            }
                        }
                        Toggle("Close the scan afterwards", isOn: Binding(get: { o.cleanup && !partial }, set: { opts?.cleanup = $0 }))
                            .disabled(partial)
                    }
                    Section {
                        Button {
                            let mb = Float(limit.replacingOccurrences(of: ",", with: "."))
                            model.export(ExportOpts(color: o.color, share: o.share, print: o.print, images: o.images, cleanup: o.cleanup && !partial, maxMb: mb), pages: pages)
                            dismiss()
                        } label: {
                            Text("Export").font(.headline).frame(maxWidth: .infinity)
                        }
                        .prominent()
                        .controlSize(.large)
                        .disabled(!(o.color || o.share || o.print || o.images))
                        .listRowInsets(EdgeInsets())
                        .listRowBackground(Color.clear)
                    }
                }
            }
            .navigationTitle("Export")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel", systemImage: "xmark") { dismiss() } } }
            .onAppear {
                let saved = model.platen.exportOpts()
                opts = saved
                limit = saved.maxMb.map { $0.truncatingRemainder(dividingBy: 1) == 0 ? "\(Int($0))" : "\($0)" } ?? ""
            }
        }
        .presentationDetents([.large])
    }

    private func toggle(_ title: String, _ detail: String, _ key: WritableKeyPath<ExportOpts, Bool>) -> some View {
        Toggle(isOn: Binding(get: { opts?[keyPath: key] ?? false }, set: { opts?[keyPath: key] = $0 })) {
            VStack(alignment: .leading) {
                Text(title)
                Text(detail).font(.caption).foregroundStyle(.secondary)
            }
        }
    }
}
