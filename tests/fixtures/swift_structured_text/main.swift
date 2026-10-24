import Foundation
import RustBridge
@testable import SampleCore

let raw = RustBridge.sampleMessage()
if ProcessInfo.processInfo.environment["ALEF_STRUCTURED_TEXT_CASE"] == "serialize" {
  let operations: [() throws -> Void] = [
    { _ = try raw.contentValue() },
    { _ = try SampleCore.Message(raw) },
  ]
  for operation in operations {
    do {
      try operation()
      fatalError("expected serializer failure")
    } catch let error as RustBridge.AlefBridgeError {
      let message = error.message().toString()
      precondition(message.contains("serializing structured content") && message.contains("parts unavailable"), message)
    }
  }
  print("structured_text_serialization_error_cases=2")
} else {
  precondition(raw.content()?.toString() == "hello world", "raw text getter changed")
  guard case let .parts(field0: publicParts)? = try raw.contentValue() else { fatalError("typed accessor lost parts") }
  precondition(publicParts.count == 3, "typed accessor changed boundaries")
  let typed = try SampleCore.Message(raw)
  guard case let .parts(field0: parts)? = typed.content else { fatalError("structured payload became text") }
  precondition(parts.count == 3, "part boundaries lost")
  guard case let .image(url: url, active: _, score: _, labels: _, empty: _) = parts[0] else { fatalError("image part lost") }
  precondition(url == "https://example.invalid/image")
  guard case let .text(text: first) = parts[1] else { fatalError("first text part lost") }
  guard case let .text(text: second) = parts[2] else { fatalError("second text part lost") }
  precondition(first == "hello" && second == " world", "adjacent text boundaries lost")
  let absentRaw = RustBridge.absentMessage()
  precondition(absentRaw.content() == nil)
  let absentValue = try absentRaw.contentValue()
  precondition(absentValue == nil)
  let absentTyped = try SampleCore.Message(absentRaw)
  precondition(absentTyped.content == nil)
  print("structured_text_native_cases=8")
}
