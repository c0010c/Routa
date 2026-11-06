import type { ConnectionInfo } from "../bindings";

export type LiveConnection = ConnectionInfo & {
  uploadRate: number | null;
  downloadRate: number | null;
};

/** Rates require two successful samples of the same live connection. */
export function createConnectionSampler() {
  let previous: { at: number; rows: Map<string, ConnectionInfo> } | null = null;
  return {
    reset() { previous = null; },
    sample(rows: ConnectionInfo[], at: number): LiveConnection[] {
      const seconds = previous ? (at - previous.at) / 1000 : 0;
      const result = rows.map((row) => {
        const before = previous?.rows.get(row.id);
        // A long gap, reconnect, or counter reset starts a fresh baseline.
        const valid = !!before && seconds > 0 && seconds <= 10 &&
          row.uploaded >= before.uploaded && row.downloaded >= before.downloaded;
        return {
          ...row,
          uploadRate: valid ? (row.uploaded - before!.uploaded) / seconds : null,
          downloadRate: valid ? (row.downloaded - before!.downloaded) / seconds : null,
        };
      });
      previous = { at, rows: new Map(rows.map((row) => [row.id, row])) };
      return result;
    },
  };
}

export function connectionApp(process: string) {
  const path = process.trim();
  return {
    key: path.replace(/\\/g, "/").toLowerCase(),
    name: path.split(/[\\/]/).pop() || "未识别应用",
  };
}

export function groupConnections(rows: LiveConnection[]) {
  const groups = new Map<string, { key: string; name: string; rows: LiveConnection[] }>();
  for (const row of rows) {
    const app = connectionApp(row.process);
    if (!groups.has(app.key)) groups.set(app.key, { ...app, rows: [] });
    groups.get(app.key)!.rows.push(row);
  }
  return [...groups.values()];
}

export function sumConnections(rows: LiveConnection[]) {
  return {
    uploaded: rows.reduce((sum, row) => sum + row.uploaded, 0),
    downloaded: rows.reduce((sum, row) => sum + row.downloaded, 0),
    uploadRate: rows.every((row) => row.uploadRate !== null)
      ? rows.reduce((sum, row) => sum + row.uploadRate!, 0) : null,
    downloadRate: rows.every((row) => row.downloadRate !== null)
      ? rows.reduce((sum, row) => sum + row.downloadRate!, 0) : null,
  };
}
