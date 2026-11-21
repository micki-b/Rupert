// Rupert's bridge to Apple Intelligence: Apple's on-device language model
// (the Foundation Models framework, macOS 26 or later on Apple silicon with
// Apple Intelligence turned on). Built into a static library by
// rupert-core's build.rs and called from `apple_ai.rs`.
//
// The C interface takes and returns JSON (UTF-8, allocated here; the caller
// frees it with `rupert_ai_free`). Calls block until the model answers, so
// Rust calls them from worker threads, never the main thread.
//
// Weak-linked: Rupert runs on macOS 13, where the framework doesn't exist.
// Everything that touches it is behind `#available(macOS 26.0, *)`.

import Foundation
@_weakLinked import FoundationModels

// MARK: - C interface

/// `{"available": true, "contextSize": 8192, "languages": ["en", …]}` or
/// `{"available": false, "reason": "deviceNotEligible" | "appleIntelligenceNotEnabled" | "modelNotReady" | "systemTooOld"}`.
@_cdecl("rupert_ai_status")
public func rupert_ai_status() -> UnsafeMutablePointer<CChar>? {
  reply(status())
}

/// Request: `{"instructions": "…", "prompt": "…", "schema": {…JSON schema…},
/// "temperature": 0.1, "maxTokens": 1000}` (all but `prompt` optional).
/// Answer: `{"content": "…"}` (JSON text when a schema was given) or
/// `{"error": "<code>", "message": "…"}`.
@_cdecl("rupert_ai_generate")
public func rupert_ai_generate(_ request: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
  guard
    let data = String(cString: request).data(using: .utf8),
    let req = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
  else {
    return reply(["error": "badRequest", "message": "The request isn't JSON."])
  }
  guard #available(macOS 26.0, iOS 26.0, *) else {
    return reply(["error": "unavailable", "message": "Apple Intelligence needs macOS 26 or later."])
  }
  return reply(blocking { await withRetries { await generate(req) } })
}

@_cdecl("rupert_ai_free")
public func rupert_ai_free(_ pointer: UnsafeMutablePointer<CChar>?) {
  free(pointer)
}

/// Called for a tool the model uses: (context, tool name, arguments as
/// JSON) → the result as text, allocated with malloc (freed here).
public typealias RupertToolCallback = @convention(c) (
  UnsafeMutableRawPointer?, UnsafePointer<CChar>, UnsafePointer<CChar>
) -> UnsafeMutablePointer<CChar>?

/// A conversation turn with tools. Request: `{"instructions": "…",
/// "prompt": "…", "tools": [{"name", "description", "schema"}],
/// "temperature", "maxTokens"}`. Answer: `{"content": "…"}` or
/// `{"error", "message"}`. Tools run through `callback` with `context`, on
/// other threads but one at a time, while this call blocks.
@_cdecl("rupert_ai_chat")
public func rupert_ai_chat(
  _ request: UnsafePointer<CChar>,
  _ context: UnsafeMutableRawPointer?,
  _ callback: RupertToolCallback
) -> UnsafeMutablePointer<CChar>? {
  guard
    let data = String(cString: request).data(using: .utf8),
    let req = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
  else {
    return reply(["error": "badRequest", "message": "The request isn't JSON."])
  }
  guard #available(macOS 26.0, iOS 26.0, *) else {
    return reply(["error": "unavailable", "message": "Apple Intelligence needs macOS 26 or later."])
  }
  let tools = Callback(context: context, function: callback)
  // Retry only before any tool ran, so no step happens twice.
  return reply(blocking { await withRetries(when: { tools.calls == 0 }) { await chat(req, tools) } })
}

// MARK: - Status

func status() -> [String: Any] {
  guard #available(macOS 26.0, iOS 26.0, *) else {
    return ["available": false, "reason": "systemTooOld"]
  }
  let model = SystemLanguageModel.default
  switch model.availability {
  case .available:
    let languages = Set(model.supportedLanguages.compactMap { $0.languageCode?.identifier })
    return ["available": true, "contextSize": model.contextSize, "languages": languages.sorted()]
  case .unavailable(let why):
    let reason: String
    switch why {
    case .deviceNotEligible: reason = "deviceNotEligible"
    case .appleIntelligenceNotEnabled: reason = "appleIntelligenceNotEnabled"
    case .modelNotReady: reason = "modelNotReady"
    @unknown default: reason = "unknown"
    }
    return ["available": false, "reason": reason]
  }
}

// MARK: - Generating

@available(macOS 26.0, iOS 26.0, *)
func generate(_ req: [String: Any]) async -> [String: Any] {
  guard let prompt = req["prompt"] as? String else {
    return ["error": "badRequest", "message": "No prompt."]
  }
  let model = SystemLanguageModel(useCase: .general, guardrails: .permissiveContentTransformations)
  let session = LanguageModelSession(model: model, instructions: req["instructions"] as? String)
  var options = GenerationOptions()
  options.temperature = req["temperature"] as? Double
  options.maximumResponseTokens = req["maxTokens"] as? Int
  do {
    if let schema = req["schema"] as? [String: Any] {
      let root = try GenerationSchema(root: dynamicSchema(schema, name: "Answer"), dependencies: [])
      let response = try await session.respond(to: prompt, schema: root, options: options)
      return ["content": response.content.jsonString]
    }
    let response = try await session.respond(to: prompt, options: options)
    return ["content": response.content]
  } catch let error as LanguageModelSession.GenerationError {
    return ["error": code(error), "message": error.localizedDescription]
  } catch {
    return ["error": otherCode(error), "message": error.localizedDescription]
  }
}

/// Errors outside GenerationError. The content check fails like this
/// (SensitiveContentAnalysisML error 15) when the Mac is busy with other
/// models; it passes again a moment later.
func otherCode(_ error: Error) -> String {
  (error as NSError).domain.contains("SensitiveContentAnalysis") ? "busy" : "other"
}

/// Runs `work` again, up to twice, after a short pause, when it failed for
/// a passing reason and `when` allows it.
func withRetries(
  when allowed: () -> Bool = { true }, _ work: () async -> [String: Any]
) async -> [String: Any] {
  var result: [String: Any] = [:]
  for attempt in 1...3 {
    result = await work()
    guard
      let code = result["error"] as? String,
      ["busy", "rateLimited", "concurrentRequests"].contains(code),
      attempt < 3, allowed()
    else { return result }
    try? await Task.sleep(nanoseconds: UInt64(attempt) * 1_500_000_000)
  }
  return result
}

@available(macOS 26.0, iOS 26.0, *)
func code(_ error: LanguageModelSession.GenerationError) -> String {
  switch error {
  case .exceededContextWindowSize: return "contextWindow"
  case .assetsUnavailable: return "assetsUnavailable"
  case .guardrailViolation: return "guardrail"
  case .unsupportedGuide: return "unsupportedGuide"
  case .unsupportedLanguageOrLocale: return "unsupportedLanguage"
  case .decodingFailure: return "decodingFailure"
  case .rateLimited: return "rateLimited"
  case .concurrentRequests: return "concurrentRequests"
  case .refusal: return "refusal"
  @unknown default: return "other"
  }
}

// MARK: - Chat with tools

/// Rust's tool callback, called one at a time.
final class Callback: @unchecked Sendable {
  let context: UnsafeMutableRawPointer?
  let function: RupertToolCallback
  let lock = NSLock()
  /// Tool calls so far.
  private(set) var calls = 0

  init(context: UnsafeMutableRawPointer?, function: RupertToolCallback) {
    self.context = context
    self.function = function
  }

  func call(_ name: String, _ arguments: String) -> String {
    lock.lock()
    defer { lock.unlock() }
    calls += 1
    let out = name.withCString { n in arguments.withCString { a in function(context, n, a) } }
    guard let out else { return "" }
    defer { free(out) }
    return String(cString: out)
  }
}

/// A tool described by Rust (name, description, JSON schema) and run by it.
@available(macOS 26.0, iOS 26.0, *)
struct BridgeTool: Tool {
  typealias Arguments = GeneratedContent
  typealias Output = String
  let name: String
  let description: String
  let parameters: GenerationSchema
  let callback: Callback

  func call(arguments: GeneratedContent) async throws -> String {
    callback.call(name, arguments.jsonString)
  }
}

@available(macOS 26.0, iOS 26.0, *)
func chat(_ req: [String: Any], _ callback: Callback) async -> [String: Any] {
  guard let prompt = req["prompt"] as? String else {
    return ["error": "badRequest", "message": "No prompt."]
  }
  do {
    var tools: [any Tool] = []
    for spec in req["tools"] as? [[String: Any]] ?? [] {
      let name = spec["name"] as? String ?? "tool"
      let schema = spec["schema"] as? [String: Any] ?? ["type": "object"]
      tools.append(
        BridgeTool(
          name: name,
          description: spec["description"] as? String ?? "",
          parameters: try GenerationSchema(root: dynamicSchema(schema, name: name), dependencies: []),
          callback: callback
        ))
    }
    let session = LanguageModelSession(
      model: SystemLanguageModel.default, tools: tools, instructions: req["instructions"] as? String)
    var options = GenerationOptions()
    options.temperature = req["temperature"] as? Double
    options.maximumResponseTokens = req["maxTokens"] as? Int
    let response = try await session.respond(to: prompt, options: options)
    return ["content": response.content]
  } catch let error as LanguageModelSession.GenerationError {
    return ["error": code(error), "message": error.localizedDescription]
  } catch {
    return ["error": otherCode(error), "message": error.localizedDescription]
  }
}

// MARK: - Schemas

/// A schema from the subset of JSON Schema Rupert uses: objects (properties
/// in the order of `required`, then the rest), strings (with `enum`),
/// integers, numbers, booleans and arrays (`minItems`/`maxItems`).
@available(macOS 26.0, iOS 26.0, *)
func dynamicSchema(_ s: [String: Any], name: String) -> DynamicGenerationSchema {
  let description = s["description"] as? String
  if let choices = s["enum"] as? [String] {
    return DynamicGenerationSchema(name: name, description: description, anyOf: choices)
  }
  switch s["type"] as? String {
  case "object":
    let properties = s["properties"] as? [String: Any] ?? [:]
    let required = s["required"] as? [String] ?? []
    let order = required.filter { properties[$0] != nil }
      + properties.keys.filter { !required.contains($0) }.sorted()
    return DynamicGenerationSchema(
      name: name,
      description: description,
      properties: order.map { key in
        DynamicGenerationSchema.Property(
          name: key,
          description: (properties[key] as? [String: Any])?["description"] as? String,
          schema: dynamicSchema(properties[key] as? [String: Any] ?? [:], name: "\(name)_\(key)"),
          isOptional: !required.contains(key)
        )
      }
    )
  case "array":
    return DynamicGenerationSchema(
      arrayOf: dynamicSchema(s["items"] as? [String: Any] ?? [:], name: "\(name)_item"),
      minimumElements: s["minItems"] as? Int,
      maximumElements: s["maxItems"] as? Int
    )
  case "integer": return DynamicGenerationSchema(type: Int.self)
  case "number": return DynamicGenerationSchema(type: Double.self)
  case "boolean": return DynamicGenerationSchema(type: Bool.self)
  default: return DynamicGenerationSchema(type: String.self)
  }
}

// MARK: - Helpers

/// Runs async work to completion on a background task, blocking the caller.
func blocking(_ work: @escaping @Sendable () async -> [String: Any]) -> [String: Any] {
  final class Box: @unchecked Sendable { var value: [String: Any] = [:] }
  let box = Box()
  let done = DispatchSemaphore(value: 0)
  Task.detached {
    box.value = await work()
    done.signal()
  }
  done.wait()
  return box.value
}

func reply(_ object: [String: Any]) -> UnsafeMutablePointer<CChar>? {
  let data = (try? JSONSerialization.data(withJSONObject: object)) ?? Data("{}".utf8)
  return strdup(String(decoding: data, as: UTF8.self))
}
