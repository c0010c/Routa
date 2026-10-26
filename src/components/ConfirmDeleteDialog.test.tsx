import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ConfirmDeleteDialog } from "./ConfirmDeleteDialog";

describe("ConfirmDeleteDialog", () => {
  it("shows a DanglingRef error inside the dialog and stays open", async () => {
    const onClose = vi.fn();
    const onConfirm = vi.fn().mockRejectedValue({
      kind: "validation",
      errors: [{ DanglingRef: { what: "Listener「办公」" } }],
    });
    render(<ConfirmDeleteDialog text="确定删除？" onConfirm={onConfirm} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("办公");
    expect(onClose).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByRole("button", { name: "删除" })).toBeEnabled());
  });

  it("closes after a successful delete", async () => {
    const onClose = vi.fn();
    render(<ConfirmDeleteDialog text="确定删除？" onConfirm={async () => {}} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
});
