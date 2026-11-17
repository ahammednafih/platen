import PhotosUI
import SwiftUI
import UniformTypeIdentifiers

private enum Tone: String, CaseIterable, Identifiable {
    case colour = "Colour", gray = "Gray", clean = "B&W"
    var id: String { rawValue }
}

private struct SrcPage: Hashable {
    let file: URL
    let index: Int
}

struct PrintTab: View {
    @Environment(AppModel.self) private var model
    @State private var pages: [SrcPage] = []
    @State private var sel: [Bool] = []
    @State private var ranges = ""
    @State private var nup = 1
    @State private var scaleMode = ScaleMode.fit
    @State private var percent = 100
    @State private var reverse = false
    @State private var copies = 1
    @State private var collate = true
    @State private var tone = Tone.colour
    @State private var media = "iso_a4_210x297mm"
    @State private var quality = Quality.normal
    @State private var mediaType: String?
    @State private var borderless = false
    @State private var twoSided = false
    @State private var cropping: URL?
    @State private var viewing: Int?
    @State private var viewingSheet: Int?
    @State private var plan: Plan?
    @State private var sheets: [UIImage?] = []
    @State private var busy: String?
    @State private var flip = false
    @State private var log: String?
    @State private var picking = false
    @State private var photos: [PhotosPickerItem] = []
    @State private var pickingPhotos = false

    private var files: [URL] { model.printFiles }

    var body: some View {
        Group {
            if files.isEmpty { empty } else { form }
        }
        .fileImporter(isPresented: $picking, allowedContentTypes: [.pdf, .jpeg, .png, .heic], allowsMultipleSelection: true) { result in
            guard case let .success(urls) = result, !urls.isEmpty else { return }
            let existing = files
            model.work("Opening", { try urls.map(Storage.incoming) }) { model.printFiles = existing + $0 }
        }
        .photosPicker(isPresented: $pickingPhotos, selection: $photos, maxSelectionCount: 20, matching: .images)
        .onChange(of: photos) { loadPhotos() }
        .onAppear { if let d = model.printCaps?.mediaDefault, media == "iso_a4_210x297mm" { media = d } }
        .task(id: files) { await loadPages() }
        .task(id: planKey) { await updatePlan() }
        .fullScreenCover(item: Binding(get: { viewing.map(Slot.init) }, set: { viewing = $0?.index })) { slot in
            PageViewer(sources: pages.map { PageSource(path: $0.file.path, page: $0.index) }, start: slot.index)
        }
        .fullScreenCover(item: Binding(get: { viewingSheet.map(Slot.init) }, set: { viewingSheet = $0?.index })) { slot in
            let j = job()
            let platen = model.platen
            SheetsViewer(count: plan?.order.count ?? 0, start: slot.index) { i in
                let data = try? await off { try platen.printPreview(job: j, slot: UInt32(i), dpi: 120) }
                return data.flatMap(UIImage.init(data:))
            }
        }
        .sheet(item: Binding(get: { cropping.map(CropTarget.init) }, set: { cropping = $0?.url })) { t in
            ImageCropView(file: t.url) { out in model.printFiles = files.map { $0 == t.url ? out : $0 } }
        }
        .alert("Flip the stack", isPresented: $flip) {
            Button("Skip", role: .cancel) {}
            Button("Print backs") { send(job(.even, reverse: !reverse), "Printing backs") }
        } message: {
            Text("Take the printed sheets, turn the stack over without changing its order, and put it back in the tray.")
        }
    }

    private var addMenu: some View {
        Menu {
            Button("Files", systemImage: "folder") { picking = true }
            Button("Photos", systemImage: "photo.on.rectangle") { pickingPhotos = true }
        } label: {
            Label("Add more", systemImage: "plus")
        }
    }

    private var empty: some View {
        ScrollView {
            VStack(spacing: 20) {
                LowInkBanner(inks: model.lowInks)
                GlassBed {
                    VStack(spacing: 16) {
                        SheetGlyph(kind: .empty, width: 72)
                        Text("Print PDFs and photos").font(.title2.bold())
                        Text("Pick pages by tapping them, put several on a sheet, or print both sides by hand.")
                            .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
                        HStack {
                            Button { picking = true } label: { Label("Files", systemImage: "folder").frame(maxWidth: .infinity) }
                            Button { pickingPhotos = true } label: { Label("Photos", systemImage: "photo.on.rectangle").frame(maxWidth: .infinity) }
                        }
                        .prominent()
                        .controlSize(.large)
                    }
                    .padding(28)
                }
            }
            .padding(16)
            .frame(maxWidth: 600)
            .frame(maxWidth: .infinity)
        }
    }

    private var form: some View {
        Form {
            if !model.lowInks.isEmpty {
                Section { LowInkBanner(inks: model.lowInks) }.listRowInsets(EdgeInsets()).listRowBackground(Color.clear)
            }
            Section("Files") {
                ForEach(Array(files.enumerated()), id: \.element) { i, f in
                    HStack(spacing: 12) {
                        Button { view(f) } label: {
                            HStack(spacing: 12) {
                                Paper { FileImage(path: f.path, width: 120, fill: true) }.frame(width: 36, height: 48)
                                Text(f.lastPathComponent).font(.subheadline.weight(.medium)).lineLimit(2)
                                Spacer(minLength: 0)
                            }
                            .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("View \(f.lastPathComponent)")
                        Menu {
                            Button("View", systemImage: "eye") { view(f) }
                            if i > 0 { Button("Move up", systemImage: "arrow.up") { moveFile(i, by: -1) } }
                            if i < files.count - 1 { Button("Move down", systemImage: "arrow.down") { moveFile(i, by: 1) } }
                            if f.pathExtension.lowercased() != "pdf" { Button("Crop", systemImage: "crop") { cropping = f } }
                            Button("Remove", systemImage: "trash", role: .destructive) { model.printFiles.removeAll { $0 == f } }
                        } label: {
                            Image(systemName: "ellipsis.circle").font(.title3).frame(width: 36, height: 44)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel("More for \(f.lastPathComponent)")
                    }
                    .swipeActions { Button("Remove", role: .destructive) { model.printFiles.removeAll { $0 == f } } }
                }
                addMenu
            }
            Section {
                ScrollView(.horizontal, showsIndicators: false) {
                    LazyHStack(spacing: 14) {
                        ForEach(Array(pages.enumerated()), id: \.offset) { i, p in
                            let on = i < sel.count ? sel[i] : true
                            VStack(spacing: 6) {
                                Paper { FileImage(path: p.file.path, page: p.index, width: 200) }
                                    .frame(width: 84, height: 116)
                                    .opacity(on ? 1 : 0.35)
                                Text("\(i + 1)").font(.callout.weight(.semibold)).foregroundStyle(on ? Color.accentColor : .secondary)
                            }
                            .contentShape(.rect)
                            .onTapGesture {
                                var s = sel
                                if i < s.count { s[i].toggle() }
                                setSel(s)
                            }
                            .onLongPressGesture { viewing = i }
                            .accessibilityAddTraits(.isButton)
                            .accessibilityLabel("Page \(i + 1), \(on ? "picked" : "left out")")
                            .accessibilityAction(named: "View") { viewing = i }
                        }
                    }
                    .padding(.vertical, 8)
                }
                .sensoryFeedback(.selection, trigger: sel)
                HStack {
                    Button("All") { setSel(pages.map { _ in true }) }
                    Button("Odd") { setSel(pages.indices.map { $0 % 2 == 0 }) }
                    Button("Even") { setSel(pages.indices.map { $0 % 2 == 1 }) }
                }
                .buttonStyle(.bordered)
                .buttonBorderShape(.capsule)
                TextField("Pages, like 1-3, 6", text: $ranges)
                    .keyboardType(.numbersAndPunctuation)
                    .onChange(of: ranges) { if let s = fromRanges(ranges, pages.count), s != sel { sel = s } }
            } header: {
                Text("Pages, \(sel.filter { $0 }.count) of \(pages.count)")
            } footer: {
                Text("Tap to pick, touch and hold to view")
            }
            if let plan {
                Section {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 12) {
                            ForEach(Array(sheets.enumerated()), id: \.offset) { i, img in
                                Button { viewingSheet = i } label: {
                                    Paper {
                                        if let img { Image(uiImage: img).resizable().aspectRatio(contentMode: .fit) } else { Color.white }
                                    }
                                    .frame(width: 80, height: 113)
                                }
                                .buttonStyle(.plain)
                                .accessibilityLabel("View sheet \(i + 1)")
                            }
                        }
                        .padding(.vertical, 6)
                    }
                } header: {
                    Text("\(plural(plan.order.count, "sheet"))\(copies > 1 ? ", \(copies) copies" : "")")
                } footer: {
                    Text("Tap one to see it bigger")
                }
            }
            Section("Layout") {
                Picker("Pages per sheet", selection: $nup) {
                    ForEach([1, 2, 4, 6, 9, 16], id: \.self) { Text("\($0)").tag($0) }
                }
                ScaleSection(mode: $scaleMode, percent: $percent)
                Toggle("Reverse order", isOn: $reverse)
                Toggle("Two-sided by hand", isOn: $twoSided)
            }
            Section("Copies and colour") {
                Stepper("Copies: \(copies)", value: $copies, in: 1...99)
                Toggle("Whole sets (1 2 3, 1 2 3)", isOn: $collate).disabled(copies < 2)
                Picker("Colour", selection: $tone) {
                    ForEach(Tone.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
            }
            Section("Paper") {
                PaperSection(caps: model.printCaps, media: $media, quality: $quality, mediaType: $mediaType)
                if model.printCaps?.borderless == true { Toggle("Borderless (to the edges of the paper)", isOn: $borderless) }
            }
            if let log { Section { Text(log).foregroundStyle(.secondary) } }
        }
        .scrollDismissesKeyboard(.interactively)
        .safeAreaInset(edge: .bottom) { printBar.padding(.horizontal, 16).padding(.bottom, 8) }
    }

    @ViewBuilder
    private var printBar: some View {
        if let busy {
            BusyPill(label: "\(busy)…") {
                Button("Stop") {
                    let platen = model.platen
                    Task.detached { _ = try? platen.stopJob() }
                }
                .fontWeight(.semibold)
            }
        } else {
            Button {
                if twoSided { send(job(.odd), "Printing fronts") { flip = true } } else { send(job(), "Printing") }
            } label: {
                Label(twoSided ? "Print fronts" : copies > 1 ? "Print \(copies) copies" : "Print", systemImage: "printer")
                    .font(.headline)
                    .frame(maxWidth: .infinity, minHeight: 36)
            }
            .prominent(glass: true)
            .controlSize(.large)
            .disabled(model.device == nil || plan == nil || !sel.contains(true))
        }
    }

    private var planKey: String {
        "\(files)|\(ranges)|\(nup)|\(scaleMode)|\(percent)|\(reverse)|\(tone)|\(media)|\(borderless)"
    }

    private func job(_ subset: Subset = .all, reverse rev: Bool? = nil) -> PrintJob {
        PrintJob(files: files.map(\.path), ranges: ranges, subset: subset, reverse: rev ?? reverse, nup: UInt32(nup), scale: scaleMode.scale(percent),
                 collate: collate, clean: tone == .clean,
                 opts: PrintOpts(copies: UInt32(copies), color: tone == .colour, media: media, scaling: "fit", quality: quality, mediaType: mediaType, borderless: borderless))
    }

    private func view(_ f: URL) {
        if let i = pages.firstIndex(where: { $0.file == f }) { viewing = i }
    }

    private func moveFile(_ i: Int, by d: Int) {
        var list = files
        list.swapAt(i, i + d)
        model.printFiles = list
    }

    private func setSel(_ s: [Bool]) {
        sel = s
        ranges = s.allSatisfy { $0 } ? "" : toRanges(s)
    }

    private func loadPages() async {
        let platen = model.platen
        let list = files
        let counts = await Task.detached { list.map { (try? platen.pageCount(path: $0.path)).map(Int.init) ?? 1 } }.value
        pages = zip(list, counts).flatMap { f, n in (0..<n).map { SrcPage(file: f, index: $0) } }
        sel = pages.map { _ in true }
        ranges = ""
    }

    private func updatePlan() async {
        guard !files.isEmpty else {
            plan = nil
            sheets = []
            return
        }
        try? await Task.sleep(for: .milliseconds(250))
        if Task.isCancelled { return }
        let j = job()
        let platen = model.platen
        let p = try? await off { try platen.printPlan(job: j) }
        if Task.isCancelled { return }
        plan = p
        let count = min(p?.order.count ?? 0, 8)
        let images = await Task.detached {
            (0..<count).map { i in (try? platen.printPreview(job: j, slot: UInt32(i), dpi: 40)).flatMap(UIImage.init(data:)) }
        }.value
        if !Task.isCancelled { sheets = images }
    }

    private func send(_ j: PrintJob, _ label: String, then: @escaping () -> Void = {}) {
        busy = label
        log = nil
        let platen = model.platen
        let progress = printProgress(label) { busy = $0 }
        Task {
            do {
                let n = try await off { try platen.print(job: j, progress: progress) }
                log = "Sent \(n) \(n == 1 ? "job" : "jobs") to the printer."
                model.outcome += 1
                then()
            } catch {
                log = error.text == "stopped" ? "Stopped. Nothing else will print." : error.text
                if error.text != "stopped" { model.failure += 1 }
            }
            busy = nil
        }
    }

    private func loadPhotos() {
        let items = photos
        guard !items.isEmpty else { return }
        photos = []
        let existing = files
        model.busy = "Opening"
        Task {
            var out: [URL] = []
            for (i, item) in items.enumerated() {
                if let data = try? await item.loadTransferable(type: Data.self),
                   let url = try? Storage.incomingImage(data, name: "Photo \(i + 1).jpg") {
                    out.append(url)
                }
            }
            model.busy = nil
            model.printFiles = existing + out
        }
    }
}

private struct CropTarget: Identifiable {
    let url: URL
    var id: URL { url }
}
