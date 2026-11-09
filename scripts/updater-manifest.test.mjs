import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildManifest, pickAssets } from "./updater-manifest.mjs";

const SCRIPT = fileURLToPath(new URL("./updater-manifest.mjs", import.meta.url));

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
    "https://gh-proxy.org/");
  assert.equal(m.platforms["windows-x86_64-nsis"].url, "https://gh-proxy.org/https://github.com/x");
});

test("pickAssets finds the setup and its exact .sig", () => {
  const { setup, sig } = pickAssets("v1", ["SHA256SUMS.txt", "routa_1_x64-setup.exe", "routa_1_x64-setup.exe.sig"]);
  assert.equal(setup, "routa_1_x64-setup.exe");
  assert.equal(sig, "routa_1_x64-setup.exe.sig");
});

test("pickAssets throws Chinese errors for missing sig or assets", () => {
  assert.throws(() => pickAssets("v1", ["routa_1_x64-setup.exe", "other.exe.sig"]), /找不到.*\.sig/);
  assert.throws(() => pickAssets("v1", []), /找不到.*安装包/);
});

test("pickAssets ignores the offline installer (it bundles WebView2 and is not signed)", () => {
  const { setup } = pickAssets("v1", ["routa_1_x64-offline-setup.exe", "routa_1_x64-setup.exe", "routa_1_x64-setup.exe.sig"]);
  assert.equal(setup, "routa_1_x64-setup.exe");
});

test("CLI writes both manifests into the assets dir from the local files", () => {
  const dir = mkdtempSync(join(tmpdir(), "routa-manifest-"));
  writeFileSync(join(dir, "routa_0.9.0_x64-setup.exe"), "");
  writeFileSync(join(dir, "routa_0.9.0_x64-setup.exe.sig"), "SIGDATA\n");
  writeFileSync(join(dir, "routa_0.9.0_x64-offline-setup.exe"), "");
  const notes = join(dir, "..", `notes-${Date.now()}.md`);
  writeFileSync(notes, "  release notes \n");
  execFileSync(process.execPath, [SCRIPT, dir], {
    env: { ...process.env, GITHUB_REPOSITORY: "c0010c/Routa", GITHUB_REF_NAME: "v0.9.0", RELEASE_NOTES_FILE: notes },
  });
  const direct = JSON.parse(readFileSync(join(dir, "latest.json"), "utf8"));
  const proxy = JSON.parse(readFileSync(join(dir, "latest-proxy.json"), "utf8"));
  const gh = "https://github.com/c0010c/Routa/releases/download/v0.9.0/routa_0.9.0_x64-setup.exe";
  assert.equal(direct.version, "0.9.0");
  assert.equal(direct.notes, "release notes");
  assert.ok(!Number.isNaN(Date.parse(direct.pub_date)));
  assert.equal(direct.platforms["windows-x86_64-nsis"].url, gh);
  assert.equal(direct.platforms["windows-x86_64-nsis"].signature, "SIGDATA");
  assert.equal(proxy.platforms["windows-x86_64-nsis"].url, `https://gh-proxy.org/${gh}`);
});
