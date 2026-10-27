import type { TrafficTick } from "../bindings";
import { getTrafficHistory, HISTORY_LEN, resetTrafficStore, setTraffic } from "./trafficStore";

const tick = (up: number, down = up): TrafficTick => ({
  total_rate: { up, down },
  per_listener_rate: {},
  totals: { total: { up: 0, down: 0 }, per_listener: {} },
});

describe("traffic history", () => {
  beforeEach(() => resetTrafficStore());

  it("starts empty", () => expect(getTrafficHistory()).toEqual({ up: [], down: [] }));

  it("records a single tick", () => {
    setTraffic(tick(5, 7));
    expect(getTrafficHistory()).toEqual({ up: [5], down: [7] });
  });

  it("keeps only the last HISTORY_LEN points", () => {
    expect(HISTORY_LEN).toBe(60);
    for (let i = 1; i <= 61; i++) setTraffic(tick(i));
    const h = getTrafficHistory();
    expect(h.up).toHaveLength(60);
    expect(h.up[0]).toBe(2);
    expect(h.up[59]).toBe(61);
  });

  it("ignores a null tick", () => {
    setTraffic(tick(1));
    setTraffic(null);
    expect(getTrafficHistory().up).toHaveLength(1);
  });

  it("returns a stable snapshot between pushes", () => {
    setTraffic(tick(1));
    expect(getTrafficHistory()).toBe(getTrafficHistory());
  });
});
