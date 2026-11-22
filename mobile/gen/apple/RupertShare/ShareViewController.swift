// Rupert in the share sheet: saves the shared documents to Rupert's inbox
// right away (InboxSaver), says where they went, and closes. Anything it
// couldn't save stays in the app group for the app to send later.

import UIKit
import UniformTypeIdentifiers

@objc(ShareViewController)
final class ShareViewController: UIViewController {
  private let settings = Shared.settings()
  private lazy var text = Strings.forLanguage(settings?.language)

  private let card = UIView()
  private let owl = UILabel()
  private let message = UILabel()
  private let spinner = UIActivityIndicatorView(style: .medium)
  private let doneButton = UIButton(type: .system)

  // MARK: Layout

  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = UIColor.black.withAlphaComponent(0.25)

    card.backgroundColor = .systemBackground
    card.layer.cornerRadius = 16
    card.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(card)

    owl.text = "Rupert"
    owl.font = .preferredFont(forTextStyle: .headline)
    message.text = text.saving
    message.font = .preferredFont(forTextStyle: .body)
    message.numberOfLines = 0
    message.textAlignment = .center
    spinner.startAnimating()
    doneButton.setTitle(text.done, for: .normal)
    doneButton.titleLabel?.font = .preferredFont(forTextStyle: .headline)
    doneButton.isHidden = true
    doneButton.addTarget(self, action: #selector(finish), for: .touchUpInside)

    let stack = UIStackView(arrangedSubviews: [owl, spinner, message, doneButton])
    stack.axis = .vertical
    stack.alignment = .center
    stack.spacing = 12
    stack.translatesAutoresizingMaskIntoConstraints = false
    card.addSubview(stack)

    NSLayoutConstraint.activate([
      card.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      card.centerYAnchor.constraint(equalTo: view.centerYAnchor),
      card.widthAnchor.constraint(equalTo: view.widthAnchor, multiplier: 0.85),
      stack.topAnchor.constraint(equalTo: card.topAnchor, constant: 20),
      stack.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -20),
      stack.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 20),
      stack.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -20),
    ])

    Task { await run() }
  }

  @objc private func finish() {
    extensionContext?.completeRequest(returningItems: nil)
  }

  private func show(_ words: String, closeSoon: Bool) {
    spinner.stopAnimating()
    spinner.isHidden = true
    message.text = words
    doneButton.isHidden = closeSoon
    if closeSoon {
      DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in self?.finish() }
    }
  }

  // MARK: Saving

  private func run() async {
    let items = (extensionContext?.inputItems as? [NSExtensionItem]) ?? []
    let providers = items.flatMap { $0.attachments ?? [] }
    var files: [URL] = []
    var skipped = 0
    for provider in providers {
      if let file = try? await load(provider) {
        files.append(file)
      } else {
        skipped += 1
      }
    }
    defer {
      for file in files { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
    }
    if files.isEmpty {
      show(skipped > 0 ? text.notDocuments : text.nothing, closeSoon: false)
      return
    }

    // Keep each one first, so nothing is lost if saving fails or stops.
    var kept: [URL] = []
    for file in files {
      if let copy = try? Shared.keep(file, name: file.lastPathComponent) { kept.append(copy) }
    }
    guard let settings = settings, settings.inbox != nil, let saver = try? InboxSaver(settings: settings)
    else {
      show(text.noInbox, closeSoon: false)
      return
    }

    var saved: [String] = []
    var problem: Error?
    for copy in kept {
      do {
        saved.append(try await saver.save(copy))
        Shared.forget(copy)
      } catch {
        problem = problem ?? error
      }
    }

    var words: [String] = []
    if saved.count == 1 {
      words.append(text.savedOne(saved[0], saver.inboxDisplayName))
    } else if saved.count > 1 {
      words.append(text.savedMany(saved.count, saver.inboxDisplayName))
    }
    if let problem = problem {
      words.append(text.kept(text.reason(problem)))
    } else if kept.count < files.count {
      words.append(text.kept(text.inboxUnreachable))
    }
    if skipped > 0 { words.append(text.notDocuments) }
    let allSaved = problem == nil && skipped == 0 && saved.count == files.count
    show(words.joined(separator: "\n\n"), closeSoon: allSaved)
  }

  /// Copies one shared document to a temporary folder, keeping its name.
  /// Fails for anything that isn't a kind of document Rupert shows.
  private func load(_ provider: NSItemProvider) async throws -> URL {
    // The first of its types that is a document kind, by file extension.
    let type = provider.registeredTypeIdentifiers.compactMap { UTType($0) }.first { type in
      type.conforms(to: .data)
        && (type.preferredFilenameExtension.map { documentExtensions.contains($0.lowercased()) }
          ?? false)
    }
    guard let type = type else { throw ShareError.service("not a document") }
    // The suggested name is read now: the provider isn't for other threads.
    let suggested = provider.suggestedName
    return try await withCheckedThrowingContinuation { done in
      _ = provider.loadFileRepresentation(forTypeIdentifier: type.identifier) { url, error in
        guard let url = url else {
          done.resume(throwing: error ?? ShareError.service("nothing to load"))
          return
        }
        do {
          let ext = type.preferredFilenameExtension ?? url.pathExtension
          var name = suggested ?? url.deletingPathExtension().lastPathComponent
          name = name.replacingOccurrences(of: "/", with: "-")
          if !name.lowercased().hasSuffix(".\(ext.lowercased())") { name += ".\(ext)" }
          // The loaded file is removed once this returns: copy it now.
          let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
          try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
          let copy = dir.appendingPathComponent(name)
          try FileManager.default.copyItem(at: url, to: copy)
          done.resume(returning: copy)
        } catch {
          done.resume(throwing: error)
        }
      }
    }
  }
}
