import Foundation

struct RustString {
  let value: String
  func toString() -> String { value }
}

enum RustBridge {
  struct AlefBridgeError: Error {
    let fields: String
    func message() -> RustString { RustString(value: "Request rejected") }
    func errorType() -> RustString { RustString(value: "ServiceError") }
    func kind() -> RustString { RustString(value: "Rejected") }
    func payload() -> RustString { RustString(value: fields) }
    func status() -> Int32 { 409 }
    func retryAfterMs() -> Int64 { -1 }
  }
}

// GENERATED_ERROR_SURFACE ~keep

for detail in ["null", "\"server detail\""] {
  let fields = ["request_id": "\"request-42\"", "attempt": "7", "recoverable": "true", "detail": detail]
  let json = String(decoding: try JSONEncoder().encode(fields), as: UTF8.self)
  let converted = alefTypedError(RustBridge.AlefBridgeError(fields: json))
  guard let typed = converted as? ServiceError,
  case let .rejected(_, requestId, attempt, recoverable, actualDetail, status) = typed else {
    fatalError("Expected the typed rejected error")
  }
  precondition(requestId == "request-42" && attempt == 7 && recoverable && status == 409)
  precondition(actualDetail == (detail == "null" ? nil : "server detail"))
}

let incomplete = RustBridge.AlefBridgeError(fields: "{}")
precondition(alefTypedError(incomplete) is RustBridge.AlefBridgeError)
