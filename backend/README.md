# Meetily Backend

FastAPI backend for meeting transcription and analysis with **Docker distribution system** for easy deployment

## Features
- **🚀 Docker-based Distribution**: Universal compatibility with comprehensive interactive setup
- **🎯 Smart Model Management**: Interactive selection from 20+ Whisper models with progress tracking
- **🌐 Multi-language Support**: Auto-detection or guided selection from 25+ languages
- **🔌 Intelligent Port Management**: Automatic port conflict detection and resolution
- **🗄️ Database Migration**: Automatic detection and migration of existing databases
- **⚡ Real-time Transcription**: Whisper-based transcription with streaming support
- **🤖 AI-Powered Analysis**: Meeting analysis with LLMs (Claude, Groq, Ollama)
- **📊 Enhanced User Experience**: Interactive prompts, health monitoring, and progress feedback
- **🔄 Easy Management**: Interactive log viewing with service control options
- **REST API endpoints** with comprehensive documentation

## ⚠️ Audio Processing Warning

**IMPORTANT: Docker Resource Requirements**

When running in Docker containers, audio processing drops can occur due to resource limitations. The audio processing system has built-in queue management that drops older audio chunks when the queue becomes full (MAX_AUDIO_QUEUE_SIZE = 10, defined in frontend/src-tauri/src/lib.rs:54).

**Symptoms of Audio Drops:**
- Log messages: "Dropped old audio chunk X due to queue overflow" (lib.rs:330)
- Missing or incomplete transcriptions
- Processing delays

**Prevention:**
- Allocate sufficient Docker resources (8GB+ RAM recommended)
- Use appropriate Whisper model size for your hardware
- Monitor container resource usage during operation

---

## 🐳 Docker Deployment (Recommended)

**The easiest way to run Meetily Backend with comprehensive interactive setup:**

```bash
# Clone and setup
git clone https://github.com/Zackriya-Solutions/meeting-minutes.git
cd meeting-minutes/backend

# Interactive setup with guided configuration
./run-docker.sh start --interactive

# Quick start with defaults
./run-docker.sh start --detach

# Management commands
./run-docker.sh status              # Check service health
./run-docker.sh logs --follow       # Interactive log viewing
./run-docker.sh models download base.en  # Pre-download models
./run-docker.sh stop                # Stop services
```

### 🎯 Interactive Setup Features

The interactive setup (`--interactive`) guides you through:

1. **🎯 Model Selection**: Choose from 20+ Whisper models with size/accuracy info
2. **🌐 Language Selection**: Select from 25+ languages or use auto-detection
3. **🔌 Port Configuration**: Smart port selection with conflict resolution
4. **🗄️ Database Setup**: Automatic detection and migration of existing databases
5. **🖥️ GPU Configuration**: Automatic GPU detection with user confirmation
6. **⚙️ Advanced Options**: Translation, speaker diarization, and more

**✅ Docker Benefits:**
- **🎯 Complete Interactive Setup**: Guided configuration for all components
- **🗄️ Database Migration**: Seamless upgrade from existing Meetily installations
- **🔌 Port Management**: Automatic conflict detection and resolution
- **📊 Progress Tracking**: Real-time download progress with size estimates
- **🔄 Enhanced Management**: Interactive log viewing with service controls
- **⚡ Health Monitoring**: Automatic service health checks and model validation
- **🌐 Universal Compatibility**: Works on any system with Docker
- **🚀 Zero Dependencies**: All libraries included in containers

For detailed Docker setup instructions, see [DOCKER_README.md](DOCKER_README.md).

### 🪟 Windows Docker Setup

**Windows users get enhanced PowerShell scripts with advanced interactive features:**

```powershell
# Clone and setup
git clone https://github.com/Zackriya-Solutions/meeting-minutes.git
cd meeting-minutes/backend

# 🎯 Complete Interactive Setup (Recommended for Windows users)
.\run-docker.ps1 start -Interactive        # Guided configuration with preferences

# ⚡ Quick Launch Options
.\run-docker.ps1 start -Detach             # Uses saved preferences or defaults
.\run-docker.ps1 start -Model large-v3 -Gpu -Detach  # Specific configuration

# 🔧 Enhanced Management & Monitoring
.\run-docker.ps1 status                    # Comprehensive health checks + connectivity tests
.\run-docker.ps1 logs -Follow -Service whisper       # Whisper server logs with controls
.\run-docker.ps1 logs -Follow -Service app           # Meeting app logs with controls
.\run-docker.ps1 models list                         # Show cached models with sizes
.\run-docker.ps1 models download large-v3            # Pre-download with progress tracking
.\run-docker.ps1 shell -Service whisper              # Interactive container access
.\run-docker.ps1 stop                                # Graceful service shutdown
```

**🚀 Windows-Specific Features:**
- **Smart Preferences System**: Automatically saves and reuses your configuration choices
- **Database Migration Assistant**: Seamless upgrade from existing Meetily installations
- **Advanced Model Selection**: Visual menu with 20+ models, size estimates, and accuracy ratings
- **Intelligent Language Detection**: Auto-detect or choose from 25+ languages
- **Port Conflict Resolution**: Smart port selection with automatic conflict detection
- **Real-time Progress Tracking**: Visual download progress with time estimates and validation
- **Interactive Log Management**: Advanced log viewing with service control options
- **Comprehensive Health Monitoring**: Automatic connectivity tests and detailed service status reporting

**⚠️ Windows Docker Resource Configuration:**

On Windows systems, Docker Desktop resource limitations can severely impact audio processing:

```powershell
# In Docker Desktop Settings -> Resources:
# - Memory: 8GB minimum (12GB+ recommended)
# - CPUs: 4+ cores
# - Disk: 20GB+

# Or configure WSL2 with .wslconfig:
[wsl2]
memory=8GB
processors=4
```

For complete Windows setup instructions, see [README-Windows.md](README-Windows.md).

---

## 🛠️ Manual Installation

If you prefer manual installation without Docker:

### Requirements
- Python 3.9+
- FFmpeg
- C++ compiler (for Whisper.cpp)
- CMake
- Git (for submodules)
- Ollama running
- API Keys (for Claude or Groq) if planning to use APIS
- ChromaDB

### Installation

### Prerequisites Installation

#### For Windows:
1. **Python 3.9+**:
   - Download and install from [Python.org](https://www.python.org/downloads/)
   - Ensure you check "Add Python to PATH" during installation
   - Verify installation: `python --version`

2. **FFmpeg**:
   - Download from [FFmpeg.org](https://ffmpeg.org/download.html) or install via [Chocolatey](https://chocolatey.org/): `choco install ffmpeg`
   - Add FFmpeg to your PATH environment variable
   - Verify installation: `ffmpeg -version`

3. **C++ Compiler**:
   - Install Visual Studio Build Tools from [Microsoft](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   - Select "Desktop development with C++" workload during installation
   - Verify installation: `cl` (should show the compiler version)

4. **CMake**:
   - Download and install from [CMake.org](https://cmake.org/download/)
   - Ensure you select "Add CMake to the system PATH" during installation
   - Verify installation: `cmake --version`

5. **Git**:
   - Download and install from [Git-scm.com](https://git-scm.com/download/win)
   - Verify installation: `git --version`

6. **Ollama**:
   - Download and install from [Ollama.com](https://ollama.com/download)
   - Start Ollama service
   - Pull required models: `ollama pull mistral` (or your preferred model)
   - Verify installation: `ollama list`

#### For macOS:
1. **Python 3.9+**:
   - Install via Homebrew: `brew install python@3.9`
   - Or download from [Python.org](https://www.python.org/downloads/)
   - Verify installation: `python3 --version`

2. **FFmpeg**:
   - Install via Homebrew: `brew install ffmpeg`
   - Verify installation: `ffmpeg -version`

3. **C++ Compiler**:
   - Install Xcode Command Line Tools: `xcode-select --install`
   - Verify installation: `clang --version`

4. **CMake**:
   - Install via Homebrew: `brew install cmake`
   - Verify installation: `cmake --version`

5. **Git**:
   - Install via Homebrew: `brew install git`
   - Or install Xcode Command Line Tools: `xcode-select --install`
   - Verify installation: `git --version`

6. **Ollama**:
   - Install via Homebrew: `brew install ollama`
   - Or download from [Ollama.com](https://ollama.com/download)
   - Start Ollama service: `ollama serve`
   - Pull required models: `ollama pull mistral` (or your preferred model)
   - Verify installation: `ollama list`



### 2. Python Dependencies
Install Python dependencies:

#### For Windows:
```cmd
python -m pip install --upgrade pip
python -m pip install -r requirements.txt
```

#### For macOS:
```bash
python3 -m pip install --upgrade pip
python3 -m pip install -r requirements.txt
```

### 3. Build Whisper Server

#### For Windows:
Run the build script which will:
- Initialize and update git submodules
- Build Whisper.cpp with custom server modifications
- Set up the server package with required files
- Download the selected Whisper model

```cmd
./build_whisper.cmd
```

If no model is specified, the script will prompt you to choose one interactively.

#### For macOS:
```bash
./build_whisper.sh
```

If you encounter permission issues, make the script executable:
```bash
chmod +x build_whisper.sh
./build_whisper.sh
```

### 4. Running the Server

#### For Windows:
The PowerShell script provides an interactive way to start the backend services:

```cmd
./start_with_output.ps1
```

Or directly with PowerShell:
```powershell
powershell -ExecutionPolicy Bypass -File start_with_output.ps1
```

The script will:
1. Check and clean up any existing processes
2. Display available models and prompt for selection
3. Download the selected model if not present
4. Start the Whisper server in a new window
5. Start the FastAPI backend in a new window

To stop all services, close the command windows or press Ctrl+C in each window.

#### For macOS:
```bash
./clean_start_backend.sh
```

If you encounter permission issues:
```bash
chmod +x clean_start_backend.sh
./clean_start_backend.sh
```

To stop all services on macOS, press Ctrl+C in the terminal or use:
```bash
pkill -f "whisper-server"
pkill -f "uvicorn main:app"
```

## API Documentation
Access Swagger UI at `http://localhost:5167/docs`

## Services
The backend runs two services:
1. Whisper.cpp Server: Handles real-time audio transcription
2. FastAPI Backend: Manages API endpoints, LLM integration, and data storage

## Platform-Specific Information

### Windows
- The Windows scripts create separate command windows for each service, allowing you to see the output in real-time
- You can check the status of services using `check_status.cmd`
- If you prefer to start services individually:
  - `start_whisper_server.cmd [model]` - Starts just the Whisper server
  - `start_python_backend.cmd [port]` - Starts just the Python backend

### macOS
- The macOS scripts run services in the foreground by default
- To run services in the background, you can use:
  ```bash
  nohup ./whisper-server/whisper-server -m ./models/ggml-base.en.bin -p 8178 > whisper.log 2>&1 &
  nohup uvicorn main:app --host 0.0.0.0 --port 5167 > backend.log 2>&1 &
  ```
- To check running services: `ps aux | grep -E "whisper-server|uvicorn"`
- To view logs: `tail -f whisper.log` or `tail -f backend.log`

## Troubleshooting

### Common Issues on Windows
- If you see "whisper-server.exe not found", run `build_whisper.cmd` first
- If a model fails to download, try running `download-ggml-model.cmd [model]` directly
- If services don't start, check if ports 8178 (Whisper) and 5167 (Backend) are available
- Ensure you have administrator privileges when running the scripts
- If PowerShell script execution is blocked, run PowerShell as administrator and use:
  ```powershell
  Set-ExecutionPolicy -ExecutionPolicy Bypass -Scope Process
  ```
- If you encounter "Access is denied" errors, try running Command Prompt as administrator
- For Visual Studio Build Tools issues, try reinstalling with the correct C++ components
- If CMake can't find the compiler, ensure Visual Studio Build Tools are properly installed and PATH variables are set

### Common Issues on macOS
- If scripts fail with "Permission denied", run `chmod +x script_name.sh` to make them executable
- If you see "command not found: python", use `python3` instead
- If building Whisper fails with compiler errors, ensure Xcode Command Line Tools are installed
- For "Port already in use" errors, find and kill the process using:
  ```bash
  lsof -i :5167  # For backend port
  lsof -i :8178  # For Whisper server port
  kill -9 PID    # Replace PID with the actual process ID
  ```
- If Ollama fails to start, check if the service is running with `ps aux | grep ollama`
- For library loading issues, ensure all dependencies are properly installed
- If you encounter "xcrun: error", reinstall the Xcode Command Line Tools:
  ```bash
  xcode-select --install
  ```
- For M1/M2 Macs, ensure you're using ARM-compatible versions of software

### General Troubleshooting
- If services fail to start, the script will automatically clean up processes
- Check logs for detailed error messages
- Ensure all ports (5167 for backend, 8178 for Whisper) are available
- Verify API keys if using Claude or Groq
- For Ollama, ensure the Ollama service is running and models are pulled
- If build fails:
  - Ensure all dependencies (CMake, C++ compiler) are installed
  - Check if git submodules are properly initialized
  - Verify you have write permissions in the directory
