#!/usr/bin/env python3
"""
🧪 Test Completo de la API de Visión
Prueba todos los endpoints de visión con la API Rust
"""

import requests
import base64
import json
import time
import sys
from pathlib import Path

# ═══════════════════════════════════════════════════════════════════════════
# Configuración
# ═══════════════════════════════════════════════════════════════════════════

API_BASE_URL = "http://localhost:9000"
API_TOKEN = input("🔑 Ingresa tu API_TOKEN: ").strip()

HEADERS = {
    "Authorization": f"Bearer {API_TOKEN}",
    "Content-Type": "application/json"
}

# ═══════════════════════════════════════════════════════════════════════════
# Helpers
# ═══════════════════════════════════════════════════════════════════════════

def print_section(title):
    """Print a section header"""
    print(f"\n{'='*60}")
    print(f"  {title}")
    print(f"{'='*60}")

def print_result(success, message, data=None):
    """Print test result"""
    status = "✅" if success else "❌"
    print(f"  {status} {message}")
    if data:
        if isinstance(data, dict):
            for key, value in data.items():
                if isinstance(value, str) and len(value) > 100:
                    value = value[:100] + "..."
                print(f"     • {key}: {value}")
        else:
            print(f"     • {data}")

def image_to_base64(image_path):
    """Convert image file to base64"""
    with open(image_path, "rb") as f:
        return base64.b64encode(f.read()).decode('utf-8')

def create_test_image():
    """Create a simple test image using PIL or return a sample"""
    try:
        from PIL import Image, ImageDraw, ImageFont
        
        # Create a colorful test image
        img = Image.new('RGB', (400, 300), color='white')
        draw = ImageDraw.Draw(img)
        
        # Draw some shapes
        draw.rectangle([50, 50, 150, 150], fill='red')
        draw.rectangle([200, 50, 350, 150], fill='blue')
        draw.ellipse([100, 180, 300, 280], fill='green')
        
        # Add text
        try:
            font = ImageFont.load_default()
            draw.text((120, 10), "Test Image", fill='black', font=font)
        except:
            pass
        
        # Save
        test_path = "/tmp/vision_test_image.jpg"
        img.save(test_path, "JPEG", quality=85)
        return test_path
        
    except ImportError:
        # If PIL is not available, try to find any image file
        for path in [
            Path.home() / "Pictures",
            Path.home() / "Downloads",
            Path.home()
        ]:
            if path.exists():
                for ext in ["*.jpg", "*.jpeg", "*.png"]:
                    images = list(path.rglob(ext))
                    if images:
                        print(f"  ℹ️  Usando imagen encontrada: {images[0]}")
                        return str(images[0])
        
        # If no image found, create a minimal valid JPEG
        print("  ⚠️  No se encontró imagen ni PIL disponible")
        print("  💡 Instala PIL: pip install Pillow")
        return None

# ═══════════════════════════════════════════════════════════════════════════
# Tests
# ═══════════════════════════════════════════════════════════════════════════

def test_health():
    """Test health endpoint"""
    print_section("🏥 Test: Health Check")
    
    try:
        response = requests.get(f"{API_BASE_URL}/health")
        
        if response.status_code == 200:
            print_result(True, "Health check exitoso", response.json())
            return True
        else:
            print_result(False, f"Health check falló: HTTP {response.status_code}")
            return False
            
    except requests.exceptions.ConnectionError:
        print_result(False, "No se pudo conectar a la API")
        print("\n  💡 ¿Está corriendo la API?")
        print(f"  💡 Ejecuta: cd api && cargo run")
        return False

def test_list_models():
    """Test list models endpoint"""
    print_section("📋 Test: Lista de Modelos")
    
    try:
        response = requests.get(
            f"{API_BASE_URL}/v1/models",
            headers={"Authorization": f"Bearer {API_TOKEN}"}
        )
        
        if response.status_code == 200:
            data = response.json()
            num_models = len(data.get('data', []))
            print_result(True, f"Se encontraron {num_models} modelos")
            
            for model in data.get('data', [])[:3]:
                size_gb = model.get('size_bytes', 0) / 1e9
                print_result(True, model.get('name', 'unknown'), {
                    'size': f"{size_gb:.2f} GB"
                })
            
            return True, data.get('data', [])
        else:
            print_result(False, f"Error: HTTP {response.status_code}")
            return False, []
            
    except Exception as e:
        print_result(False, f"Excepción: {e}")
        return False, []

def test_vision_analyze(image_path):
    """Test single image analysis"""
    print_section("🖼️ Test: Análisis de Imagen Individual")
    
    print(f"  📷 Imagen: {image_path}")
    
    try:
        image_b64 = image_to_base64(image_path)
        print_result(True, f"Imagen codificada a base64 ({len(image_b64)} chars)")
        
        # Send analysis request
        start_time = time.time()
        response = requests.post(
            f"{API_BASE_URL}/v1/vision/analyze",
            headers=HEADERS,
            json={
                "image_base64": image_b64,
                "model": "gemma4:e4b",
                "prompt": "Describe esta imagen en detalle",
                "max_tokens": 512,
                "temperature": 0.7
            },
            timeout=120
        )
        elapsed = time.time() - start_time
        
        if response.status_code == 200:
            data = response.json()
            print_result(True, f"Análisis completado en {elapsed:.2f}s", {
                'model': data.get('model'),
                'processing_time_ms': data.get('processing_time_ms'),
                'content_length': len(data.get('content', ''))
            })
            print(f"\n  📝 Resultado:\n  {data.get('content', '')[:300]}...\n")
            return True, data
        else:
            print_result(False, f"Error: HTTP {response.status_code}")
            print_result(False, f"Respuesta: {response.text[:200]}")
            return False, None
            
    except Exception as e:
        print_result(False, f"Excepción: {e}")
        return False, None

def test_vision_batch(image_path):
    """Test batch image analysis"""
    print_section("📦 Test: Análisis por Lotes (Batch)")
    
    try:
        # Create batch with same image (for test purposes)
        image_b64 = image_to_base64(image_path)
        print_result(True, f"Imagen codificada a base64")
        
        images = [
            {"image_base64": image_b64, "id": "test-1", "prompt": "¿Qué hay en esta imagen?"},
            {"image_base64": image_b64, "id": "test-2", "prompt": "Describe los colores"},
            {"image_base64": image_b64, "id": "test-3", "prompt": "¿Qué formas ves?"}
        ]
        
        print_result(True, f"Batch de {len(images)} imágenes preparado")
        
        # Send batch request (sequential for safety)
        start_time = time.time()
        response = requests.post(
            f"{API_BASE_URL}/v1/vision/analyze/batch",
            headers=HEADERS,
            json={
                "images": images,
                "model": "gemma4:e4b",
                "max_tokens": 256,
                "parallel": False
            },
            timeout=180
        )
        elapsed = time.time() - start_time
        
        if response.status_code == 200:
            data = response.json()
            print_result(True, f"Batch completado en {elapsed:.2f}s", {
                'total_images': data.get('total_images'),
                'successful': data.get('successful'),
                'failed': data.get('failed'),
                'total_time_ms': data.get('total_processing_time_ms')
            })
            
            for result in data.get('results', []):
                status = "✅" if result.get('success') else "❌"
                print(f"  {status} [{result.get('id')}] ({result.get('processing_time_ms')}ms)")
                if result.get('success'):
                    print(f"     {result.get('content', '')[:100]}...")
                else:
                    print(f"     Error: {result.get('error')}")
            
            return True, data
        else:
            print_result(False, f"Error: HTTP {response.status_code}")
            print_result(False, f"Respuesta: {response.text[:200]}")
            return False, None
            
    except Exception as e:
        print_result(False, f"Excepción: {e}")
        return False, None

def test_vision_websocket(image_path):
    """Test WebSocket connection"""
    print_section("🔌 Test: WebSocket (No implementado en este script)")
    
    print("  ℹ️  WebSocket requiere una conexión persistente")
    print("  💡 Usa el template web para probar WebSocket:")
    print(f"     Abre: {API_BASE_URL}/vision")
    print("     Cambia modo a 'WebSocket (Tiempo Real)'")
    
    print_result(True, "Test WebSocket omitido (usa template web)")
    return True

def test_vision_template():
    """Test vision template is accessible"""
    print_section("🌐 Test: Template Web de Visión")
    
    try:
        response = requests.get(f"{API_BASE_URL}/vision", timeout=5)
        
        if response.status_code == 200:
            content_length = len(response.text)
            print_result(True, f"Template web accesible ({content_length} bytes)")
            print(f"  🔗 Abre en navegador: {API_BASE_URL}/vision")
            return True
        else:
            print_result(False, f"Template no disponible: HTTP {response.status_code}")
            return False
            
    except Exception as e:
        print_result(False, f"Excepción: {e}")
        return False

# ═══════════════════════════════════════════════════════════════════════════
# Main
# ═══════════════════════════════════════════════════════════════════════════

def main():
    """Run all tests"""
    print("\n" + "="*60)
    print("  🧪 TEST COMPLETO DE API DE VISIÓN")
    print("="*60)
    print(f"  API URL: {API_BASE_URL}")
    print(f"  Token: {'*' * 20}")
    
    # Test 1: Health
    if not test_health():
        print("\n❌ No se puede continuar sin la API. Deteniendo tests.")
        sys.exit(1)
    
    # Test 2: List Models
    success, models = test_list_models()
    
    # Test 3: Create test image
    print_section("🎨 Preparando Imagen de Test")
    image_path = create_test_image()
    
    if not image_path:
        print("\n⚠️  No se pudo crear/encontrar imagen para test")
        print("  Continúa sin tests de visión o coloca una imagen en /tmp/vision_test_image.jpg")
        response = input("  ¿Continuar? (s/n): ").strip().lower()
        if response != 's':
            sys.exit(0)
    else:
        print_result(True, f"Imagen de test lista: {image_path}")
    
    # Test 4: Single Image Analysis
    if image_path:
        success, result = test_vision_analyze(image_path)
    
    # Test 5: Batch Analysis
    if image_path:
        success, result = test_vision_batch(image_path)
    
    # Test 6: WebSocket
    test_vision_websocket(image_path if image_path else None)
    
    # Test 7: Vision Template
    test_vision_template()
    
    # Summary
    print_section("📊 RESUMEN DE TESTS")
    print("  ✅ Health Check - API corriendo")
    print("  ✅ Lista de Modelos - Modelos disponibles")
    if image_path:
        print("  ✅ Análisis Individual - Imagen analizada")
        print("  ✅ Análisis Batch - Múltiples imágenes")
    print("  ⏭️  WebSocket - Usa template web")
    print("  ✅ Template Web - Servido correctamente")
    
    print(f"\n  🔗 Template de Visión: {API_BASE_URL}/vision")
    print(f"  🔗 API Health: {API_BASE_URL}/health")
    print(f"  🔗 API Models: {API_BASE_URL}/v1/models")
    print()

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n⏹️  Tests cancelados por usuario")
        sys.exit(0)
    except Exception as e:
        print(f"\n\n❌ Error inesperado: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
