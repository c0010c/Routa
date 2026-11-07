import { act, render, screen } from "@testing-library/react";
import { useEffect } from "react";
import { ApplyResultProvider, useApplyResult } from "./ApplyResultSnackbar";

function Show({ text, severity }: { text: string; severity: "success" | "error" | "info" }) {
  const { showMessage } = useApplyResult();
  useEffect(() => showMessage(text, severity), [showMessage, text, severity]);
  return null;
}

describe("ApplyResultProvider", () => {
  afterEach(() => vi.useRealTimers());

  it("keeps an info message longer than a success and breaks its lines", () => {
    vi.useFakeTimers();
    render(
      <ApplyResultProvider>
        <Show text={"第一行\n第二行"} severity="info" />
      </ApplyResultProvider>,
    );
    const alert = screen.getByText(/第一行/).closest(".MuiAlert-root")!;
    expect(alert).toHaveClass("MuiAlert-colorInfo");
    expect(alert).toHaveStyle({ whiteSpace: "pre-line" });
    act(() => vi.advanceTimersByTime(5000));
    expect(screen.getByText(/第一行/)).toBeInTheDocument();
    act(() => vi.advanceTimersByTime(6000));
    expect(screen.queryByText(/第一行/)).not.toBeInTheDocument();
  });
});
