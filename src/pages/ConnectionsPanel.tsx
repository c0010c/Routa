import { Fragment, useId, type ReactNode } from "react";
import ExpandMore from "@mui/icons-material/ExpandMore";
import ChevronRight from "@mui/icons-material/ChevronRight";
import {
  Box, ButtonBase, Checkbox, FormControlLabel, Paper, Stack, Table,
  TableBody, TableCell, TableContainer, TableHead, TableRow, Typography,
} from "@mui/material";
import {
  connectionApp, groupConnections, sumConnections, type LiveConnection,
} from "../lib/connections";
import { formatBytes, formatRate } from "../lib/format";

function TrafficCells({ traffic, stale }: {
  traffic: ReturnType<typeof sumConnections>; stale: boolean;
}) {
  return ([
    [traffic.uploadRate, traffic.uploaded],
    [traffic.downloadRate, traffic.downloaded],
  ] as const).map(([rate, total], index) => (
    <TableCell key={index} align="right" sx={{ whiteSpace: "nowrap", fontVariantNumeric: "tabular-nums" }}>
      <Typography component="div" sx={{ fontSize: 13, fontWeight: 500 }}>
        {stale || rate === null ? "—" : formatRate(rate)}
      </Typography>
      <Typography component="div" variant="caption" color="text.secondary" sx={{ fontSize: 11 }}>
        累计 {formatBytes(total)}
      </Typography>
    </TableCell>
  ));
}

function PathCell({ row }: { row: LiveConnection }) {
  return <TableCell sx={{ overflowWrap: "anywhere" }}>
    <Typography component="div" sx={{ fontSize: 12 }}>{row.node || "节点未确认"}</Typography>
    <Typography component="div" variant="caption" color="text.secondary" sx={{ fontSize: 11 }}>
      {row.listener || "—"}
    </Typography>
  </TableCell>;
}

export function ConnectionsPanel({ rows, loading, stale, grouped, onGroupedChange, collapsed, onCollapse, feedback, showAppColumn = true }: {
  rows: LiveConnection[]; loading: boolean; stale: boolean;
  grouped: boolean; onGroupedChange: (value: boolean) => void;
  collapsed: Set<string>; onCollapse: (key: string) => void;
  feedback?: ReactNode; showAppColumn?: boolean;
}) {
  const id = useId();
  const groups = groupConnections(rows);
  // Only offer grouping when the table has the application column.
  const useGroups = showAppColumn && grouped;
  const headings = useGroups
    ? ["应用 / 目标", "协议", "节点 / 代理", "上传速度", "下载速度"]
    : [...(showAppColumn ? ["应用"] : []), "目标", "节点 / 代理", "上传速度", "下载速度"];
  return (
    <Paper sx={{ overflow: "hidden" }}>
      <Stack direction="row" sx={{ px: 2, minHeight: 54, alignItems: "center", justifyContent: "space-between", borderBottom: 1, borderColor: "divider" }}>
        <Stack direction="row" spacing={1} sx={{ alignItems: "baseline" }}>
          <Typography sx={{ fontWeight: 600 }}>当前连接</Typography>
          <Typography variant="caption" color="text.secondary">{rows.length} 个连接</Typography>
        </Stack>
        {showAppColumn && <FormControlLabel sx={{ mr: 0 }} control={
          <Checkbox size="small" checked={grouped} onChange={(_, checked) => onGroupedChange(checked)} />
        } label={<Typography variant="body2">按应用分组</Typography>} />}
      </Stack>
      <Box sx={{ height: "var(--overview-content-height, 480px)", display: "flex", flexDirection: "column" }}>
        {feedback}
      <TableContainer sx={{ flex: 1, minHeight: 0, overflow: "auto", scrollbarGutter: "stable" }}>
        <Table stickyHeader size="small" aria-label="当前连接" sx={{ tableLayout: "fixed", minWidth: 640, "& td": { px: 1.5, py: 1 }, "& th": { px: 1.5, whiteSpace: "nowrap" } }}>
          <colgroup>
            {(useGroups ? [40, 9, 21, 15, 15] : showAppColumn ? [18, 31, 21, 15, 15] : [40, 30, 15, 15])
              .map((width, index) => <col key={index} style={{ width: `${width}%` }} />)}
          </colgroup>
          <TableHead><TableRow>
            {headings.map((label, i) => <TableCell key={label} align={i >= headings.length - 2 ? "right" : "left"}>{label}</TableCell>)}
          </TableRow></TableHead>
          {useGroups ? groups.map((group, index) => {
            const open = !collapsed.has(group.key);
            const bodyId = `${id}-app-${index}`;
            return <Fragment key={group.key}>
              <TableBody><TableRow sx={{ bgcolor: "action.hover" }}>
                <TableCell colSpan={3} sx={{ p: "0 !important" }}>
                  <ButtonBase onClick={() => onCollapse(group.key)} aria-expanded={open} aria-controls={bodyId} title={group.rows[0].process || "未识别应用"}
                    aria-label={`${group.name}，${group.rows.length} 个连接`}
                    sx={{ width: "100%", justifyContent: "flex-start", gap: 0.75, px: 1.5, py: 1.25, minHeight: 52 }}>
                    {open ? <ExpandMore fontSize="small" /> : <ChevronRight fontSize="small" />}
                    <Box component="span" sx={{ fontSize: 13, fontWeight: 600, overflowWrap: "anywhere" }}>{group.name}</Box>
                    <Typography component="span" variant="caption" color="text.secondary">{group.rows.length} 个连接</Typography>
                  </ButtonBase>
                </TableCell>
                <TrafficCells traffic={sumConnections(group.rows)} stale={stale} />
              </TableRow></TableBody>
              <TableBody id={bodyId} hidden={!open} sx={{ display: open ? "table-row-group" : "none" }}>
                {group.rows.map((row) => <TableRow key={row.id}>
                  <TableCell data-selectable="true" sx={{ pl: "44px !important", overflowWrap: "anywhere" }}>{row.target}</TableCell>
                  <TableCell>{row.protocol || "—"}</TableCell>
                  <PathCell row={row} /><TrafficCells traffic={row} stale={stale} />
                </TableRow>)}
              </TableBody>
            </Fragment>;
          }) : <TableBody>{rows.map((row) => <TableRow key={row.id}>
            {showAppColumn && <TableCell title={row.process} sx={{ overflowWrap: "anywhere" }}>{connectionApp(row.process).name}</TableCell>}
            <TableCell data-selectable="true" sx={{ overflowWrap: "anywhere" }}>{row.target}</TableCell>
            <PathCell row={row} /><TrafficCells traffic={row} stale={stale} />
          </TableRow>)}</TableBody>}
          {!rows.length && <TableBody><TableRow><TableCell colSpan={headings.length} sx={{ height: 180, textAlign: "center", color: "text.secondary" }}>
            {loading ? "正在读取连接…" : stale ? "连接读取失败，请重试" : "当前没有连接"}
          </TableCell></TableRow></TableBody>}
        </Table>
      </TableContainer>
      </Box>
    </Paper>
  );
}
