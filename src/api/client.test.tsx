import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(async () => ({})),
    getStatus: vi.fn(async () => ({})),
    resetTraffic: vi.fn(),
    saveListener: vi.fn(),
    testDelays: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import type { DelayResult } from "../bindings";
import { useResetTraffic, useSaveListener, useTestDelays } from "./client";

function Probe() {
  const reset = useResetTraffic();
  const save = useSaveListener();
  return (
    <>
      <button onClick={() => reset.mutate()}>reset</button>
      <button onClick={() => save.mutate({} as never)}>save</button>
    </>
  );
}

function renderProbe(qc = new QueryClient()) {
  render(
    <QueryClientProvider client={qc}>
      <ApplyResultProvider>
        <Probe />
      </ApplyResultProvider>
    </QueryClientProvider>,
  );
}

describe("mutation error reporting", () => {
  it("shows string rejections verbatim", async () => {
    vi.mocked(commands.resetTraffic).mockRejectedValue("core 未运行");
    renderProbe();
    fireEvent.click(screen.getByText("reset"));
    expect(await screen.findByText("core 未运行")).toBeInTheDocument();
  });

  it("shows non-validation failures in the snackbar", async () => {
    vi.mocked(commands.resetTraffic).mockResolvedValue({ status: "error", error: null });
    renderProbe();
    fireEvent.click(screen.getByText("reset"));
    expect(await screen.findByText("操作失败")).toBeInTheDocument();
  });

  it("shows validation errors from fire-and-forget mutate as a fallback, without invalidating", async () => {
    vi.mocked(commands.saveListener).mockResolvedValue({
      status: "error",
      error: [{ DanglingRef: { what: "x" } }],
    });
    const qc = new QueryClient();
    const spy = vi.spyOn(qc, "invalidateQueries");
    renderProbe(qc);
    fireEvent.click(screen.getByText("save"));
    await waitFor(() => expect(commands.saveListener).toHaveBeenCalled());
    // fallback snackbar proves onError ran
    expect(await screen.findByText(/仍被引用/)).toBeInTheDocument();
    expect(spy).not.toHaveBeenCalled();
    expect(screen.queryByText("操作失败")).not.toBeInTheDocument();
  });
});

function DelayProbe({ results }: { results: Record<string, DelayResult | undefined> }) {
  const test = useTestDelays();
  return (
    <>
      <button onClick={() => test.mutate(["a", "b"])}>measure</button>
      {["a", "b", "c"].map((n) => <span key={n}>{`${n}:${test.waiting(n, results[n])}`}</span>)}
    </>
  );
}

describe("useTestDelays.waiting", () => {
  it("stops waiting for a node once this run's result lands, while the batch is still running", async () => {
    vi.mocked(commands.testDelays).mockImplementation(() => new Promise(() => {}));
    const old: DelayResult = { ms: 90, tested_at: 1, error: null };
    const qc = new QueryClient();
    const view = (results: Record<string, DelayResult | undefined>) => (
      <QueryClientProvider client={qc}>
        <ApplyResultProvider>
          <DelayProbe results={results} />
        </ApplyResultProvider>
      </QueryClientProvider>
    );
    const { rerender } = render(view({ a: old }));
    expect(screen.getByText("a:false")).toBeInTheDocument();
    fireEvent.click(screen.getByText("measure"));
    // An earlier result does not count; nodes outside the batch never wait.
    expect(await screen.findByText("a:true")).toBeInTheDocument();
    expect(screen.getByText("b:true")).toBeInTheDocument();
    expect(screen.getByText("c:false")).toBeInTheDocument();
    const fresh: DelayResult = { ms: 50, tested_at: Math.floor(Date.now() / 1000), error: null };
    rerender(view({ a: fresh, b: { ...fresh, stale: true } }));
    expect(screen.getByText("a:false")).toBeInTheDocument();
    // A stale result is not this run's answer.
    expect(screen.getByText("b:true")).toBeInTheDocument();
  });
});
