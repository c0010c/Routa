# Routa 图标（C7「钉钉蓝 · 轨道」精修版）

倾斜椭圆轨道 + 三个近大远小的节点：三个节点沿同一条轨道运行，表达「连接」与「路由」。

## 文件

| 文件 | 用途 |
|---|---|
| `svg/master.svg` | 主图标母版（48 px 及以上） |
| `svg/small.svg` | 小尺寸专用版（16 / 24 / 32 px）：圆角更小、轨道加粗、节点放大 |
| `svg/tray-dark.svg` / `svg/tray-light.svg` | 托盘单色版（透明底），分别用于深色 / 浅色任务栏 |
| `svg/tray-connected.svg` | 托盘状态：已连接（前方节点绿色） |
| `svg/tray-warning.svg` | 托盘状态：部分异常（前方节点琥珀色，远处节点变空心） |
| `svg/tray-disconnected.svg` | 托盘状态：未连接（整体灰色，轨道虚线） |
| `png/` | 以上 SVG 的各尺寸 PNG |
| `icon.ico` | 多尺寸 ICO：16 / 24 / 32 用小尺寸版，48 / 64 / 128 / 256 用主图标 |
| `build.py` | 重新生成全部 SVG；`python build.py ico` 重新打包 ICO（需先渲染 `png/`） |

## 规格

- 背景：纵向渐变 `#2BA6FF` → `#0071E6`，圆角 58 / 256（小尺寸版 44 / 256）
- 轨道与节点：纯白；远处小节点 `#BFE3FF`
- 状态色：已连接 `#4ADE80`、异常 `#FBBF24`、未连接 `#8B95A5`

## 已应用到项目

`src-tauri/icons/` 下原有的 17 个文件已全部替换：其中 `icon.ico` 用本目录打包的多尺寸版本，`32x32.png` 用小尺寸版，其余由 `tauri icon` 从 `png/master-1024.png` 生成。

托盘已接入：`png/tray-*-32.png` 拷贝到 `src-tauri/icons/tray/`，由 `src-tauri/src/tray.rs` 按连接状态（已连接 / 部分异常 / 未连接，启动中用单色版）和任务栏深浅色切换；浅色任务栏下状态图标的白色部分在运行时改为 `#1A1A1A`。
