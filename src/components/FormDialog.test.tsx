import { fireEvent, render, screen } from "@testing-library/react";
import { Button, DialogActions, DialogContent, TextField } from "@mui/material";
import { FormDialog } from "./FormDialog";

function renderForm() {
  const onClose = vi.fn();
  render(
    <FormDialog onClose={onClose}>
      <DialogContent>
        <TextField label="名称" />
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
      </DialogActions>
    </FormDialog>,
  );
  return onClose;
}

const esc = () => fireEvent.keyDown(screen.getAllByRole("dialog")[0], { key: "Escape" });

describe("FormDialog", () => {
  it("closes on Esc while untouched", () => {
    const onClose = renderForm();
    esc();
    expect(onClose).toHaveBeenCalled();
  });

  it("asks before discarding typed input, and can keep editing", () => {
    const onClose = renderForm();
    fireEvent.input(screen.getByLabelText("名称"), { target: { value: "x" } });
    esc();
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByText("放弃未保存的修改？")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "继续编辑" }));
    expect(screen.queryByText("放弃未保存的修改？")).not.toBeInTheDocument();
    esc();
    fireEvent.click(screen.getByRole("button", { name: "放弃" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("取消 still closes directly", () => {
    const onClose = renderForm();
    fireEvent.input(screen.getByLabelText("名称"), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(onClose).toHaveBeenCalled();
    expect(screen.queryByText("放弃未保存的修改？")).not.toBeInTheDocument();
  });
});
