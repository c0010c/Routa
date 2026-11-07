import { fireEvent, render, screen } from "@testing-library/react";
import type { ProcessRule, RunningProcess } from "../bindings";
import { LearnedLocations } from "./LearnedLocations";

const OLD = "C:\\Apps\\Codex\\1.0\\ChatGPT.exe";
const NOW = "C:\\Apps\\Codex\\2.0\\ChatGPT.exe";
const CANARY = "C:\\Canary\\chrome.exe";

const rule = (over: Partial<ProcessRule>): ProcessRule => ({
  id: "r1",
  name: "AI",
  processes: [{ kind: "Name", value: "ChatGPT.exe" }],
  learned: [],
  node: null,
  interface: null,
  on_fault: "Block",
  reset_on_change: false,
  user_enabled: true,
  ...over,
});
const running: RunningProcess[] = [{ pid: 1, name: "ChatGPT.exe", exe_path: NOW }];

describe("LearnedLocations", () => {
  it("expands to paths and marks not-running ones", () => {
    render(<LearnedLocations rule={rule({ learned: [OLD, NOW] })} others={[]} routingEnabled running={running} />);
    fireEvent.click(screen.getByRole("button", { name: /已记录 2 个位置/ }));
    expect(screen.getByText(OLD)).toBeTruthy();
    expect(screen.getAllByText("未运行")).toHaveLength(1);
  });

  it("warns when nothing is learned yet", () => {
    render(<LearnedLocations rule={rule({})} others={[]} routingEnabled running={[]} />);
    fireEvent.click(screen.getByRole("button", { name: /尚未记录位置/ }));
    expect(screen.getByText("程序联网后自动记录。在此之前，分流服务停止时拦不住。")).toBeTruthy();
  });

  it("shows locations owned by other rules as skipped", () => {
    const other = rule({ id: "r2", name: "Canary", processes: [{ kind: "Path", value: CANARY }], on_fault: "Direct" });
    const r = rule({ processes: [{ kind: "Name", value: "chrome.exe" }], learned: [OLD] });
    render(<LearnedLocations rule={r} others={[other]} routingEnabled running={[]} />);
    fireEvent.click(screen.getByRole("button", { name: /已记录 1 个位置，1 个位置属于其他规则/ }));
    expect(screen.getByText(CANARY)).toBeTruthy();
    expect(screen.getByText(/不记录：这个位置属于规则「Canary」/)).toBeTruthy();
  });

  it("doesn't count learned paths another enabled rule claims by path", () => {
    const STABLE = "C:\\Stable\\chrome.exe";
    const CLAIMED = "c:\\canary\\CHROME.exe";
    const other = rule({ id: "r2", name: "Canary", processes: [{ kind: "Path", value: CANARY }], on_fault: "Direct" });
    const r = rule({ processes: [{ kind: "Name", value: "chrome.exe" }], learned: [CLAIMED, STABLE] });
    render(<LearnedLocations rule={r} others={[other]} routingEnabled running={[]} />);
    fireEvent.click(screen.getByRole("button", { name: /已记录 1 个位置，1 个位置属于其他规则/ }));
    expect(screen.getByText(STABLE)).toBeTruthy();
    expect(screen.queryByText(CLAIMED)).toBeNull();
    expect(screen.getByText(CANARY)).toBeTruthy();
  });

  it("calls onRemove with the path", () => {
    const onRemove = vi.fn();
    render(<LearnedLocations rule={rule({ learned: [OLD] })} others={[]} routingEnabled running={[]} onRemove={onRemove} />);
    fireEvent.click(screen.getByRole("button", { name: /已记录 1 个位置/ }));
    fireEvent.click(screen.getByRole("button", { name: `移除 ${OLD}` }));
    expect(onRemove).toHaveBeenCalledWith(OLD);
  });

  it("renders nothing when not learning", () => {
    const { container } = render(<LearnedLocations rule={rule({ on_fault: "Direct" })} others={[]} routingEnabled running={[]} />);
    expect(container.firstChild).toBeNull();
  });
});
