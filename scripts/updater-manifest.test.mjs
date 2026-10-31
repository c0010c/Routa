import { test } from "node:test";
import assert from "node:assert/strict";
import { buildManifest } from "./updater-manifest.mjs";

test("manifest points both platform keys at the setup asset", () => {
  const m = buildManifest({ tag: "v0.4.0", notes: "n", publishedAt: "2026-11-02T00:00:00Z",
    setupUrl: "https://github.com/c0010c/Routa/releases/download/v0.4.0/routa_0.4.0_x64-setup.exe", signature: "SIG" });
  assert.equal(m.version, "0.4.0");
  assert.equal(m.notes, "n");
  assert.equal(m.pub_date, "2026-11-02T00:00:00Z");
  assert.deepEqual(m.platforms["windows-x86_64-nsis"], m.platforms["windows-x86_64"]);
  assert.equal(m.platforms["windows-x86_64-nsis"].signature, "SIG");
});

test("proxy manifest prefixes the mirror", () => {
  const m = buildManifest({ tag: "v0.4.0", notes: "", publishedAt: "", setupUrl: "https://github.com/x", signature: "S" },
    "https://update.hwdns.net/");
  assert.equal(m.platforms["windows-x86_64-nsis"].url, "https://update.hwdns.net/https://github.com/x");
});
