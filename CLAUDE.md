# CLAUDE.md

## 网络代理

- 下载依赖或相关内容（pnpm / npm、cargo、git clone、curl 等）时，可以使用本地代理 `http://127.0.0.1:10809`。
- 不走代理时如果遇到网络问题（超时、连接失败、下载很慢），也可以改用这个代理重试。
- 用法示例（只对当前命令生效，不要改全局配置）：
  ```bash
  HTTP_PROXY=http://127.0.0.1:10809 HTTPS_PROXY=http://127.0.0.1:10809 pnpm install
  ```
