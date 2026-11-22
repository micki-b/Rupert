// Saves a document shared from another app into Rupert's inbox: a folder
// picked in Files (reopened from its bookmark), or a folder on OneDrive or
// Google Drive (uploaded with the app's sign-in, from the shared keychain).
//
// The app writes the inbox's details to `share-settings.json` in the app
// group (mobile/src/share.rs). A document is first kept in the app group's
// `Pending/<id>/` folder; once saved it's removed, and whatever is left
// there the app sends the next time it opens. The uploads match
// rupert_core::onedrive and rupert_core::gdrive: never replace a file, add
// " (2)" instead.

import Foundation
import Security
import UniformTypeIdentifiers

// MARK: - Settings from the app

struct ShareSettings: Decodable {
  struct Inbox: Decodable {
    /// "files", "onedrive" or "googledrive".
    let kind: String
    let name: String
    let bookmark: String?
    let itemId: String?
  }
  let inbox: Inbox?
  let language: String?
  let onedriveClientId: String?
  let googleClientId: String?
}

enum Shared {
  static let appGroup = "group.io.github.micki-b.rupert"

  static var container: URL? {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroup)
  }

  static func settings() -> ShareSettings? {
    guard let url = container?.appendingPathComponent("share-settings.json"),
      let data = try? Data(contentsOf: url)
    else { return nil }
    return try? JSONDecoder().decode(ShareSettings.self, from: data)
  }

  /// Keeps a copy of `file` as `Pending/<id>/<name>` until it's saved.
  static func keep(_ file: URL, name: String) throws -> URL {
    guard let container = container else { throw ShareError.noAppGroup }
    let dir = container.appendingPathComponent("Pending", isDirectory: true)
      .appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let kept = dir.appendingPathComponent(name)
    try FileManager.default.copyItem(at: file, to: kept)
    return kept
  }

  /// Done with a kept document: removes its folder.
  static func forget(_ kept: URL) {
    try? FileManager.default.removeItem(at: kept.deletingLastPathComponent())
  }
}

/// File name extensions of the documents Rupert shows (rupert_core::FileKind).
let documentExtensions: Set<String> = [
  "pdf", "jpg", "jpeg", "png", "tif", "tiff", "heic", "heif", "gif", "webp", "bmp",
  "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf",
  "pages", "numbers", "key", "txt", "md", "markdown", "csv", "eml", "emlx", "msg",
]

// MARK: - Errors

enum ShareError: Error {
  case noAppGroup
  case noInbox
  case inboxUnreachable
  case signIn(String)
  case notSetUp(String)
  case service(String)
}

// MARK: - Keychain (shared with the app)

enum Keychain {
  /// The app's keychain service; both list the app's keychain group.
  static let service = "io.github.micki-b.rupert"

  private static func query(_ key: String) -> [String: Any] {
    [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: service,
      kSecAttrAccount as String: key,
    ]
  }

  static func get(_ key: String) -> String? {
    var q = query(key)
    q[kSecReturnData as String] = true
    q[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: AnyObject?
    guard SecItemCopyMatching(q as CFDictionary, &result) == errSecSuccess,
      let data = result as? Data
    else { return nil }
    return String(data: data, encoding: .utf8)
  }

  /// Replaces a stored value (a renewed sign-in), keeping its group.
  static func update(_ key: String, _ value: String) {
    let change = [kSecValueData as String: Data(value.utf8)]
    SecItemUpdate(query(key) as CFDictionary, change as CFDictionary)
  }
}

// MARK: - Saving

final class InboxSaver {
  private let settings: ShareSettings
  private let inbox: ShareSettings.Inbox
  private let session = URLSession(configuration: .ephemeral)
  private var accessToken: String?

  init(settings: ShareSettings) throws {
    guard let inbox = settings.inbox else { throw ShareError.noInbox }
    self.settings = settings
    self.inbox = inbox
  }

  /// "OneDrive ▸ Inbox", or the Files folder's name.
  var inboxDisplayName: String {
    switch inbox.kind {
    case "onedrive": return "OneDrive ▸ \(inbox.name)"
    case "googledrive": return "Google Drive ▸ \(inbox.name)"
    default: return inbox.name
    }
  }

  /// Saves a kept document to the inbox; the name it got there.
  func save(_ file: URL) async throws -> String {
    switch inbox.kind {
    case "onedrive": return try await uploadToOneDrive(file)
    case "googledrive": return try await uploadToDrive(file)
    default: return try saveToFolder(file)
    }
  }

  // MARK: Files

  private func saveToFolder(_ file: URL) throws -> String {
    guard let encoded = inbox.bookmark, let data = Data(base64Encoded: encoded) else {
      throw ShareError.inboxUnreachable
    }
    var stale = false
    guard let folder = try? URL(resolvingBookmarkData: data, bookmarkDataIsStale: &stale) else {
      throw ShareError.inboxUnreachable
    }
    let opened = folder.startAccessingSecurityScopedResource()
    defer { if opened { folder.stopAccessingSecurityScopedResource() } }

    let fm = FileManager.default
    func taken(_ name: String) -> Bool {
      fm.fileExists(atPath: folder.appendingPathComponent(name).path)
        || fm.fileExists(atPath: folder.appendingPathComponent(".\(name).icloud").path)
    }
    let name = freeName(file.lastPathComponent, taken: taken)
    let target = folder.appendingPathComponent(name)
    var coordinationError: NSError?
    var copyError: Error?
    NSFileCoordinator().coordinate(
      writingItemAt: target, options: .forReplacing, error: &coordinationError
    ) { url in
      do {
        try fm.copyItem(at: file, to: url)
      } catch {
        copyError = error
      }
    }
    if let error = coordinationError ?? copyError {
      throw ShareError.service(error.localizedDescription)
    }
    return name
  }

  // MARK: OneDrive

  private static let oneDriveToken = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token"
  private static let graph = "https://graph.microsoft.com/v1.0"
  private static let simpleUploadMax = 4 * 1024 * 1024
  private static let uploadChunk = 320 * 1024 * 12

  private func oneDriveAccess() async throws -> String {
    if let token = accessToken { return token }
    guard let client = settings.onedriveClientId else { throw ShareError.notSetUp("OneDrive") }
    guard let refresh = Keychain.get("onedrive.refresh-token") else {
      throw ShareError.signIn("OneDrive")
    }
    let answer = try await tokenRequest(
      Self.oneDriveToken,
      [
        "client_id": client, "grant_type": "refresh_token", "refresh_token": refresh,
        "scope": "Files.ReadWrite offline_access",
      ], service: "OneDrive")
    if let renewed = answer.refresh_token { Keychain.update("onedrive.refresh-token", renewed) }
    accessToken = answer.access_token
    return answer.access_token
  }

  private func uploadToOneDrive(_ file: URL) async throws -> String {
    guard let folder = inbox.itemId else { throw ShareError.inboxUnreachable }
    let token = try await oneDriveAccess()
    let target = "\(Self.graph)/me/drive/items/\(encode(folder)):/\(encode(file.lastPathComponent)):"
    let size = (try? file.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
    if size <= Self.simpleUploadMax {
      var req = URLRequest(
        url: URL(string: "\(target)/content?@microsoft.graph.conflictBehavior=rename")!)
      req.httpMethod = "PUT"
      req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
      req.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
      let (data, resp) = try await session.upload(for: req, fromFile: file)
      return try named(data, resp, service: "OneDrive")
    }
    // Larger files go up in pieces through an upload session.
    var start = URLRequest(url: URL(string: "\(target)/createUploadSession")!)
    start.httpMethod = "POST"
    start.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    start.setValue("application/json", forHTTPHeaderField: "Content-Type")
    start.httpBody = try JSONSerialization.data(withJSONObject: [
      "item": ["@microsoft.graph.conflictBehavior": "rename"]
    ])
    let (sessionData, sessionResp) = try await session.data(for: start)
    try check(sessionData, sessionResp, service: "OneDrive")
    struct UploadSession: Decodable { let uploadUrl: String }
    let upload = try JSONDecoder().decode(UploadSession.self, from: sessionData)
    let handle = try FileHandle(forReadingFrom: file)
    defer { try? handle.close() }
    var offset = 0
    var last: (Data, URLResponse)?
    while offset < size {
      let chunk = try handle.read(upToCount: Self.uploadChunk) ?? Data()
      if chunk.isEmpty { break }
      // The upload address is pre-authorized: no bearer token.
      var put = URLRequest(url: URL(string: upload.uploadUrl)!)
      put.httpMethod = "PUT"
      put.setValue(
        "bytes \(offset)-\(offset + chunk.count - 1)/\(size)", forHTTPHeaderField: "Content-Range")
      let answer = try await session.upload(for: put, from: chunk)
      try check(answer.0, answer.1, service: "OneDrive")
      last = answer
      offset += chunk.count
    }
    guard let (data, resp) = last else { throw ShareError.service("empty upload") }
    return try named(data, resp, service: "OneDrive")
  }

  // MARK: Google Drive

  private static let googleToken = "https://oauth2.googleapis.com/token"
  private static let driveApi = "https://www.googleapis.com/drive/v3"
  private static let driveUpload = "https://www.googleapis.com/upload/drive/v3"

  private func driveAccess() async throws -> String {
    if let token = accessToken { return token }
    guard let client = settings.googleClientId else { throw ShareError.notSetUp("Google Drive") }
    guard let refresh = Keychain.get("gdrive.refresh-token") else {
      throw ShareError.signIn("Google Drive")
    }
    let answer = try await tokenRequest(
      Self.googleToken,
      ["client_id": client, "grant_type": "refresh_token", "refresh_token": refresh],
      service: "Google Drive")
    accessToken = answer.access_token
    return answer.access_token
  }

  private func uploadToDrive(_ file: URL) async throws -> String {
    guard let folder = inbox.itemId else { throw ShareError.inboxUnreachable }
    let token = try await driveAccess()

    // Drive allows two files with one name; Rupert doesn't.
    var taken = Set<String>()
    var pageToken: String?
    repeat {
      var parts = URLComponents(string: "\(Self.driveApi)/files")!
      let quoted = "'\(folder.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "'", with: "\\'"))'"
      parts.queryItems = [
        URLQueryItem(name: "q", value: "\(quoted) in parents and trashed = false"),
        URLQueryItem(name: "fields", value: "nextPageToken,files(id,name)"),
        URLQueryItem(name: "pageSize", value: "1000"),
        URLQueryItem(name: "spaces", value: "drive"),
      ] + (pageToken.map { [URLQueryItem(name: "pageToken", value: $0)] } ?? [])
      var list = URLRequest(url: parts.url!)
      list.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
      let (data, resp) = try await session.data(for: list)
      try check(data, resp, service: "Google Drive")
      struct Page: Decodable {
        struct File: Decodable { let name: String }
        let files: [File]
        let nextPageToken: String?
      }
      let page = try JSONDecoder().decode(Page.self, from: data)
      taken.formUnion(page.files.map(\.name))
      pageToken = page.nextPageToken
    } while pageToken != nil
    let name = freeName(file.lastPathComponent) { taken.contains($0) }

    let size = (try? file.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
    let mime =
      UTType(filenameExtension: file.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
    var start = URLRequest(
      url: URL(string: "\(Self.driveUpload)/files?uploadType=resumable&fields=id,name")!)
    start.httpMethod = "POST"
    start.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    start.setValue("application/json; charset=UTF-8", forHTTPHeaderField: "Content-Type")
    start.setValue(mime, forHTTPHeaderField: "X-Upload-Content-Type")
    start.setValue(String(size), forHTTPHeaderField: "X-Upload-Content-Length")
    start.httpBody = try JSONSerialization.data(withJSONObject: [
      "name": name, "parents": [folder],
    ])
    let (startData, startResp) = try await session.data(for: start)
    try check(startData, startResp, service: "Google Drive")
    guard let location = (startResp as? HTTPURLResponse)?.value(forHTTPHeaderField: "Location"),
      let uploadUrl = URL(string: location)
    else { throw ShareError.service("no upload address") }
    var put = URLRequest(url: uploadUrl)
    put.httpMethod = "PUT"
    put.setValue(mime, forHTTPHeaderField: "Content-Type")
    let (data, resp) = try await session.upload(for: put, fromFile: file)
    return try named(data, resp, service: "Google Drive")
  }

  // MARK: Shared helpers

  private struct TokenAnswer: Decodable {
    let access_token: String
    let refresh_token: String?
  }

  private func tokenRequest(_ url: String, _ form: [String: String], service: String) async throws
    -> TokenAnswer
  {
    var req = URLRequest(url: URL(string: url)!)
    req.httpMethod = "POST"
    req.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
    req.httpBody = form.map { "\($0.key)=\(encode($0.value))" }.joined(separator: "&")
      .data(using: .utf8)
    let (data, resp) = try await session.data(for: req)
    let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
    // A refused renewal means the sign-in ran out or was withdrawn.
    if status == 400 || status == 401 { throw ShareError.signIn(service) }
    try check(data, resp, service: service)
    return try JSONDecoder().decode(TokenAnswer.self, from: data)
  }

  private func check(_ data: Data, _ resp: URLResponse, service: String) throws {
    let status = (resp as? HTTPURLResponse)?.statusCode ?? 0
    if (200..<300).contains(status) { return }
    if status == 401 { throw ShareError.signIn(service) }
    if status == 404 { throw ShareError.inboxUnreachable }
    // Both services answer {"error": {"message": …}}.
    struct Answer: Decodable {
      struct Inner: Decodable { let message: String? }
      let error: Inner?
    }
    let message = (try? JSONDecoder().decode(Answer.self, from: data))?.error?.message
    throw ShareError.service(message ?? "\(service): HTTP \(status)")
  }

  /// The name in a service's answer about the new file.
  private func named(_ data: Data, _ resp: URLResponse, service: String) throws -> String {
    try check(data, resp, service: service)
    struct Item: Decodable { let name: String }
    return try JSONDecoder().decode(Item.self, from: data).name
  }
}

/// `name`, or "name (2).ext", "name (3).ext"… if taken.
func freeName(_ name: String, taken: (String) -> Bool) -> String {
  if !taken(name) { return name }
  let url = URL(fileURLWithPath: name)
  let ext = url.pathExtension
  let stem = ext.isEmpty ? name : url.deletingPathExtension().lastPathComponent
  var n = 2
  while true {
    let candidate = ext.isEmpty ? "\(stem) (\(n))" : "\(stem) (\(n)).\(ext)"
    if !taken(candidate) { return candidate }
    n += 1
  }
}

/// Percent-encodes everything but unreserved characters.
private func encode(_ s: String) -> String {
  let allowed = CharacterSet(
    charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
  return s.addingPercentEncoding(withAllowedCharacters: allowed) ?? s
}
