class Worker {}
const nativeBinding = {
  Mode: Object.freeze({ Ready: 0 }),
  JsMode: undefined,
  Worker,
  JsWorker: undefined,
  settingsDefault: () => ({ enabled: true }),
  streamItems: () => [1, 2],
};
module.exports = require("./binding.cjs")(nativeBinding);
