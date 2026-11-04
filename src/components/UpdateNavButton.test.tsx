import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { UpdateView } from "../bindings";
import { installUpdate } from "../api/update";
import { UpdateNavButton } from "./UpdateNavButton";

let view: UpdateView;
vi.mock("../api/update", () => ({
  useUpdate: () => ({ data: view }),
  installUpdate: vi.fn(async () => {}),
}));

const base = { current_version: "0.3.0", last_checked_unix: 1 };

describe("UpdateNavButton", () => {
  beforeEach(() => vi.mocked(installUpdate).mockClear());

  it("renders nothing unless an installer is Ready", () => {
    for (const state of [
      { kind: "Idle" },
      { kind: "Unsupported" },
      { kind: "Downloading", version: "0.4.0", received: 1, total: 2 },
    ] as const) {
      view = { ...base, state };
      const { unmount } = render(<UpdateNavButton />);
      expect(screen.queryByRole("button")).toBeNull();
      unmount();
    }
  });

  it("opens a confirm popover and installs from it", async () => {
    view = { ...base, state: { kind: "Ready", version: "0.4.0", notes: "修复若干问题" } };
    render(<UpdateNavButton />);
    fireEvent.click(screen.getByRole("button", { name: "更新到 v0.4.0" }));
    expect(await screen.findByText("v0.4.0 已就绪")).toBeInTheDocument();
    expect(screen.getByText("修复若干问题")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "现在安装" }));
    expect(installUpdate).toHaveBeenCalledTimes(1);
  });

  it("closes on 稍后 without installing", async () => {
    view = { ...base, state: { kind: "Ready", version: "0.4.0", notes: null } };
    render(<UpdateNavButton />);
    fireEvent.click(screen.getByRole("button", { name: "更新到 v0.4.0" }));
    fireEvent.click(await screen.findByRole("button", { name: "稍后" }));
    await waitFor(() => expect(screen.queryByText("v0.4.0 已就绪")).toBeNull());
    expect(installUpdate).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "更新到 v0.4.0" })).toBeInTheDocument();
  });

  it("shows the install error in the popover", async () => {
    vi.mocked(installUpdate).mockRejectedValueOnce(new Error("安装包校验失败"));
    view = { ...base, state: { kind: "Ready", version: "0.4.0", notes: null } };
    render(<UpdateNavButton />);
    fireEvent.click(screen.getByRole("button", { name: "更新到 v0.4.0" }));
    fireEvent.click(await screen.findByRole("button", { name: "现在安装" }));
    expect(await screen.findByText("安装包校验失败")).toBeInTheDocument();
  });
});
