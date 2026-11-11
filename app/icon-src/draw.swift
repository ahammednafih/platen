// swift draw.swift HEX OUT.png — the app icon on a 1024 canvas (Apple's grid: 824 body, 100 inset).
import AppKit

func color(_ hex: String, _ a: CGFloat = 1) -> NSColor {
  let v = Int(hex.dropFirst(), radix: 16)!
  return NSColor(srgbRed: CGFloat((v >> 16) & 255) / 255, green: CGFloat((v >> 8) & 255) / 255, blue: CGFloat(v & 255) / 255, alpha: a)
}

let args = CommandLine.arguments
let accent = color(args[1])
let size = 1024
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8, samplesPerPixel: 4,
                           hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

// Body: accent squircle, lit from the top.
let body = NSBezierPath(roundedRect: NSRect(x: 100, y: 100, width: 824, height: 824), xRadius: 185, yRadius: 185)
NSGraphicsContext.saveGraphicsState()
let drop = NSShadow(); drop.shadowColor = NSColor(white: 0, alpha: 0.28); drop.shadowOffset = NSSize(width: 0, height: -10); drop.shadowBlurRadius = 24
drop.set()
accent.setFill(); body.fill()
NSGraphicsContext.restoreGraphicsState()
NSGradient(starting: NSColor(white: 1, alpha: 0.16), ending: NSColor(white: 1, alpha: 0))!.draw(in: body, angle: -90)

// Paper: a white sheet with a few lines of text.
let paper = NSRect(x: 292, y: 210, width: 440, height: 580)
NSGraphicsContext.saveGraphicsState()
let lift = NSShadow(); lift.shadowColor = NSColor(white: 0, alpha: 0.32); lift.shadowOffset = NSSize(width: 0, height: -8); lift.shadowBlurRadius = 22
lift.set()
NSColor.white.setFill(); NSBezierPath(roundedRect: paper, xRadius: 16, yRadius: 16).fill()
NSGraphicsContext.restoreGraphicsState()
let ink = color(args[1], 0.22)
ink.setFill()
NSBezierPath(roundedRect: NSRect(x: 340, y: 700, width: 210, height: 26), xRadius: 13, yRadius: 13).fill()
for (i, w) in [344.0, 300, 330, 250, 320, 190].enumerated() {
  NSBezierPath(roundedRect: NSRect(x: 340, y: 630 - Double(i) * 62, width: w, height: 16), xRadius: 8, yRadius: 8).fill()
}

// Scan line: a bright bar across the glass, wider than the sheet, with a soft glow.
let beam = NSRect(x: 196, y: 476, width: 632, height: 22)
NSGraphicsContext.saveGraphicsState()
let glow = NSShadow(); glow.shadowColor = NSColor(white: 1, alpha: 0.95); glow.shadowBlurRadius = 40; glow.shadowOffset = .zero
glow.set()
NSColor.white.setFill(); NSBezierPath(roundedRect: beam, xRadius: 11, yRadius: 11).fill()
NSGraphicsContext.restoreGraphicsState()
color(args[1], 0.18).setFill(); NSBezierPath(roundedRect: beam.insetBy(dx: 0, dy: 7), xRadius: 4, yRadius: 4).fill()

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[2]))
