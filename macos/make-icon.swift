// Renders KBViewer.iconset. Run by build-app.sh; iconutil turns the result into .icns.
//
// The icon is generated rather than committed as a binary blob so it stays editable
// in the same place as everything else about the app.

import AppKit
import Foundation

// The base edges an .iconset is made of, named because five bare numbers do not say
// where any of them is used. iconutil wants every one of them at 1x and at 2x.
let menuBarEdge = 16
let listViewEdge = 32
let finderIconEdge = 128
let quickLookEdge = 256
let dockEdge = 512

let iconSizes: [(name: String, pixels: Int)] =
    [menuBarEdge, listViewEdge, finderIconEdge, quickLookEdge, dockEdge].flatMap { edge in
        [("icon_\(edge)x\(edge)", edge), ("icon_\(edge)x\(edge)@2x", edge * 2)]
    }

// Proportions of the icon's edge. macOS app icons are a rounded square inset from the
// canvas rather than edge to edge, with the glyph smaller again inside that.
let glyphPointFraction: CGFloat = 0.44
let plateInsetFraction: CGFloat = 0.06
let plateCornerFraction: CGFloat = 0.225

/// White glyph on a transparent ground, built before the bitmap context is current:
/// lockFocus inside an already-current context does not nest reliably.
func whiteGlyph(pointSize: CGFloat) -> NSImage? {
    let configuration = NSImage.SymbolConfiguration(pointSize: pointSize, weight: .semibold)
    guard
        let symbol = NSImage(
            systemSymbolName: "books.vertical.fill", accessibilityDescription: nil)?
            .withSymbolConfiguration(configuration)
    else { return nil }

    let bounds = NSRect(origin: .zero, size: symbol.size)
    let tinted = NSImage(size: symbol.size)
    tinted.lockFocus()
    symbol.draw(at: .zero, from: bounds, operation: .sourceOver, fraction: 1)
    NSColor.white.set()
    bounds.fill(using: .sourceAtop)
    tinted.unlockFocus()
    return tinted
}

func renderIcon(pixels: Int) -> Data? {
    let size = CGFloat(pixels)
    let glyph = whiteGlyph(pointSize: size * glyphPointFraction)

    guard
        let representation = NSBitmapImageRep(
            bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)
    else { return nil }
    representation.size = NSSize(width: size, height: size)

    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: representation)

    let inset = size * plateInsetFraction
    let plate = NSRect(x: inset, y: inset, width: size - inset * 2, height: size - inset * 2)
    let corner = size * plateCornerFraction
    let path = NSBezierPath(roundedRect: plate, xRadius: corner, yRadius: corner)
    let gradient = NSGradient(
        starting: NSColor(srgbRed: 0.35, green: 0.42, blue: 0.85, alpha: 1),
        ending: NSColor(srgbRed: 0.16, green: 0.19, blue: 0.48, alpha: 1))
    gradient?.draw(in: path, angle: -90)

    if let glyph {
        let origin = NSPoint(x: (size - glyph.size.width) / 2, y: (size - glyph.size.height) / 2)
        glyph.draw(
            at: origin, from: NSRect(origin: .zero, size: glyph.size), operation: .sourceOver,
            fraction: 1)
    }

    NSGraphicsContext.restoreGraphicsState()
    return representation.representation(using: .png, properties: [:])
}

let arguments = CommandLine.arguments
guard arguments.count == 2 else {
    FileHandle.standardError.write(Data("usage: make-icon <output.iconset>\n".utf8))
    exit(2)
}

let outputDirectory = URL(fileURLWithPath: arguments[1])
try? FileManager.default.removeItem(at: outputDirectory)
try FileManager.default.createDirectory(at: outputDirectory, withIntermediateDirectories: true)

for icon in iconSizes {
    guard let png = renderIcon(pixels: icon.pixels) else {
        FileHandle.standardError.write(Data("could not render \(icon.name)\n".utf8))
        exit(1)
    }
    try png.write(to: outputDirectory.appendingPathComponent("\(icon.name).png"))
}
