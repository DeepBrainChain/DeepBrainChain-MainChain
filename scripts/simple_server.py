#!/usr/bin/env python3

from http.server import HTTPServer, BaseHTTPRequestHandler
import json
import sys

filename = 'machine_info.json'

with open(filename) as f:
    data = f.read().encode()
 
host = ('localhost', int(sys.argv[1]))

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
