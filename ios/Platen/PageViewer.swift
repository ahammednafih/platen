import SwiftUI
import UIKit

struct PageSource: Hashable {
    let path: String
    let page: Int
}

struct PageViewer: View {
    @Environment(\.dismiss) private var dismiss
    let sources: [PageSource]
    let start: Int
    var title: (Int) -> String = { "Page \($0 + 1)" }
    @State private var current = 0

    var body: some View {
        NavigationStack {
            TabView(selection: $current) {
                ForEach(Array(sources.enumerated()), id: \.offset) { i, s in
                    ZoomPage(source: s).tag(i)
                }
            }
            .tabViewStyle(.page(indexDisplayMode: .never))
            .background(Color.black)
            .ignoresSafeArea(edges: .bottom)
            .navigationTitle(sources.count > 1 ? "\(title(current)) of \(sources.count)" : title(current))
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close", systemImage: "xmark") { dismiss() } } }
        }
        .onAppear { current = start }
    }
}

private struct ZoomPage: View {
    let source: PageSource
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image { ZoomableImage(image: image) } else { ProgressView().tint(.white) }
        }
        .task(id: source) { image = await Thumbnails.load(source.path, page: source.page, width: 2000) }
    }
}

struct ZoomableImage: UIViewRepresentable {
    let image: UIImage

    func makeUIView(context: Context) -> UIScrollView {
        let scroll = UIScrollView()
        scroll.delegate = context.coordinator
        scroll.minimumZoomScale = 1
        scroll.maximumZoomScale = 6
        scroll.showsHorizontalScrollIndicator = false
        scroll.showsVerticalScrollIndicator = false
        scroll.backgroundColor = .clear
        let view = UIImageView(image: image)
        view.contentMode = .scaleAspectFit
        view.frame = scroll.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scroll.addSubview(view)
        context.coordinator.view = view
        let tap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.doubleTap(_:)))
        tap.numberOfTapsRequired = 2
        scroll.addGestureRecognizer(tap)
        return scroll
    }

    func updateUIView(_ scroll: UIScrollView, context: Context) {
        context.coordinator.view?.image = image
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    final class Coordinator: NSObject, UIScrollViewDelegate {
        weak var view: UIImageView?

        func viewForZooming(in scrollView: UIScrollView) -> UIView? { view }

        @objc func doubleTap(_ g: UITapGestureRecognizer) {
            guard let scroll = g.view as? UIScrollView else { return }
            if scroll.zoomScale > 1 {
                scroll.setZoomScale(1, animated: true)
            } else {
                let p = g.location(in: view)
                scroll.zoom(to: CGRect(x: p.x - 60, y: p.y - 80, width: 120, height: 160), animated: true)
            }
        }
    }
}

struct SheetsViewer: View {
    @Environment(\.dismiss) private var dismiss
    let count: Int
    let start: Int
    let render: (Int) async -> UIImage?
    @State private var current = 0

    var body: some View {
        NavigationStack {
            TabView(selection: $current) {
                ForEach(0..<count, id: \.self) { i in
                    SheetPage(index: i, render: render).tag(i)
                }
            }
            .tabViewStyle(.page(indexDisplayMode: .never))
            .background(Color.black)
            .ignoresSafeArea(edges: .bottom)
            .navigationTitle("Sheet \(current + 1) of \(count)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbarBackground(.visible, for: .navigationBar)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close", systemImage: "xmark") { dismiss() } } }
        }
        .onAppear { current = start }
    }
}

private struct SheetPage: View {
    let index: Int
    let render: (Int) async -> UIImage?
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            if let image { ZoomableImage(image: image) } else { ProgressView().tint(.white) }
        }
        .task { image = await render(index) }
    }
}

struct PdfCompare: View {
    @Environment(\.dismiss) private var dismiss
    let before: URL
    let after: URL
    @State private var count = 0
    @State private var current = 0

    var body: some View {
        NavigationStack {
            TabView(selection: $current) {
                ForEach(0..<count, id: \.self) { i in
                    VStack(spacing: 8) {
                        half("Before", before, i)
                        half("After", after, i)
                    }
                    .padding(12)
                    .tag(i)
                }
            }
            .tabViewStyle(.page(indexDisplayMode: .never))
            .navigationTitle(count > 0 ? "Page \(current + 1) of \(count)" : "Compare")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Close", systemImage: "xmark") { dismiss() } } }
        }
        .task { count = min(Pdf.pageCount(before), Pdf.pageCount(after)) }
    }

    private func half(_ label: String, _ url: URL, _ i: Int) -> some View {
        VStack(spacing: 4) {
            Text(label).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            ZoomPage(source: PageSource(path: url.path, page: i))
                .background(Color.black.opacity(0.85), in: .rect(cornerRadius: 12))
        }
    }
}
