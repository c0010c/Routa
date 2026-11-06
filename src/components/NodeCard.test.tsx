import { fireEvent, render, screen } from "@testing-library/react";
import { NodeCard } from "./NodeCard";

it("keeps secondary actions out of the card and invokes them from its context menu", async () => {
  const use = vi.fn(),
    test = vi.fn();
  render(
    <NodeCard
      name="香港节点"
      source="机场"
      protocol="ss"
      delay={{ ms: 116, tested_at: 0 }}
      actions={[
        { label: "用于新代理", onClick: use },
        { label: "测速", onClick: test },
      ]}
    />,
  );
  expect(
    screen.queryByRole("button", { name: "用于新代理" }),
  ).not.toBeInTheDocument();
  expect(screen.queryByRole("menuitem")).not.toBeInTheDocument();
  expect(screen.getByLabelText("116 ms")).toBeInTheDocument();
  fireEvent.contextMenu(screen.getByRole("article", { name: "香港节点" }), {
    clientX: 100,
    clientY: 120,
  });
  fireEvent.click(
    await screen.findByRole("menuitem", { name: "用于新代理 香港节点" }),
  );
  expect(use).toHaveBeenCalledOnce();
  expect(test).not.toHaveBeenCalled();
});
it("provides the same actions through the ellipsis button and disables unavailable actions", async () => {
  render(
    <NodeCard
      name="节点"
      source="手动"
      protocol="http"
      actions={[{ label: "测速", disabled: true, onClick: vi.fn() }]}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "操作 节点" }));
  expect(
    await screen.findByRole("menuitem", { name: "测速 节点" }),
  ).toHaveAttribute("aria-disabled", "true");
});
it("opens the menu from the keyboard without saving or selecting anything", async () => {
  const edit = vi.fn();
  render(
    <NodeCard
      name="节点"
      source="手动"
      protocol="http"
      actions={[{ label: "编辑", onClick: edit }]}
    />,
  );
  fireEvent.keyDown(screen.getByRole("article", { name: "节点" }), {
    key: "F10",
    shiftKey: true,
  });
  expect(
    await screen.findByRole("menuitem", { name: "编辑 节点" }),
  ).toBeInTheDocument();
  expect(edit).not.toHaveBeenCalled();
});

it("shares the three delay bands across successful measurements", () => {
  const values = [100, 200, 450, 900];
  render(
    <>
      {values.map((ms) => (
        <NodeCard
          key={ms}
          name={`节点${ms}`}
          source="订阅"
          protocol="ss"
          delay={{ ms, tested_at: 0 }}
          actions={[]}
        />
      ))}
    </>,
  );
  const tones = values.map((ms) => {
    const element = screen.getByLabelText(`${ms} ms`);
    const style = getComputedStyle(element);
    const channels = style.color.match(/\d+/g)!.map(Number);
    return channels[0] / channels[1];
  });
  expect(tones[0]).toBe(tones[1]);
  expect(tones[1]).toBeLessThan(tones[2]);
  expect(tones[2]).toBeLessThan(tones[3]);
});

it("keeps the picker selection separate from its more and right-click actions", async () => {
  const select = vi.fn(), test = vi.fn();
  render(<NodeCard name="节点" source="来源" protocol="VLESS" selected onSelect={select} actions={[{ label: "测延迟", onClick: test }]} />);
  fireEvent.click(screen.getByRole("button", { name: "操作 节点" }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "测延迟 节点" }));
  expect(test).toHaveBeenCalledOnce();
  expect(select).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("radio", { name: "节点 · 来源" }));
  expect(select).toHaveBeenCalledOnce();
});
