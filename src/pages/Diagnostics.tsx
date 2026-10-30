import { Alert, Button, Paper, Stack, Typography } from "@mui/material";
import { useModel, useStatus, useOpenLogDir } from "../api/client";
import { useProcRouteStatus } from "../api/procroute";
import { useInterface } from "../components/InterfaceProvider";
import { BackHeading } from "../components/BackHeading";
export function Diagnostics() {
  const model = useModel(),
    status = useStatus(),
    routing = useProcRouteStatus(),
    logs = useOpenLogDir();
  const ui = useInterface();
  const st = status.data,
    p = ui?.state?.preferences,
    l = model.data?.listeners.find((x) => x.id === p?.listener_id);
  const rows = [
    [
      "mihomo 内核",
      !st
        ? "正在读取"
        : st.core === "Running"
          ? "运行中"
          : st.core === "Starting"
            ? "启动中"
            : st.core === "Stopped"
              ? "已停止"
              : "异常退出",
    ],
    ["默认监听器", l ? `${l.listen}:${l.port}` : "尚未绑定"],
    [
      "监听器生效状态",
      l && st?.effective_listeners.includes(l.id) ? "已生效" : "未生效",
    ],
    [
      "端口冲突",
      st?.failed_listeners.map((x) => `${x.name} (${x.port})`).join("、") ||
        "未发现",
    ],
    ["系统代理 / TUN", ui?.state?.connection ?? "未开启"],
    [
      "应用分流组件",
      typeof routing.data?.component === "string"
        ? routing.data.component
        : "状态异常或尚未读取",
    ],
  ];
  return (
    <Stack spacing={2.5}>
      {p?.mode === "Simple" ? (
        <BackHeading title="故障诊断" />
      ) : (
        <Typography variant="h5">故障诊断</Typography>
      )}
      {[st?.startup_error, st?.save_error, ui?.state?.error]
        .filter(Boolean)
        .map((text, i) => (
          <Alert key={i} severity="error">
            {text}
          </Alert>
        ))}
      <Paper data-selectable="true">
        {rows.map(([a, b]) => (
          <Stack
            key={a}
            direction="row"
            spacing={2}
            sx={{
              p: 2,
              borderBottom: 1,
              borderColor: "divider",
              justifyContent: "space-between",
            }}
          >
            <Typography>{a}</Typography>
            <Typography color="text.secondary">{b}</Typography>
          </Stack>
        ))}
      </Paper>
      <Stack direction="row" spacing={1}>
        <Button
          variant="contained"
          onClick={() => {
            void model.refetch();
            void status.refetch();
            void routing.refetch();
            ui?.retry();
          }}
        >
          重新检查
        </Button>
        <Button onClick={() => logs.mutate()} disabled={logs.isPending}>
          打开内核日志目录
        </Button>
      </Stack>
    </Stack>
  );
}
