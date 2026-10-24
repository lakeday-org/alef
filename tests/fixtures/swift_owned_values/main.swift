import Foundation
import RustBridge
@testable import SampleCore

func retainedEntry() -> RustBridge.Entry {
  let source = RustBridge.sampleContainer()
  return source.entries()[0]
}
func retainedString() -> RustString {
  let source = RustBridge.sampleContainer()
  return source.labels()[0]
}
let entry = retainedEntry()
let string = retainedString()
precondition(RustBridge.entryLabel(entry).toString() == "first")
precondition(string.toString() == "alpha")
precondition(RustBridge.entryLabel(RustBridge.sampleContainer().entries()[0]).toString() == "first")
precondition(RustBridge.sampleContainer().labels()[1].toString() == "beta")
precondition(RustBridge.entryLabel(RustBridge.sampleEntries()[1]).toString() == "second")
precondition(RustBridge.sampleLabels()[0].toString() == "alpha")
let factory = RustBridge.sampleFactory()
precondition(RustBridge.entryLabel(RustBridge.factoryEntries(factory)[0]).toString() == "first")
precondition(RustBridge.factoryLabels(factory)[1].toString() == "beta")
precondition(RustBridge.sampleSourceLabels()[0].toString() == "alpha")
let input = RustVec<RustString>()
input.push(value: RustString("middle"))
let forwarded = RustBridge.forwardStrings(RustString("prefix"), RustString("suffix"), input)
precondition(forwarded.map { $0.toString() } == ["prefix", "middle", "suffix"])
let fallible = try RustBridge.fallibleEntries(false)
precondition(RustBridge.entryLabel(fallible[0]).toString() == "first")
do {
  _ = try RustBridge.fallibleEntries(true)
  fatalError("expected typed bridge error")
} catch let error as RustBridge.AlefBridgeError {
  precondition(error.message().toString().contains("entries unavailable"))
}
let asynchronous = try await RustBridge.asyncEntries()
precondition(RustBridge.entryLabel(asynchronous[1]).toString() == "second")
let projectedEntries = RustBridge.sampleContainer().maybeEntries().toString()
let projectedLabels = RustBridge.sampleContainer().maybeLabels().toString()
let decodedEntries = try JSONSerialization.jsonObject(with: Data(projectedEntries.utf8)) as? [[String: String]]
precondition(decodedEntries?[1]["label"] == "second")
let decodedLabels = try JSONSerialization.jsonObject(with: Data(projectedLabels.utf8)) as? [String]
precondition(decodedLabels?[1] == "beta")
let existingField = RustBridge.sampleContainer().labelsRustVec()
let existingScalar = RustBridge.sampleLabelsRustVec()
precondition(existingField.toString() == "existing field")
precondition(existingScalar.toString() == "existing scalar")
precondition(SampleCore.sampleEntries()[0].label() == "first")
precondition(SampleCore.sampleLabels()[1] == "beta")
precondition(SampleCore.sampleFactory().entries()[0].label() == "first")
let facadeFallible = try SampleCore.fallibleEntries(fail: false)
precondition(facadeFallible[0].label() == "first")
let facadeAsync = try await SampleCore.asyncEntries()
precondition(facadeAsync[1].label() == "second")
let decodedFacade = try SampleCore.entryFromJson("{\"label\":\"decoded\"}")
precondition(decodedFacade.label() == "decoded")
print("owned_swift_native_cases=23")
