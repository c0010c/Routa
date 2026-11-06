import { Button, Stack, Typography } from "@mui/material";
import { useEffect, useRef, useState } from "react";

export type SaveNotice = { kind: "pending" | "success" | "error" | "warning"; text: string };

/** One synchronous submission lock, with feedback kept next to each edited setting. */
export function useSaveFeedback<K extends string>() {
  const [notices, setNotices] = useState<Partial<Record<K, SaveNotice>>>({});
  const [pending, setPending] = useState(false);
  const locked = useRef(false);
  const mounted = useRef(true);
  const timers = useRef(new Map<K, ReturnType<typeof setTimeout>>());
  useEffect(() => {
    mounted.current = true;
    const activeTimers = timers.current;
    return () => {
      mounted.current = false;
      activeTimers.forEach(clearTimeout);
    };
  }, []);

  const clear = (key: K) => {
    clearTimeout(timers.current.get(key));
    setNotices((current) => ({ ...current, [key]: undefined }));
  };
  const run = async (key: K, action: () => Promise<SaveNotice | void>) => {
    if (locked.current) return;
    locked.current = true;
    clearTimeout(timers.current.get(key));
    setPending(true);
    setNotices((current) => ({ ...current, [key]: { kind: "pending", text: "保存中…" } }));
    try {
      const notice = (await action()) ?? { kind: "success", text: "已保存" };
      if (!mounted.current) return;
      setNotices((current) => ({ ...current, [key]: notice }));
      if (notice.kind === "success") {
        timers.current.set(key, setTimeout(() => {
          setNotices((current) => ({ ...current, [key]: undefined }));
          timers.current.delete(key);
        }, 2500));
      }
    } catch (error) {
      if (mounted.current) setNotices((current) => ({
        ...current,
        [key]: { kind: "error", text: error instanceof Error ? error.message : "未能保存，请重试" },
      }));
    } finally {
      locked.current = false;
      if (mounted.current) setPending(false);
    }
  };
  return { notices, pending, run, clear };
}

export function SaveFeedback({ notice, onRetry }: { notice?: SaveNotice; onRetry?: () => void }) {
  return (
    <Stack direction="row" spacing={1} sx={{ minHeight: 24, alignItems: "center" }}>
      <Typography
        variant="caption"
        role={notice?.kind === "error" || notice?.kind === "warning" ? "alert" : "status"}
        sx={{ color: notice?.kind === "error" ? "error.main" : notice?.kind === "warning" ? "warning.main" : "text.secondary" }}
      >
        {notice?.text}
      </Typography>
      {notice?.kind === "error" && onRetry && <Button size="small" onClick={onRetry}>重试</Button>}
    </Stack>
  );
}
