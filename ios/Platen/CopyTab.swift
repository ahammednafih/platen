import SwiftUI

private enum CopyKind: String, CaseIterable, Identifiable {
    case one, twoInOne, fourInOne, idCard, poster, long

    var id: String { rawValue }

    var title: String {
        switch self {
        case .one: "Copy"
        case .twoInOne: "2 in 1"
        case .fourInOne: "4 in 1"
        case .idCard: "ID card"
        case .poster: "Poster"
        case .long: "Long page"
        }
    }

    var short: String {
        switch self {
        case .one: "One page, one sheet"
        case .twoInOne: "Two pages side by side"
        case .fourInOne: "Four pages on one sheet"
        case .idCard: "Both sides, actual size"
        case .poster: "One page across several sheets"
        case .long: "Taller than the glass"
        }
    }

    var glyph: Glyph {
        switch self {
        case .one: .one
        case .twoInOne: .twoInOne
        case .fourInOne: .fourInOne
        case .idCard: .idCard
        case .poster: .poster
        case .long: .long
        }
    }

    var steps: [(String, String)] {
        switch self {
        case .one:
            [("Put the page face down on the glass, top edge against the far edge.", "Copy")]
        case .twoInOne:
            [("Put the first page face down on the glass. It goes on the left of the sheet.", "Scan first page"),
             ("Put the second page face down on the glass. The copy prints right after this scan.", "Scan second page")]
        case .fourInOne:
            [("Put the first page face down on the glass. It goes top left on the sheet.", "Scan page 1"),
             ("Put the second page face down on the glass. It goes top right.", "Scan page 2"),
             ("Put the third page face down on the glass. It goes bottom left.", "Scan page 3"),
             ("Put the fourth page face down on the glass. The copy prints right after this scan.", "Scan page 4")]
        case .idCard:
            [("Put the card face down in the top half of the glass, a little away from the corner.", "Scan front"),
             ("Turn the card over and put it back in the same spot. The copy prints right after this scan.", "Scan back")]
        case .poster:
            [("Put the page face down on the glass, top edge against the far edge. It prints across several sheets.", "Scan and print poster")]
        case .long:
            [("Put the top of the page face down on the glass, top edge against the far edge.", "Scan top half"),
             ("Turn the page around so the bottom is on the glass. Leave a few centimetres of overlap.", "Scan bottom half")]
        }
    }
}

private final class Progress: PrintProgress, @unchecked Sendable {
    let report: @Sendable (UInt32, UInt32) -> Void
    init(_ report: @escaping @Sendable (UInt32, UInt32) -> Void) { self.report = report }
    func progress(done: UInt32, total: UInt32) { report(done, total) }
}

func printProgress(_ label: String, into update: @escaping @MainActor (String) -> Void) -> PrintProgress {
    Progress { done, total in Task { @MainActor in update("\(label) \(done) of \(total)") } }
}

struct CopyTab: View {
    @Environment(AppModel.self) private var model
    @State private var kind = CopyKind.one
    @State private var original = SizeId.a4
    @State private var copies = 1
    @State private var color = false
    @State private var density = 0.0
    @State private var background = 0
    @State private var scaleMode = ScaleMode.actual
    @State private var percent = 100
    @State private var media = "iso_a4_210x297mm"
    @State private var quality = Quality.normal
    @State private var mediaType: String?
    @State private var posterSize = 3
    @State private var step = 0
    @State private var scans: [String] = []
    @State private var top: TopHalf?
    @State private var busy: String?
    @State private var log: String?

    var body: some View {
        Form {
            if !model.lowInks.isEmpty {
                Section { LowInkBanner(inks: model.lowInks) }.listRowInsets(EdgeInsets()).listRowBackground(Color.clear)
            }
            Section {
                LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
                    ForEach(CopyKind.allCases) { k in modeCard(k) }
                }
                .padding(.vertical, 4)
            }
            .listRowInsets(EdgeInsets())
            .listRowBackground(Color.clear)
            Section { run }
                .listRowInsets(EdgeInsets())
                .listRowBackground(Color.clear)
            Section("Copy") {
                if [.one, .twoInOne, .fourInOne, .poster].contains(kind) {
                    Picker("Original size", selection: $original) {
                        ForEach(sizeChoices(model.caps), id: \.0) { Text($0.1).tag($0.0) }
                    }
                }
                if kind == .one { ScaleSection(mode: $scaleMode, percent: $percent) }
                if kind == .poster {
                    Picker("Poster size", selection: $posterSize) {
                        Text("2 × 2 sheets").tag(2)
                        Text("3 × 3 sheets").tag(3)
                    }
                    .pickerStyle(.segmented)
                }
                Stepper("Copies: \(copies)", value: $copies, in: 1...99)
                Picker("Colour", selection: $color) {
                    Text("Black and white").tag(false)
                    Text("Colour").tag(true)
                }
                .pickerStyle(.segmented)
                VStack(alignment: .leading) {
                    let d = Int(density)
                    Text("Density: \(d > 0 ? "+\(d) darker" : d < 0 ? "\(d) lighter" : "normal")")
                    Slider(value: $density, in: -3...3, step: 1)
                }
                .sensoryFeedback(.selection, trigger: density)
                Picker("Background", selection: $background) {
                    Text("Keep").tag(0)
                    Text("Low").tag(1)
                    Text("Medium").tag(2)
                    Text("High").tag(3)
                }
            }
            Section("Paper") { PaperSection(caps: model.printCaps, media: $media, quality: $quality, mediaType: $mediaType) }
        }
        .onAppear { if let d = model.printCaps?.mediaDefault, media == "iso_a4_210x297mm" { media = d } }
        .onDisappear { if busy == nil { reset() } }
        .sensoryFeedback(.selection, trigger: kind)
    }

    private func modeCard(_ k: CopyKind) -> some View {
        let on = kind == k
        return Button {
            guard busy == nil, kind != k else { return }
            kind = k
            log = nil
            reset()
        } label: {
            HStack(spacing: 12) {
                SheetGlyph(kind: k.glyph, active: on, width: 30)
                VStack(alignment: .leading, spacing: 2) {
                    Text(k.title).font(.headline)
                    Text(k.short).font(.caption).foregroundStyle(.secondary).lineLimit(2).multilineTextAlignment(.leading)
                }
                Spacer(minLength: 0)
            }
            .padding(14)
            .frame(maxWidth: .infinity, minHeight: 76, alignment: .leading)
            .background(on ? Color.accentColor.opacity(0.14) : Color(.secondarySystemGroupedBackground), in: .rect(cornerRadius: 20))
            .overlay(RoundedRectangle(cornerRadius: 20).strokeBorder(on ? Color.accentColor : .clear, lineWidth: 2))
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private var run: some View {
        GlassBed {
            VStack(alignment: .leading, spacing: 16) {
                if kind.steps.count > 1 {
                    HStack(spacing: 6) {
                        ForEach(kind.steps.indices, id: \.self) { i in
                            Capsule().fill(i <= step ? Color.accentColor : Color(.separator)).frame(width: i == step ? 28 : 10, height: 10)
                        }
                    }
                    .animation(.spring(duration: 0.3), value: step)
                }
                Text(kind.steps[min(step, kind.steps.count - 1)].0).font(.title3.weight(.semibold))
                    .contentTransition(.opacity)
                if let busy {
                    BusyPill(label: "\(busy)…") {
                        Button("Stop") {
                            let platen = model.platen
                            Task.detached { _ = try? platen.stopJob() }
                        }
                        .fontWeight(.semibold)
                    }
                } else {
                    HStack {
                        Button { go() } label: {
                            Text(kind.steps[min(step, kind.steps.count - 1)].1).font(.headline).frame(maxWidth: .infinity)
                        }
                        .prominent()
                        .controlSize(.large)
                        .disabled(model.device == nil)
                        if step > 0 { Button("Start over") { reset() } }
                    }
                }
                if let log { Text(log).font(.subheadline).foregroundStyle(.secondary) }
            }
            .padding(20)
        }
    }

    private func reset() {
        step = 0
        scans = []
        top = nil
        model.platen.copyReset()
    }

    private func go() {
        log = nil
        busy = "Scanning"
        let platen = model.platen
        let (kind, step, color, original, top) = (kind, step, color, original, top)
        Task {
            do {
                if kind == .long, step == 0 {
                    let t = try await off { try platen.copyLongTop(color: color) }
                    self.top = t
                    scans.append(t.path)
                } else if kind == .long, let top {
                    scans.append(try await off { try platen.copyLongBottom(top: top, color: color) })
                } else {
                    let area: Area = kind == .idCard ? .halfTop : .size(size: original)
                    scans.append(try await off { try platen.copyScan(area: area, color: color) })
                }
                if step < kind.steps.count - 1 {
                    self.step += 1
                    busy = nil
                    return
                }
                try await finish()
                log = kind == .poster ? "Sent the poster: \(posterSize * posterSize) sheets, row by row from the top left." : "Sent \(copies) \(copies == 1 ? "copy" : "copies") to the printer."
                model.outcome += 1
                reset()
            } catch {
                log = error.text == "stopped" ? "Stopped. Nothing else will print." : error.text
                if error.text != "stopped" { model.failure += 1 }
            }
            busy = nil
        }
    }

    private func finish() async throws {
        let platen = model.platen
        var files = kind == .long ? [scans.last!] : scans
        if kind == .idCard {
            busy = "Placing front and back"
            let (front, back) = (scans[0], scans[1])
            files = [try await off { try platen.copyIdCard(front: front, back: back) }]
        }
        if kind == .poster {
            busy = "Making the poster"
            let (first, n, media) = (files[0], UInt32(posterSize), media)
            files = try await off { try platen.copyPoster(path: first, n: n, media: media) }
        }
        if density != 0 || background != 0 {
            busy = "Adjusting"
            let (input, d, bg) = (files, Int32(density), UInt8(background))
            files = try await off { try platen.copyTone(files: input, density: d, background: bg) }
        }
        busy = "Printing"
        let scale: Scale = switch kind {
        case .one: scaleMode.scale(percent)
        case .idCard: .actual
        default: .fit
        }
        let nup: UInt32 = kind == .twoInOne ? 2 : kind == .fourInOne ? 4 : 1
        let job = PrintJob(files: files, ranges: "", subset: .all, reverse: false, nup: nup, scale: scale, collate: true, clean: false,
                           opts: PrintOpts(copies: UInt32(copies), color: color, media: media, scaling: "fit", quality: quality, mediaType: mediaType, borderless: false))
        let progress = printProgress("Printing") { busy = $0 }
        _ = try await off { try platen.print(job: job, progress: progress) }
    }
}
