// Rupert's document camera: full-resolution photos from the main camera
// (with automatic macro on phones that have it), the page's edges found by
// Vision while aiming and again on each photo, perspective correction, and
// the chosen color mode and resolution.
//
// Pages are written as files: JPEG for color and grayscale, PNG (black or
// white pixels) for black and white, which Rust turns into 1-bit images.

import AVFoundation
import CoreImage
import CoreImage.CIFilterBuiltins
import ImageIO
import UIKit
import UniformTypeIdentifiers
import Vision

struct ScanOptions: Decodable {
  /// "color", "gray" or "bw".
  let mode: String
  /// Output resolution, assuming the page's long side is 11 inches.
  let dpi: Int
}

// MARK: - Page processing

enum PageProcessor {
  static let context = CIContext(options: [.useSoftwareRenderer: false])

  /// Crops and straightens the page (if one is found), applies the mode,
  /// scales to the resolution and writes the file. Returns its path.
  static func process(_ photo: CIImage, options: ScanOptions, to url: URL) -> String? {
    var image = straighten(photo)
    let long = max(image.extent.width, image.extent.height)
    let target = CGFloat(options.dpi) * 11
    if long > target {
      let f = CIFilter.lanczosScaleTransform()
      f.inputImage = image
      f.scale = Float(target / long)
      f.aspectRatio = 1
      image = f.outputImage ?? image
    }
    image = image.transformed(
      by: CGAffineTransform(translationX: -image.extent.origin.x, y: -image.extent.origin.y))
    let extent = image.extent.integral

    switch options.mode {
    case "bw":
      let bw = blackAndWhite(image)
      guard
        let cg = context.createCGImage(
          bw, from: extent, format: .L8, colorSpace: CGColorSpaceCreateDeviceGray())
      else { return nil }
      return write(cg, to: url.appendingPathExtension("png"), type: .png, quality: nil)
    case "gray":
      let gray = image.applyingFilter(
        "CIColorControls",
        parameters: [kCIInputSaturationKey: 0, kCIInputContrastKey: 1.08])
      guard
        let cg = context.createCGImage(
          gray, from: extent, format: .L8, colorSpace: CGColorSpaceCreateDeviceGray())
      else { return nil }
      return write(cg, to: url.appendingPathExtension("jpg"), type: .jpeg, quality: 0.85)
    default:
      guard
        let cg = context.createCGImage(
          image, from: extent, format: .RGBA8, colorSpace: CGColorSpace(name: CGColorSpace.sRGB)!)
      else { return nil }
      return write(cg, to: url.appendingPathExtension("jpg"), type: .jpeg, quality: 0.82)
    }
  }

  /// The page cut out and squared up, or the whole photo if no page is found.
  static func straighten(_ image: CIImage) -> CIImage {
    guard let quad = detect(in: image) else { return image }
    let e = image.extent
    func point(_ p: CGPoint) -> CGPoint {
      CGPoint(x: e.origin.x + p.x * e.width, y: e.origin.y + p.y * e.height)
    }
    let f = CIFilter.perspectiveCorrection()
    f.inputImage = image
    f.topLeft = point(quad.topLeft)
    f.topRight = point(quad.topRight)
    f.bottomLeft = point(quad.bottomLeft)
    f.bottomRight = point(quad.bottomRight)
    return f.outputImage ?? image
  }

  /// The page's corners (normalized, origin bottom left): Vision's
  /// document detector, or its plain rectangle detector if that finds
  /// nothing (it needs a clear edge all round, but works everywhere).
  static func detect(in image: CIImage) -> VNRectangleObservation? {
    detect(VNImageRequestHandler(ciImage: image, options: [:]))
  }

  static func detect(_ handler: VNImageRequestHandler) -> VNRectangleObservation? {
    let document = VNDetectDocumentSegmentationRequest()
    let rectangle = VNDetectRectanglesRequest()
    rectangle.minimumSize = 0.3
    rectangle.minimumConfidence = 0.6
    rectangle.minimumAspectRatio = 0.2
    rectangle.quadratureTolerance = 25
    try? handler.perform([document, rectangle])
    if let found = document.results?.first, found.confidence > 0.6 { return found }
    return rectangle.results?.first
  }

  /// Gray, with the paper's shading evened out (each pixel divided by a
  /// heavily blurred copy), then thresholded.
  static func blackAndWhite(_ image: CIImage) -> CIImage {
    let gray = image.applyingFilter("CIColorControls", parameters: [kCIInputSaturationKey: 0])
    let radius = max(image.extent.width, image.extent.height) / 40
    let background = gray.clampedToExtent()
      .applyingGaussianBlur(sigma: Double(radius))
      .cropped(to: gray.extent)
    // CIDivideBlendMode gives background ÷ input; here: gray ÷ blurred.
    let flat = background.applyingFilter(
      "CIDivideBlendMode", parameters: [kCIInputBackgroundImageKey: gray])
    let t = CIFilter.colorThreshold()
    t.inputImage = flat
    t.threshold = 0.82
    return t.outputImage ?? flat
  }

  static func write(_ cg: CGImage, to url: URL, type: UTType, quality: CGFloat?) -> String? {
    guard
      let dest = CGImageDestinationCreateWithURL(url as CFURL, type.identifier as CFString, 1, nil)
    else { return nil }
    var props: [CFString: Any] = [:]
    if let q = quality {
      props[kCGImageDestinationLossyCompressionQuality] = q
    }
    CGImageDestinationAddImage(dest, cg, props as CFDictionary)
    return CGImageDestinationFinalize(dest) ? url.path : nil
  }
}

// MARK: - Camera

final class ScannerViewController: UIViewController, AVCapturePhotoCaptureDelegate,
  AVCaptureVideoDataOutputSampleBufferDelegate
{
  private let options: ScanOptions
  /// Called once with the page files, or `nil` if cancelled.
  private let onFinish: ([String]?) -> Void
  private let outDir: URL

  private let session = AVCaptureSession()
  private let photoOutput = AVCapturePhotoOutput()
  private let videoOutput = AVCaptureVideoDataOutput()
  private let sessionQueue = DispatchQueue(label: "rupert.scan.session")
  private let videoQueue = DispatchQueue(label: "rupert.scan.video")
  private let workQueue = DispatchQueue(label: "rupert.scan.work")
  private var preview: AVCaptureVideoPreviewLayer!
  private let quadLayer = CAShapeLayer()
  private var lastDetection = Date.distantPast
  private var pages: [String] = []
  private var pending = 0
  private var finished = false

  private let shutter = UIButton(type: .custom)
  private let doneButton = UIButton(type: .system)
  private let cancelButton = UIButton(type: .system)
  private let thumbnail = UIImageView()
  private let countLabel = UILabel()
  private let hintLabel = UILabel()

  init(options: ScanOptions, onFinish: @escaping ([String]?) -> Void) {
    self.options = options
    self.onFinish = onFinish
    self.outDir = FileManager.default.temporaryDirectory
      .appendingPathComponent("rupert-scan-\(UUID().uuidString)", isDirectory: true)
    try? FileManager.default.createDirectory(at: outDir, withIntermediateDirectories: true)
    super.init(nibName: nil, bundle: nil)
    modalPresentationStyle = .fullScreen
  }

  required init?(coder: NSCoder) { fatalError("not used") }

  override var prefersStatusBarHidden: Bool { true }

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .black
    preview = AVCaptureVideoPreviewLayer(session: session)
    preview.videoGravity = .resizeAspect
    view.layer.addSublayer(preview)
    quadLayer.strokeColor = UIColor.systemYellow.cgColor
    quadLayer.fillColor = UIColor.systemYellow.withAlphaComponent(0.15).cgColor
    quadLayer.lineWidth = 3
    view.layer.addSublayer(quadLayer)
    buildControls()
    sessionQueue.async { self.configure() }
  }

  override func viewDidLayoutSubviews() {
    super.viewDidLayoutSubviews()
    preview.frame = view.bounds
  }

  override func viewWillDisappear(_ animated: Bool) {
    super.viewWillDisappear(animated)
    sessionQueue.async { self.session.stopRunning() }
  }

  // MARK: Setup

  private func configure() {
    let types: [AVCaptureDevice.DeviceType] = [
      .builtInTripleCamera, .builtInDualWideCamera, .builtInDualCamera, .builtInWideAngleCamera,
    ]
    guard
      let device = AVCaptureDevice.DiscoverySession(
        deviceTypes: types, mediaType: .video, position: .back
      ).devices.first,
      let input = try? AVCaptureDeviceInput(device: device)
    else {
      DispatchQueue.main.async { self.fail("No camera available.") }
      return
    }
    session.beginConfiguration()
    session.sessionPreset = .photo
    if session.canAddInput(input) { session.addInput(input) }
    if session.canAddOutput(photoOutput) { session.addOutput(photoOutput) }
    videoOutput.alwaysDiscardsLateVideoFrames = true
    videoOutput.setSampleBufferDelegate(self, queue: videoQueue)
    if session.canAddOutput(videoOutput) { session.addOutput(videoOutput) }
    photoOutput.maxPhotoQualityPrioritization = .quality
    if #available(iOS 16.0, *),
      let largest = device.activeFormat.supportedMaxPhotoDimensions.max(by: {
        $0.width * $0.height < $1.width * $1.height
      })
    {
      photoOutput.maxPhotoDimensions = largest
    }
    for connection in [photoOutput.connection(with: .video), videoOutput.connection(with: .video)] {
      guard let c = connection else { continue }
      if #available(iOS 17.0, *) {
        if c.isVideoRotationAngleSupported(90) { c.videoRotationAngle = 90 }
      } else if c.isVideoOrientationSupported {
        c.videoOrientation = .portrait
      }
    }
    session.commitConfiguration()

    // Start at the main ("1×") lens; virtual devices switch to the ultra
    // wide for macro on their own when the page is close.
    if (try? device.lockForConfiguration()) != nil {
      if let main = device.virtualDeviceSwitchOverVideoZoomFactors.first {
        device.videoZoomFactor = CGFloat(truncating: main)
      }
      if device.isFocusModeSupported(.continuousAutoFocus) {
        device.focusMode = .continuousAutoFocus
      }
      if device.isAutoFocusRangeRestrictionSupported {
        device.autoFocusRangeRestriction = .near
      }
      device.unlockForConfiguration()
    }
    session.startRunning()
  }

  private func buildControls() {
    shutter.translatesAutoresizingMaskIntoConstraints = false
    shutter.backgroundColor = .white
    shutter.layer.cornerRadius = 36
    shutter.layer.borderWidth = 5
    shutter.layer.borderColor = UIColor.lightGray.cgColor
    shutter.accessibilityLabel = "Take page"
    shutter.addTarget(self, action: #selector(capture), for: .touchUpInside)

    cancelButton.setTitle("Cancel", for: .normal)
    cancelButton.addTarget(self, action: #selector(cancel), for: .touchUpInside)
    doneButton.setTitle("Save", for: .normal)
    doneButton.titleLabel?.font = .boldSystemFont(ofSize: 17)
    doneButton.addTarget(self, action: #selector(done), for: .touchUpInside)
    doneButton.isEnabled = false
    for b in [cancelButton, doneButton] {
      b.translatesAutoresizingMaskIntoConstraints = false
      b.tintColor = .white
    }

    thumbnail.translatesAutoresizingMaskIntoConstraints = false
    thumbnail.contentMode = .scaleAspectFit
    thumbnail.layer.borderColor = UIColor.white.cgColor
    thumbnail.layer.borderWidth = 1
    thumbnail.isUserInteractionEnabled = true
    thumbnail.addGestureRecognizer(
      UITapGestureRecognizer(target: self, action: #selector(removeLast)))
    countLabel.translatesAutoresizingMaskIntoConstraints = false
    countLabel.textColor = .white
    countLabel.font = .systemFont(ofSize: 13)
    hintLabel.translatesAutoresizingMaskIntoConstraints = false
    hintLabel.textColor = .white
    hintLabel.font = .systemFont(ofSize: 14)
    hintLabel.textAlignment = .center
    hintLabel.numberOfLines = 2
    let mode = ["color": "Color", "gray": "Grayscale", "bw": "Black & white"][options.mode] ?? ""
    hintLabel.text = "\(mode) · \(options.dpi) dpi\nHold steady over the page"

    let bar = UIView()
    bar.translatesAutoresizingMaskIntoConstraints = false
    bar.backgroundColor = UIColor.black.withAlphaComponent(0.55)
    view.addSubview(bar)
    for v in [hintLabel, shutter, cancelButton, doneButton, thumbnail, countLabel] as [UIView] {
      view.addSubview(v)
    }
    let g = view.safeAreaLayoutGuide
    NSLayoutConstraint.activate([
      bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      bar.bottomAnchor.constraint(equalTo: view.bottomAnchor),
      bar.topAnchor.constraint(equalTo: shutter.topAnchor, constant: -16),
      shutter.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      shutter.bottomAnchor.constraint(equalTo: g.bottomAnchor, constant: -16),
      shutter.widthAnchor.constraint(equalToConstant: 72),
      shutter.heightAnchor.constraint(equalToConstant: 72),
      cancelButton.leadingAnchor.constraint(equalTo: g.leadingAnchor, constant: 20),
      cancelButton.topAnchor.constraint(equalTo: g.topAnchor, constant: 12),
      doneButton.trailingAnchor.constraint(equalTo: g.trailingAnchor, constant: -20),
      doneButton.centerYAnchor.constraint(equalTo: shutter.centerYAnchor),
      thumbnail.leadingAnchor.constraint(equalTo: g.leadingAnchor, constant: 20),
      thumbnail.centerYAnchor.constraint(equalTo: shutter.centerYAnchor),
      thumbnail.widthAnchor.constraint(equalToConstant: 44),
      thumbnail.heightAnchor.constraint(equalToConstant: 58),
      countLabel.leadingAnchor.constraint(equalTo: thumbnail.trailingAnchor, constant: 8),
      countLabel.centerYAnchor.constraint(equalTo: shutter.centerYAnchor),
      hintLabel.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      hintLabel.topAnchor.constraint(equalTo: g.topAnchor, constant: 12),
      hintLabel.leadingAnchor.constraint(greaterThanOrEqualTo: cancelButton.trailingAnchor, constant: 8),
    ])
    refreshControls()
  }

  private func refreshControls() {
    let n = pages.count + pending
    doneButton.isEnabled = !pages.isEmpty && pending == 0
    doneButton.setTitle(pages.count > 1 ? "Save \(pages.count) pages" : "Save", for: .normal)
    countLabel.text = pending > 0 ? "Processing…" : (n == 0 ? "" : "Tap to remove")
    thumbnail.isHidden = pages.isEmpty
    if let last = pages.last { thumbnail.image = UIImage(contentsOfFile: last) }
  }

  // MARK: Live edge detection

  func captureOutput(
    _ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
    from connection: AVCaptureConnection
  ) {
    guard Date().timeIntervalSince(lastDetection) > 0.15,
      let buffer = CMSampleBufferGetImageBuffer(sampleBuffer)
    else { return }
    lastDetection = Date()
    let quad = PageProcessor.detect(VNImageRequestHandler(cvPixelBuffer: buffer, options: [:]))
    let size = CGSize(
      width: CVPixelBufferGetWidth(buffer), height: CVPixelBufferGetHeight(buffer))
    DispatchQueue.main.async { self.show(quad, frame: size) }
  }

  /// Draws the found page over the preview (aspect-fit mapping).
  private func show(_ quad: VNRectangleObservation?, frame: CGSize) {
    guard let q = quad, frame.width > 0 else {
      quadLayer.path = nil
      return
    }
    let bounds = view.bounds
    let scale = min(bounds.width / frame.width, bounds.height / frame.height)
    let shown = CGSize(width: frame.width * scale, height: frame.height * scale)
    let origin = CGPoint(
      x: (bounds.width - shown.width) / 2, y: (bounds.height - shown.height) / 2)
    func p(_ n: CGPoint) -> CGPoint {
      CGPoint(x: origin.x + n.x * shown.width, y: origin.y + (1 - n.y) * shown.height)
    }
    let path = UIBezierPath()
    path.move(to: p(q.topLeft))
    path.addLine(to: p(q.topRight))
    path.addLine(to: p(q.bottomRight))
    path.addLine(to: p(q.bottomLeft))
    path.close()
    quadLayer.path = path.cgPath
  }

  // MARK: Capture

  @objc private func capture() {
    let settings = AVCapturePhotoSettings()
    settings.photoQualityPrioritization = .quality
    if #available(iOS 16.0, *) {
      settings.maxPhotoDimensions = photoOutput.maxPhotoDimensions
    }
    pending += 1
    refreshControls()
    shutter.isEnabled = false
    photoOutput.capturePhoto(with: settings, delegate: self)
    UIView.animate(withDuration: 0.08, animations: { self.view.alpha = 0.6 }) { _ in
      UIView.animate(withDuration: 0.12) { self.view.alpha = 1 }
    }
  }

  func photoOutput(
    _ output: AVCapturePhotoOutput, didFinishProcessingPhoto photo: AVCapturePhoto, error: Error?
  ) {
    DispatchQueue.main.async { self.shutter.isEnabled = true }
    guard error == nil, let data = photo.fileDataRepresentation(),
      let image = CIImage(data: data, options: [.applyOrientationProperty: true])
    else {
      DispatchQueue.main.async {
        self.pending -= 1
        self.refreshControls()
      }
      return
    }
    // Named in capture order (the work queue keeps that order too).
    let base = outDir.appendingPathComponent(
      String(format: "page-%06lld", photo.resolvedSettings.uniqueID))
    workQueue.async {
      let path = PageProcessor.process(image, options: self.options, to: base)
      DispatchQueue.main.async {
        self.pending -= 1
        if let path = path { self.pages.append(path) }
        self.refreshControls()
      }
    }
  }

  // MARK: Finishing

  @objc private func removeLast() {
    guard let last = pages.popLast() else { return }
    try? FileManager.default.removeItem(atPath: last)
    refreshControls()
  }

  @objc private func cancel() {
    try? FileManager.default.removeItem(at: outDir)
    finish(nil)
  }

  @objc private func done() {
    finish(pages)
  }

  private func fail(_ message: String) {
    hintLabel.text = message
  }

  private func finish(_ result: [String]?) {
    guard !finished else { return }
    finished = true
    dismiss(animated: true) { self.onFinish(result) }
  }
}
