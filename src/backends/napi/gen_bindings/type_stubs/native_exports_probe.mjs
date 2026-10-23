import assert from "node:assert/strict";
import { createRequire } from "node:module";
import * as namespace from "@sample/native";
import { JsMode, JsWorker, Mode, Worker, settingsDefault, streamItems } from "@sample/native";

const common = createRequire(import.meta.url)("@sample/native");
const expected = { JsMode, JsWorker, Mode, Worker, settingsDefault, streamItems };
assert.equal(Object.keys(expected).length, 6);
assert.equal(Object.keys(common).length, 6);
for (const [name, value] of Object.entries(expected)) {
  assert.ok(Object.hasOwn(namespace, name));
  assert.equal(value, common[name]);
}
assert.equal(JsMode, undefined);
assert.equal(JsWorker, undefined);
assert.deepEqual(streamItems(), [1, 2]);
assert.deepEqual(settingsDefault(), { enabled: true });
