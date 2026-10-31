import { fireEvent, render, screen } from "@testing-library/react";
import type { UpdateState, UpdateView } from "../bindings";
import { installUpdate } from "../api/update";
import { UpdatePanel } from "./UpdatePanel";

let view: UpdateView;
vi.mock("../api/update", () => ({
  useUpdate: () => ({ data: view }),
  checkUpdate: vi.fn(),
  installUpdate: vi.fn(async () => {}),
}));

function show(state: UpdateState, last: number | null = 1) {
  view = { current_version: "0.3.0", state, last_checked_unix: last };
  render(<UpdatePanel />);
}

describe("UpdatePanel", () => {
  it("Idle shows up-to-date and an enabled check button", () => {
    show({ kind: "Idle" });
    expect(screen.getByText("当前版本 v0.3.0")).toBeInTheDocument();
    expect(screen.getByText("已是最新版本")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "检查更新" })).toBeEnabled();
  });
  it("Idle without a last check shows no status", () => {
    show({ kind: "Idle" }, null);
    expect(screen.queryByText("已是最新版本")).toBeNull();
  });
  it("Ready offers install once", () => {
    show({ kind: "Ready", version: "0.4.0", notes: null });
    expect(screen.getByText("v0.4.0 已就绪")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "现在安装" }));
    expect(installUpdate).toHaveBeenCalledTimes(1);
  });
  it("Failed shows the message", () => {
    show({ kind: "Failed", message: "网络错误" });
    expect(screen.getByText("检查更新失败：网络错误")).toBeInTheDocument();
  });
  it("Unsupported has no buttons", () => {
    show({ kind: "Unsupported" });
    expect(screen.getByText("当前安装方式不支持自动更新")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "检查更新" })).toBeNull();
    expect(screen.queryByRole("button", { name: "现在安装" })).toBeNull();
  });
  it("Downloading disables check and shows percent", () => {
    show({ kind: "Downloading", version: "0.4.0", received: 50, total: 200 });
    expect(screen.getByText(/正在下载 v0.4.0/)).toHaveTextContent("25%");
    expect(screen.getByRole("button", { name: "检查更新" })).toBeDisabled();
  });
});
