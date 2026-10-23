const assert = require("node:assert/strict");
const { fail } = require("./pkg/error_payload_probe.js");
const metadata = process.env.ALEF_EXPECT_PAYLOAD_METADATA === "1";
for (const unit of [false, true]) {
  assert.throws(
    () => fail(unit),
    (error) => {
      assert.ok(error instanceof Error);
      assert.equal(
        error.message,
        unit ? "empty operation" : "service_failure: blocked: raw payload",
        "Display text must survive",
      );
      assert.equal(error.name, "Error");
      assert.equal(error.constructor, Error);
      assert.equal(typeof error.toString, "function");
      assert.equal(typeof error.stack, "string");
      assert.equal(error.cause, undefined);
      assert.equal(error.code, unit ? "empty" : "failure");
      assert.equal(error.kind, unit ? "Empty" : "Failure");
      assert.equal(error.statusCode, 429);
      assert.equal(typeof error.errorCode, "number");
      const payload = error[Symbol.for("alef.error.payload")];
      assert.equal(Object.getPrototypeOf(payload), null);
      if (metadata) assert.equal(error.payload, 37);
      else assert.equal(error.payload, payload);
      if (!unit) {
        assert.deepEqual(
          Object.keys(payload).sort(),
          [
            "message",
            "name",
            "code",
            "kind",
            "errorCode",
            "statusCode",
            "stack",
            "cause",
            "constructor",
            "toString",
            "payload",
            "obj",
            "message_value",
            "details",
          ].sort(),
        );
        assert.equal(payload.message, "raw payload");
        assert.equal(payload.code, "payload code");
        assert.equal(payload.kind, "payload kind");
        assert.equal(payload.errorCode, "payload numeric code");
        assert.equal(payload.statusCode, "payload status");
        assert.equal(payload.name, "payload name");
        assert.equal(payload.stack, "payload stack");
        assert.equal(payload.cause, "payload cause");
        assert.equal(payload.constructor, "payload constructor");
        assert.equal(payload.toString, "payload stringifier");
        assert.equal(payload.payload, "nested raw payload");
        assert.equal(payload.obj, "raw obj");
        assert.equal(payload.message_value, "raw message_value");
        assert.equal(error.obj, "raw obj");
        assert.equal(error.message_value, "raw message_value");
        assert.equal(error.details, "ordinary detail");
      }
      return true;
    },
  );
}
process.stdout.write("verified 2 thrown errors and 14 payload fields\n");
