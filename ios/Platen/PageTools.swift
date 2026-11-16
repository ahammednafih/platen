import CoreImage
import SwiftUI

struct PageTools: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let page: Page
    let index: Int
    let count: Int
    let onView: () -> Void
    @State private var cropping = false
    @State private var levels = false

    var body: some View {
        NavigationStack {
            List {
                Section("Page") {
                    tool("View", "eye", close: false, onView)
                    tool("Retake", "arrow.clockwise") { model.scan(replace: index) }
                    tool("Move earlier", "arrow.left", enabled: index > 0) { model.movePage(index, index - 1) }
                    tool("Move later", "arrow.right", enabled: index < count - 1) { model.movePage(index, index + 1) }
                    tool("Delete", "trash", role: .destructive) { model.deletePage(index) }
                }
                Section("Turn and trim") {
                    tool("Rotate left", "rotate.left") { model.edit(index, .rotateLeft) }
                    tool("Rotate right", "rotate.right") { model.edit(index, .rotateRight) }
                    tool("Rotate 180°", "arrow.triangle.2.circlepath") { model.edit(index, .rotate180) }
                    tool("Straighten", "level") { model.edit(index, .straighten) }
                    tool("Trim to paper", "crop") { model.edit(index, .crop) }
                    tool("Crop…", "crop.rotate", close: false) { cropping = true }
                }
                Section("Clean up") {
                    tool("Brightness and contrast…", "sun.max", close: false) { levels = true }
                    tool("Remove specks", "sparkles") { model.edit(index, .despeckle(dpi: page.dpi)) }
                    tool("Fill torn edges", "square.dashed") { model.fill(index) }
                }
                Section("Colour") {
                    tool("Gray", "circle.lefthalf.filled") { model.edit(index, .gray) }
                    tool("Black and white", "circle.righthalf.filled") { model.edit(index, .blackWhite) }
                    if page.color != nil { tool("Back to colour", "paintpalette") { model.edit(index, .colour) } }
                }
            }
            .navigationTitle("Page \(index + 1)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", systemImage: "checkmark") { dismiss() } } }
            .navigationDestination(isPresented: $cropping) { CropView(page: page, index: index) { dismiss() } }
            .navigationDestination(isPresented: $levels) { LevelsView(page: page, index: index) { dismiss() } }
        }
        .presentationDetents([.medium, .large])
    }

    private func tool(_ title: String, _ icon: String, enabled: Bool = true, role: ButtonRole? = nil, close: Bool = true, _ action: @escaping () -> Void) -> some View {
        Button(role: role) {
            action()
            if close { dismiss() }
        } label: {
            Label(title, systemImage: icon)
        }
        .disabled(!enabled)
    }
}

private let cropSizes: [(String, CGFloat, CGFloat)] = [
    ("A4", 210, 297), ("A5", 148, 210), ("A6", 105, 148), ("B5", 176, 250), ("Letter", 216, 279), ("Photo 10×15", 102, 152), ("Card", 86, 54),
]

struct CropRect: Equatable {
    var l: CGFloat = 0, t: CGFloat = 0, r: CGFloat = 1, b: CGFloat = 1
    var whole: Bool { l == 0 && t == 0 && r == 1 && b == 1 }
}

private struct CropView: View {
    @Environment(AppModel.self) private var model
    let page: Page
    let index: Int
    let done: () -> Void
    @State private var rect = CropRect()
    @State private var picked = "Auto"
    @State private var note: String?
    @State private var image: UIImage?

    var body: some View {
        let pageW = CGFloat(page.w) * 25.4 / CGFloat(page.dpi)
        let pageH = CGFloat(page.h) * 25.4 / CGFloat(page.dpi)
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        chip("Whole page") { rect = CropRect() }
                        chip("Auto") { auto() }
                        ForEach(cropSizes.filter { $0.1 <= pageW + 2 && $0.2 <= pageH + 2 }, id: \.0) { s in
                            chip(s.0) { rect = CropRect(l: 0, t: 0, r: min(s.1 / pageW, 1), b: min(s.2 / pageH, 1)) }
                        }
                    }
                }
                if let note { Text(note).font(.footnote).foregroundStyle(.secondary) }
                ZStack {
                    if let image { Image(uiImage: image).resizable() }
                    CropOverlay(rect: $rect) { picked = "Custom" }
                }
                .aspectRatio(CGFloat(page.w) / CGFloat(max(page.h, 1)), contentMode: .fit)
            }
            .padding(16)
        }
        .navigationTitle("Crop")
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Crop") {
                    model.edit(index, .cropTo(x: Float(rect.l), y: Float(rect.t), w: Float(rect.r - rect.l), h: Float(rect.b - rect.t)))
                    done()
                }
                .disabled(rect.whole)
            }
        }
        .task {
            image = await Thumbnails.load(page.path, width: 1200)
            auto()
        }
    }

    private func chip(_ name: String, _ action: @escaping () -> Void) -> some View {
        Button(name) {
            picked = name
            note = nil
            action()
        }
        .buttonStyle(.bordered)
        .buttonBorderShape(.capsule)
        .tint(picked == name ? .accentColor : .secondary)
    }

    private func auto() {
        picked = "Auto"
        note = nil
        let platen = model.platen
        let i = UInt32(index)
        model.work(nil, { try platen.paperBounds(index: i) }) { b in
            if let b {
                rect = CropRect(l: CGFloat(b.x), t: CGFloat(b.y), r: CGFloat(b.x + b.w), b: CGFloat(b.y + b.h))
            } else {
                note = "No dark border around the paper to find. Pick a size or drag the corners."
            }
        }
    }
}

struct CropOverlay: View {
    @Binding var rect: CropRect
    var onDrag: () -> Void = {}
    @State private var grabbed = -1
    @State private var start = CropRect()

    var body: some View {
        GeometryReader { geo in
            let s = geo.size
            let box = CGRect(x: rect.l * s.width, y: rect.t * s.height, width: (rect.r - rect.l) * s.width, height: (rect.b - rect.t) * s.height)
            ZStack(alignment: .topLeading) {
                Path { p in
                    p.addRect(CGRect(origin: .zero, size: s))
                    p.addRect(box)
                }
                .fill(Color.black.opacity(0.45), style: FillStyle(eoFill: true))
                Rectangle().stroke(Color.accentColor, lineWidth: 3).frame(width: box.width, height: box.height).offset(x: box.minX, y: box.minY)
                ForEach(Array(corners(box).enumerated()), id: \.offset) { _, c in
                    Circle().fill(Color.accentColor).frame(width: 18, height: 18).position(c)
                }
            }
            .contentShape(.rect)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { g in
                        if grabbed < 0 {
                            start = rect
                            let near = corners(box).firstIndex { hypot($0.x - g.startLocation.x, $0.y - g.startLocation.y) < 44 }
                            grabbed = near ?? 4
                        }
                        drag(dx: g.translation.width / s.width, dy: g.translation.height / s.height)
                        onDrag()
                    }
                    .onEnded { _ in grabbed = -1 }
            )
        }
    }

    private func corners(_ b: CGRect) -> [CGPoint] {
        [CGPoint(x: b.minX, y: b.minY), CGPoint(x: b.maxX, y: b.minY), CGPoint(x: b.minX, y: b.maxY), CGPoint(x: b.maxX, y: b.maxY)]
    }

    private func drag(dx: CGFloat, dy: CGFloat) {
        var r = start
        switch grabbed {
        case 0: r.l += dx; r.t += dy
        case 1: r.r += dx; r.t += dy
        case 2: r.l += dx; r.b += dy
        case 3: r.r += dx; r.b += dy
        default:
            let mx = min(max(dx, -r.l), 1 - r.r)
            let my = min(max(dy, -r.t), 1 - r.b)
            r.l += mx; r.r += mx; r.t += my; r.b += my
        }
        r.l = min(max(r.l, 0), r.r - 0.05)
        r.t = min(max(r.t, 0), r.b - 0.05)
        r.r = min(max(r.r, r.l + 0.05), 1)
        r.b = min(max(r.b, r.t + 0.05), 1)
        rect = r
    }
}

private struct LevelsView: View {
    @Environment(AppModel.self) private var model
    let page: Page
    let index: Int
    let done: () -> Void
    @State private var source: CIImage?
    @State private var preview: UIImage?
    @State private var brightness: Double = 1
    @State private var contrast: Double = 1
    private static let context = CIContext()

    var body: some View {
        Form {
            Section {
                if let preview {
                    Image(uiImage: preview).resizable().aspectRatio(contentMode: .fit).frame(maxWidth: .infinity)
                }
            }
            Section {
                VStack(alignment: .leading) {
                    Text("Brightness \(Int((brightness * 100).rounded()))%")
                    Slider(value: $brightness, in: 0.5...1.5)
                }
                VStack(alignment: .leading) {
                    Text("Contrast \(Int((contrast * 100).rounded()))%")
                    Slider(value: $contrast, in: 0.5...2)
                }
                Button("Reset") {
                    brightness = 1
                    contrast = 1
                }
            }
        }
        .navigationTitle("Brightness and contrast")
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Apply") {
                    model.edit(index, .levels(brightness: Float(brightness), contrast: Float(contrast)))
                    done()
                }
                .disabled(brightness == 1 && contrast == 1)
            }
        }
        .task {
            if let img = await Thumbnails.load(page.path, width: 1000), let cg = img.cgImage { source = CIImage(cgImage: cg) }
            render()
        }
        .onChange(of: brightness) { render() }
        .onChange(of: contrast) { render() }
    }

    private func render() {
        guard let source else { return }
        let k = CGFloat(brightness * contrast)
        let offset = CGFloat(0.5 - 0.5 * contrast)
        let f = CIFilter(name: "CIColorMatrix")!
        f.setValue(source, forKey: kCIInputImageKey)
        f.setValue(CIVector(x: k, y: 0, z: 0, w: 0), forKey: "inputRVector")
        f.setValue(CIVector(x: 0, y: k, z: 0, w: 0), forKey: "inputGVector")
        f.setValue(CIVector(x: 0, y: 0, z: k, w: 0), forKey: "inputBVector")
        f.setValue(CIVector(x: offset, y: offset, z: offset, w: 0), forKey: "inputBiasVector")
        guard let out = f.outputImage?.cropped(to: source.extent), let cg = Self.context.createCGImage(out, from: source.extent) else { return }
        preview = UIImage(cgImage: cg)
    }
}

private let photoAspects: [(String, CGFloat?)] = [("Free", nil), ("4×6", 2.0 / 3.0), ("5×7", 5.0 / 7.0), ("Square", 1)]

struct ImageCropView: View {
    @Environment(\.dismiss) private var dismiss
    let file: URL
    let done: (URL) -> Void
    @State private var image: UIImage?
    @State private var rect = CropRect()
    @State private var aspect = "Free"
    @State private var saving = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 12) {
                    HStack(spacing: 6) {
                        ForEach(photoAspects, id: \.0) { a in
                            Button(a.0) {
                                aspect = a.0
                                guard let image else { return }
                                rect = a.1.map { centred(image.size.width, image.size.height, $0) } ?? CropRect()
                            }
                            .buttonStyle(.bordered)
                            .buttonBorderShape(.capsule)
                            .tint(aspect == a.0 ? .accentColor : .secondary)
                        }
                    }
                    if let image {
                        ZStack {
                            Image(uiImage: image).resizable()
                            CropOverlay(rect: $rect) { aspect = "Free" }
                        }
                        .aspectRatio(image.size.width / max(image.size.height, 1), contentMode: .fit)
                    }
                }
                .padding(16)
            }
            .navigationTitle("Crop before printing")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel", systemImage: "xmark") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(saving ? "Cropping…" : "Crop") {
                        saving = true
                        let r = rect
                        Task {
                            let out = await Task.detached { crop(file, r) }.value
                            saving = false
                            if let out {
                                done(out)
                                dismiss()
                            }
                        }
                    }
                    .disabled(saving || image == nil)
                }
            }
            .task { image = await Thumbnails.load(file.path, width: 1400) }
        }
    }
}

private func centred(_ w: CGFloat, _ h: CGFloat, _ ratio: CGFloat) -> CropRect {
    let target = w < h ? ratio : 1 / ratio
    if w / h > target {
        let cw = h * target / w
        return CropRect(l: (1 - cw) / 2, t: 0, r: (1 + cw) / 2, b: 1)
    }
    let ch = w / target / h
    return CropRect(l: 0, t: (1 - ch) / 2, r: 1, b: (1 + ch) / 2)
}

private func crop(_ file: URL, _ r: CropRect) -> URL? {
    guard let image = UIImage(contentsOfFile: file.path) else { return nil }
    let upright = UIGraphicsImageRenderer(size: image.size, format: { let f = UIGraphicsImageRendererFormat(); f.scale = 1; return f }()).image { _ in
        image.draw(at: .zero)
    }
    guard let cg = upright.cgImage else { return nil }
    let w = CGFloat(cg.width), h = CGFloat(cg.height)
    let area = CGRect(x: r.l * w, y: r.t * h, width: (r.r - r.l) * w, height: (r.b - r.t) * h).integral
    guard let cut = cg.cropping(to: area), let data = UIImage(cgImage: cut).jpegData(compressionQuality: 0.95) else { return nil }
    let out = file.deletingLastPathComponent().appendingPathComponent(file.deletingPathExtension().lastPathComponent + "_cropped.jpg")
    do {
        try data.write(to: out)
        return out
    } catch {
        return nil
    }
}
