# Complete Meetily Setup Instructions - Cross Platform with AI Summary

.\run-docker.ps1 start -Gpu -Interactive

## Prerequisites

### System Requirements
- **RAM:** 16GB+ (8GB minimum) 
- **Storage:** 15GB+ free space
- **OS:** Windows 10/11, macOS 10.15+, or Linux
- **Internet:** Required for initial setup and model downloads

### Required Software
1. **Docker Desktop:** https://docs.docker.com/desktop/install/
2. **Git:** https://git-scm.com/downloads
3. **LLM Provider:** Choose one option below

## Part 1: Core Meetily Setup

### 1. Install Docker Desktop

**Windows:**
```powershell
# Download from https://docs.docker.com/desktop/install/windows/
# Run installer, enable WSL 2 support
# Allocate 12GB+ RAM in Docker settings
# Restart computer
```

**macOS:**
```bash
# Download from https://docs.docker.com/desktop/install/mac/
# Install and start Docker Desktop
# Allocate 12GB+ RAM in preferences
```

### 2. Clone Repository

```bash
# Clone with all dependencies
git clone --recursive https://github.com/Zackriya-Solutions/meeting-minutes.git
cd meeting-minutes

# Verify submodules loaded
ls backend/whisper.cpp/CMakeLists.txt
```

**If submodule is missing:**
```bash
# Fix corrupted submodule
git rm --cached backend/whisper.cpp
rm -rf backend/whisper.cpp .git/modules/backend/whisper.cpp
git submodule add https://github.com/Zackriya-Solutions/whisper.cpp.git backend/whisper.cpp
git submodule update --init --recursive
```

### 3. Build Docker Images

Navigate to backend:
```bash
cd backend
```

**GPU Build (NVIDIA users):**
```bash
# Windows
.\build-docker.ps1 gpu

# macOS/Linux  
./build-docker.sh gpu
```

**CPU Build (All users):**
```bash
# Windows
.\build-docker.ps1 cpu

# macOS/Linux
./build-docker.sh cpu
```

### 4. Start Backend Services

```bash
# Windows
.\run-docker.ps1 start -Interactive
.\run-docker.ps1 start -Gpu -Interactive


# macOS/Linux
./run-docker.sh start --interactive
```

**Configuration choices:**
- Model: `base` (good balance) or `medium` (better accuracy)
- Language: `auto` (automatic detection) or `en` (English only)
- Ports: Default 8178 (Whisper) and 5167 (API)

## Part 2: AI Summary Setup (Choose One Option)

Your transcription is working, but you need an LLM for generating summaries.

### Option A: Local Ollama (Recommended - Fully Offline)

**Install Ollama:**

Windows:
```powershell
# Method 1: Winget
winget install Ollama.Ollama

# Method 2: Direct download
# Download from https://ollama.com/download/windows
# Run installer
```

macOS:
```bash
# Method 1: Homebrew
brew install ollama

# Method 2: Direct download
# Download from https://ollama.com/download/mac
```

**Start Ollama and Install Models:**
```bash
# Start Ollama service
ollama serve

# In a new terminal, pull the model
ollama pull llama3.1:8b-instruct-q8_0

# Verify installation
ollama list
```

**Test Ollama Connection:**
```bash
# Test API endpoint
curl http://localhost:11434/api/version

# Test model
ollama run llama3.1:8b-instruct-q8_0 "Hello, how are you?"
```

### Option B: Claude API (Fast Setup)

**Get API Key:**
1. Sign up at https://console.anthropic.com
2. Create an API key
3. Note the key for configuration

**Configure in Meetily:**
- Provider: `anthropic`
- Model: `claude-3-sonnet-20240229`
- API Key: Your Anthropic API key

### Option C: Groq API (Fastest Inference)

**Get API Key:**
1. Sign up at https://console.groq.com
2. Create a free API key
3. Note the key for configuration

**Configure in Meetily:**
- Provider: `groq`
- Model: `llama3.1-70b-versatile`
- API Key: Your Groq API key

## Part 3: Frontend Installation

### Option A: Desktop App (Recommended)

**Windows:**
1. Download `meetily-frontend_0.0.5_x64-setup.exe` from releases
2. Right-click → Properties → Unblock → OK
3. Run installer
4. Launch from desktop/start menu

**macOS:**
```bash
# Complete installation (frontend + backend in one)
brew tap zackriya-solutions/meetily
brew install --cask meetily

# Or manual download
# Download dmg_darwin_arch64.zip from releases
# Extract and install .dmg file
```

### Option B: Web Interface

Access directly via browser:
- **Main Interface:** http://localhost:5167/docs
- **API Testing:** Use the FastAPI interface for all endpoints

## Part 4: Configuration and Testing

### 1. Configure LLM Provider

Access the settings in your chosen interface and configure:

**For Ollama (Local):**
- Provider: `ollama`
- Model: `llama3.1:8b-instruct-q8_0`
- API URL: `http://localhost:11434` (or `http://host.docker.internal:11434` for Docker)

**For Claude:**
- Provider: `anthropic`
- Model: `claude-3-sonnet-20240229`
- API Key: [Your API key]

**For Groq:**
- Provider: `groq`
- Model: `llama3.1-70b-versatile`
- API Key: [Your API key]

### 2. Test Complete Workflow

1. **Start a meeting recording**
2. **Speak or play audio** - verify transcription appears
3. **Stop recording** and request summary
4. **Verify summary generation** works without errors

### 3. Verify All Services

```bash
# Check Docker containers
docker ps

# Check service endpoints
curl http://localhost:8178  # Whisper server
curl http://localhost:5167/docs  # Meeting app API

# Check Ollama (if using)
curl http://localhost:11434/api/version
```

## Part 5: Management and Troubleshooting

### Daily Usage Commands

```bash
# Start services
cd meeting-minutes/backend
.\run-docker.ps1 start  # Windows
./run-docker.sh start   # macOS/Linux

# Stop services
.\run-docker.ps1 stop   # Windows
./run-docker.sh stop    # macOS/Linux

# View logs
.\run-docker.ps1 logs   # Windows
./run-docker.sh logs    # macOS/Linux
```

### Common Issues and Solutions

**Issue: "Ollama connection failed"**
```bash
# Solution: Ensure Ollama is running
ollama serve

# Check if accessible from Docker
docker exec -it meetily-backend curl http://host.docker.internal:11434/api/version
```

**Issue: "Model not found"**
```bash
# Pull the model
ollama pull llama3.1:8b-instruct-q8_0

# List available models
ollama list
```

**Issue: "Port already in use"**
```bash
# Find what's using the port
netstat -an | grep 5167  # Linux/macOS
netstat -an | findstr 5167  # Windows

# Kill process or change ports in configuration
```

**Issue: "Docker build failed"**
```bash
# Clean rebuild
docker system prune -f
cd meeting-minutes/backend
# Re-run build command
```

**Issue: "Frontend can't connect to backend"**
- Ensure backend is running: `docker ps`
- Check backend responds: `curl http://localhost:5167/docs`
- Restart frontend application

### Performance Optimization

**For best performance:**
- Use GPU build if you have NVIDIA GPU
- Allocate maximum RAM to Docker (16GB+ recommended)
- Use local Ollama instead of API services for lower latency
- Use smaller Whisper models (`base` vs `large`) for faster transcription

**Model size recommendations by RAM:**
- 8GB RAM: `tiny` or `base` Whisper, `llama3.1:8b` Ollama
- 16GB RAM: `medium` Whisper, `llama3.1:8b` or `llama2:13b` Ollama  
- 32GB+ RAM: `large-v3` Whisper, `llama3.1:70b` or `codellama:34b` Ollama

### Data Backup

Your meeting data is stored in Docker volumes. To backup:

```bash
# Create backup directory
mkdir meetily-backup

# Backup data
docker run --rm -v backend_meeting_app_logs:/data -v $(pwd)/meetily-backup:/backup alpine tar czf /backup/meetily-data.tar.gz /data
```

### Cross-Platform File Structure

For portable setup across Windows/macOS:
```
meetily-portable/
├── meeting-minutes/          # Git repository
├── data/                    # Persistent data
├── docker-compose.yml       # Custom Docker config
├── start-windows.ps1       # Windows launcher
├── start-macos.sh          # macOS launcher
└── ollama-models/          # Shared model storage
```

## Part 6: Advanced Configuration

### Custom Docker Compose

Create `docker-compose.override.yml`:
```yaml
version: '3.8'
services:
  meetily-backend:
    volumes:
      - ./shared-data:/app/data
      - ./custom-models:/app/models
    environment:
      - OLLAMA_HOST=host.docker.internal:11434
      - DEFAULT_MODEL=llama3.1:8b-instruct-q8_0
```

### Multiple LLM Providers

Configure fallback providers in case primary fails:
1. Primary: Ollama (local, private)
2. Fallback 1: Claude (high quality)
3. Fallback 2: Groq (fast, free tier)

## Success Verification Checklist

- [ ] Docker containers running (`docker ps` shows both services)
- [ ] Whisper transcription working (http://localhost:8178)
- [ ] Meeting API responding (http://localhost:5167/docs)
- [ ] LLM provider accessible (Ollama/Claude/Groq)
- [ ] Frontend app installed and connecting
- [ ] End-to-end test: Record → Transcribe → Summarize
- [ ] Data persistence across container restarts

This setup provides a complete, privacy-first AI meeting assistant that runs entirely on your infrastructure with professional-quality transcription and AI-powered summaries.