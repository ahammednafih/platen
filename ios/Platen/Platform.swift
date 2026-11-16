import Foundation
import ImageIO
import Security
import UIKit
import UniformTypeIdentifiers

final class PdfPages: PdfRenderer {
    func render(path: String, index: UInt32, dpi: UInt32) throws -> RenderedPage {
        guard let image = Pdf.render(URL(fileURLWithPath: path), page: Int(index), scale: CGFloat(dpi) / 72) else {
            throw PlatenError.Failed(message: "Can't read page \(index + 1) of \(URL(fileURLWithPath: path).lastPathComponent)")
        }
        let (w, h) = (image.width, image.height)
        var rgba = [UInt8](repeating: 0, count: w * h * 4)
        rgba.withUnsafeMutableBytes { buf in
            let ctx = CGContext(data: buf.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
            ctx?.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        }
        var rgb = Data(count: w * h * 3)
        rgb.withUnsafeMutableBytes { out in
            let o = out.bindMemory(to: UInt8.self)
            for i in 0..<(w * h) {
                o[i * 3] = rgba[i * 4]
                o[i * 3 + 1] = rgba[i * 4 + 1]
                o[i * 3 + 2] = rgba[i * 4 + 2]
            }
        }
        return RenderedPage(width: UInt32(w), height: UInt32(h), rgb: rgb)
    }
}

enum Pdf {
    static func pageCount(_ url: URL) -> Int {
        CGPDFDocument(url as CFURL)?.numberOfPages ?? 0
    }

    static func render(_ url: URL, page index: Int, scale: CGFloat) -> CGImage? {
        guard let doc = CGPDFDocument(url as CFURL), let page = doc.page(at: index + 1) else { return nil }
        let box = page.getBoxRect(.cropBox)
        let turned = page.rotationAngle % 180 != 0
        let size = turned ? CGSize(width: box.height, height: box.width) : box.size
        let w = max(1, Int((size.width * scale).rounded()))
        let h = max(1, Int((size.height * scale).rounded()))
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return nil }
        ctx.setFillColor(UIColor.white.cgColor)
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        ctx.scaleBy(x: CGFloat(w) / size.width, y: CGFloat(h) / size.height)
        ctx.concatenate(page.getDrawingTransform(.cropBox, rect: CGRect(origin: .zero, size: size), rotate: 0, preserveAspectRatio: true))
        ctx.drawPDFPage(page)
        return ctx.makeImage()
    }

    static func pageSize(_ url: URL, page index: Int) -> CGSize? {
        guard let page = CGPDFDocument(url as CFURL)?.page(at: index + 1) else { return nil }
        let box = page.getBoxRect(.cropBox)
        return page.rotationAngle % 180 != 0 ? CGSize(width: box.height, height: box.width) : box.size
    }
}

enum Thumbnails {
    private static let cache = NSCache<NSString, UIImage>()

    static func load(_ path: String, page: Int = 0, width: Int) async -> UIImage? {
        let key = "\(path)#\(page)@\(width)" as NSString
        if let hit = cache.object(forKey: key) { return hit }
        let image = await Task.detached(priority: .userInitiated) { make(path, page: page, width: width) }.value
        if let image { cache.setObject(image, forKey: key) }
        return image
    }

    static func make(_ path: String, page: Int, width: Int) -> UIImage? {
        let url = URL(fileURLWithPath: path)
        if url.pathExtension.lowercased() == "pdf" {
            guard let size = Pdf.pageSize(url, page: page), size.width > 0 else { return nil }
            return Pdf.render(url, page: page, scale: CGFloat(width) / size.width).map { UIImage(cgImage: $0) }
        }
        guard let src = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        let opts: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: width * 3 / 2,
        ]
        return CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary).map { UIImage(cgImage: $0) }
    }
}

enum Secrets {
    private static let service = "platen-device-web"

    static func password(_ host: String) -> String? {
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword, kSecAttrService: service, kSecAttrAccount: host,
            kSecReturnData: true, kSecMatchLimit: kSecMatchLimitOne,
        ]
        var out: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func save(_ host: String, _ password: String) {
        forget(host)
        let item: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword, kSecAttrService: service, kSecAttrAccount: host,
            kSecValueData: Data(password.utf8), kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        ]
        SecItemAdd(item as CFDictionary, nil)
    }

    static func forget(_ host: String) {
        let query: [CFString: Any] = [kSecClass: kSecClassGenericPassword, kSecAttrService: service, kSecAttrAccount: host]
        SecItemDelete(query as CFDictionary)
    }
}

struct SavedFile: Identifiable, Hashable {
    let url: URL
    let folder: String
    let size: Int64
    let modified: Date
    var id: URL { url }
    var name: String { url.lastPathComponent }
    var isPdf: Bool { url.pathExtension.lowercased() == "pdf" }
    var printable: Bool { ["pdf", "jpg", "jpeg", "png"].contains(url.pathExtension.lowercased()) }
}

enum Storage {
    static let label = "Files › On My iPhone › Platen"
    static let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    static let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
    static let caches = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
    static let config = support.appendingPathComponent("config")
    static let scans = support.appendingPathComponent("scans")
    static let cache = caches.appendingPathComponent("Platen")

    static func prepare() {
        for d in [config, scans, cache] {
            try? FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        }
    }

    @discardableResult
    static func save(_ file: URL, into folder: String?) throws -> URL {
        let dir = folder.map { documents.appendingPathComponent($0) } ?? documents
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let out = free(dir.appendingPathComponent(file.lastPathComponent))
        try FileManager.default.copyItem(at: file, to: out)
        return out
    }

    static func list() -> [SavedFile] {
        let keys: [URLResourceKey] = [.fileSizeKey, .contentModificationDateKey, .isRegularFileKey]
        guard let walk = FileManager.default.enumerator(at: documents, includingPropertiesForKeys: keys, options: [.skipsHiddenFiles]) else { return [] }
        var out: [SavedFile] = []
        for case let url as URL in walk {
            if url.lastPathComponent == "Inbox" {
                walk.skipDescendants()
                continue
            }
            guard let v = try? url.resourceValues(forKeys: Set(keys)), v.isRegularFile == true else { continue }
            let parent = url.deletingLastPathComponent().standardizedFileURL.path
            let folder = parent == documents.standardizedFileURL.path ? "" : url.deletingLastPathComponent().lastPathComponent
            out.append(SavedFile(url: url, folder: folder, size: Int64(v.fileSize ?? 0), modified: v.contentModificationDate ?? .distantPast))
        }
        return out.sorted { $0.modified > $1.modified }
    }

    static func delete(_ f: SavedFile) throws {
        try FileManager.default.removeItem(at: f.url)
        let dir = f.url.deletingLastPathComponent()
        if dir.standardizedFileURL != documents.standardizedFileURL,
           (try? FileManager.default.contentsOfDirectory(atPath: dir.path))?.isEmpty == true {
            try? FileManager.default.removeItem(at: dir)
        }
    }

    /// A copy in the cache that the core can read, for files picked in or shared from other apps.
    static func incoming(_ url: URL) throws -> URL {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let dir = cache.appendingPathComponent("incoming").appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let out = dir.appendingPathComponent(url.lastPathComponent)
        try FileManager.default.copyItem(at: url, to: out)
        if url.standardizedFileURL.path.hasPrefix(documents.appendingPathComponent("Inbox").standardizedFileURL.path) {
            try? FileManager.default.removeItem(at: url)
        }
        return try normalised(out)
    }

    /// Writes picked photo data as JPEG, since the core prints JPEG and PNG only.
    static func incomingImage(_ data: Data, name: String) throws -> URL {
        let dir = cache.appendingPathComponent("incoming").appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        guard let image = UIImage(data: data), let jpeg = image.jpegData(compressionQuality: 0.95) else {
            throw PlatenError.Failed(message: "Can't read that photo")
        }
        let out = dir.appendingPathComponent(name)
        try jpeg.write(to: out)
        return out
    }

    private static func normalised(_ url: URL) throws -> URL {
        let ext = url.pathExtension.lowercased()
        if ["pdf", "jpg", "jpeg", "png"].contains(ext) { return url }
        guard let data = try? Data(contentsOf: url), UIImage(data: data) != nil else { return url }
        return try incomingImage(data, name: url.deletingPathExtension().lastPathComponent + ".jpg")
    }

    private static func free(_ url: URL) -> URL {
        guard FileManager.default.fileExists(atPath: url.path) else { return url }
        let stem = url.deletingPathExtension().lastPathComponent
        let ext = url.pathExtension
        var n = 2
        while true {
            let next = url.deletingLastPathComponent().appendingPathComponent("\(stem) \(n)").appendingPathExtension(ext)
            if !FileManager.default.fileExists(atPath: next.path) { return next }
            n += 1
        }
    }
}
