# QR Code Generator API

This is a Rust-based QR code generation service with the following features:

## Core Features
- Generate QR codes from text/URLs
- Customize QR code size
- Customize colors (foreground and background)
- Add custom logos in center
- Caching system for improved performance

## Technical Details

### API Endpoints
1. `GET /generate-qr`
   - Query Parameters:
     - `content`: Text/URL to encode (required)
     - `size`: Size in pixels (optional, default: 512)
     - `fg_color`: Foreground color in hex (optional, default: #000000)
     - `bg_color`: Background color in hex (optional, default: #FFFFFF)
     - `logo_url`: URL of logo to overlay (optional)

2. `GET /health`
   - Health check endpoint

### Caching System
- Uses Moka cache
- 1-hour time-to-live
- 30-minute idle timeout
- 1000 items maximum capacity
- Thread-safe concurrent access

### Example Usage
```
http://localhost:8080/generate-qr?content=https://example.com&size=512&fg_color=%23FF0000&bg_color=%23FFFFFF&logo_url=https://example.com/logo.png
```

### Logo Handling
- Automatically resizes logos
- Places logos in center
- Adds white margin for visibility
- Preserves QR code readability
- Supports transparent PNGs

### Response
- Returns PNG image directly
- Content-Type: image/png
- Binary response

## Dependencies
- actix-web: Web framework
- qrc: QR code generation
- image: Image processing
- moka: Caching
- reqwest: HTTP client
- serde: Serialization
- tokio: Async runtime
