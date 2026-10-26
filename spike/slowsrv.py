import http.server, socketserver, time, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200); self.send_header("Content-Length","100000000"); self.end_headers()
        try:
            while True:
                self.wfile.write(b"x"*100); self.wfile.flush(); time.sleep(0.2)
        except Exception: pass
    def log_message(self,*a): pass
class S(socketserver.ThreadingTCPServer): allow_reuse_address=True; daemon_threads=True
S(("127.0.0.1",19200),H).serve_forever()
