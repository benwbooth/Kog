import assert from "node:assert/strict";
import { test } from "node:test";
import { integerToLongTime } from "../src/maki-time.js";

test("Wasabi long time formats signed milliseconds, not seconds", () => {
  for (const [value, expected] of [[0, "0:00:00"], [59999, "0:00:59"],
    [60000, "0:01:00"], [3599999, "0:59:59"], [3600000, "1:00:00"],
    [86400000, "24:00:00"], [-1000, "0:00:-1"], [-3600000, "-1:00:00"]]) {
    assert.equal(integerToLongTime(value), expected);
  }
});
