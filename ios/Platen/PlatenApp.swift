import SwiftUI

@main
struct PlatenApp: App {
    @State private var model = AppModel()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .tint(model.accent.color)
                .onOpenURL { model.open([$0]) }
                .onChange(of: phase) {
                    guard phase == .active else { return }
                    if model.device == nil { model.find() } else { model.refreshDevice() }
                }
        }
    }
}

struct RootView: View {
    @Environment(AppModel.self) private var model
    @State private var showDevice = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        @Bindable var model = model
        TabView(selection: $model.tab) {
            ForEach(AppTab.allCases, id: \.self) { t in
                SwiftUI.Tab(t.rawValue, systemImage: t.icon, value: t) {
                    NavigationStack {
                        screen(t)
                            .toolbar { bar(t) }
                            .navigationBarTitleDisplayMode(.inline)
                    }
                }
            }
        }
        .sensoryFeedback(.selection, trigger: model.tab)
        .sensoryFeedback(.success, trigger: model.outcome)
        .sensoryFeedback(.error, trigger: model.failure)
        .overlay(alignment: .bottom) {
            if let m = model.message {
                Toast(text: m)
                    .padding(.bottom, model.tab == .scan && model.session != nil ? 170 : 96)
                    .transition(reduceMotion ? .opacity : .move(edge: .bottom).combined(with: .opacity))
                    .onTapGesture { model.message = nil }
                    .task(id: m) {
                        announce(m)
                        try? await Task.sleep(for: .seconds(UIAccessibility.isVoiceOverRunning ? 7 : 3.5))
                        withAnimation { model.message = nil }
                    }
            }
        }
        .animation(.spring(duration: 0.3), value: model.message)
        .sheet(isPresented: $showDevice) { DeviceView() }
        .fullScreenCover(isPresented: Binding(get: { model.shrinking != nil }, set: { if !$0 { model.shrinking = nil } })) {
            if let s = model.shrinking { ShrinkView(file: s.file, folder: s.folder) }
        }
    }

    @ViewBuilder
    private func screen(_ t: AppTab) -> some View {
        switch t {
        case .scan: ScanTab()
        case .copy: CopyTab()
        case .print: PrintTab()
        case .files: FilesTab()
        }
    }

    @ToolbarContentBuilder
    private func bar(_ t: AppTab) -> some ToolbarContent {
        ToolbarItem(placement: .principal) {
            PrinterMenu(title: t == .scan ? model.session?.name ?? "Platen" : t.rawValue, onSettings: { showDevice = true })
        }
        if t == .scan { ScanActions() }
        ToolbarItem(placement: .topBarTrailing) {
            Button("Printer and settings", systemImage: "gearshape") { showDevice = true }
        }
    }
}

struct PrinterMenu: View {
    @Environment(AppModel.self) private var model
    let title: String
    let onSettings: () -> Void

    var body: some View {
        Menu {
            Section("Printers") {
                ForEach(model.printers) { p in
                    Button {
                        if p.base != model.current { model.usePrinter(p) }
                    } label: {
                        if p.base == model.current { Label(p.label, systemImage: "checkmark") } else { Text(p.label) }
                    }
                }
            }
            Button("Look for printers", systemImage: "magnifyingglass") { model.searchPrinters() }
            Button("Printer and settings", systemImage: "gearshape", action: onSettings)
        } label: {
            VStack(spacing: 0) {
                Text(title).font(.headline).lineLimit(1)
                HStack(spacing: 3) {
                    Text(model.finding ? "Looking for the printer…" : model.device?.name ?? "No printer found").lineLimit(1)
                    Image(systemName: "chevron.down").imageScale(.small)
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            .frame(maxWidth: 240)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(title). Printer: \(model.device?.name ?? "none"). Switch printer")
    }
}
