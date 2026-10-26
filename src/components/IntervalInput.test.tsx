import { fireEvent, render, screen } from "@testing-library/react";
import { IntervalInput } from "./IntervalInput";

function setup(value: number | null) {
  const onChange = vi.fn();
  render(<IntervalInput value={value} onChange={onChange} defaultMins={1440} />);
  return onChange;
}

describe("IntervalInput", () => {
  it("emits minutes after choosing 小时 and typing 6", () => {
    const onChange = setup(10);
    fireEvent.change(screen.getByLabelText("单位"), { target: { value: "小时" } });
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "6" } });
    expect(onChange).toHaveBeenLastCalledWith(360);
  });

  it("emits null when the switch is turned off", () => {
    const onChange = setup(10);
    fireEvent.click(screen.getByRole("switch", { name: "启用" }));
    expect(onChange).toHaveBeenLastCalledWith(null);
  });

  it("rejects 400 天 as out of range without emitting", () => {
    const onChange = setup(10);
    fireEvent.change(screen.getByLabelText("单位"), { target: { value: "天" } });
    onChange.mockClear();
    fireEvent.change(screen.getByLabelText("间隔"), { target: { value: "400" } });
    expect(screen.getByText("超出范围")).toBeInTheDocument();
    expect(onChange).not.toHaveBeenCalled();
  });

  it.each([
    [1440, "1", "天"],
    [360, "6", "小时"],
    [10, "10", "分钟"],
    [90, "90", "分钟"],
  ])("displays %i minutes as %s %s", (mins, amount, unit) => {
    setup(mins);
    expect(screen.getByLabelText("间隔")).toHaveValue(Number(amount));
    expect(screen.getByLabelText("单位")).toHaveValue(unit);
  });

  it("is disabled and empty-valued when value is null", () => {
    setup(null);
    expect(screen.getByRole("switch", { name: "启用" })).not.toBeChecked();
    expect(screen.getByLabelText("间隔")).toBeDisabled();
  });
});
