import { Alert, Button } from "@mui/material";
import { OpenInNew } from "@mui/icons-material";
import { useState } from "react";
import { commands, type ProjectLink } from "../bindings";
import { Row } from "./SettingsSection";

const LINKS: Record<ProjectLink, { label: string; description: string; host: string }> = {
  Website: { label: "官网", description: "使用文档与下载", host: "www.routa.cn" },
  GitHub: { label: "GitHub", description: "源码与问题反馈", host: "github.com/c0010c/Routa" },
};

export const PROJECT_LINKS = Object.keys(LINKS) as ProjectLink[];

/** A Support-section row that opens the project website or GitHub page in the default browser. */
export function ProjectLinkRow({ link, compact = false }: { link: ProjectLink; compact?: boolean }) {
  const { label, description, host } = LINKS[link];
  const [error, setError] = useState<string | null>(null);
  return (
    <Row
      label={label}
      description={compact ? undefined : `${description} · ${host}`}
      below={error && <Alert severity="error" onClose={() => setError(null)}>{error}</Alert>}
    >
      <Button
        variant={compact ? "text" : "outlined"}
        endIcon={<OpenInNew fontSize="small" />}
        aria-label={`打开${label}`}
        onClick={async () => {
          const r = await commands.openProjectLink(link);
          setError(r.status === "ok" ? null : r.error);
        }}
      >
        打开
      </Button>
    </Row>
  );
}
