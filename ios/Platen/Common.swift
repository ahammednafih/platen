import SwiftUI

enum Accent: String, CaseIterable, Identifiable {
    case ink, magenta, orange, teal, coffee

    var id: String { rawValue }

    var label: String {
        switch self {
        case .ink: "Ink black"
        case .magenta: "Magenta"
        case .orange: "Toner orange"
        case .teal: "Teal"
        case .coffee: "Coffee"
        }
    }

    var color: Color {
        switch self {
        case .ink: Color(light: 0x1C1D1F, dark: 0xE8E6E1)
        case .magenta: Color(light: 0xC2185B, dark: 0xE0457F)
        case .orange: Color(light: 0xD9480F, dark: 0xF2703A)
        case .teal: Color(light: 0x0F766E, dark: 0x2BB3A3)
        case .coffee: Color(light: 0x7A4A2A, dark: 0x8B5A3C)
        }
    }
}

extension Accent {
    var onPaper: Color {
        switch self {
        case .ink: Color(hex: 0x1C1D1F)
        case .magenta: Color(hex: 0xC2185B)
        case .orange: Color(hex: 0xD9480F)
        case .teal: Color(hex: 0x0F766E)
        case .coffee: Color(hex: 0x7A4A2A)
        }
    }

    var on: Color {
        switch self {
        case .ink: Color(light: 0xFFFFFF, dark: 0x111214)
        case .orange: Color(light: 0xFFFFFF, dark: 0x1B120C)
        case .teal: Color(light: 0xFFFFFF, dark: 0x04201D)
        case .magenta, .coffee: .white
        }
    }
}

private struct Prominent: ViewModifier {
    @Environment(AppModel.self) private var model
    let glass: Bool

    func body(content: Content) -> some View {
        if glass {
            content.buttonStyle(.glassProminent).foregroundStyle(model.accent.on)
        } else {
            content.buttonStyle(.borderedProminent).foregroundStyle(model.accent.on)
        }
    }
}

extension View {
    func prominent(glass: Bool = false) -> some View { modifier(Prominent(glass: glass)) }
}

extension Color {
    init(hex: UInt32) {
        self.init(red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255, blue: Double(hex & 0xFF) / 255)
    }

    init(light: UInt32, dark: UInt32) {
        self.init(UIColor { $0.userInterfaceStyle == .dark ? UIColor(Color(hex: dark)) : UIColor(Color(hex: light)) })
    }
}

func inkColor(_ name: String) -> Color {
    let n = name.lowercased()
    if n.contains("cyan") { return Color(hex: 0x00A6D6) }
    if n.contains("magenta") { return Color(hex: 0xD6247D) }
    if n.contains("yellow") { return Color(hex: 0xF2C200) }
    return Color(light: 0x2A2B2E, dark: 0x8E9199)
}

func size(_ bytes: Int64) -> String {
    bytes >= 1_000_000 ? String(format: "%.1f MB", Double(bytes) / 1e6) : "\(max(1, Int((Double(bytes) / 1e3).rounded()))) KB"
}

func dur(_ secs: Float) -> String {
    let s = Int(secs.rounded())
    if s < 60 { return "\(s)s" }
    if s < 3600 { return "\(s / 60)m \(String(format: "%02d", s % 60))s" }
    return "\(s / 3600)h \(String(format: "%02d", (s % 3600) / 60))m"
}

func plural(_ n: Int, _ word: String) -> String { n == 1 ? "1 \(word)" : "\(n) \(word)s" }

func paperName(_ kw: String) -> String {
    let named = [
        "iso_a4": "A4", "iso_a5": "A5", "iso_a6": "A6", "na_letter": "Letter", "na_legal": "Legal", "jis_b5": "B5",
        "na_executive": "Executive", "iso_dl": "DL envelope", "iso_c5": "C5 envelope", "na_number-10": "No. 10 envelope",
        "na_monarch": "Monarch envelope", "na_foolscap": "Foolscap", "na_oficio": "Oficio", "om_india-legal": "India legal",
        "na_index-4x6": "Photo 4×6 in", "na_5x7": "Photo 5×7 in", "oe_photo-l": "Photo 3.5×5 in", "na_index-5x8": "Card 5×8 in",
    ]
    let parts = kw.split(separator: "_").map(String.init)
    if let n = named[parts.prefix(2).joined(separator: "_")] { return n }
    let dims = parts.last ?? kw
    return dims.replacingOccurrences(of: "x", with: " × ").replacingOccurrences(of: "mm", with: " mm").replacingOccurrences(of: "in", with: " in")
}

func paperTypeName(_ kw: String) -> String {
    switch kw {
    case "stationery": return "Plain"
    case "stationery-inkjet": return "Inkjet"
    case "photographic-glossy": return "Glossy photo"
    case "photographic-matte": return "Matte photo"
    case "photographic": return "Photo"
    case "photographic-high-gloss": return "High-gloss photo"
    case "photographic-semi-gloss": return "Semi-gloss photo"
    case "envelope": return "Envelope"
    case "labels": return "Labels"
    case "cardstock": return "Card"
    default:
        if kw.hasPrefix("com.") || kw.hasPrefix("org.") { return "Printer's photo paper" }
        let s = kw.replacingOccurrences(of: "-", with: " ")
        return s.prefix(1).uppercased() + s.dropFirst()
    }
}

func modeLabel(_ m: ColorMode) -> String {
    switch m {
    case .color: "Colour"
    case .gray: "Gray"
    case .blackWhite: "Black and white"
    }
}

func qualityLabel(_ q: Quality) -> String {
    switch q {
    case .draft: "Draft"
    case .normal: "Normal"
    case .high: "High"
    }
}

let fallbackSizes: [(SizeId, String)] = [(.a4, "A4"), (.letter, "Letter"), (.a5, "A5"), (.a6, "A6")]

func sizeChoices(_ caps: Caps?) -> [(SizeId, String)] {
    guard let caps, !caps.sizes.isEmpty else { return fallbackSizes }
    return caps.sizes.map { ($0.id, $0.name) }
}

func sizeName(_ caps: Caps?, _ id: SizeId) -> String {
    sizeChoices(caps).first { $0.0 == id }?.1 ?? "\(id)".capitalized
}

enum ScaleMode: String, CaseIterable, Identifiable {
    case fit = "Fit to sheet", actual = "Actual size", percent = "Percent"
    var id: String { rawValue }

    func scale(_ percent: Int) -> Scale {
        switch self {
        case .fit: .fit
        case .actual: .actual
        case .percent: .percent(percent: UInt32(percent))
        }
    }
}

func toRanges(_ sel: [Bool]) -> String {
    var out: [String] = []
    var i = 0
    while i < sel.count {
        guard sel[i] else { i += 1; continue }
        var j = i
        while j + 1 < sel.count, sel[j + 1] { j += 1 }
        out.append(i == j ? "\(i + 1)" : "\(i + 1)-\(j + 1)")
        i = j + 1
    }
    return out.joined(separator: ", ")
}

func fromRanges(_ text: String, _ n: Int) -> [Bool]? {
    if text.trimmingCharacters(in: .whitespaces).isEmpty { return Array(repeating: true, count: n) }
    var sel = Array(repeating: false, count: n)
    for part in text.split(separator: ",").map({ $0.trimmingCharacters(in: .whitespaces) }) where !part.isEmpty {
        let bits = part.split(separator: "-", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespaces) }
        guard let a = Int(bits[0]) else { return nil }
        let b: Int
        if bits.count > 1 {
            guard let v = bits[1].isEmpty ? n : Int(bits[1]) else { return nil }
            b = v
        } else {
            b = a
        }
        guard a <= b else { continue }
        for k in a...b where (1...max(n, 1)).contains(k) && k <= n { sel[k - 1] = true }
    }
    return sel
}

// MARK: Shared views

struct GlassBed<Content: View>: View {
    var corner: CGFloat = 28
    @ViewBuilder var content: Content

    var body: some View {
        content
            .frame(maxWidth: .infinity)
            .background(Color(.secondarySystemBackground), in: .rect(cornerRadius: corner))
            .overlay(RoundedRectangle(cornerRadius: corner).strokeBorder(Color(.separator).opacity(0.5), lineWidth: 1))
    }
}

struct Paper<Content: View>: View {
    var fresh = false
    @ViewBuilder var content: Content
    @State private var dropped = false

    var body: some View {
        content
            .background(Color.white)
            .clipShape(.rect(cornerRadius: 4))
            .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(Color.black.opacity(0.08), lineWidth: 0.5))
            .shadow(color: .black.opacity(0.2), radius: 2, y: 1)
            .scaleEffect(fresh && !dropped ? 0.92 : 1)
            .opacity(fresh && !dropped ? 0 : 1)
            .offset(y: fresh && !dropped ? -24 : 0)
            .onAppear {
                guard fresh else { return }
                withAnimation(.spring(response: 0.45, dampingFraction: 0.6)) { dropped = true }
            }
    }
}

struct FileImage: View {
    let path: String
    var page = 0
    var width = 400
    var fill = false
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image {
                Image(uiImage: image).resizable().aspectRatio(contentMode: fill ? .fill : .fit)
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .task(id: "\(path)#\(page)@\(width)") {
            let loaded = await Thumbnails.load(path, page: page, width: width)
            withAnimation(.easeOut(duration: 0.15)) { image = loaded }
        }
    }
}

struct BusyPill<Trailing: View>: View {
    let label: String
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 10) {
            ProgressView().tint(.white)
            Text(label).font(.subheadline.weight(.semibold)).lineLimit(2)
            Spacer(minLength: 0)
            trailing.tint(.white)
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .frame(minHeight: 50)
        .background(Color(white: 0.15), in: .capsule)
    }
}

extension BusyPill where Trailing == EmptyView {
    init(label: String) {
        self.label = label
        trailing = EmptyView()
    }
}

struct LowInkBanner: View {
    let inks: [Ink]

    var body: some View {
        if !inks.isEmpty {
            HStack(spacing: 10) {
                ForEach(inks, id: \.name) { RoundedRectangle(cornerRadius: 6).fill(inkColor($0.name)).frame(width: 12, height: 12) }
                Text(inks.map { "\($0.name.capitalized) (\($0.level)%)" }.joined(separator: " and ") + (inks.count == 1 ? " ink is low" : " inks are low"))
                    .font(.subheadline.weight(.semibold))
                Spacer(minLength: 0)
            }
            .padding(14)
            .background(Color.red.opacity(0.14), in: .rect(cornerRadius: 16))
        }
    }
}

struct InkTank: View {
    let name: String
    let level: Int32
    @State private var shown: Double = 0

    var body: some View {
        VStack(spacing: 6) {
            ZStack(alignment: .bottom) {
                RoundedRectangle(cornerRadius: 12).fill(Color(.tertiarySystemFill))
                RoundedRectangle(cornerRadius: 12).fill(inkColor(name)).frame(height: 96 * shown)
            }
            .frame(width: 40, height: 96)
            Text("\(max(0, level))%").font(.subheadline.weight(.semibold))
            Text(name.capitalized).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
        }
        .onAppear { withAnimation(.spring(duration: 1.2)) { shown = Double(min(max(level, 0), 100)) / 100 } }
        .onChange(of: level) { withAnimation(.spring(duration: 1.2)) { shown = Double(min(max(level, 0), 100)) / 100 } }
        .accessibilityElement(children: .combine)
    }
}

enum Glyph { case one, twoInOne, fourInOne, idCard, long, poster, empty }

struct SheetGlyph: View {
    @Environment(AppModel.self) private var model
    let kind: Glyph
    var active = false
    var width: CGFloat = 40

    var body: some View {
        Canvas { ctx, size in
            let rect = CGRect(origin: .zero, size: size)
            let shape = Path(roundedRect: rect.insetBy(dx: 0.75, dy: 0.75), cornerRadius: 3)
            if kind == .empty {
                ctx.fill(shape, with: .color(Color(.systemBackground)))
                ctx.stroke(shape, with: .color(Color(.separator)), style: StrokeStyle(lineWidth: 1.5, dash: [5, 4]))
                return
            }
            ctx.fill(shape, with: .color(.white))
            ctx.stroke(shape, with: .color(Color(.separator)), lineWidth: 1)
            let ink = active ? model.accent.onPaper : Color.gray.opacity(0.6)
            let pad = size.width * 0.14
            let w = size.width - pad * 2
            let h = size.height - pad * 2
            func lines(_ color: Color, _ at: CGPoint, _ area: CGSize, _ count: Int) {
                let gap = area.height / (CGFloat(count) * 1.6)
                let bar = gap * 0.75
                for i in 0..<count {
                    let lw = i == count - 1 ? area.width * 0.6 : area.width
                    ctx.fill(Path(roundedRect: CGRect(x: at.x, y: at.y + CGFloat(i) * (bar + gap), width: lw, height: bar), cornerRadius: bar / 2), with: .color(color))
                }
            }
            switch kind {
            case .one:
                lines(ink, CGPoint(x: pad, y: pad), CGSize(width: w, height: h), 6)
            case .twoInOne:
                let half = (w - pad * 0.6) / 2
                lines(ink, CGPoint(x: pad, y: pad), CGSize(width: half, height: h * 0.62), 4)
                lines(ink, CGPoint(x: pad + half + pad * 0.6, y: pad), CGSize(width: half, height: h * 0.62), 4)
            case .fourInOne:
                let half = (w - pad * 0.6) / 2
                let tall = (h - pad * 0.6) / 2
                for row in 0..<2 {
                    for col in 0..<2 {
                        lines(ink, CGPoint(x: pad + CGFloat(col) * (half + pad * 0.6), y: pad + CGFloat(row) * (tall + pad * 0.6)), CGSize(width: half, height: tall * 0.8), 3)
                    }
                }
            case .poster:
                lines(ink.opacity(0.35), CGPoint(x: pad, y: pad), CGSize(width: w, height: h), 6)
                for i in 1...2 {
                    var p = Path()
                    p.move(to: CGPoint(x: pad + CGFloat(i) * w / 3, y: pad))
                    p.addLine(to: CGPoint(x: pad + CGFloat(i) * w / 3, y: pad + h))
                    p.move(to: CGPoint(x: pad, y: pad + CGFloat(i) * h / 3))
                    p.addLine(to: CGPoint(x: pad + w, y: pad + CGFloat(i) * h / 3))
                    ctx.stroke(p, with: .color(ink), lineWidth: 1.5)
                }
            case .idCard:
                let cw = w * 0.72
                let ch = cw * 0.63
                ctx.fill(Path(roundedRect: CGRect(x: pad + (w - cw) / 2, y: pad, width: cw, height: ch), cornerRadius: 2), with: .color(ink))
                ctx.fill(Path(roundedRect: CGRect(x: pad + (w - cw) / 2, y: pad + ch + pad * 0.7, width: cw, height: ch), cornerRadius: 2), with: .color(ink.opacity(0.6)))
            case .long:
                lines(ink, CGPoint(x: pad + w * 0.18, y: pad), CGSize(width: w * 0.64, height: h), 9)
            case .empty:
                break
            }
        }
        .frame(width: width, height: width * 1.36)
        .accessibilityHidden(true)
    }
}

struct NameAlert: ViewModifier {
    let title: String
    @Binding var shown: Bool
    let initial: String
    let done: (String) -> Void
    @State private var text = ""

    func body(content: Content) -> some View {
        content.alert(title, isPresented: $shown) {
            TextField("Name", text: $text)
            Button("Cancel", role: .cancel) {}
            Button("Save") {
                let t = text.trimmingCharacters(in: .whitespaces)
                if !t.isEmpty { done(t) }
            }
        }
        .onChange(of: shown) { if shown { text = initial } }
    }
}

extension View {
    func nameAlert(_ title: String, shown: Binding<Bool>, initial: String, done: @escaping (String) -> Void) -> some View {
        modifier(NameAlert(title: title, shown: shown, initial: initial, done: done))
    }
}

struct Toast: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.subheadline.weight(.medium))
            .multilineTextAlignment(.leading)
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            .glassEffect(.regular, in: .rect(cornerRadius: 22))
            .padding(.horizontal, 16)
    }
}

struct PaperSection: View {
    let caps: PrintCaps?
    @Binding var media: String
    @Binding var quality: Quality
    @Binding var mediaType: String?

    var body: some View {
        let papers = (caps?.media ?? ["iso_a4_210x297mm", "na_letter_8.5x11in"]).filter { !$0.hasPrefix("custom_") }
        let qualities = caps?.qualities.isEmpty == false ? caps!.qualities : [.draft, .normal, .high]
        let types = caps?.mediaTypes ?? []
        Picker("Paper size", selection: $media) {
            ForEach(papers, id: \.self) { Text(paperName($0)).tag($0) }
        }
        if !types.isEmpty {
            Picker("Paper type", selection: $mediaType) {
                Text("Printer default").tag(String?.none)
                ForEach(types, id: \.self) { Text(paperTypeName($0)).tag(Optional($0)) }
            }
        }
        Picker("Quality", selection: $quality) {
            ForEach(qualities, id: \.self) { Text(qualityLabel($0)).tag($0) }
        }
        .pickerStyle(.segmented)
    }
}

struct ScaleSection: View {
    @Binding var mode: ScaleMode
    @Binding var percent: Int

    var body: some View {
        Picker("Size", selection: $mode) {
            ForEach(ScaleMode.allCases) { Text($0.rawValue).tag($0) }
        }
        if mode == .percent {
            Stepper("Scale: \(percent)%", value: $percent, in: 25...400, step: 5)
        }
    }
}
