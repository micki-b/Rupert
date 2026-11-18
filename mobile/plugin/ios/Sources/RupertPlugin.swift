// iOS services for Rupert's phone app. Called from Rust only.
//
// - Folders come from the Files picker (iCloud Drive, OneDrive, On My
//   iPhone). Access is kept across launches with bookmarks.
// - Documents are never read in the background. `download` fetches one file
//   the user opened; `fetchFolder` fetches Rupert's own small hidden files
//   (sidecars and the shared catalog), never documents.
// - `scan` runs the system document camera and returns JPEG pages; Rust
//   turns them into a PDF and `place` copies it into the inbox folder.
// - `signIn` shows a web sign-in page (Microsoft, for OneDrive) and returns
//   the redirect; tokens are kept with the `keychain*` commands.

import AuthenticationServices
import Foundation
import Security
import QuickLook
import SwiftRs
import Tauri
import UIKit
import UniformTypeIdentifiers
import VisionKit

// MARK: - Arguments and results

struct PathArgs: Decodable {
  let path: String
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

// MARK: - Plugin

class RupertPlugin: Plugin {
  private var pickerDelegate: FolderPickerDelegate?
  private var scanDelegate: ScanDelegate?
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

  // MARK: Scanning

  @objc public func scan(_ invoke: Invoke) {
    DispatchQueue.main.async {
      #if targetEnvironment(simulator)
        // The simulator has no camera: hand back a generated test page so
        // the rest of the pipeline can be tried.
        invoke.resolve(ScanResult(pages: Self.save([Self.testPage()])))
      #else
        guard VNDocumentCameraViewController.isSupported else {
          invoke.reject("This device can't scan documents.")
          return
        }
        let camera = VNDocumentCameraViewController()
        let delegate = ScanDelegate { result in
          self.scanDelegate = nil
          switch result {
          case .none:
            invoke.resolve(ScanResult(pages: []))
          case .some(.success(let images)):
            DispatchQueue.global(qos: .userInitiated).async {
              invoke.resolve(ScanResult(pages: Self.save(images)))
            }
          case .some(.failure(let error)):
            invoke.reject(error.localizedDescription)
          }
        }
        self.scanDelegate = delegate
        camera.delegate = delegate
        self.manager.viewController?.present(camera, animated: true)
      #endif
    }
  }

  /// Longest side kept: 11 inches at 300 dpi, plenty for OCR.
  private static let maxPixels: CGFloat = 3300

  private static func save(_ images: [UIImage]) -> [String] {
    let dir = FileManager.default.temporaryDirectory
      .appendingPathComponent("rupert-scan-\(UUID().uuidString)", isDirectory: true)
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    var paths: [String] = []
    for (i, image) in images.enumerated() {
      let url = dir.appendingPathComponent(String(format: "page-%03d.jpg", i + 1))
      if let data = shrink(image).jpegData(compressionQuality: 0.7),
        (try? data.write(to: url)) != nil
      {
        paths.append(url.path)
      }
    }
    return paths
  }

  private static func shrink(_ image: UIImage) -> UIImage {
    let size = image.size
    let scale = min(1, maxPixels / max(size.width * image.scale, size.height * image.scale))
    let target = CGSize(
      width: (size.width * image.scale * scale).rounded(),
      height: (size.height * image.scale * scale).rounded())
    let format = UIGraphicsImageRendererFormat()
    format.scale = 1
    format.opaque = true
    return UIGraphicsImageRenderer(size: target, format: format).image { _ in
      image.draw(in: CGRect(origin: .zero, size: target))
    }
  }

  #if targetEnvironment(simulator)
    private static func testPage() -> UIImage {
      let size = CGSize(width: 1275, height: 1650)
      let format = UIGraphicsImageRendererFormat()
      format.scale = 1
      return UIGraphicsImageRenderer(size: size, format: format).image { ctx in
        UIColor.white.setFill()
        ctx.fill(CGRect(origin: .zero, size: size))
        let text = "Rupert test scan\n\nInvoice 1042\nHarbor Marine Supply\nAmount due: $86.40"
        text.draw(
          in: CGRect(x: 120, y: 160, width: 1035, height: 1300),
          withAttributes: [.font: UIFont.systemFont(ofSize: 56), .foregroundColor: UIColor.black])
      }
    }
  #endif

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

class ScanDelegate: NSObject, VNDocumentCameraViewControllerDelegate {
  /// `nil` when cancelled.
  private let done: (Result<[UIImage], Error>?) -> Void

  init(_ done: @escaping (Result<[UIImage], Error>?) -> Void) {
    self.done = done
  }

  func documentCameraViewController(
    _ controller: VNDocumentCameraViewController, didFinishWith scan: VNDocumentCameraScan
  ) {
    let images = (0..<scan.pageCount).map { scan.imageOfPage(at: $0) }
    controller.dismiss(animated: true)
    done(.success(images))
  }

  func documentCameraViewControllerDidCancel(_ controller: VNDocumentCameraViewController) {
    controller.dismiss(animated: true)
    done(nil)
  }

  func documentCameraViewController(
    _ controller: VNDocumentCameraViewController, didFailWithError error: Error
  ) {
    controller.dismiss(animated: true)
    done(.failure(error))
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
