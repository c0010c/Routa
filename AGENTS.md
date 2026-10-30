# 项目规则入口

## 项目技能

- [using-superpowers](.agents/skills/using-superpowers/SKILL.md)：每次开始当前项目的对话时读取，用于发现并按需使用相关技能；Codex 平台说明见技能内的 `references/codex-tools.md`。遵循上级指令及用户明确要求。
- 安装来源：`obra/superpowers` 的 `skills/using-superpowers`，版本 `2ce8aa15d05bd57213e6f746f953aafb7ded7a8b`；技能及参考文件仅安装于当前项目的 `.agents/skills/`。

## 界面提示规则

减少常驻说明：必要信息直接展示，辅助解释放入 ⓘ 或折叠区域；配置相关提示按需就近出现，重要风险明确提醒。文案简短，避免干扰主要操作。

保持布局稳定：同一弹窗或面板保持固定高度，切换标签、筛选及加载、空态时不跳动。标题和底部按钮固定，内容区域内部滚动；小屏下限制最大高度，确保操作可见。
