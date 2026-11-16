import { Alert, Button } from "@mui/material";
import { useState } from "react";
import type { ProcRouteStatus } from "../bindings";
import { useProcRouteInstall, useProcRouteRetry } from "../api/procroute";

const DISMISSED_INTERRUPTION_KEY = "routa.dismissed-process-routing-interruption";
const formatTime = (unix: number) => new Date(unix * 1000).toLocaleString();

/** 只在需要用户处理时出现的提示：安装/更新/修复组件、中断、冲突、应用失败。 */
export function ProcessRoutingAlerts({ status }: { status: ProcRouteStatus }) {
  const install = useProcRouteInstall();
  const retry = useProcRouteRetry();
  const [dismissedInterruption, setDismissedInterruption] = useState<
    string | null
  >(() => localStorage.getItem(DISMISSED_INTERRUPTION_KEY));
  const interruption = status.last_interruption;
  const interruptionKey = interruption
    ? JSON.stringify([interruption.at_unix, interruption.reason])
    : null;
  const c = status.component;
  const busy = install.isPending || retry.isPending;
  const installAction = (text: string) => (
    <Button color="inherit" loading={install.isPending} disabled={busy} onClick={() => install.mutate()}>
      {text}
    </Button>
  );

  return (
    <>
      {c === "NotInstalled" && (
        <Alert severity="info" action={installAction("安装组件")}>
          进程分流需要先安装分流组件（需要管理员授权），装好后规则才会生效。
        </Alert>
      )}
      {c === "NeedsUpdate" && (
        <Alert severity="warning" action={installAction("更新组件")}>
          分流服务与当前程序版本不匹配。更新需要管理员权限，期间进程分流会短暂中断。
        </Alert>
      )}
      {c === "Stopped" && (
        <Alert severity="warning" action={<Button color="inherit" loading={retry.isPending} disabled={busy} onClick={() => retry.mutate()}>启动组件</Button>}>
          分流组件已停止，已启用的规则暂时无法生效。
        </Alert>
      )}
      {typeof c === "object" && (
        <Alert severity="error" action={<><Button color="inherit" loading={retry.isPending} disabled={busy} onClick={() => retry.mutate()}>重试</Button>{installAction("修复组件")}</>}>
          分流组件异常：{c.Error.message}
        </Alert>
      )}
      {c === "OwnedByOtherUser" && (
        <Alert severity="warning">分流组件被其他 Windows 用户占用，请在该用户下卸载组件后重试。</Alert>
      )}
      {interruption &&
        interruptionKey &&
        interruptionKey !== dismissedInterruption && (
          <Alert
            severity="warning"
            closeText="关闭中断提示"
            onClose={() => {
              setDismissedInterruption(interruptionKey);
              localStorage.setItem(DISMISSED_INTERRUPTION_KEY, interruptionKey);
            }}
          >
            {`最近一次中断：${formatTime(interruption.at_unix)}，${interruption.reason}`}
          </Alert>
        )}
      {status.conflicts.length > 0 && (
        <Alert severity="warning">
          {status.conflicts.map((m) => (
            <div key={m}>{m}</div>
          ))}
        </Alert>
      )}
      {status.apply_errors.length > 0 && (
        <Alert
          severity="warning"
          action={
            <Button
              color="inherit"
              loading={retry.isPending}
              disabled={busy || c === "NeedsUpdate" || c === "NotInstalled" || c === "OwnedByOtherUser"}
              onClick={() => retry.mutate()}
            >
              重试
            </Button>
          }
        >
          {status.apply_errors.map((m) => (
            <div key={m}>{m}</div>
          ))}
        </Alert>
      )}
    </>
  );
}
