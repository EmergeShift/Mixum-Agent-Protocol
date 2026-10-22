// Schema conformance: envelope.schema.json must accept the valid vectors and
// reject node values that violate the 1..=64 / no-colon constraint.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import Ajv2020 from "ajv/dist/2020.js";

const root = new URL("../../../", import.meta.url);
const schema = JSON.parse(readFileSync(new URL("schemas/envelope.schema.json", root), "utf8"));
const vectors = JSON.parse(
  readFileSync(new URL("vectors/envelope/envelope-examples.json", root), "utf8"),
);
const ajv = new Ajv2020({ allErrors: true, strict: false });
const validate = ajv.compile(schema);

test("all valid envelope vectors satisfy the schema", () => {
  for (const vector of vectors.valid) {
    assert.equal(
      validate(vector.envelope),
      true,
      `${vector.name}: ${ajv.errorsText(validate.errors)}`,
    );
  }
});

test("node empty, too long or containing a colon is rejected", () => {
  const base = vectors.valid[0].envelope;
  const cases = [
    { name: "single character", node: "a", ok: true },
    { name: "64 characters", node: "n".repeat(64), ok: true },
    { name: "empty", node: "", ok: false },
    { name: "65 characters", node: "n".repeat(65), ok: false },
    { name: "colon", node: "mac:mini", ok: false },
    { name: "non-string", node: 42, ok: false },
  ];
  for (const item of cases) {
    const ok = validate({ ...base, node: item.node });
    assert.equal(ok, item.ok, `${item.name}: ${ajv.errorsText(validate.errors)}`);
  }
});

test("invalid node vectors are rejected", () => {
  for (const vector of vectors.invalid.filter((item) => "node" in item.envelope)) {
    assert.equal(validate(vector.envelope), false, vector.name);
  }
});
