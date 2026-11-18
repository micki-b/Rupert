// Re-saves PNGs without an alpha channel (the App Store rejects app icons that have one).
// Usage: swift scripts/icons/flatten-alpha.swift <png>...
import CoreGraphics; import Foundation; import ImageIO; import UniformTypeIdentifiers
for path in CommandLine.arguments.dropFirst() {
  let url = URL(fileURLWithPath: path)
  let img = CGImageSourceCreateImageAtIndex(CGImageSourceCreateWithURL(url as CFURL, nil)!, 0, nil)!
  let ctx = CGContext(data: nil, width: img.width, height: img.height, bitsPerComponent: 8, bytesPerRow: 0,
    space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
  ctx.draw(img, in: CGRect(x: 0, y: 0, width: img.width, height: img.height))
  let d = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
  CGImageDestinationAddImage(d, ctx.makeImage()!, nil); CGImageDestinationFinalize(d)
}
