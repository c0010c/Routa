import type { ConnectionInfo } from "../bindings";
import { connectionApp, createConnectionSampler, groupConnections, sumConnections } from "./connections";

const row: ConnectionInfo = {
  id: "c1", listener: "机场", process: "C:\\Apps\\Chrome\\chrome.exe", target: "example.com:443",
  node: "新加坡01", protocol: "TCP", uploaded: 4096, downloaded: 8192,
};

it("calculates rates by connection ID and elapsed sample time, not total lifetime traffic", () => {
  const sampler = createConnectionSampler();
  expect(sampler.sample([row], 1000)[0].uploadRate).toBeNull();
  const next = sampler.sample([{ ...row, uploaded: 7168, downloaded: 14336 }], 2500)[0];
  expect(next.uploadRate).toBe(2048);
  expect(next.downloadRate).toBe(4096);
  expect(sampler.sample([{ ...row, id: "new" }], 4000)[0].uploadRate).toBeNull();
});

it("does not invent speeds after gaps, failed reads, disappearing IDs, or reset counters", () => {
  const sampler = createConnectionSampler();
  sampler.sample([row], 0);
  expect(sampler.sample([row], 11000)[0].uploadRate).toBeNull();
  expect(sampler.sample([{ ...row, uploaded: 0 }], 12000)[0].uploadRate).toBeNull();
  sampler.reset();
  expect(sampler.sample([row], 13000)[0].uploadRate).toBeNull();
  sampler.sample([], 14000);
  expect(sampler.sample([row], 15000)[0].uploadRate).toBeNull();
  expect(sampler.sample([row], 16000)[0].uploadRate).toBe(0);
});

it("groups by full executable path and sums live connections without conflating same-name apps", () => {
  const rows = [row, { ...row, id: "c2", process: "c:/apps/chrome/chrome.exe" },
    { ...row, id: "c3", process: "D:\\Other\\chrome.exe" }, { ...row, id: "c4", process: "" }];
  const live = rows.map((r) => ({ ...r, uploadRate: 1024, downloadRate: 2048 }));
  // Windows path separators are normalized for grouping.
  expect(connectionApp("").name).toBe("未识别应用");
  const groups = groupConnections(live);
  expect(groups.map((g) => g.rows.length)).toEqual([2, 1, 1]);
  expect(sumConnections(groups[0].rows)).toEqual({ uploaded: 8192, downloaded: 16384, uploadRate: 2048, downloadRate: 4096 });
  expect(sumConnections([{ ...live[0], uploadRate: null }]).uploadRate).toBeNull();
});
