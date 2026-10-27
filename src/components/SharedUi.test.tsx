import { Button, ThemeProvider } from "@mui/material";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { theme } from "../theme";
import { DelayText } from "./DelayText";
import { EmptyState } from "./EmptyState";
import { RowActions } from "./RowActions";

describe("EmptyState", () => {
  it("renders title, description and action", () => {
    render(<EmptyState icon={<span>i</span>} title="还没有出口" description="说明文字" action={<Button>去新建</Button>} />);
    expect(screen.getByText("还没有出口")).toBeInTheDocument();
    expect(screen.getByText("说明文字")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "去新建" })).toBeInTheDocument();
  });
});

describe("RowActions", () => {
  it("opens a menu and runs the chosen item", async () => {
    const onEdit = vi.fn();
    const onDelete = vi.fn();
    render(
      <RowActions
        label="办公"
        items={[
          { label: "编辑", onClick: onEdit },
          { label: "删除", onClick: onDelete, danger: true },
        ]}
      />,
    );
    expect(screen.queryByRole("menuitem")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "操作 办公" }));
    expect(screen.getByRole("menuitem", { name: "编辑 办公" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("menuitem", { name: "删除 办公" }));
    expect(onDelete).toHaveBeenCalledTimes(1);
    expect(onEdit).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByRole("menuitem")).toBeNull());
  });
});

describe("RowActions disabled item", () => {
  it("renders a disabled item that cannot be chosen", () => {
    render(<RowActions label="机场A" items={[{ label: "立即更新", onClick: vi.fn(), disabled: true }]} />);
    fireEvent.click(screen.getByRole("button", { name: "操作 机场A" }));
    expect(screen.getByRole("menuitem", { name: "立即更新 机场A" })).toHaveAttribute("aria-disabled", "true");
  });
});

describe("DelayText", () => {
  const colorOf = (text: string) => getComputedStyle(screen.getByText(text)).color;
  const renderDelay = (ms: number | null | undefined) =>
    render(
      <ThemeProvider theme={theme}>
        <DelayText ms={ms} />
      </ThemeProvider>,
    );

  it("shows a fast delay in the success colour", () => {
    renderDelay(48);
    expect(colorOf("48 ms")).toMatch(/success/i);
  });

  it("shows a slow delay in the warning colour", () => {
    renderDelay(286);
    expect(colorOf("286 ms")).toMatch(/warning/i);
  });

  it("shows a timeout", () => {
    renderDelay(null);
    expect(colorOf("超时")).toMatch(/error/i);
  });

  it("shows a dash when untested", () => {
    renderDelay(undefined);
    expect(screen.getByText("—")).toBeInTheDocument();
  });
});
