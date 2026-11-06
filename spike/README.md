# spike: how to reproduce

Binary: download `mihomo-windows-amd64-v1.19.33.zip` from https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.33 into an empty directory, check the zip sha256 `6ab2180875f9f6fff8b19386d760daec257f66b45f72b3afc650634272fa837a`, extract `mihomo-windows-amd64.exe` to `spike/bin/` (gitignored). `spike/work/` (gitignored) is mihomo's home dir (`-d`).

Start (from `spike/work`):

    ../bin/mihomo-windows-amd64.exe -d . -f ../s1.yaml

Controller 127.0.0.1:19097, secret `spk-3f9a1c` (`-H "Authorization: Bearer spk-3f9a1c"`). Listeners 19101-19109 in s1.yaml, one per interface-name variant. s1.yaml hard-codes this machine's adapter names and GUID; edit them for another machine.

- S1/S2: `curl -x http://127.0.0.1:<port> https://www.gstatic.com/generate_204` per listener; read the mihomo log for `dial d-xxx ... error`.
- S3/S4: `python -I slowsrv.py` (endless slow stream on 127.0.0.1:19200); `curl -x http://127.0.0.1:19101 http://127.0.0.1:19200/`; then `GET /connections` and `DELETE /connections/<id>`.
- S5: copy `s5a.yaml` / `s5b.yaml` / `s5c.yaml` into `spike/work/` (the file must be inside mihomo's home dir), then
  `curl -X PUT -H "Authorization: Bearer spk-3f9a1c" -H "Content-Type: application/json" "http://127.0.0.1:19097/configs?force=true" -d '{"path":"C:/.../spike/work/s5a.yaml"}'`
- Port conflict: occupier = `python -I -c "import socket,time; s=socket.socket(); s.bind(('127.0.0.1',19120)); s.listen(5); time.sleep(90)"`, then PUT `s5b.yaml` (adds `l-occ` on 19120). Open `curl -N -H "Authorization: Bearer spk-3f9a1c" "http://127.0.0.1:19097/logs?level=error"` before the PUT to see the error. Owner check: `Get-NetTCPConnection -State Listen -LocalPort 19120 | Select OwningProcess` (compare with the mihomo PID).
- Port change to an occupied port: `s5c.yaml` is `s5b.yaml` with `l-none` port 19111 changed to 19120.

Stop mihomo and the occupier by PID afterwards (do not kill all python/mihomo processes: a user Clash Verge may be running).
