import InfoOutlined from "@mui/icons-material/InfoOutlined";
import { Alert, Button, IconButton, Snackbar, Tooltip } from "@mui/material";
import { useState } from "react";
import { errorMessage } from "../api/client";
import { installUpdate, useUpdate } from "../api/update";
import { UAC_NOTE } from "../lib/updateCopy";

export function UpdateNotice() {
  const { data } = useUpdate();
  const [dismissed, setDismissed] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const state = data?.state;
  if (!state || state.kind !== "Ready" || dismissed) return null;
  const install = async () => {
    try {
      await installUpdate();
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  return (
    <Snackbar open anchorOrigin={{ vertical: "bottom", horizontal: "center" }}>
      <Alert
        severity={error ? "error" : "info"}
        icon={false}
        action={
          <>
            <Button color="inherit" size="small" onClick={() => void install()}>
              现在安装
            </Button>
            <Button color="inherit" size="small" onClick={() => setDismissed(true)}>
              稍后
            </Button>
          </>
        }
      >
        {error ?? `新版本 v${state.version} 已就绪`}
        {!error && (
          <Tooltip
            title={
              <>
                {state.notes && <div>{state.notes}</div>}
                <div>{UAC_NOTE}</div>
              </>
            }
          >
            <IconButton size="small" aria-label="更新内容" color="inherit" sx={{ p: 0.25, ml: 0.5 }}>
              <InfoOutlined sx={{ fontSize: 16 }} />
            </IconButton>
          </Tooltip>
        )}
      </Alert>
    </Snackbar>
  );
}
