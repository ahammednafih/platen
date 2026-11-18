import QuickLook
import SwiftUI
import UniformTypeIdentifiers

struct FilesTab: View {
    @Environment(AppModel.self) private var model
    @State private var previewing: URL?
    @State private var pickingPdf = false

    var body: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Saved in").font(.caption).foregroundStyle(.secondary)
                    Text(Storage.label).font(.subheadline.weight(.semibold))
                }
                Button { pickingPdf = true } label: {
                    Label("Shrink a PDF", systemImage: "arrow.down.right.and.arrow.up.left")
                }
            }
            if model.files.isEmpty {
                Section {
                    VStack(spacing: 14) {
                        SheetGlyph(kind: .empty, width: 64)
                        Text("Nothing saved yet").font(.title3.bold())
                        Text("Export a scan and its PDFs and images land here.")
                            .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 28)
                }
            }
            ForEach(model.files) { f in
                Button { previewing = f.url } label: { FileRow(file: f) }
                    .buttonStyle(.plain)
                    .contextMenu { actions(f) }
                    .swipeActions(edge: .trailing) {
                        Button("Delete", systemImage: "trash", role: .destructive) { model.deleteFile(f) }
                        ShareLink(item: f.url) { Label("Share", systemImage: "square.and.arrow.up") }.tint(.blue)
                    }
            }
        }
        .refreshable { model.loadFiles() }
        .onAppear { model.loadFiles() }
        .quickLookPreview($previewing, in: model.files.map(\.url))
        .fileImporter(isPresented: $pickingPdf, allowedContentTypes: [.pdf]) { result in
            guard case let .success(url) = result else { return }
            model.work("Opening", { try Storage.incoming(url) }) { model.shrinking = ($0, nil) }
        }
    }

    @ViewBuilder
    private func actions(_ f: SavedFile) -> some View {
        ShareLink(item: f.url) { Label("Share", systemImage: "square.and.arrow.up") }
        if f.printable { Button("Print", systemImage: "printer") { model.print(f) } }
        if f.isPdf { Button("Shrink", systemImage: "arrow.down.right.and.arrow.up.left") { model.shrink(f) } }
        Button("Delete", systemImage: "trash", role: .destructive) { model.deleteFile(f) }
    }
}

private struct FileRow: View {
    let file: SavedFile

    var body: some View {
        HStack(spacing: 14) {
            Paper {
                ZStack {
                    Text(file.url.pathExtension.uppercased()).font(.caption2).foregroundStyle(.secondary)
                    FileImage(path: file.url.path, width: 160, fill: true)
                }
            }
            .frame(width: 44, height: 60)
            VStack(alignment: .leading, spacing: 2) {
                Text(file.name).font(.subheadline.weight(.semibold)).lineLimit(2)
                Text([file.folder.isEmpty ? nil : file.folder, size(file.size), file.modified.formatted(date: .abbreviated, time: .omitted)]
                    .compactMap { $0 }.joined(separator: ", "))
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .contentShape(.rect)
    }
}

private let limits: [(Float, String)] = [(0.5, "½ MB"), (1, "1 MB"), (2, "2 MB"), (5, "5 MB")]

struct ShrinkView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let file: URL
    let folder: String?
    @State private var count = 0
    @State private var skip: Set<Int> = []
    @State private var limit: Float? = 2
    @State private var custom = false
    @State private var customText = ""
    @State private var result: ShrinkResult?
    @State private var comparing = false
    @State private var viewing: Int?

    private var target: Float? { custom ? Float(customText.replacingOccurrences(of: ",", with: ".")) : limit }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Text("Keep it under").font(.headline).accessibilityAddTraits(.isHeader)
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(limits, id: \.0) { mb, label in
                                chip(label, on: !custom && limit == mb) { custom = false; limit = mb }
                            }
                            chip("Other", on: custom) { custom = true }
                            chip("No limit", on: !custom && limit == nil) { custom = false; limit = nil }
                        }
                    }
                    if custom {
                        TextField("Megabytes", text: $customText)
                            .keyboardType(.decimalPad)
                            .padding(12)
                            .background(Color(.tertiarySystemFill), in: .rect(cornerRadius: 12))
                            .onChange(of: customText) { discard() }
                    }
                    Text(skip.isEmpty ? "Tap a page to leave it out, touch and hold to view it" : "\(plural(count - skip.count, "page")) kept, \(skip.count) left out")
                        .font(.subheadline).foregroundStyle(.secondary).padding(.top, 8)
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 14)], spacing: 14) {
                        ForEach(0..<count, id: \.self) { i in
                            let out = skip.contains(i)
                            VStack(spacing: 6) {
                                Paper { FileImage(path: file.path, page: i, width: 240) }
                                    .aspectRatio(0.72, contentMode: .fit)
                                    .opacity(out ? 0.3 : 1)
                                Text(out ? "Left out" : "\(i + 1)").font(.callout.weight(.semibold)).foregroundStyle(out ? .red : .primary)
                            }
                            .contentShape(.rect)
                            .onTapGesture {
                                if out { skip.remove(i) } else { skip.insert(i) }
                                discard()
                            }
                            .onLongPressGesture { viewing = i }
                            .accessibilityElement(children: .ignore)
                            .accessibilityLabel("Page \(i + 1)")
                            .accessibilityValue(out ? "Left out" : "Kept")
                            .accessibilityHint(out ? "Double-tap to keep it" : "Double-tap to leave it out")
                            .accessibilityAddTraits(.isButton)
                            .accessibilityAction(named: "View") { viewing = i }
                        }
                    }
                    .sensoryFeedback(.selection, trigger: skip)
                }
                .padding(16)
                .padding(.bottom, 120)
            }
            .scrollDismissesKeyboard(.interactively)
            .safeAreaInset(edge: .bottom) { bottom.padding(16) }
            .navigationTitle("Shrink")
            .navigationSubtitle(file.lastPathComponent)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close", systemImage: "xmark") {
                        discard()
                        dismiss()
                    }
                }
            }
            .task { count = Pdf.pageCount(file) }
            .fullScreenCover(item: Binding(get: { viewing.map(Slot.init) }, set: { viewing = $0?.index })) { slot in
                PageViewer(sources: (0..<count).map { PageSource(path: file.path, page: $0) }, start: slot.index)
            }
            .fullScreenCover(isPresented: $comparing) {
                if let r = result { PdfCompare(before: file, after: URL(fileURLWithPath: r.out)) }
            }
        }
    }

    @ViewBuilder
    private var bottom: some View {
        if let busy = model.busy {
            BusyPill(label: "\(busy)…")
        } else if let r = result {
            resultCard(r)
        } else {
            Button {
                let platen = model.platen
                let (path, t, s) = (file.path, target, skip.map { UInt32($0 + 1) })
                model.work("Shrinking", { try platen.shrinkPdf(path: path, maxMb: t, skip: s) }) { result = $0 }
            } label: {
                Text(target == nil ? "Leave out \(plural(skip.count, "page"))" : skip.isEmpty ? "Shrink under \(fmt(target!)) MB" : "Shrink and leave out \(plural(skip.count, "page"))")
                    .font(.headline)
                    .frame(maxWidth: .infinity, minHeight: 36)
            }
            .prominent(glass: true)
            .controlSize(.large)
            .disabled((target == nil && skip.isEmpty) || skip.count >= count)
        }
    }

    private func resultCard(_ r: ShrinkResult) -> some View {
        let saved = r.before > 0 ? 100 - Int(r.after * 100 / r.before) : 0
        let out = URL(fileURLWithPath: r.out)
        return VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .lastTextBaseline, spacing: 10) {
                Text(size(Int64(r.after))).font(.title.bold())
                Text("from \(size(Int64(r.before)))").font(.subheadline).foregroundStyle(.secondary)
            }
            Text(r.fits ? "\(saved)% smaller, \(plural(Int(r.pages), "page"))" : "As small as it gets without losing pages, \(plural(Int(r.pages), "page"))")
                .font(.subheadline)
                .foregroundStyle(r.fits ? Color.primary : Color.red)
            HStack {
                Button {
                    model.saveFile(out, into: folder) {
                        try? FileManager.default.removeItem(at: out)
                        result = nil
                        dismiss()
                    }
                } label: { Text("Save").frame(maxWidth: .infinity) }
                    .prominent()
                ShareLink(item: out) { Text("Share") }.buttonStyle(.bordered)
                Button("Compare") { comparing = true }.buttonStyle(.bordered)
            }
            .controlSize(.large)
            Button("Try a different size") { discard() }
        }
        .padding(20)
        .glassEffect(.regular, in: .rect(cornerRadius: 28))
    }

    private func chip(_ label: String, on: Bool, _ action: @escaping () -> Void) -> some View {
        Button(label) {
            action()
            discard()
        }
        .buttonStyle(.bordered)
        .buttonBorderShape(.capsule)
        .tint(on ? .accentColor : .secondary)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private func discard() {
        if let r = result { try? FileManager.default.removeItem(atPath: r.out) }
        result = nil
    }

    private func fmt(_ mb: Float) -> String {
        mb.truncatingRemainder(dividingBy: 1) == 0 ? "\(Int(mb))" : "\(mb)"
    }
}
