import type { TrafficTick } from "../bindings";
import { getTraffic, getTrafficHistory, HISTORY_LEN, resetTrafficStore, setTraffic, subscribeTraffic } from "./trafficStore";

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

  it("updates stop/reset snapshots and notifies readers without changing history", () => {
    const previous = { ...tick(5, 7), totals: { total: { up: 100, down: 200 }, per_listener: {} } };
    setTraffic(previous);
    const history = getTrafficHistory();
    const changed = vi.fn();
    const unsubscribe = subscribeTraffic(changed);
    const stopped = { ...tick(0), totals: previous.totals };
    setTraffic(stopped, false);
    expect(getTraffic()).toEqual(stopped);
    expect(getTrafficHistory()).toBe(history);
    setTraffic(tick(0), false);
    expect(getTraffic()?.totals.total).toEqual({ up: 0, down: 0 });
    expect(getTrafficHistory()).toBe(history);
    expect(changed).toHaveBeenCalledTimes(2);
    unsubscribe();
    setTraffic(tick(3, 4));
    expect(getTrafficHistory()).toEqual({ up: [5, 3], down: [7, 4] });
  });
});
