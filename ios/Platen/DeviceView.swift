import SwiftUI

private enum HeadAction: Identifiable {
    case check, clean
    var id: Self { self }
}

struct DeviceView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var details: [DetailGroup]?
    @State private var detailsOpen = true
    @State private var loading = false
    @State private var askPassword = false
    @State private var wrong = false
    @State private var password = ""
    @State private var renaming: SavedPrinter?
    @State private var afterUnlock: (() -> Void)?
    @State private var inks = Inks.black
    @State private var strength = Strength.normal
    @State private var confirming: HeadAction?
    @State private var lowInks: [Ink] = []
    @State private var headBusy = false
    @State private var headNote: String?

    var body: some View {
        NavigationStack {
            Form {
                if !model.lowInks.isEmpty {
                    Section { LowInkBanner(inks: model.lowInks) }.listRowInsets(EdgeInsets()).listRowBackground(Color.clear)
                }
                printerCard
                printersSection
                detailsSection
                headSection
                Section {
                    Text(Storage.label)
                } header: {
                    Text("Where scans are saved")
                } footer: {
                    Text("Exports show up in the Files app, in a folder for each scan.")
                }
                Section {
                    Picker("Delete drafts", selection: Binding(get: { model.keepDays }, set: { model.setKeepDays($0) })) {
                        Text("Keep all").tag(0)
                        Text("After 30 days").tag(30)
                        Text("After 90 days").tag(90)
                    }
                } header: {
                    Text("Drafts")
                } footer: {
                    Text("Delete unfinished scans you haven't touched for a while. Exported files are never removed.")
                }
                Section("Accent colour") {
                    HStack {
                        ForEach(Accent.allCases) { a in
                            Button { model.setAccent(a) } label: {
                                Circle().fill(a.color).frame(width: 32, height: 32)
                                    .padding(4)
                                    .overlay(Circle().strokeBorder(Color.primary, lineWidth: model.accent == a ? 2 : 0))
                                    .frame(minWidth: 44, minHeight: 44)
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel(a.label)
                            .accessibilityAddTraits(model.accent == a ? .isSelected : [])
                            if a != Accent.allCases.last { Spacer() }
                        }
                    }
                    Text(model.accent.label).foregroundStyle(.secondary)
                }
            }
            .navigationTitle("Printer and settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", systemImage: "checkmark") { dismiss() } } }
            .onAppear { model.refreshDevice() }
            .onChange(of: inks) { if inks != .all, strength == .special { strength = .normal } }
            .sheet(isPresented: $askPassword, onDismiss: { wrong = false }) { passwordSheet }
            .alert(confirming == .clean ? "Clean the print head?" : "Print a nozzle check?", isPresented: Binding(get: { confirming != nil }, set: { if !$0 { confirming = nil } }), presenting: confirming) { action in
                Button("Cancel", role: .cancel) {}
                Button(action == .check ? "Print" : lowInks.isEmpty ? "Start cleaning" : "Clean anyway", role: lowInks.isEmpty ? nil : .destructive) { runHead(action) }
            } message: { action in
                Text(confirmText(action))
            }
            .alert("Rename printer", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } }), presenting: renaming) { p in
                RenameField(initial: p.nickname ?? p.model) { model.renamePrinter(p, $0) }
            }
        }
    }

    private var printerCard: some View {
        Section {
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(model.device?.name ?? "No printer").font(.title3.bold())
                    Text(statusLine).font(.subheadline).foregroundStyle(.secondary)
                    if let reasons = model.status?.printer?.reasons.filter({ $0 != "none" }), !reasons.isEmpty {
                        Text(reasons.joined(separator: ", ").replacingOccurrences(of: "-", with: " ")).font(.caption).foregroundStyle(.red)
                    }
                }
                if let ink = model.status?.printer?.ink, !ink.isEmpty {
                    HStack {
                        ForEach(ink, id: \.name) { i in
                            InkTank(name: i.name, level: i.level).frame(maxWidth: .infinity)
                        }
                    }
                }
                Button("Refresh", systemImage: "arrow.clockwise") { model.refreshDevice() }
                    .buttonStyle(.bordered)
            }
            .padding(.vertical, 6)
        }
    }

    private var statusLine: String {
        if let s = model.status {
            return (["Scanner \(s.scanner.lowercased())"] + (s.printer.map { ["printer \($0.state.lowercased())"] } ?? [])).joined(separator: ", ")
        }
        return model.device?.host ?? "Make sure it's on and this phone is on the same Wi-Fi."
    }

    private var printersSection: some View {
        Section("Printers") {
            ForEach(model.printers) { p in
                let current = p.base == model.current
                Button {
                    if !current { model.usePrinter(p) }
                } label: {
                    HStack(spacing: 12) {
                        Image(systemName: current ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(current ? Color.accentColor : .secondary)
                            .font(.title3)
                            .accessibilityHidden(true)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(p.label).font(.headline).lineLimit(1)
                            if p.nickname != nil { Text(p.model).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                            Text(p.ip).font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer(minLength: 0)
                    }
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(current ? .isSelected : [])
                .accessibilityActions {
                    Button("Rename") { renaming = p }
                    Button("Forget") { model.forgetPrinter(p) }
                }
                .swipeActions {
                    Button("Forget", systemImage: "trash", role: .destructive) { model.forgetPrinter(p) }
                    Button("Rename", systemImage: "pencil") { renaming = p }.tint(.orange)
                }
                .contextMenu {
                    Button("Rename", systemImage: "pencil") { renaming = p }
                    Button("Forget", systemImage: "trash", role: .destructive) { model.forgetPrinter(p) }
                }
            }
            if model.printers.isEmpty { Text("No printers saved yet.").foregroundStyle(.secondary) }
            HStack {
                Button("Look for printers", systemImage: "magnifyingglass") { model.searchPrinters() }
                    .disabled(model.searching)
                if model.searching { Spacer(); ProgressView() }
            }
        }
    }

    private var detailsSection: some View {
        Section {
            Button {
                if details == nil { load(nil) } else { withAnimation { detailsOpen.toggle() } }
            } label: {
                HStack(spacing: 14) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 10).fill(Color.accentColor.opacity(0.15)).frame(width: 40, height: 40)
                        if loading { ProgressView() } else { Image(systemName: "info.circle").foregroundStyle(Color.accentColor) }
                    }
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Counters, maintenance and network").font(.headline)
                        Text(loading ? "Reading the printer's pages…" : details == nil ? "Read from the printer's own web page" : detailsOpen ? "Tap to hide" : "Tap to show")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.right")
                        .foregroundStyle(.secondary)
                        .rotationEffect(.degrees(details != nil && detailsOpen ? 90 : 0))
                }
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .disabled(model.host == nil || loading)
            .accessibilityValue(details == nil ? "" : detailsOpen ? "Shown" : "Hidden")
            if let details, detailsOpen {
                ForEach(Array(details.enumerated()), id: \.offset) { _, g in
                    if !g.title.trimmingCharacters(in: .whitespaces).isEmpty {
                        Text(g.title).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).padding(.top, 6).accessibilityAddTraits(.isHeader)
                    }
                    ForEach(Array(g.rows.enumerated()), id: \.offset) { _, r in DetailLine(row: r) }
                }
                if let host = model.host {
                    Button("Forget the printer password", role: .destructive) {
                        Secrets.forget(host)
                        self.details = nil
                    }
                }
            }
        }
    }

    private var headSection: some View {
        Section {
            Text("For faint, streaky or missing colours. Print a nozzle check first and clean only the colours with gaps.")
                .font(.subheadline).foregroundStyle(.secondary)
            Button("Print nozzle check", systemImage: "printer.dotmatrix") { ask(.check) }
                .disabled(model.host == nil || headBusy)
            Picker("Inks", selection: $inks) {
                Text("Black").tag(Inks.black)
                Text("Colour").tag(Inks.colour)
                Text("All").tag(Inks.all)
            }
            .disabled(headBusy)
            Picker("Strength", selection: $strength) {
                Text("Normal").tag(Strength.normal)
                Text("Strong").tag(Strength.strong)
                Text("Strongest").tag(Strength.strongest)
                if inks == .all { Text("Special").tag(Strength.special) }
            }
            .disabled(headBusy)
            HStack {
                Button("Clean print head", systemImage: "drop") { ask(.clean) }
                    .disabled(model.host == nil || headBusy)
                if headBusy { Spacer(); ProgressView() }
            }
            if let headNote { Text(headNote).font(.subheadline).onAppear { announce(headNote) } }
        } header: {
            Text("Print head")
        }
    }

    private var passwordSheet: some View {
        NavigationStack {
            Form {
                Section {
                    SecureField("Password", text: $password)
                        .textContentType(.password)
                        .onChange(of: password) { wrong = false }
                        .onSubmit { if !password.isEmpty { load(password) } }
                } footer: {
                    Text(wrong ? "That password didn't work. Try again." : "The admin password for the printer's web page. It's kept in this phone's Keychain.")
                        .foregroundStyle(wrong ? .red : .secondary)
                }
                Section {
                    Button {
                        load(password)
                    } label: {
                        Group {
                            if loading { ProgressView().tint(model.accent.on) } else { Text("Unlock").font(.headline) }
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .prominent()
                    .controlSize(.large)
                    .disabled(password.isEmpty || loading)
                    .listRowInsets(EdgeInsets())
                    .listRowBackground(Color.clear)
                }
            }
            .navigationTitle("Printer password")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel", systemImage: "xmark") { askPassword = false } } }
        }
        .presentationDetents([.medium])
    }

    private func confirmText(_ action: HeadAction) -> String {
        if action == .check { return "Prints one sheet. Load A4 or Letter plain paper in the main tray." }
        let extra = switch strength {
        case .normal: ""
        case .special: ", and Special uses the most"
        case .strong: ", and Strong uses more than Normal"
        case .strongest: ", and Strongest uses more than Normal"
        }
        var text = "Cleaning uses ink\(extra). The printer can't print or copy until it's done, about a minute."
        if !lowInks.isEmpty {
            text += "\n\n" + lowInks.map { "\($0.name) is at \($0.level)%" }.joined(separator: ", ") + ". Cleaning with little ink left can damage the print head. Refill first."
        }
        return text
    }

    private func load(_ typed: String?) {
        guard let host = model.host else { return }
        guard let pw = typed ?? Secrets.password(host) else {
            askPassword = true
            return
        }
        loading = true
        let platen = model.platen
        Task {
            defer { loading = false }
            do {
                details = try await off { try platen.deviceDetails(password: pw) }
                detailsOpen = true
                if let typed { Secrets.save(host, typed) }
                askPassword = false
                wrong = false
                password = ""
                afterUnlock?()
                afterUnlock = nil
            } catch {
                if error.text == "wrong-password" {
                    askPassword = true
                    wrong = true
                } else {
                    model.say(error.text)
                }
            }
        }
    }

    private func ask(_ action: HeadAction) {
        let platen = model.platen
        let inks = inks
        Task {
            lowInks = action == .clean ? ((try? await off { try platen.cleaningLowInks(inks: inks) }) ?? []) : []
            confirming = action
        }
    }

    private func runHead(_ action: HeadAction) {
        guard let host = model.host else { return }
        guard let pw = Secrets.password(host) else {
            afterUnlock = { runHead(action) }
            askPassword = true
            return
        }
        headBusy = true
        headNote = nil
        let platen = model.platen
        let (inks, strength) = (inks, strength)
        Task {
            defer { headBusy = false }
            do {
                if action == .clean {
                    try await off { try platen.cleanHead(password: pw, inks: inks, strength: strength) }
                    headNote = "Cleaning. The printer is busy for a minute or more; print a nozzle check afterwards to see if it helped."
                } else {
                    try await off { try platen.nozzleCheck(password: pw) }
                    headNote = "Printing. Every dotted line in the four colour blocks should be there with no gaps. If any are missing, clean that colour."
                }
            } catch {
                if error.text == "wrong-password" {
                    afterUnlock = { runHead(action) }
                    askPassword = true
                    wrong = true
                } else {
                    model.say(error.text)
                }
            }
        }
    }
}

private struct DetailLine: View {
    let row: DetailRow

    var body: some View {
        let value = row.value.trimmingCharacters(in: .whitespaces)
        let numeric = value.range(of: #"^[\d,]+%?( .*)?$"#, options: .regularExpression) != nil
        HStack(spacing: 12) {
            Text(row.label)
                .font(row.sub ? .subheadline : .subheadline.weight(.semibold))
                .foregroundStyle(row.sub ? .secondary : .primary)
            Spacer(minLength: 8)
            Text(value.isEmpty ? "—" : value)
                .font(numeric && !row.sub ? .title3.weight(.semibold) : .subheadline)
                .foregroundStyle(row.sub ? .secondary : .primary)
                .multilineTextAlignment(.trailing)
        }
        .padding(.leading, row.sub ? 16 : 0)
        .accessibilityElement(children: .combine)
    }
}

private struct RenameField: View {
    let initial: String
    let done: (String) -> Void
    @State private var text = ""

    var body: some View {
        TextField("Name", text: $text).onAppear { text = initial }
        Button("Cancel", role: .cancel) {}
        Button("Save") { done(text) }
    }
}
