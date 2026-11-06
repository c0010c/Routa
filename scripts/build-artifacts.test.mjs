import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { treeHash, assertMatch, serviceExe, verifyService, sha256, protocolVersion, readBuildInfo } from "./build-artifacts.mjs";

test("source fingerprint catches same-size edits and added files, ignoring generated receipts", () => {
  const dir = mkdtempSync(join(tmpdir(), "routa-build-test-"));
  try {
    mkdirSync(join(dir, "nested"));
    writeFileSync(join(dir, "nested/a.rs"), "old");
    const initial = treeHash(dir, ["."]);
    writeFileSync(join(dir, "nested/a.rs"), "new");
    const modified = treeHash(dir, ["."]);
    assert.notEqual(initial, modified);
    writeFileSync(join(dir, "receipt.json"), "{}");
    assert.equal(treeHash(dir, ["."], ["receipt.json"]), modified);
    writeFileSync(join(dir, "nested/b.rs"), "another input");
    assert.notEqual(treeHash(dir, ["."], ["receipt.json"]), modified);
    assert.throws(() => assertMatch(initial, modified, "stale service"), /stale service/);
    const oldGui = join(dir, "old-routa.exe");
    writeFileSync(oldGui, "old GUI with no informational CLI");
    assert.throws(() => readBuildInfo(oldGui), /构建信息缺失/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test("service verification rejects stale sources, overwritten EXEs, and mismatched protocols", () => {
  const dir = mkdtempSync(join(tmpdir(), "routa-service-test-"));
  try {
    const receiptPath = join(dir, "receipt.json");
    const receipt = { sourceSha256: "current source", exeSha256: sha256(serviceExe) };
    writeFileSync(receiptPath, JSON.stringify(receipt));
    const options = { receiptPath, exe: serviceExe, sourceHash: receipt.sourceSha256, protocol: protocolVersion() };
    assert.deepEqual(verifyService(options), receipt);
    assert.throws(() => verifyService({ ...options, sourceHash: "new protocol source" }), /源码已变化/);
    writeFileSync(receiptPath, JSON.stringify({ ...receipt, exeSha256: "old executable hash" }));
    assert.throws(() => verifyService(options), /EXE 与构建记录不一致/);
    writeFileSync(receiptPath, JSON.stringify(receipt));
    assert.throws(() => verifyService({ ...options, protocol: protocolVersion() + 1 }), /协议版本与 GUI 源码不一致/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
