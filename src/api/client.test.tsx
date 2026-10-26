import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("../bindings", () => ({
  commands: {
    getModel: vi.fn(async () => ({})),
    getStatus: vi.fn(async () => ({})),
    resetTraffic: vi.fn(),
    saveListener: vi.fn(),
  },
}));

import { commands } from "../bindings";
import { ApplyResultProvider } from "../components/ApplyResultSnackbar";
import { useResetTraffic, useSaveListener } from "./client";

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
