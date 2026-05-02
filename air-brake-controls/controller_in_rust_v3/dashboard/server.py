import http.server
import socketserver
import urllib.parse
import subprocess
import json

PORT = 8000

class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == '/simulate':
            params = urllib.parse.parse_qs(parsed.query)
            alt = params.get('alt', ['800.0'])[0]
            vel = params.get('vel', ['240.0'])[0]
            
            try:
                # Run the rust binary
                result = subprocess.run(
                    ['../target/debug/examples/closed_loop_sim', alt, vel, '--json'],
                    capture_output=True, text=True, check=True
                )
                self.send_response(200)
                self.send_header('Content-type', 'application/json')
                self.end_headers()
                self.wfile.write(result.stdout.encode())
            except subprocess.CalledProcessError as e:
                self.send_response(500)
                self.send_header('Content-type', 'application/json')
                self.end_headers()
                self.wfile.write(json.dumps({'error': e.stderr}).encode())
        else:
            super().do_GET()

class ReusableTCPServer(socketserver.TCPServer):
    allow_reuse_address = True

with ReusableTCPServer(("", PORT), Handler) as httpd:
    print(f"Serving at http://localhost:{PORT}")
    httpd.serve_forever()
