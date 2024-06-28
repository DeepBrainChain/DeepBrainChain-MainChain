#!/usr/bin/env python3

# example: python simple_server.py a.json 8000
# 在8000端口返回该json文件

from http.server import HTTPServer, BaseHTTPRequestHandler
import json
import sys

# filename = 'machine_info.json'

with open(sys.argv[1]) as f:
    data = f.read().encode()

host = ('0.0.0.0', int(sys.argv[2]))

class Response(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.end_headers()
        self.wfile.write(data)

if __name__ == '__main__':
    server = HTTPServer(host, Response)
    print("Starting server, listen at: %s:%s" %host)
    server.serve_forever()
