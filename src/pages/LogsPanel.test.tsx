import { fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { LogsPanel } from "./LogsPanel";

const text = [
  'time="2026-10-29T12:00:00+08:00" level=warning msg="旧错误"',
  String.raw`time="2026-10-30T20:00:00+08:00" level=error msg="连接超时\n第二个地址失败\n最后一行详情"`,
  'time="2026-10-30T20:01:00+08:00" level=info msg="内核已启动"',
].join("\n");

function Fixture({ value = text }: { value?: string }) {
  const [paused, setPaused] = useState(false);
  return <LogsPanel text={value} loading={false} fetching={false} paused={paused}
    onPausedChange={setPaused} onRefresh={() => {}} />;
}

it("shows latest entries first and expands escaped details only when inspected", () => {
  render(<Fixture />);
  const records = within(screen.getByRole("list", { name: "日志记录" })).getAllByRole("listitem");
  expect(records.map((row) => row.textContent)).toEqual([
    expect.stringContaining("内核已启动"), expect.stringContaining("连接超时"), expect.stringContaining("旧错误"),
  ]);
  expect(screen.queryByText(/第二个地址失败/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "展开日志：连接超时" }));
  const detail = screen.getByRole("region", { name: "日志详情" });
  expect(within(detail).getByText("连接超时\n第二个地址失败\n最后一行详情", { normalizer: (value) => value })).toBeInTheDocument();
  expect(screen.getByText(/已暂停更新/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "继续更新" })).toBeInTheDocument();
});

it("filters on message details and log severity, including a distinct no-match state", () => {
  render(<Fixture />);
  fireEvent.change(screen.getByRole("textbox", { name: "搜索日志消息" }), { target: { value: "第二个地址" } });
  expect(screen.getByRole("list", { name: "日志记录" }).children).toHaveLength(1);
  expect(screen.getByText("连接超时")).toBeInTheDocument();
  fireEvent.change(screen.getByRole("textbox", { name: "搜索日志消息" }), { target: { value: "" } });
  fireEvent.mouseDown(screen.getByRole("combobox", { name: "日志级别" }));
  fireEvent.click(screen.getByRole("option", { name: "警告与错误" }));
  expect(screen.queryByText("内核已启动")).not.toBeInTheDocument();
  expect(screen.getByRole("list", { name: "日志记录" }).children).toHaveLength(2);
  fireEvent.change(screen.getByRole("textbox", { name: "搜索日志消息" }), { target: { value: "不存在" } });
  expect(screen.getByText("没有匹配的日志")).toBeInTheDocument();
});

it("keeps an expanded record open when a refreshed snapshot gains new entries", () => {
  const { rerender } = render(<Fixture />);
  fireEvent.click(screen.getByRole("button", { name: "展开日志：连接超时" }));
  rerender(<Fixture value={text + '\n[INFO] 更新后的新记录'} />);
  expect(screen.getByRole("button", { name: "收起日志：连接超时" })).toHaveAttribute("aria-expanded", "true");
  expect(screen.getByRole("region", { name: "日志详情" })).toBeInTheDocument();
});

it("pauses refresh when scrolling back through historical records", () => {
  render(<Fixture />);
  fireEvent.scroll(screen.getByRole("region", { name: "内核日志列表" }), { target: { scrollTop: 80 } });
  expect(screen.getByRole("button", { name: "继续更新" })).toBeInTheDocument();
});
