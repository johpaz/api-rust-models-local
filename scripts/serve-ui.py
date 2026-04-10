#!/usr/bin/env python3
"""
Mini HTTP server que:
1. Sirve la UI en :3000
2. Sirve models.json con la lista de modelos
3. Permite cambiar modelo vía POST /api/switch
"""

import http.server
import json
import os
import subprocess
import threading
from pathlib import Path
from urllib.parse import urlparse

PROJECT_ROOT = Path(__file__).resolve().parent.parent
UI_FILE = PROJECT_ROOT / "examples" / "vision-template.html"
MODELS_DIR = PROJECT_ROOT / "models"
MODELS_JSON = MODELS_DIR / "models.json"
PORT = 3001


def scan_models():
    """Generate models.json from .gguf files."""
    models = []
    if MODELS_DIR.exists():
        for f in sorted(MODELS_DIR.glob("*.gguf")):
            size = f.stat().st_size
            models.append({
                "id": f.name,
                "name": f.name,
                "size_bytes": size,
                "size_human": f"{size / (1024**3):.1f} GB",
                "path": str(f)
            })
    with open(MODELS_JSON, "w") as jf:
        json.dump({"models": models, "count": len(models)}, jf, indent=2)
    return models


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        path = urlparse(self.path).path

        if path == "/":
            # Serve UI
            if UI_FILE.exists():
                self.send_file(UI_FILE, "text/html")
            else:
                self.send_error(404, "UI not found")

        elif path == "/models.json" or path == "/api/models":
            # Return models list
            if not MODELS_JSON.exists():
                scan_models()
            if MODELS_JSON.exists():
                self.send_file(MODELS_JSON, "application/json")
            else:
                self.send_json({"models": [], "count": 0})

        elif path == "/health":
            # Proxy to llama-server
            try:
                import urllib.request
                resp = urllib.request.urlopen("http://localhost:8080/health", timeout=5)
                self.send_json(json.loads(resp.read()))
            except Exception:
                self.send_json({"status": "error", "llama-server": "unreachable"}, 503)

        else:
            self.send_error(404)

    def do_POST(self):
        path = urlparse(self.path).path

        if path == "/api/switch":
            content_length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(content_length)) if content_length else {}
            model = body.get("model", "")

            if not model:
                self.send_json({"error": "model is required"}, 400)
                return

            # Call switch script
            switch_script = PROJECT_ROOT / "scripts" / "switch-model.sh"
            try:
                result = subprocess.run(
                    [str(switch_script), model],
                    capture_output=True, text=True, timeout=130
                )
                if result.returncode == 0:
                    self.send_json({"status": "ok", "model": model})
                else:
                    self.send_json({
                        "status": "error",
                        "error": result.stderr.strip() or "Switch failed"
                    }, 500)
            except subprocess.TimeoutExpired:
                self.send_json({"error": "Timeout: model load taking too long"}, 504)
            except Exception as e:
                self.send_json({"error": str(e)}, 500)

        elif path == "/api/rescan":
            # Rescan models
            models = scan_models()
            self.send_json({"models": models, "count": len(models)})

        else:
            self.send_error(404)

    def send_file(self, path, content_type):
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        with open(path, "rb") as f:
            self.wfile.write(f.read())

    def send_json(self, data, status=200):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def log_message(self, format, *args):
        print(f"[UI] {args[0]}")


if __name__ == "__main__":
    # Initial scan
    models = scan_models()
    print(f"📋 Found {len(models)} models:")
    for m in models:
        print(f"   - {m['name']} ({m['size_human']})")

    print(f"\n🌐 UI: http://localhost:{PORT}")
    print(f"📡 llama-server: http://localhost:8080")
    print(f"📁 Models JSON: http://localhost:{PORT}/models.json")
    print(f"🔄 Switch model: POST http://localhost:{PORT}/api/switch {{\"model\": \"file.gguf\"}}")
    print(f"\n⏹️  Ctrl+C to stop\n")

    server = http.server.ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n👋 Shutting down...")
        server.shutdown()
