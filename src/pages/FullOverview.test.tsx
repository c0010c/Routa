import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { desktopApi } from "../api/desktop";
import { FullOverview } from "./FullOverview";
import { ConnectionsPanel } from "./ConnectionsPanel";
import type { ConnectionInfo } from "../bindings";

vi.mock("../api/desktop", () => ({ desktopApi: { connections: vi.fn(), logs: vi.fn() } }));
vi.mock("./RuntimeSummary", () => ({ RuntimeSummary: () => <div>运行概况内容</div> }));

const initial: ConnectionInfo[] = [
  { id: "1", listener: "机场", process: "C:\\Chrome\\chrome.exe", target: "example.com:443", node: "新加坡01", protocol: "TCP", uploaded: 1024, downloaded: 2048 },
  { id: "2", listener: "机场", process: "C:\\Chrome\\chrome.exe", target: "cdn.example.com:443", node: "香港02", protocol: "UDP", uploaded: 4096, downloaded: 8192 },
  { id: "3", listener: "办公", process: "", target: "api.example.org:443", node: "", protocol: "TCP", uploaded: 0, downloaded: 0 },
];
let now = 1000;
beforeEach(() => {
  now = 1000;
  vi.spyOn(performance, "now").mockImplementation(() => now);
  vi.mocked(desktopApi.connections).mockReset().mockResolvedValue(initial);
  vi.mocked(desktopApi.logs).mockReset().mockResolvedValue("[INFO] 已启动\n[INFO] 真实日志内容");
});
afterEach(() => vi.restoreAllMocks());

function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  render(<QueryClientProvider client={client}><FullOverview /></QueryClientProvider>);
  return client;
}
async function update(client: QueryClient) {
  await act(async () => { await client.invalidateQueries({ queryKey: ["connections"] }); });
}

it("loads only the active tab, groups apps, and preserves folding through data and tab changes", async () => {
  const client = mount();
  expect(desktopApi.connections).not.toHaveBeenCalled();
  expect(desktopApi.logs).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("tab", { name: "连接" }));
  const group = await screen.findByRole("button", { name: "chrome.exe，2 个连接" });
  expect(screen.getByRole("checkbox", { name: "按应用分组" })).toBeChecked();
  expect(screen.getByText("新加坡01")).toBeInTheDocument();
  expect(screen.getByText("香港02")).toBeInTheDocument();
  expect(screen.getByText("节点未确认")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "未识别应用，1 个连接" })).toBeInTheDocument();
  fireEvent.click(group);
  expect(group).toHaveAttribute("aria-expanded", "false");
  expect(screen.getByText("example.com:443")).not.toBeVisible();
  now = 3000;
  vi.mocked(desktopApi.connections).mockResolvedValue(initial.map((r) => ({ ...r, uploaded: r.uploaded + 2048, downloaded: r.downloaded + 4096 })));
  await update(client);
  expect(group).toHaveAttribute("aria-expanded", "false");
  await waitFor(() => expect(within(group.closest("tr")!).getByText("2 KB/s")).toBeInTheDocument());
  expect(within(group.closest("tr")!).getByText("4 KB/s")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: "按应用分组" }));
  expect(screen.getByText("example.com:443")).toBeVisible();
  expect(screen.getAllByRole("row")).toHaveLength(4);
  fireEvent.click(screen.getByRole("checkbox", { name: "按应用分组" }));
  expect(screen.getByRole("button", { name: "chrome.exe，2 个连接" })).toHaveAttribute("aria-expanded", "false");
  fireEvent.click(screen.getByRole("tab", { name: "日志" }));
  expect(await screen.findByText(/真实日志内容/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("tab", { name: "连接" }));
  expect(await screen.findByRole("button", { name: "chrome.exe，2 个连接" })).toHaveAttribute("aria-expanded", "false");
});

it("suppresses stale rates after a failed read and establishes a fresh baseline after recovery", async () => {
  const client = mount();
  fireEvent.click(screen.getByRole("tab", { name: "连接" }));
  await screen.findByText("新加坡01");
  now = 3000;
  vi.mocked(desktopApi.connections).mockResolvedValue(initial.map((r) => ({ ...r, uploaded: r.uploaded + 2048, downloaded: r.downloaded + 4096 })));
  await update(client);
  await waitFor(() => expect(screen.getAllByText("1 KB/s").length).toBeGreaterThan(0));
  vi.mocked(desktopApi.connections).mockRejectedValue(new Error("内核无响应"));
  await update(client);
  expect(await screen.findByRole("alert")).toHaveTextContent("内核无响应");
  expect(screen.queryByText("1 KB/s")).not.toBeInTheDocument();
  expect(screen.getByText("新加坡01")).toBeInTheDocument();
  vi.mocked(desktopApi.connections).mockResolvedValue(initial);
  now = 5000;
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  expect(screen.queryByText(/KB\/s/)).not.toBeInTheDocument();
});

it("refreshes the actual log contents and keeps prior logs visible when refresh fails", async () => {
  mount();
  fireEvent.click(screen.getByRole("tab", { name: "日志" }));
  expect(await screen.findByText(/真实日志内容/)).toBeInTheDocument();
  vi.mocked(desktopApi.logs).mockResolvedValue("[INFO] 新的日志");
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  expect(await screen.findByText("新的日志")).toBeInTheDocument();
  vi.mocked(desktopApi.logs).mockRejectedValue(new Error("日志文件无法读取"));
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("日志文件无法读取");
  expect(screen.getByText("新的日志")).toBeInTheDocument();
});

it("does not offer grouping when the application column is absent", () => {
  render(<ConnectionsPanel rows={initial.map((r) => ({ ...r, uploadRate: null, downloadRate: null }))}
    loading={false} stale={false} grouped showAppColumn={false} onGroupedChange={() => {}}
    collapsed={new Set()} onCollapse={() => {}} />);
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  expect(screen.getAllByRole("columnheader")).toHaveLength(4);
  expect(screen.getByText("example.com:443")).toBeVisible();
});

it("pauses automatic reads when inspecting a log, while manual refresh remains available", async () => {
  const client = mount();
  fireEvent.click(screen.getByRole("tab", { name: "日志" }));
  fireEvent.click(await screen.findByRole("button", { name: "展开日志：真实日志内容" }));
  expect(screen.getByRole("button", { name: "继续更新" })).toBeInTheDocument();
  const before = vi.mocked(desktopApi.logs).mock.calls.length;
  await act(async () => { await client.invalidateQueries({ queryKey: ["core-log"] }); });
  expect(desktopApi.logs).toHaveBeenCalledTimes(before);
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  await waitFor(() => expect(desktopApi.logs).toHaveBeenCalledTimes(before + 1));
  expect(screen.getByRole("button", { name: "收起日志：真实日志内容" })).toHaveAttribute("aria-expanded", "true");
  fireEvent.click(screen.getByRole("button", { name: "继续更新" }));
  await waitFor(() => expect(desktopApi.logs).toHaveBeenCalledTimes(before + 2));
});
