import DeleteOutline from "@mui/icons-material/DeleteOutlined";
import FolderOutlined from "@mui/icons-material/FolderOutlined";
import HelpOutline from "@mui/icons-material/HelpOutlineOutlined";
import InsertDriveFileOutlined from "@mui/icons-material/InsertDriveFileOutlined";
import {
  Autocomplete,
  Box,
  Checkbox,
  Chip,
  Divider,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Menu,
  MenuItem,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import { useImperativeHandle, useState, type ReactNode, type Ref } from "react";
import type { ProcessEntry, ProcessKind, ProcessRule, RunningProcess } from "../bindings";
import {
  chipLabel,
  cleanEntry,
  defaultKind,
  entryError,
  fileName,
  groupsOf,
  pathOwners,
  ruleLabel,
  type ProcessGroup,
} from "../lib/processRule";

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
const uniq = (xs: string[]) => xs.filter((x, i) => xs.findIndex((y) => same(x, y)) === i);
const mono = { fontFamily: "Consolas, monospace", wordBreak: "break-all" } as const;

const HELP =
  "所有位置（按进程名）：同名程序装在哪里都会命中。\n指定位置（按路径）：只命中这个位置的程序。\n同一个程序可以两个都选。\n同名程序：填了路径的规则优先。";

function Help({ children }: { children: ReactNode }) {
  return (
    <Tooltip title={<Box sx={{ whiteSpace: "pre-line" }}>{children}</Box>}>
      <HelpOutline sx={{ fontSize: 16, color: "text.secondary", verticalAlign: "-3px", ml: 0.5, cursor: "help" }} />
    </Tooltip>
  );
}

export interface ProcessEntriesHandle {
  /** 把输入框里还没回车的文字当作一个条目加上：返回加上后的全部条目；不能添加时在字段里显示原因并返回 `null`。 */
  commit(): ProcessEntry[] | null;
}

/** 进程：一个程序一个标签，点开多选「所有位置」和具体路径。 */
export function ProcessEntriesField({
  ref,
  entries,
  others,
  routingEnabled,
  running,
  error,
  onChange,
}: {
  ref?: Ref<ProcessEntriesHandle>;
  entries: ProcessEntry[];
  others: ProcessRule[];
  routingEnabled: boolean;
  running: RunningProcess[];
  error?: string;
  onChange(entries: ProcessEntry[]): void;
}) {
  const [input, setInput] = useState("");
  const [err, setErr] = useState<string>();
  const [menu, setMenu] = useState<{ anchor: HTMLElement; key: string }>();
  const groups = groupsOf(entries);
  const options = running.filter((r, i) => running.findIndex((x) => same(x.exe_path, r.exe_path)) === i);
  const pathsOf = (name: string) => options.filter((r) => same(r.name, name)).map((r) => r.exe_path);
  const isRunning = (path: string) => running.some((r) => same(r.exe_path, path));
  const mk = (kind: ProcessKind, value: string): ProcessEntry => ({ kind, value });
  const errOf = (e: ProcessEntry) => entryError(e, [...entries, e], others);

  /** 下拉里选一个运行中的程序：优先按进程名；进程名被别的规则占了就按这个路径。 */
  const pickPlan = (o: RunningProcess): { e?: ProcessEntry; why?: string } => {
    const n = mk("Name", o.name);
    if (!errOf(n)) return { e: n };
    const p = mk("Path", o.exe_path);
    const pe = errOf(p);
    return pe ? { why: pe } : { e: p };
  };

  /** 返回加上后的全部条目；不能添加时显示原因并返回 `null`。 */
  const add = (e: ProcessEntry): ProcessEntry[] | null => {
    const msg = errOf(e);
    if (msg) {
      setErr(`${fileName(e.value)}：${msg}`);
      return null;
    }
    const next = [...entries, e];
    setErr(undefined);
    setInput("");
    onChange(next);
    return next;
  };
  const addRaw = (raw: string): ProcessEntry[] | null => {
    const v = cleanEntry(raw);
    if (!v) return entries;
    const kind = defaultKind(v);
    return add(mk(kind, kind === "Name" ? fileName(v) : v));
  };
  useImperativeHandle(ref, () => ({ commit: () => addRaw(input) }));

  const g = groups.find((x) => x.key === menu?.key);
  const candidates = g ? uniq([...g.paths, ...pathsOf(g.name)]) : [];
  const checkedCount = g ? (g.hasName ? 1 : 0) + g.paths.length : 0;
  const toggle = (kind: ProcessKind, value: string, on: boolean) => {
    if (on) add(mk(kind, value));
    else
      onChange(
        entries.filter((x) => !(x.kind === kind && (kind === "Path" ? same(x.value, value) : same(fileName(x.value), value)))),
      );
  };
  const takenByPath = g ? pathOwners(g.name, others, routingEnabled) : [];
  const shownError = err ?? error;

  return (
    <Box>
      <Autocomplete<RunningProcess | ProcessGroup, true, false, true>
        multiple
        freeSolo
        options={options}
        value={groups}
        inputValue={input}
        onInputChange={(_, v, reason) => {
          if (reason === "reset") return;
          setInput(v);
          setErr(undefined);
        }}
        getOptionLabel={(o) => (typeof o === "string" ? o : o.name)}
        getOptionKey={(o) => (typeof o === "string" ? o : "exe_path" in o ? o.exe_path : o.key)}
        isOptionEqualToValue={() => false}
        filterOptions={(opts, s) => {
          const q = s.inputValue.toLowerCase();
          return (opts as RunningProcess[]).filter((o) => `${o.name} ${o.exe_path}`.toLowerCase().includes(q));
        }}
        getOptionDisabled={(o) => "exe_path" in o && !pickPlan(o).e}
        onChange={(_, value, reason) => {
          if (reason === "removeOption" || reason === "clear") {
            const keep = new Set(value.filter((v): v is ProcessGroup => typeof v !== "string" && "key" in v).map((v) => v.key));
            onChange(entries.filter((x) => keep.has(fileName(x.value).toLowerCase())));
            return;
          }
          const last = value[value.length - 1];
          if (last === undefined) return;
          if (typeof last === "string") addRaw(last);
          else if ("exe_path" in last) {
            const { e, why } = pickPlan(last);
            if (e) add(e);
            else setErr(`${last.name}：${why}`);
          }
        }}
        renderOption={({ key, ...props }, o) => {
          const p = o as RunningProcess;
          const { e, why } = pickPlan(p);
          const dupName = pathsOf(p.name).length > 1;
          return (
            <Box component="li" key={key} {...props}>
              <InsertDriveFileOutlined fontSize="small" sx={{ mr: 1, color: "text.secondary" }} />
              <Box sx={{ minWidth: 0, flex: 1 }}>
                <Typography variant="body2">{p.name}</Typography>
                <Typography variant="caption" sx={{ color: "text.secondary", wordBreak: "break-all" }} component="div">
                  {p.exe_path}
                </Typography>
                {(why || e?.kind === "Path" || dupName) && (
                  <Typography variant="caption" sx={{ color: why ? "error.main" : "text.secondary" }} component="div">
                    {why ?? (e?.kind === "Path" ? "进程名已被其他规则占用，将按这个路径添加" : "同名程序有多个位置，添加后可点标签选择具体位置")}
                  </Typography>
                )}
              </Box>
            </Box>
          );
        }}
        renderValue={(value, getItemProps) =>
          value.map((v, index) => {
            const gr = v as ProcessGroup;
            const { key, ...itemProps } = getItemProps({ index });
            const tip = [...(gr.hasName ? ["所有位置（按进程名）"] : []), ...gr.paths, "", "点击修改"].join("\n");
            return (
              <Tooltip key={key} describeChild title={<Box sx={{ whiteSpace: "pre-line", wordBreak: "break-all" }}>{tip}</Box>}>
                <Chip
                  {...itemProps}
                  size="small"
                  icon={gr.paths.length ? <FolderOutlined /> : undefined}
                  label={chipLabel(gr)}
                  onClick={(e) => setMenu({ anchor: e.currentTarget, key: gr.key })}
                />
              </Tooltip>
            );
          })
        }
        renderInput={(params) => (
          <TextField
            {...params}
            label={
              <>
                进程
                <Help>{HELP}</Help>
              </>
            }
            size="small"
            placeholder={entries.length ? "" : "如 ChatGPT.exe，回车添加；也可以粘贴完整路径"}
            error={!!shownError}
            helperText={shownError ?? "点击程序可选择匹配所有位置，或指定位置。"}
            slotProps={{ ...params.slotProps, inputLabel: { ...params.slotProps?.inputLabel, sx: { pointerEvents: "auto" } } }}
          />
        )}
      />
      <Menu anchorEl={menu?.anchor} open={!!g} onClose={() => setMenu(undefined)} slotProps={{ paper: { sx: { maxWidth: 520 } } }}>
        {g && [
          <ListSubheader key="h0" sx={{ lineHeight: "32px", fontWeight: 600, color: "text.primary" }}>
            {g.name}
          </ListSubheader>,
          (() => {
            const why = g.hasName ? undefined : errOf(mk("Name", g.name));
            const last = g.hasName && checkedCount === 1;
            const note = takenByPath.length
              ? `其中 ${takenByPath.map((t) => `「${ruleLabel(t.rule)}」`).join("")}指定的位置走那条规则`
              : "同名程序装在哪里都会命中";
            return (
              <MenuItem key="name" disabled={!!why || last} onClick={() => toggle("Name", g.name, !g.hasName)}>
                <ListItemIcon>
                  <Checkbox size="small" edge="start" checked={g.hasName} tabIndex={-1} disableRipple sx={{ p: 0 }} />
                </ListItemIcon>
                <ListItemText primary="所有位置（按进程名）" secondary={why ?? (last ? `${note}（至少保留一项）` : note)} />
              </MenuItem>
            );
          })(),
          <ListSubheader key="h1" sx={{ lineHeight: "32px" }}>
            指定位置（按路径）
          </ListSubheader>,
          ...(candidates.length === 0
            ? [
                <MenuItem key="none" disabled>
                  <ListItemIcon />
                  <ListItemText primary={`${g.name} 没在运行`} secondary="在输入框粘贴完整路径即可添加位置" />
                </MenuItem>,
              ]
            : candidates.map((c) => {
                const on = g.paths.some((x) => same(x, c));
                const why = on ? undefined : errOf(mk("Path", c));
                const last = on && checkedCount === 1;
                return (
                  <MenuItem key={c} disabled={!!why || last} onClick={() => toggle("Path", c, !on)}>
                    <ListItemIcon>
                      <Checkbox size="small" edge="start" checked={on} tabIndex={-1} disableRipple sx={{ p: 0 }} />
                    </ListItemIcon>
                    <ListItemText
                      primary={c}
                      secondary={why ?? ([isRunning(c) ? "" : "当前没在运行", last ? "至少保留一项" : ""].filter(Boolean).join("；") || undefined)}
                      slotProps={{ primary: { sx: { ...mono, fontSize: 13 } } }}
                    />
                  </MenuItem>
                );
              })),
          <Divider key="d2" />,
          <MenuItem
            key="del"
            onClick={() => {
              onChange(entries.filter((x) => fileName(x.value).toLowerCase() !== g.key));
              setMenu(undefined);
            }}
          >
            <ListItemIcon>
              <DeleteOutline fontSize="small" />
            </ListItemIcon>
            <ListItemText primary="删除" />
          </MenuItem>,
        ]}
      </Menu>
    </Box>
  );
}
