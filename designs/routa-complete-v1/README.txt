Routa 完整交互与 UI 设计 v1
==========================

打开 index.html 查看可点击原型，或使用当前预览地址：
http://127.0.0.1:48773/

左侧为设计目录，不属于产品界面；可以直接查看不同页面和异常状态。
右上角“模拟结果”可以切换正常、网络失败、权限拒绝和端口冲突。
点击“重置演示”回到首次启动，所有演示修改仅保存在当前页面内存。

文件：
- index.html / prototype.js / styles.css：可编辑交互原型
- icons.js：项目现有 Material 图标库的离线路径
- specification.html：交互规则、状态、视觉规范和实施边界
- design-tokens.json：颜色、字号、间距、窗口和导航图标
- design-overview.png：关键界面总览
- screens/index.html：全部 29 个页面 / 状态的截图目录
- verification.txt：原型验证记录

双击本地 index.html 可以离线查看；不需要安装 npm 依赖。
若浏览器不允许直接运行本地页面，可在本目录启动：
python -m http.server 48773 --bind 127.0.0.1

重要边界：这是设计原型，所有节点、运行数据、组件安装、系统代理、TUN、
Tailscale 登录、程序选择器和复制反馈均为示例或模拟，不操作真实网络配置。
具体待实现能力见 specification.html 第 10 节。

本轮只新增 designs/routa-complete-v1 下的设计资产，没有修改应用生产代码。
