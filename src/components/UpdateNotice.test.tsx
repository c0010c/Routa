import { fireEvent, render, screen } from "@testing-library/react";
import type { UpdateView } from "../bindings";
import { UAC_NOTE } from "../lib/updateCopy";
import { UpdateNotice } from "./UpdateNotice";

let view: UpdateView;
vi.mock("../api/update", () => ({
  useUpdate: () => ({ data: view }),
  installUpdate: vi.fn(async () => {}),
}));

const base = { current_version: "0.3.0", last_checked_unix: 1 };

describe("UpdateNotice", () => {
  it("shows when Ready and hides after 稍后", () => {
    view = { ...base, state: { kind: "Ready", version: "0.4.0", notes: "修复" } };
    render(<UpdateNotice />);
    expect(screen.getByText("新版本 v0.4.0 已就绪")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "稍后" }));
    expect(screen.queryByText("新版本 v0.4.0 已就绪")).toBeNull();
  });
  it("explains the possible UAC prompt in the tooltip", async () => {
    view = { ...base, state: { kind: "Ready", version: "0.4.0", notes: null } };
    render(<UpdateNotice />);
    fireEvent.mouseOver(screen.getByRole("button", { name: "更新内容" }));
    expect(await screen.findByText(UAC_NOTE)).toBeInTheDocument();
  });
  it("renders nothing when Idle", () => {
    view = { ...base, state: { kind: "Idle" } };
    render(<UpdateNotice />);
    expect(screen.queryByText(/已就绪/)).toBeNull();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
