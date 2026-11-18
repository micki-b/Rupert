// Turns the Rupert owl concept preview into production icon masters.
// Usage: swift scripts/icons/make-icons.swift <concept.png> <out-dir>
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let args = CommandLine.arguments
guard args.count == 3,
  let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: args[1]) as CFURL, nil),
  let concept = CGImageSourceCreateImageAtIndex(src, 0, nil)
else { fatalError("usage: make_icons <concept.png> <out-dir>") }
let outDir = URL(fileURLWithPath: args[2])

// MARK: - Pixels

let W = concept.width, H = concept.height
let rgb = CGColorSpace(name: CGColorSpace.sRGB)!
var pixels = [UInt8](repeating: 0, count: W * H * 4)
let readCtx = CGContext(
  data: &pixels, width: W, height: H, bitsPerComponent: 8, bytesPerRow: W * 4, space: rgb,
  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
readCtx.draw(concept, in: CGRect(x: 0, y: 0, width: W, height: H))
// Row 0 of `pixels` is the top of the image.
func px(_ x: Int, _ y: Int) -> (Int, Int, Int) {
  let i = (y * W + x) * 4
  return (Int(pixels[i]), Int(pixels[i + 1]), Int(pixels[i + 2]))
}
/// The tile is dark brown; the presentation background is light gray.
func isTile(_ x: Int, _ y: Int) -> Bool {
  let (r, g, b) = px(x, y)
  return (r + g + b) / 3 < 150 && r > b
}

// MARK: - Tile bounds and corner radius

func firstTile(_ range: [Int], _ hit: (Int) -> Bool) -> Int { range.first(where: hit)! }
let midX = W / 2, midY = H / 2
let left = firstTile(Array(0..<W)) { isTile($0, midY) }
let right = firstTile(Array((0..<W).reversed())) { isTile($0, midY) }
let top = firstTile(Array(0..<H)) { isTile(midX, $0) }
let bottom = firstTile(Array((0..<H).reversed())) { isTile(midX, $0) }
// Along the diagonal from the top-left corner, the edge sits at
// r·(1 − 1/√2) from each side for a circular corner.
var d = 0
while !isTile(left + d, top + d) { d += 1 }
let radius = Double(d) / (1 - 1 / 2.0.squareRoot())
let tile = CGRect(x: left, y: top, width: right - left + 1, height: bottom - top + 1)
print("tile \(tile) corner radius ≈ \(Int(radius)) px (\(String(format: "%.1f", radius / tile.width * 100))% of width)")

// The tile brown near the top edge, for filling corners.
var sum = (0, 0, 0)
let samples = 40
for i in 0..<samples {
  let (r, g, b) = px(Int(tile.midX) - 200 + i * 10, top + 12)
  sum = (sum.0 + r, sum.1 + g, sum.2 + b)
}
let brown = CGColor(
  srgbRed: CGFloat(sum.0 / samples) / 255, green: CGFloat(sum.1 / samples) / 255,
  blue: CGFloat(sum.2 / samples) / 255, alpha: 1)

// MARK: - Tile mask from the actual edge

/// The presentation background: light and nearly neutral. (The owl's
/// ivory is warmer, so it doesn't count.)
func isBackground(_ x: Int, _ y: Int) -> Bool {
  let (r, g, b) = px(x, y)
  return min(r, g, b) > 195 && max(r, g, b) - min(r, g, b) < 16
}
/// How far inside the measured edge to cut, past the light bevel.
let erode = 5.0
// For each row, where the background ends on the left and right; for each
// column, at the top and bottom (all within the tile's bounding box).
let x0 = Int(tile.minX), x1 = Int(tile.maxX) - 1, y0 = Int(tile.minY), y1 = Int(tile.maxY) - 1
var leftEdge = [Double](repeating: 0, count: H), rightEdge = [Double](repeating: 0, count: H)
var topEdge = [Double](repeating: 0, count: W), bottomEdge = [Double](repeating: 0, count: W)
for y in y0...y1 {
  var l = x0; while l < x1 && isBackground(l, y) { l += 1 }
  var r = x1; while r > x0 && isBackground(r, y) { r -= 1 }
  leftEdge[y] = Double(l); rightEdge[y] = Double(r)
}
for x in x0...x1 {
  var t = y0; while t < y1 && isBackground(x, t) { t += 1 }
  var b = y1; while b > y0 && isBackground(x, b) { b -= 1 }
  topEdge[x] = Double(t); bottomEdge[x] = Double(b)
}
/// Light smoothing of an edge profile (texture makes it ragged).
func smooth(_ v: [Double], _ range: ClosedRange<Int>) -> [Double] {
  var out = v
  for i in range {
    let lo = max(range.lowerBound, i - 3), hi = min(range.upperBound, i + 3)
    out[i] = v[lo...hi].reduce(0, +) / Double(hi - lo + 1)
  }
  return out
}
leftEdge = smooth(leftEdge, y0...y1); rightEdge = smooth(rightEdge, y0...y1)
topEdge = smooth(topEdge, x0...x1); bottomEdge = smooth(bottomEdge, x0...x1)

/// The tile with everything outside its (eroded) edge transparent.
let tileImage: CGImage = {
  let tw = Int(tile.width), th = Int(tile.height)
  var out = [UInt8](repeating: 0, count: tw * th * 4)
  for ty in 0..<th {
    let y = y0 + ty
    for tx in 0..<tw {
      let x = x0 + tx
      let inside = min(
        Double(x) - leftEdge[y] - erode, rightEdge[y] - erode - Double(x),
        Double(y) - topEdge[x] - erode, bottomEdge[x] - erode - Double(y))
      let alpha = max(0, min(1, inside + 0.5))
      let i = (y * W + x) * 4, o = (ty * tw + tx) * 4
      for c in 0..<3 { out[o + c] = UInt8(Double(pixels[i + c]) * alpha) }
      out[o + 3] = UInt8(alpha * 255)
    }
  }
  let ctx = CGContext(
    data: &out, width: tw, height: th, bitsPerComponent: 8, bytesPerRow: tw * 4, space: rgb,
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
  return ctx.makeImage()!
}()
/// The tile as is (for full-bleed use, where nothing outside shows).
let tileOpaque = concept.cropping(to: tile)!

// MARK: - Rendering

func canvas(_ size: Int, opaque: Bool) -> CGContext {
  CGContext(
    data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0, space: rgb,
    bitmapInfo: opaque
      ? CGImageAlphaInfo.noneSkipLast.rawValue : CGImageAlphaInfo.premultipliedLast.rawValue)!
}

/// Draws the masked tile into `rect`.
func drawTile(_ ctx: CGContext, in rect: CGRect) {
  ctx.interpolationQuality = .high
  ctx.draw(tileImage, in: rect)
}

func save(_ ctx: CGContext, _ name: String) {
  let url = outDir.appendingPathComponent(name)
  let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
  CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
  CGImageDestinationFinalize(dest)
  print("wrote \(url.path)")
}

// Desktop (macOS grid): 1024 canvas, 824 tile centered, soft shadow.
do {
  let ctx = canvas(1024, opaque: false)
  let rect = CGRect(x: 100, y: 100, width: 824, height: 824)
  ctx.saveGState()
  ctx.setShadow(
    offset: CGSize(width: 0, height: -10), blur: 28, color: CGColor(gray: 0, alpha: 0.32))
  ctx.beginTransparencyLayer(auxiliaryInfo: nil)
  drawTile(ctx, in: rect)
  ctx.endTransparencyLayer()
  ctx.restoreGState()
  save(ctx, "rupert-icon-desktop-1024.png")
}

// iOS: full bleed and opaque; iOS applies its own (slightly squarer)
// mask, so zoom in a little until the concept's rounder corners fall
// outside it, over a blurred enlargement that fills any gap.
do {
  let ctx = canvas(1024, opaque: true)
  ctx.interpolationQuality = .high
  ctx.setFillColor(brown)
  ctx.fill(CGRect(x: 0, y: 0, width: 1024, height: 1024))
  let zoom = 1.045
  let side = 1024 * zoom
  let rect = CGRect(x: (1024 - side) / 2, y: (1024 - side) / 2, width: side, height: side)
  // Bleed: the tile enlarged and blurred (cheaply, by drawing it small and
  // scaling up) behind the real one.
  let small = CGContext(
    data: nil, width: 48, height: 48, bitsPerComponent: 8, bytesPerRow: 0, space: rgb,
    bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
  small.interpolationQuality = .high
  small.setFillColor(brown)
  small.fill(CGRect(x: 0, y: 0, width: 48, height: 48))
  small.draw(tileOpaque, in: CGRect(x: -6, y: -6, width: 60, height: 60))
  ctx.draw(small.makeImage()!, in: rect.insetBy(dx: -60, dy: -60))
  drawTile(ctx, in: rect)
  save(ctx, "rupert-icon-ios-1024.png")
}

// Favicon: the tile with little padding, transparent outside.
do {
  let ctx = canvas(512, opaque: false)
  drawTile(ctx, in: CGRect(x: 8, y: 8, width: 496, height: 496))
  save(ctx, "rupert-favicon-512.png")
}
