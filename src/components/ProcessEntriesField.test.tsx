import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import type { ProcessEntry, ProcessRule, RunningProcess } from "../bindings";
import { ProcessEntriesField } from "./ProcessEntriesField";

const CHROME = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe";
const CANARY = "C:\\Users\\Q\\AppData\\Local\\Google\\Chrome SxS\\Application\\chrome.exe";
const TG = "D:\\Apps\\Telegram\\Telegram.exe";

const running: RunningProcess[] = [
  { pid: 1, name: "chrome.exe", exe_path: CHROME },
  { pid: 2, name: "chrome.exe", exe_path: CANARY },
  { pid: 3, name: "Telegram.exe", exe_path: TG },
];

const other = (id: string, name: string, processes: ProcessEntry[]): ProcessRule => ({
  id,
  name,
  processes,
  learned: [],
  node: null,
  interface: null,
  on_fault: "Direct",
  reset_on_change: false,
  user_enabled: true,
});

function Harness({ initial = [], others = [] }: { initial?: ProcessEntry[]; others?: ProcessRule[] }) {
  const [entries, setEntries] = useState(initial);
  return (
    <>
      <ProcessEntriesField entries={entries} others={others} routingEnabled running={running} onChange={setEntries} />
      <pre data-testid="out">{JSON.stringify(entries)}</pre>
    </>
  );
}
const out = () => JSON.parse(screen.getByTestId("out").textContent!) as ProcessEntry[];
const input = () => screen.getByRole("combobox") as HTMLInputElement;
const type = (v: string) => {
  fireEvent.change(input(), { target: { value: v } });
  fireEvent.keyDown(input(), { key: "Enter" });
};

describe("ProcessEntriesField", () => {
  it("Enter adds a chip by name", async () => {
    render(<Harness />);
    type("chrome.exe");
    await waitFor(() => expect(out()).toEqual([{ kind: "Name", value: "chrome.exe" }]));
    expect(screen.getByRole("button", { name: "chrome.exe" })).toBeTruthy();
  });

  it("checking a path in the menu adds a location; disabled and last-item rules", async () => {
    const canaryRule = other("r2", "Chrome Canary", [{ kind: "Path", value: CANARY }]);
    render(<Harness initial={[{ kind: "Name", value: "chrome.exe" }]} others={[canaryRule]} />);
    fireEvent.click(screen.getByRole("button", { name: "chrome.exe" }));
    const canary = await screen.findByRole("menuitem", { name: new RegExp("Chrome SxS") });
    expect(canary.getAttribute("aria-disabled")).toBe("true");
    expect(canary.textContent).toContain("Chrome Canary");
    // the only checked item cannot be unchecked
    expect(screen.getByRole("menuitem", { name: /所有位置（按进程名）/ }).getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(screen.getByRole("menuitem", { name: new RegExp("Program Files") }));
    await waitFor(() => expect(out()).toHaveLength(2));
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    expect(screen.getByRole("button", { name: "chrome.exe + 1 个位置" })).toBeTruthy();
  });

  it("reports duplicates within the rule", async () => {
    render(<Harness initial={[{ kind: "Name", value: "chrome.exe" }]} />);
    type("chrome.exe");
    expect(await screen.findByText(/已经在这条规则里/)).toBeTruthy();
    expect(out()).toHaveLength(1);
  });

  it("name taken by another rule errors, but pasting its path works", async () => {
    const o = other("r2", "TG", [{ kind: "Name", value: "Telegram.exe" }]);
    render(<Harness others={[o]} />);
    type("Telegram.exe");
    expect(await screen.findByText(/已在规则「TG」中按进程名匹配/)).toBeTruthy();
    expect(out()).toHaveLength(0);
    type(TG);
    await waitFor(() => expect(out()).toEqual([{ kind: "Path", value: TG }]));
  });

  it("shows the external error and the help tooltip trigger", () => {
    render(<ProcessEntriesField entries={[]} others={[]} routingEnabled running={[]} error="至少添加一个进程" onChange={() => {}} />);
    expect(screen.getByText("至少添加一个进程")).toBeTruthy();
    expect(screen.getAllByTestId("HelpOutlineOutlinedIcon").length).toBeGreaterThan(0);
  });
});
