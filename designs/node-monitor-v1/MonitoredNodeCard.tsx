// Prototype wrapper around the production NodeCard: adds the monitor mark and
// "加入监测 / 移出监测" to the card menu. Swapped in by vite.config.mts.
import NotificationsActiveOutlined from "@mui/icons-material/NotificationsActiveOutlined";
import NotificationsOffOutlined from "@mui/icons-material/NotificationsOffOutlined";
import { Box, Tooltip } from "@mui/material";
import type { ComponentProps } from "react";
import { NodeCard as BaseNodeCard } from "../../src/components/NodeCard";
import { useModel } from "../../src/api/client";
import { IN_USE, memberOf, monitoredProxies, setMonitor, useMonitor } from "./monitorStore";

export function NodeCard(props: ComponentProps<typeof BaseNodeCard>) {
  const model = useModel().data;
  const mon = useMonitor();
  const proxy = props.testTarget;
  if (!proxy || !model) return <BaseNodeCard {...props} />;

  const watched = monitoredProxies(model, mon).includes(proxy);
  const node = mon.nodes[proxy];
  const down = watched && node?.down;

  const toggle = () => {
    const self = memberOf(model, proxy);
    if (!self) return;
    if (watched) {
      // Removing turns the scope into an explicit per-node list without this node.
      const rest = monitoredProxies(model, mon).filter((p) => p !== proxy).map((p) => memberOf(model, p)).filter((m) => m !== null);
      setMonitor({ scope: "custom", members: rest });
    } else {
      const base = !mon.enabled ? [] : mon.scope === "inUse" ? IN_USE.map((x) => memberOf(model, x.proxy)!).filter(Boolean) : mon.members;
      setMonitor({ enabled: true, scope: "custom", members: [...base, self] });
    }
  };

  const mark = watched ? (
    <Tooltip title={down ? `不可用 · 连续失败 ${node!.fails} 次` : "监测中"}>
      <Box component="span" aria-label={down ? "监测：不可用" : "监测中"} sx={{ display: "inline-flex", alignItems: "center", gap: "2px", color: down ? "error.main" : "primary.main" }}>
        {down ? <NotificationsOffOutlined sx={{ fontSize: 13 }} /> : <NotificationsActiveOutlined sx={{ fontSize: 13 }} />}
        {down ? "不可用" : "监测"}
      </Box>
    </Tooltip>
  ) : undefined;

  return (
    <BaseNodeCard
      {...props}
      status={props.status ?? mark}
      actions={[...props.actions, { label: watched ? "移出监测" : "加入监测", onClick: toggle }]}
    />
  );
}
