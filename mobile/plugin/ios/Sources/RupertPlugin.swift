// iOS services for Rupert's phone app. Called from Rust only.
//
// - Folders come from the Files picker (iCloud Drive, OneDrive, On My
//   iPhone). Access is kept across launches with bookmarks.
// - Documents are never read in the background. `download` fetches one file
//   the user opened; `fetchFolder` fetches Rupert's own small hidden files
//   (sidecars and the shared catalog), never documents.
// - `scan` runs Rupert's document camera (Scanner.swift) and returns the
//   page images; Rust turns them into a PDF and `place` copies it into the
//   inbox folder.
// - `signIn` shows a web sign-in page (Microsoft, for OneDrive) and returns
//   the redirect; tokens are kept with the `keychain*` commands.
// - `sharedFolders` points at the app group shared with the share extension
//   (RupertShare in gen/apple), which saves documents straight to the inbox.

import AuthenticationServices
import Foundation
import Security
import QuickLook
import QuickLookThumbnailing
import SwiftRs
import Tauri
import UIKit
import UniformTypeIdentifiers
import AVFoundation

// MARK: - Arguments and results

struct PathArgs: Decodable {
  let path: String
}

struct ShareArgs: Decodable {
  let path: String
  /// The Share button on the page, which the sheet opens from.
  let x: Double
  let y: Double
  let width: Double
  let height: Double
}

struct UrlArgs: Decodable {
  let url: String
}

struct BookmarksArgs: Decodable {
  let bookmarks: [String]
}

struct PlaceArgs: Decodable {
  let from: String
  let folder: String
  let name: String
}

struct PickedFolder: Encodable {
  let path: String
  let name: String
  let bookmark: String
}

struct PickResult: Encodable {
  let folder: PickedFolder?
}

struct ResolvedBookmark: Encodable {
  let path: String?
  /// A fresh bookmark when the old one went stale; save it instead.
  let bookmark: String?
  let error: String?
}

struct ResolveResult: Encodable {
  let folders: [ResolvedBookmark]
}

struct FetchResult: Encodable {
  let files: Int
}

struct ScanResult: Encodable {
  /// JPEG files, one per page, in a temporary folder. Empty if cancelled.
  let pages: [String]
}

struct PlaceResult: Encodable {
  let path: String
  let name: String
}

struct WriteArgs: Decodable {
  let path: String
  let text: String
}

struct SignInArgs: Decodable {
  let url: String
  let callbackScheme: String
}

struct SignInResult: Encodable {
  /// The redirect with the answer; `nil` if the user cancelled.
  let url: String?
}

struct KeyArgs: Decodable {
  let key: String
}

struct KeyValueArgs: Decodable {
  let key: String
  let value: String
}

struct KeyValue: Encodable {
  let value: String?
}

struct ThumbnailArgs: Decodable {
  let path: String
  let side: Double
}

struct ThumbnailResult: Encodable {
  let path: String?
}

struct SharedFoldersResult: Encodable {
  let pending: String
  let settings: String?
}

// MARK: - Plugin

class RupertPlugin: Plugin {
  private var pickerDelegate: FolderPickerDelegate?
  private var previewSource: PreviewSource?
  /// Folders whose security-scoped access is open for the app's lifetime.
  private var accessed: [URL] = []
  private var authSession: ASWebAuthenticationSession?
  private var authAnchor: AuthAnchor?

  // MARK: Folders

  @objc public func pickFolder(_ invoke: Invoke) {
    DispatchQueue.main.async {
      let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.folder])
      picker.allowsMultipleSelection = false
      let delegate = FolderPickerDelegate { url in
        self.pickerDelegate = nil
        guard let url = url else {
          invoke.resolve(PickResult(folder: nil))
          return
        }
        guard url.startAccessingSecurityScopedResource() else {
          invoke.reject("Rupert wasn't given access to that folder.")
          return
        }
        self.accessed.append(url)
        do {
          let bookmark = try url.bookmarkData()
          invoke.resolve(
            PickResult(
              folder: PickedFolder(
                path: url.path, name: url.lastPathComponent,
                bookmark: bookmark.base64EncodedString())))
        } catch {
          invoke.reject("Couldn't remember that folder: \(error.localizedDescription)")
        }
      }
      self.pickerDelegate = delegate
      picker.delegate = delegate
      self.manager.viewController?.present(picker, animated: true)
    }
  }

  /// Reopens saved folders at launch and keeps them open.
  @objc public func resolveBookmarks(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(BookmarksArgs.self)
    var out: [ResolvedBookmark] = []
    for encoded in args.bookmarks {
      guard let data = Data(base64Encoded: encoded) else {
        out.append(ResolvedBookmark(path: nil, bookmark: nil, error: "damaged bookmark"))
        continue
      }
      do {
        var stale = false
        let url = try URL(resolvingBookmarkData: data, bookmarkDataIsStale: &stale)
        guard url.startAccessingSecurityScopedResource() else {
          out.append(
            ResolvedBookmark(path: url.path, bookmark: nil, error: "access was withdrawn"))
          continue
        }
        accessed.append(url)
        let fresh = stale ? (try? url.bookmarkData())?.base64EncodedString() : nil
        out.append(ResolvedBookmark(path: url.path, bookmark: fresh, error: nil))
      } catch {
        out.append(ResolvedBookmark(path: nil, bookmark: nil, error: error.localizedDescription))
      }
    }
    invoke.resolve(ResolveResult(folders: out))
  }

  // MARK: Downloads

  /// Fetches every file in one of Rupert's hidden `.rupert` folders.
  @objc public func fetchFolder(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PathArgs.self)
    let dir = URL(fileURLWithPath: args.path, isDirectory: true)
    guard dir.pathComponents.contains(".rupert") else {
      invoke.reject("Only Rupert's own folders are fetched.")
      return
    }
    let fm = FileManager.default
    var intents: [NSFileAccessIntent] = []
    if let walker = fm.enumerator(at: dir, includingPropertiesForKeys: [.isDirectoryKey]) {
      for case let url as URL in walker {
        let name = url.lastPathComponent
        if name.hasPrefix(".") && name.hasSuffix(".icloud") {
          // An iCloud placeholder for ".name.icloud" stands for "name".
          let real = url.deletingLastPathComponent()
            .appendingPathComponent(String(name.dropFirst().dropLast(".icloud".count)))
          try? fm.startDownloadingUbiquitousItem(at: real)
          intents.append(.readingIntent(with: real))
        } else if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) != true {
          intents.append(.readingIntent(with: url))
        }
      }
    }
    if intents.isEmpty {
      invoke.resolve(FetchResult(files: 0))
      return
    }
    NSFileCoordinator().coordinate(with: intents, queue: OperationQueue()) { error in
      if let error = error {
        invoke.reject(error.localizedDescription)
      } else {
        invoke.resolve(FetchResult(files: intents.count))
      }
    }
  }

  /// Downloads one document the user opened.
  @objc public func download(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PathArgs.self)
    let url = URL(fileURLWithPath: args.path)
    try? FileManager.default.startDownloadingUbiquitousItem(at: url)
    NSFileCoordinator().coordinate(with: [.readingIntent(with: url)], queue: OperationQueue()) {
      error in
      if let error = error {
        invoke.reject(error.localizedDescription)
      } else {
        invoke.resolve()
      }
    }
  }

  /// Shows a (downloaded) document in Quick Look, with sharing and printing.
  /// Markup is turned off: the phone never changes documents.
  @objc public func preview(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PathArgs.self)
    DispatchQueue.main.async {
      let source = PreviewSource(URL(fileURLWithPath: args.path))
      self.previewSource = source
      let controller = QLPreviewController()
      controller.dataSource = source
      controller.delegate = source
      self.manager.viewController?.present(controller, animated: true)
      invoke.resolve()
    }
  }

  /// The share sheet for a (downloaded) document (#69): Mail, Messages,
  /// AirDrop and the apps that take files. Where it opens as a popover
  /// (an iPad, and iOS 26 on the iPhone) it points at the Share button.
  @objc public func share(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(ShareArgs.self)
    DispatchQueue.main.async {
      guard let presenter = self.manager.viewController else {
        invoke.reject("Rupert's window isn't ready.")
        return
      }
      let sheet = UIActivityViewController(
        activityItems: [URL(fileURLWithPath: args.path)], applicationActivities: nil)
      if let popover = sheet.popoverPresentationController {
        // The page fills the view, so its coordinates are the view's.
        popover.sourceView = presenter.view
        popover.sourceRect = CGRect(
          x: args.x, y: args.y, width: max(args.width, 1), height: max(args.height, 1))
        popover.permittedArrowDirections = .any
      }
      presenter.present(sheet, animated: true)
      invoke.resolve()
    }
  }

  /// Opens a web page (Rupert's privacy policy) in Safari.
  @objc public func openUrl(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(UrlArgs.self)
    guard let url = URL(string: args.url) else {
      invoke.reject("Not a web address: \(args.url)")
      return
    }
    DispatchQueue.main.async {
      UIApplication.shared.open(url)
      invoke.resolve()
    }
  }

  // MARK: Scanning

  @objc public func scan(_ invoke: Invoke) throws {
    let options = try invoke.parseArgs(ScanOptions.self)
    DispatchQueue.main.async {
      #if targetEnvironment(simulator)
        // The simulator has no camera: run a generated photo of a page
        // through the same processing.
        DispatchQueue.global(qos: .userInitiated).async {
          let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("rupert-scan-\(UUID().uuidString)", isDirectory: true)
          try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
          let path = PageProcessor.process(
            Self.testPhoto(), options: options, to: dir.appendingPathComponent("page-001"))
          invoke.resolve(ScanResult(pages: path.map { [$0] } ?? []))
        }
      #else
        guard AVCaptureDevice.authorizationStatus(for: .video) != .denied else {
          invoke.reject("Rupert isn't allowed to use the camera (Settings ▸ Apps ▸ Rupert).")
          return
        }
        let camera = ScannerViewController(options: options) { pages in
          invoke.resolve(ScanResult(pages: pages ?? []))
        }
        self.manager.viewController?.present(camera, animated: true)
      #endif
    }
  }

  #if targetEnvironment(simulator)
    /// A page lying slightly askew on a dark table, with uneven light.
    private static func testPhoto() -> CIImage {
      let size = CGSize(width: 3000, height: 4000)
      let format = UIGraphicsImageRendererFormat()
      format.scale = 1
      let photo = UIGraphicsImageRenderer(size: size, format: format).image { ctx in
        UIColor(white: 0.18, alpha: 1).setFill()
        ctx.fill(CGRect(origin: .zero, size: size))
        let page = UIBezierPath()
        page.move(to: CGPoint(x: 420, y: 380))
        page.addLine(to: CGPoint(x: 2620, y: 470))
        page.addLine(to: CGPoint(x: 2540, y: 3560))
        page.addLine(to: CGPoint(x: 360, y: 3480))
        page.close()
        let colors = [UIColor(white: 0.97, alpha: 1).cgColor, UIColor(white: 0.72, alpha: 1).cgColor]
        let gradient = CGGradient(
          colorsSpace: CGColorSpaceCreateDeviceGray(), colors: colors as CFArray, locations: [0, 1])!
        ctx.cgContext.saveGState()
        page.addClip()
        ctx.cgContext.drawLinearGradient(
          gradient, start: CGPoint(x: 400, y: 400), end: CGPoint(x: 2600, y: 3500), options: [])
        ctx.cgContext.restoreGState()
        ctx.cgContext.translateBy(x: 520, y: 520)
        ctx.cgContext.rotate(by: 0.04)
        let text =
          "HARBOR MARINE SUPPLY\n\nInvoice 1042\nRaw water impeller kit\nYanmar 3JH5E\n\nAmount due: $86.40\nThank you for your business."
        text.draw(
          in: CGRect(x: 0, y: 0, width: 1900, height: 2600),
          withAttributes: [.font: UIFont.systemFont(ofSize: 110), .foregroundColor: UIColor.black])
      }
      return CIImage(image: photo) ?? CIImage()
    }
  #endif

  // MARK: Rupert's own small files

  /// Writes one of Rupert's small files (e.g. a folder's `.folder.json`),
  /// creating its `.rupert` folder if needed, coordinated with the sync
  /// service. Documents are never written this way.
  @objc public func writeFile(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(WriteArgs.self)
    let url = URL(fileURLWithPath: args.path)
    guard url.pathComponents.contains(".rupert") else {
      invoke.reject("Only Rupert's own files are written.")
      return
    }
    var coordinationError: NSError?
    var writeError: Error?
    NSFileCoordinator().coordinate(
      writingItemAt: url, options: .forReplacing, error: &coordinationError
    ) { target in
      do {
        try FileManager.default.createDirectory(
          at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(args.text.utf8).write(to: target, options: .atomic)
      } catch {
        writeError = error
      }
    }
    if let error = coordinationError ?? writeError {
      invoke.reject("Couldn't save: \(error.localizedDescription)")
    } else {
      invoke.resolve()
    }
  }

  /// Removes one of Rupert's small files, and its `.rupert` folder if that
  /// is then empty. Already gone counts as done.
  @objc public func removeFile(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PathArgs.self)
    let url = URL(fileURLWithPath: args.path)
    guard url.pathComponents.contains(".rupert") else {
      invoke.reject("Only Rupert's own files are removed.")
      return
    }
    let fm = FileManager.default
    var coordinationError: NSError?
    var removeError: Error?
    NSFileCoordinator().coordinate(
      writingItemAt: url, options: .forDeleting, error: &coordinationError
    ) { target in
      guard fm.fileExists(atPath: target.path) else { return }
      do {
        try fm.removeItem(at: target)
      } catch {
        removeError = error
      }
    }
    if let error = coordinationError ?? removeError {
      invoke.reject("Couldn't save: \(error.localizedDescription)")
      return
    }
    let dir = url.deletingLastPathComponent()
    if let left = try? fm.contentsOfDirectory(atPath: dir.path), left.isEmpty {
      NSFileCoordinator().coordinate(
        writingItemAt: dir, options: .forDeleting, error: nil
      ) { target in
        try? fm.removeItem(at: target)
      }
    }
    invoke.resolve()
  }

  /// Copies a finished scan into the inbox folder under a free name.
  @objc public func place(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PlaceArgs.self)
    let fm = FileManager.default
    let source = URL(fileURLWithPath: args.from)
    let folder = URL(fileURLWithPath: args.folder, isDirectory: true)
    let ext = source.pathExtension
    func taken(_ name: String) -> Bool {
      fm.fileExists(atPath: folder.appendingPathComponent(name).path)
        || fm.fileExists(atPath: folder.appendingPathComponent(".\(name).icloud").path)
    }
    var name = "\(args.name).\(ext)"
    var n = 2
    while taken(name) {
      name = "\(args.name) (\(n)).\(ext)"
      n += 1
    }
    let target = folder.appendingPathComponent(name)
    var coordinationError: NSError?
    var copyError: Error?
    NSFileCoordinator().coordinate(
      writingItemAt: target, options: .forReplacing, error: &coordinationError
    ) { url in
      do {
        try fm.copyItem(at: source, to: url)
      } catch {
        copyError = error
      }
    }
    if let error = coordinationError ?? copyError {
      invoke.reject("Couldn't save the scan: \(error.localizedDescription)")
      return
    }
    try? fm.removeItem(at: source.deletingLastPathComponent())
    invoke.resolve(PlaceResult(path: target.path, name: name))
  }
}

// MARK: - Thumbnails

extension RupertPlugin {
  /// The system's picture of a document on the phone, as a JPEG in a
  /// temporary file (`{path}`), or `{path: null}`. Rust only asks about
  /// files that are on the phone.
  @objc public func thumbnail(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(ThumbnailArgs.self)
    let request = QLThumbnailGenerator.Request(
      fileAt: URL(fileURLWithPath: args.path),
      size: CGSize(width: args.side, height: args.side),
      scale: 1,
      representationTypes: .thumbnail)
    QLThumbnailGenerator.shared.generateBestRepresentation(for: request) { thumb, _ in
      guard let data = thumb?.uiImage.jpegData(compressionQuality: 0.8) else {
        invoke.resolve(ThumbnailResult(path: nil))
        return
      }
      let out = FileManager.default.temporaryDirectory
        .appendingPathComponent("thumb-\(UUID().uuidString).jpg")
      do {
        try data.write(to: out)
        invoke.resolve(ThumbnailResult(path: out.path))
      } catch {
        invoke.resolve(ThumbnailResult(path: nil))
      }
    }
  }
}

// MARK: - Sharing from other apps

extension RupertPlugin {
  /// The app group shared with the share extension (RupertShare). Keychain
  /// items are shared too: both list the app's own keychain group.
  static let appGroup = "group.io.github.micki-b.rupert"

  /// Where the share extension keeps documents it couldn't save to the inbox
  /// (`Pending/<id>/<name>`), and where its settings file goes.
  @objc public func sharedFolders(_ invoke: Invoke) throws {
    guard
      let group = FileManager.default.containerURL(
        forSecurityApplicationGroupIdentifier: Self.appGroup)
    else {
      invoke.reject("The app group is missing from this build.")
      return
    }
    let pending = group.appendingPathComponent("Pending", isDirectory: true)
    try FileManager.default.createDirectory(at: pending, withIntermediateDirectories: true)
    invoke.resolve(SharedFoldersResult(pending: pending.path, settings: group.path))
  }
}

// MARK: - Sign-in and Keychain

extension RupertPlugin {
  @objc public func signIn(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(SignInArgs.self)
    guard let url = URL(string: args.url) else {
      invoke.reject("Bad sign-in address.")
      return
    }
    DispatchQueue.main.async {
      let session = ASWebAuthenticationSession(
        url: url, callbackURLScheme: args.callbackScheme
      ) { callback, error in
        self.authSession = nil
        if let callback = callback {
          invoke.resolve(SignInResult(url: callback.absoluteString))
        } else if let e = error as? ASWebAuthenticationSessionError, e.code == .canceledLogin {
          invoke.resolve(SignInResult(url: nil))
        } else {
          invoke.reject(error?.localizedDescription ?? "Sign-in failed.")
        }
      }
      let anchor = AuthAnchor(self.manager.viewController?.view.window)
      self.authAnchor = anchor
      session.presentationContextProvider = anchor
      // A private session with its own cookies: Safari's cookies (stale
      // Microsoft sessions) broke the sign-in's session cookie.
      session.prefersEphemeralWebBrowserSession = true
      self.authSession = session
      if !session.start() {
        self.authSession = nil
        invoke.reject("Couldn't show the sign-in page.")
      }
    }
  }

  private static let keychainService = "io.github.micki-b.rupert"

  private func keychainQuery(_ key: String) -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: Self.keychainService,
      kSecAttrAccount as String: key,
    ]
  }

  @objc public func keychainGet(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(KeyArgs.self)
    var query = keychainQuery(args.key)
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: AnyObject?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    switch status {
    case errSecSuccess:
      let value = (result as? Data).flatMap { String(data: $0, encoding: .utf8) }
      invoke.resolve(KeyValue(value: value))
    case errSecItemNotFound:
      invoke.resolve(KeyValue(value: nil))
    default:
      invoke.reject("Keychain error \(status)")
    }
  }

  @objc public func keychainSet(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(KeyValueArgs.self)
    let query = keychainQuery(args.key)
    SecItemDelete(query as CFDictionary)
    var add = query
    add[kSecValueData as String] = Data(args.value.utf8)
    add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
    let status = SecItemAdd(add as CFDictionary, nil)
    if status == errSecSuccess {
      invoke.resolve()
    } else {
      invoke.reject("Keychain error \(status)")
    }
  }

  @objc public func keychainDelete(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(KeyArgs.self)
    SecItemDelete(keychainQuery(args.key) as CFDictionary)
    invoke.resolve()
  }
}

class AuthAnchor: NSObject, ASWebAuthenticationPresentationContextProviding {
  private let window: UIWindow?

  init(_ window: UIWindow?) {
    self.window = window
  }

  func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
    window ?? ASPresentationAnchor()
  }
}

// MARK: - Delegates

class FolderPickerDelegate: NSObject, UIDocumentPickerDelegate {
  private let done: (URL?) -> Void

  init(_ done: @escaping (URL?) -> Void) {
    self.done = done
  }

  func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL])
  {
    done(urls.first)
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    done(nil)
  }
}

class PreviewSource: NSObject, QLPreviewControllerDataSource, QLPreviewControllerDelegate {
  private let url: URL

  init(_ url: URL) {
    self.url = url
  }

  func numberOfPreviewItems(in controller: QLPreviewController) -> Int {
    1
  }

  func previewController(_ controller: QLPreviewController, previewItemAt index: Int)
    -> QLPreviewItem
  {
    url as NSURL
  }

  func previewController(_ controller: QLPreviewController, editingModeFor previewItem: QLPreviewItem)
    -> QLPreviewItemEditingMode
  {
    .disabled
  }
}

@_cdecl("init_plugin_rupert_ios")
public func initPlugin() -> Plugin {
  return RupertPlugin()
}
