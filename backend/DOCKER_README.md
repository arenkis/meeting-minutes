# Whisper Docker Distribution System

A **Docker-based distribution system** for the Whisper speech-to-text server with integrated Meeting Summarizer App that solves cross-platform executable distribution challenges. This project wraps the whisper.cpp server in Docker containers with automatic GPU detection, model management, universal compatibility, and includes a FastAPI-based meeting transcript processor with AI summarization capabilities.

## ⚠️ Audio Processing Warning

**IMPORTANT: Docker Resource Limitations and Audio Drops**

When running in Docker containers, audio processing drops can occur if the container doesn't have sufficient resources or if transcription processing falls behind. This is controlled by:

- **Queue Size Limit**: `MAX_AUDIO_QUEUE_SIZE = 10` (frontend/src-tauri/src/lib.rs:54)
- **Drop Behavior**: When queue is full, older audio chunks are dropped (lib.rs:330-333)

**Recommended Docker Settings to Prevent Audio Drops:**
```bash
# Increase container resources
docker run --memory=4g --cpus=2 ...

# Or in docker-compose.yml:
services:
  whisper:
    deploy:
      resources:
        limits:
          memory: 4G
          cpus: '2'
        reservations:
          memory: 2G
          cpus: '1'
```

**Monitor for Audio Drops:**
- Watch logs for "Dropped old audio chunk" messages
- If drops occur frequently, increase container memory/CPU
- Consider using smaller Whisper models (base vs large-v3) for better performance

---

## ✅ Working Solution

This Docker distribution has been **tested and verified** to work perfectly:
- ✅ **Builds successfully** on Mac (ARM64) and Linux (AMD64)
- ✅ **Auto-detects GPU** (NVIDIA, Metal, OpenCL) with CPU fallback
- ✅ **Interactive model selection** with guided setup and progress tracking
- ✅ **Smart downloads** with real-time progress and size estimates
- ✅ **Serves Whisper web interface** at http://localhost:8178
- ✅ **Meeting App API** at http://localhost:5167 with AI summarization
- ✅ **Enhanced user experience** with intelligent prompts and health monitoring
- ✅ **Database migration** from existing Meetily installations
- ✅ **Cross-platform compatible** - single solution for all systems

## 🚀 Interactive Setup Features

- **🎯 Smart Model Selection**: Choose from 20+ models with interactive menu, size estimates, and accuracy trade-offs
- **🌐 Intelligent Language Support**: Auto-detection or guided selection from 25+ languages with performance optimization
- **🔌 Port Management**: Automatic port conflict detection with resolution options
- **🗄️ Database Migration**: Automatic detection and migration of existing Meetily databases
- **🖥️ GPU Configuration**: Automatic GPU detection with user confirmation
- **📊 Real-time Progress Tracking**: Visual download progress with time estimates and file validation
- **🔄 Interactive Log Management**: Press Ctrl+C during log viewing for service control options (continue, exit, restart, status)
- **💡 Context-aware Assistance**: Smart prompts for missing options with clear guidance and troubleshooting
- **⚡ Health Monitoring**: Automatic service health checks with connectivity tests and status reporting
- **🛠️ Enhanced Error Handling**: Clear error messages with actionable solutions and recovery steps

## Why Docker Solution?

**The Problem with Static Builds:**
- GPU libraries (CUDA, Metal, OpenCL) may not be available on target systems
- Runtime library dependencies and architecture mismatches
- Different CPU features and system configurations
- Complex distribution of executables with all dependencies

**Docker Solution Benefits:**
- ✅ **Universal Compatibility**: Works on any system with Docker installed
- ✅ **Automatic GPU Detection**: Uses GPU acceleration when available, gracefully falls back to CPU
- ✅ **Cross-Platform**: Supports AMD64 and ARM64 architectures  
- ✅ **Zero Dependencies**: All libraries included in container
- ✅ **Easy Distribution**: Single command deployment anywhere
- ✅ **Model Management**: Automatic model downloading and caching with validation
- ✅ **Port Management**: Automatic conflict detection and resolution
- ✅ **Database Migration**: Seamless upgrade from existing Meetily installations
- ✅ **AI Integration**: Built-in meeting summarization with multiple AI providers
- ✅ **Interactive Setup**: Comprehensive guided configuration for all components
- ✅ **Production Ready**: Health checks, logging, resource limits, security

## Quick Start

### 1. Prerequisites

**Required:**
- Docker Desktop (Mac/Windows) or Docker Engine (Linux)
- Git (for cloning repository)
- sqlite3 (for database operations)
- 2GB+ RAM, 1GB+ disk space

**For GPU Support:**
- NVIDIA GPU with drivers
- NVIDIA Docker runtime (`nvidia-docker2`)

### 2. Clone and Setup

```bash
# Clone the repository
git clone https://github.com/Zackriya-Solutions/meeting-minutes.git
cd meeting-minutes/backend
```

### 3. Database Setup (First Time)

```bash
# Interactive setup - migrates existing database or creates new one
./run-docker.sh setup-db

# Auto-detect existing Meetily installation
./run-docker.sh setup-db --auto

# Fresh installation (no existing database)
./run-docker.sh setup-db --fresh

# Custom database path
./run-docker.sh setup-db --db-path /path/to/meeting_minutes.db
```

### 4. Build the Images

**Linux/macOS (Bash):**
```bash
# Build whisper CPU server + meeting app (recommended)
./build-docker.sh cpu

# Build whisper GPU server + meeting app
./build-docker.sh gpu

# Build both whisper server versions + meeting app
./build-docker.sh both

# Build without cache
./build-docker.sh cpu --no-cache

# Build for different architecture
./build-docker.sh cpu --platforms linux/amd64

# Note: Meeting app is always built together with whisper server as a package
```

**Windows (Enhanced PowerShell):**
```powershell
# Build whisper CPU server + meeting app (universal compatibility)
.\build-docker.ps1 cpu

# Build whisper GPU server + meeting app (NVIDIA CUDA support)
.\build-docker.ps1 gpu

# Build macOS-optimized server + meeting app (Apple Silicon compatibility)
.\build-docker.ps1 macos

# Build all versions (CPU + GPU + Meeting App)
.\build-docker.ps1 both

# Advanced build options
.\build-docker.ps1 cpu -NoCache                    # Build without cache
.\build-docker.ps1 cpu -Platforms "linux/amd64"   # Single platform
.\build-docker.ps1 gpu -BuildArgs "CUDA_VERSION=12.1.1"  # Custom CUDA version
.\build-docker.ps1 both -Registry "your-username" -Push  # Build and push to registry
.\build-docker.ps1 cpu -Tag "custom-build" -DryRun       # Test commands without executing
```

### 5. Start Services

**🎯 Interactive Setup (Recommended):**

**Linux/macOS (Bash):**
```bash
# Complete interactive setup - prompts for all configuration options
./run-docker.sh start --interactive

# Guided setup includes:
# - Model selection (20+ options with size/accuracy info)
# - Language selection (25+ languages or auto-detection)
# - Port configuration (automatic conflict detection)
# - Database setup (migration from existing installations)
# - GPU configuration (automatic detection with user confirmation)
# - Advanced options (translation, diarization, etc.)

# Quick start with intelligent defaults
./run-docker.sh start --detach
```

**Windows (Enhanced PowerShell):**
```powershell
# 🎯 Complete Interactive Setup (Recommended) - Guided configuration with preferences
.\run-docker.ps1 start -Interactive

# Quick start with intelligent defaults (uses saved preferences)
.\run-docker.ps1 start -Detach

# The interactive setup includes:
# - Smart Preferences System: Automatically saves and reuses your configuration choices
# - Advanced Model Selection: Visual menu with 20+ models, size estimates, and accuracy ratings
# - Intelligent Language Detection: Auto-detect or choose from 25+ languages
# - Port Conflict Resolution: Smart port selection with automatic conflict detection
# - Database Migration Assistant: Seamless upgrade from existing Meetily installations
# - Real-time Progress Tracking: Visual download progress with time estimates and validation
```

**⚡ Quick Commands:**

**Linux/macOS (Bash):**
```bash
# Start with large model on custom port  
./run-docker.sh start --model large-v3 --port 8081 --detach

# Start with GPU and custom language
./run-docker.sh start --gpu --language es --detach

# Start with translation enabled
./run-docker.sh start --model base --translate --language auto --detach

# Language-specific examples
./run-docker.sh start --model base --language en --detach      # English
./run-docker.sh start --model base --language es --detach      # Spanish
./run-docker.sh start --model base --language fr --detach      # French
./run-docker.sh start --model base --language de --detach      # German
./run-docker.sh start --model base --language it --detach      # Italian
./run-docker.sh start --model base --language pt --detach      # Portuguese
./run-docker.sh start --model base --language ru --detach      # Russian
./run-docker.sh start --model base --language ja --detach      # Japanese
./run-docker.sh start --model base --language ko --detach      # Korean
./run-docker.sh start --model base --language zh --detach      # Chinese
./run-docker.sh start --model base --language ar --detach      # Arabic
./run-docker.sh start --model base --language hi --detach      # Hindi
./run-docker.sh start --model base --language tr --detach      # Turkish
./run-docker.sh start --model base --language pl --detach      # Polish
./run-docker.sh start --model base --language nl --detach      # Dutch
./run-docker.sh start --model base --language auto --detach    # Auto-detect

# Alternative: Use docker-compose directly
docker-compose up -d
DOCKERFILE=Dockerfile.server-gpu docker-compose up -d
```

**Windows (Enhanced PowerShell):**
```powershell
# Start both services with saved preferences or defaults
.\run-docker.ps1 start -Detach

# Start with large model and GPU acceleration
.\run-docker.ps1 start -Model large-v3 -Port 8081 -Gpu -Detach

# Start with advanced audio processing features
.\run-docker.ps1 start -Model base -Language es -Translate -Diarize -Detach

# Language-specific examples with enhanced features
.\run-docker.ps1 start -Model base -Language en -Detach      # English
.\run-docker.ps1 start -Model base -Language es -Detach      # Spanish
.\run-docker.ps1 start -Model base -Language fr -Detach      # French
.\run-docker.ps1 start -Model base -Language de -Detach      # German
.\run-docker.ps1 start -Model base -Language auto -Translate -Detach  # Auto-detect with translation

# Alternative: Use docker-compose directly
docker-compose up -d
$env:DOCKERFILE="Dockerfile.server-gpu"; docker-compose up -d
```

**Example Output:**
```
[INFO] Starting Whisper Server...
[INFO] Auto-setting threads to 8 (detected 8 CPU cores)
[INFO] Downloading model: large-v3...
[INFO] Model downloaded successfully: models/ggml-large-v3.bin
[INFO] Server configuration:
[INFO]   Model: models/ggml-large-v3.bin
[INFO]   Host: 0.0.0.0
[INFO]   Port: 8178
[INFO]   Threads: 8
[INFO]   GPU: cpu (CPU-only inference)
[INFO]   Language: en

[INFO] Whisper server listening at http://0.0.0.0:8178
✓ Server started successfully!
```

### 6. Access Services

**Whisper Server:**
- **URL**: http://localhost:8178
- **Web Interface**: Upload audio files, real-time transcription
- **API**: RESTful API for programmatic access

**Meeting App:**
- **URL**: http://localhost:5167
- **API Documentation**: http://localhost:5167/docs
- **Features**: AI-powered meeting summarization, transcript management

**API Examples:**
```bash
# Whisper transcription
curl -X POST http://localhost:8178/inference \
  -F file="@audio.wav" \
  -F response_format="json"

# Meeting app - get all meetings
curl http://localhost:5167/get-meetings

# Process transcript for summarization
curl -X POST http://localhost:5167/process-transcript \
  -H "Content-Type: application/json" \
  -d '{"text": "Meeting transcript...", "model": "openai", "model_name": "gpt-4", "meeting_id": "meeting-123"}'
```

**🔧 Enhanced Management Commands:**

**Linux/macOS (Bash):**
```bash
# Interactive log viewing - Press Ctrl+C for service management options
./run-docker.sh logs --follow       # View all service logs with interactive controls
./run-docker.sh logs --service whisper -f  # View whisper logs only
./run-docker.sh logs --service app -f      # View app logs only

# Service management with health checks
./run-docker.sh status              # Detailed service status with connectivity tests
./run-docker.sh stop                # Stop all services with confirmation
./run-docker.sh restart             # Restart services with health validation

# Model management with progress tracking
./run-docker.sh models list         # Show downloaded models with sizes
./run-docker.sh models download large-v3  # Pre-download models with progress
./run-docker.sh gpu-test            # Test GPU availability and configuration

# Debugging and maintenance
./run-docker.sh shell --service whisper    # Open shell in whisper container
./run-docker.sh shell --service app        # Open shell in app container
./run-docker.sh clean --images      # Clean up containers and images
```

**Windows (Enhanced PowerShell):**
```powershell
# Enhanced Interactive log viewing with service controls
.\run-docker.ps1 logs -Follow                     # All service logs with interactive controls
.\run-docker.ps1 logs -Service whisper -Follow    # Whisper-specific logs with controls
.\run-docker.ps1 logs -Service app -Follow        # Meeting app logs with controls
.\run-docker.ps1 logs -Lines 50                   # Last 50 lines from both services

# Comprehensive service management with health monitoring
.\run-docker.ps1 status                           # Detailed health checks + connectivity tests
.\run-docker.ps1 stop                             # Graceful service shutdown with confirmation
.\run-docker.ps1 restart                          # Restart services with health validation

# Advanced model management with progress tracking
.\run-docker.ps1 models list                      # Show cached models with sizes and metadata
.\run-docker.ps1 models download large-v3         # Pre-download with real-time progress tracking
.\run-docker.ps1 gpu-test                         # Comprehensive GPU detection and testing

# Interactive debugging and maintenance
.\run-docker.ps1 shell -Service whisper          # Interactive shell access to whisper container
.\run-docker.ps1 shell -Service app              # Interactive shell access to meeting app container
.\run-docker.ps1 clean                            # Remove containers with confirmation
.\run-docker.ps1 clean --images                   # Remove containers and images with confirmation

# Database migration and setup
.\setup-db.ps1                                    # Interactive database discovery and migration
.\setup-db.ps1 -Auto                              # Auto-detect and migrate existing databases
.\setup-db.ps1 -Fresh                             # Fresh installation setup
```

## 🪟 Windows PowerShell Enhancements

**Windows users get a significantly enhanced experience with advanced PowerShell scripts:**

### 🚀 Key Windows-Specific Features

- **Smart Preferences System**: Automatically saves and reuses your configuration choices across sessions
- **Previous Settings Recovery**: On startup, choose to reuse, customize, or reset your saved preferences
- **Database Migration Assistant**: Seamless upgrade from existing Meetily installations with intelligent discovery
- **Advanced Model Selection**: Visual menu with 20+ models, size estimates, and accuracy ratings
- **Intelligent Language Detection**: Auto-detect or guided selection from 25+ languages with optimization tips
- **Port Conflict Resolution**: Smart port selection with automatic conflict detection and resolution
- **Real-time Progress Tracking**: Visual download progress with time estimates and file validation
- **Interactive Log Management**: Advanced log viewing with service control options during monitoring
- **Comprehensive Health Monitoring**: Automatic connectivity tests and detailed service status reporting

### ⚠️ Windows Docker Resource Configuration

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

**Monitor for Issues:**
- Watch container logs for "Dropped old audio chunk" warnings
- Use Windows Resource Monitor to check Docker resource usage
- Consider using smaller Whisper models on resource-constrained systems

### 🛠️ Windows Prerequisites

- **Docker Desktop** with WSL2 backend enabled
- **PowerShell 5.1** or **PowerShell 7+** (PowerShell 7+ recommended for better performance)
- **Git** (optional, for automatic version tagging in builds)
- **sqlite3** (for database setup script)
- **NVIDIA Docker** (optional, for GPU support)

### 🔧 PowerShell Execution Policy

If you get execution policy errors, run this in an elevated PowerShell:
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

## 🤖 Meeting Summarizer App

The integrated Meeting App provides AI-powered transcript processing and summarization:

### Core Features
- **📝 Transcript Processing**: Convert raw transcripts into structured summaries
- **🤖 Multi-AI Support**: OpenAI, Claude, Groq, and Ollama integration
- **💾 Data Persistence**: SQLite database with meeting and transcript storage
- **🔍 Full-Text Search**: Search across all meeting transcripts
- **📊 Meeting Management**: Complete CRUD operations for meetings
- **🔄 Background Processing**: Async transcript processing with status tracking

### AI Model Configuration
```bash
# Configure AI models via API
curl -X POST http://localhost:5167/save-model-config \
  -H "Content-Type: application/json" \
  -d '{
    "provider": "openai",
    "model": "gpt-4",
    "whisperModel": "large-v3",
    "apiKey": "your-api-key"
  }'
```

### API Endpoints

**Meeting Management:**
- `GET /get-meetings` - List all meetings
- `GET /get-meeting/{id}` - Get meeting details
- `POST /save-meeting-title` - Update meeting title
- `POST /delete-meeting` - Delete meeting and data

**Transcript Processing:**
- `POST /process-transcript` - Process transcript with AI
- `GET /get-summary/{id}` - Get processing results
- `POST /save-transcript` - Save raw transcript segments
- `POST /search-transcripts` - Search across transcripts

**Configuration:**
- `GET|POST /get-model-config` - AI model settings
- `GET|POST /get-transcript-config` - Transcription settings

### Database Migration

The system seamlessly migrates from existing Meetily installations:

```bash
# The setup script automatically checks these locations:
# - /opt/homebrew/Cellar/meetily-backend/0.0.4/backend/
# - Other common homebrew installation paths
# - User-specified custom locations

./run-docker.sh setup-db  # Interactive migration wizard
```

**Migration Features:**
- ✅ Automatic database detection and validation
- ✅ Data integrity checks before migration
- ✅ Interactive confirmation with database statistics
- ✅ Backup creation during migration
- ✅ Support for custom database locations

### Workflow Example

1. **Setup Database**: Migrate existing data or start fresh
2. **Start Services**: Both Whisper server and Meeting app
3. **Record/Upload Audio**: Use Whisper server for transcription
4. **Process Transcript**: Send to Meeting app for AI summarization
5. **Review Results**: Structured summaries with action items, decisions, etc.
6. **Search & Manage**: Full-text search and meeting organization

## Distribution Options

### Option 1: Docker Compose (Recommended)

```bash
# Start with docker-compose
docker-compose up

# Start GPU version
DOCKERFILE=Dockerfile.server-gpu docker-compose up

# Start with custom model
WHISPER_MODEL_NAME=large-v3 docker-compose --profile download up
```

### Option 2: Pre-built Images

Build and push to a registry:

```bash
# Build and push to Docker Hub
./build-docker.sh both --registry your-username --push

# Users can then run:
docker run -p 8178:8178 your-username/whisper-server:cpu
```

### Option 3: Single Archive Distribution

Create a complete package for offline distribution:

```bash
# Build images
./build-docker.sh both

# Save images to files
docker save whisper-server:cpu | gzip > whisper-server-cpu.tar.gz
docker save whisper-server:gpu | gzip > whisper-server-gpu.tar.gz

# Distribute the .tar.gz files to other PCs
# Users load and run:
docker load < whisper-server-cpu.tar.gz
docker run -p 8178:8178 whisper-server:cpu
```

### Option 4: Complete Project Distribution

For the full solution with all scripts:

```bash
# Clone or download this repository
git clone <your-repo-url>
cd whisper-docker

# Build and run in one command
./run-docker.sh start --model base.en

# The run script will automatically build if image doesn't exist
```

## Configuration

### Environment Variables

**Whisper Server:**
| Variable | Default | Description |
|----------|---------|-------------|
| `WHISPER_MODEL` | `models/ggml-base.en.bin` | Model file path |
| `WHISPER_HOST` | `0.0.0.0` | Server bind address |
| `WHISPER_PORT` | `8178` | Server port |
| `WHISPER_THREADS` | `0` | CPU threads (0 = auto) |
| `WHISPER_USE_GPU` | `true` | Enable GPU acceleration |
| `WHISPER_LANGUAGE` | `en` | Default language (see supported languages below) |
| `WHISPER_TRANSLATE` | `false` | Translate to English |
| `WHISPER_DIARIZE` | `false` | Enable speaker diarization |

**Supported Language Codes:**
- `en` (English), `es` (Spanish), `fr` (French), `de` (German), `it` (Italian)
- `pt` (Portuguese), `ru` (Russian), `ja` (Japanese), `ko` (Korean), `zh` (Chinese)
- `ar` (Arabic), `hi` (Hindi), `tr` (Turkish), `pl` (Polish), `nl` (Dutch)
- `sv` (Swedish), `da` (Danish), `no` (Norwegian), `fi` (Finnish), `is` (Icelandic)
- `he` (Hebrew), `th` (Thai), `vi` (Vietnamese), `ms` (Malay), `id` (Indonesian)
- `auto` (Auto-detection - recommended for mixed language content)

**Meeting App:**
| Variable | Default | Description |
|----------|---------|-------------|
| `APP_PORT` | `5167` | Meeting app port |
| `DATABASE_PATH` | `/app/data/meeting_minutes.db` | SQLite database location |
| `PYTHONUNBUFFERED` | `1` | Python logging mode |

### Volume Mounts

**Whisper Server:**
- `/app/models` - Model storage (persistent)
- `/app/uploads` - Temporary upload files
- `/app/config` - Configuration files (optional)

**Meeting App:**
- `./data:/app/data` - Local database directory (bind mount)
- `meeting_app_logs:/app/logs` - Application logs (Docker volume)

## Available Models

### Standard Models (Multilingual)
| Model | Size | Description | Best For |
|-------|------|-------------|----------|
| `tiny` | ~39 MB | Fastest, least accurate | Quick testing, low resources |
| `base` | ~74 MB | Good speed/accuracy balance | General use, moderate resources |
| `small` | ~244 MB | Better accuracy | Production use, good balance |
| `medium` | ~769 MB | High accuracy | Professional use, quality focus |
| `large-v3` | ~1550 MB | Best accuracy | Maximum quality, enterprise use |

### English-Optimized Models (Faster for English)
| Model | Size | Description | Performance Boost |
|-------|------|-------------|-------------------|
| `tiny.en` | ~39 MB | English-only tiny | ~30% faster than multilingual |
| `base.en` | ~74 MB | English-only base | ~25% faster than multilingual |
| `small.en` | ~244 MB | English-only small | ~20% faster than multilingual |
| `medium.en` | ~769 MB | English-only medium | ~15% faster than multilingual |

### Advanced Models
| Model | Size | Description | Special Features |
|-------|------|-------------|------------------|
| `large-v3-turbo` | ~1550 MB | Optimized large model | Faster inference, same quality |
| `small.en-tdrz` | ~244 MB | With speaker diarization | Identifies different speakers |

### Quantized Models (Reduced Size)
| Model | Size | Quality | Use Case |
|-------|------|---------|----------|
| `tiny-q5_1` | ~32 MB | 95% of tiny | Memory-constrained systems |
| `base-q5_1` | ~57 MB | 95% of base | IoT devices, edge computing |
| `small-q5_1` | ~182 MB | 95% of small | Mobile applications |
| `medium-q5_0` | ~515 MB | 94% of medium | Balanced size/quality |

**Model Selection Guide:**
- **For English-only content**: Use `.en` models for better performance
- **For multilingual content**: Use standard models
- **For low-resource systems**: Use quantized models (`-q5_1`, `-q5_0`)
- **For speaker identification**: Use `-tdrz` models
- **For maximum quality**: Use `large-v3` or `large-v3-turbo`

Models are automatically downloaded when first used and cached for subsequent runs.

## Scripts Reference

### ./run-docker.sh

Main deployment script with commands:

- `start` - Start both whisper server and meeting app
- `stop` - Stop all services  
- `restart` - Restart all services
- `logs` - Show service logs (use --service to specify)
- `status` - Show service status with health checks
- `shell` - Open shell in container (use --service to specify)
- `clean` - Remove containers/images
- `models` - Manage whisper models
- `gpu-test` - Test GPU detection
- `setup-db` - Setup/migrate database from existing installation
- `compose` - Pass commands directly to docker-compose

**Examples:**
- `./run-docker.sh start --model large-v3 --port 8081 --detach`
- `./run-docker.sh start --model base.en --language en --detach`  # English-optimized
- `./run-docker.sh start --model small --language auto --translate --detach`  # Auto-detect with translation
- `./run-docker.sh logs --service whisper -f`
- `./run-docker.sh shell --service app`

**Language-Specific Examples:**
- `./run-docker.sh start --model medium --language es --detach`  # Spanish meetings
- `./run-docker.sh start --model base --language fr --detach`   # French meetings
- `./run-docker.sh start --model small --language de --detach`  # German meetings
- `./run-docker.sh start --model base --language auto --detach` # Mixed languages

### ./build-docker.sh

Multi-platform build script:

- Build whisper CPU + meeting app: `./build-docker.sh cpu`
- Build whisper GPU + meeting app: `./build-docker.sh gpu`
- Build both whisper versions + meeting app: `./build-docker.sh both`
- Build for multiple platforms: `./build-docker.sh cpu --platforms linux/amd64,linux/arm64`
- Push to registry: `./build-docker.sh both --registry your-name --push`

Note: The meeting app is always built alongside the whisper server as they work as a complete package.

### ./setup-db.sh

Database setup and migration script:

- Interactive setup: `./setup-db.sh`
- Auto-detect existing: `./setup-db.sh --auto`
- Fresh installation: `./setup-db.sh --fresh`
- Custom database path: `./setup-db.sh --db-path /path/to/db`

## GPU Support

### NVIDIA GPUs

Requires NVIDIA Docker runtime:

```bash
# Install nvidia-docker2
sudo apt-get install nvidia-docker2
sudo systemctl restart docker

# Test GPU access
docker run --rm --gpus all nvidia/cuda:12.3.1-runtime-ubuntu22.04 nvidia-smi
```

### AMD GPUs (Future)

ROCm support planned for future versions.

## Troubleshooting

### Common Issues

**Docker daemon not running:**
```bash
# Linux
sudo systemctl start docker

# Mac/Windows
# Start Docker Desktop application
```

**Multi-platform build errors:**
```bash
# For local builds, use single platform (automatic)
./build-docker.sh cpu

# For registry distribution, use multi-platform
./build-docker.sh cpu --platforms linux/amd64,linux/arm64 --push --registry your-name
```

**Image build issues:**
```bash
# Check prerequisites
./run-docker.sh gpu-test

# Clean build without cache
./build-docker.sh cpu --no-cache

# Build with verbose output (add to Dockerfile: ENV VERBOSE=1)
```

**Container startup issues:**
```bash
# Check container logs
./run-docker.sh logs

# Check container status  
./run-docker.sh status

# Run in interactive mode to debug
./run-docker.sh start --model base.en
```

### Runtime Issues

```bash
# Check container status
./run-docker.sh status

# View detailed logs
./run-docker.sh logs --follow

# Test in interactive mode
./run-docker.sh start --model base.en

# Open shell for debugging
./run-docker.sh shell
```

### Model Issues

```bash
# List available models
./run-docker.sh models list

# Download specific model
./run-docker.sh models download large-v3

# Use local models directory
./run-docker.sh start --volume /path/to/your/models
```

### Meeting App Issues

```bash
# Check database exists and is accessible
ls -la ./data/meeting_minutes.db
sqlite3 ./data/meeting_minutes.db "SELECT COUNT(*) FROM meetings;"

# View meeting app logs
./run-docker.sh compose logs meeting-app

# Reset database (CAUTION: deletes all data)
./run-docker.sh compose down
rm ./data/meeting_minutes.db
./run-docker.sh setup-db --fresh
./run-docker.sh compose up -d

# Test API connectivity
curl http://localhost:5167/get-meetings

# Database migration issues
./run-docker.sh setup-db --db-path /custom/path/meeting_minutes.db
```

### Common Database Issues

```bash
# Permission denied errors
sudo chown $(whoami):$(whoami) ./data/meeting_minutes.db
chmod 644 ./data/meeting_minutes.db

# Database locked errors
./run-docker.sh compose down  # Stop all services first
# Then restart services

# Corrupted database recovery
sqlite3 ./data/meeting_minutes.db ".dump" > backup.sql
./run-docker.sh setup-db --fresh
sqlite3 ./data/meeting_minutes.db < backup.sql
```

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   User Request  │───▶│  Docker Runtime  │───▶│ Whisper Server  │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                │                        │
                                ▼                        ▼
                       ┌──────────────────┐    ┌─────────────────┐
                       │  GPU Detection   │    │ Model Manager   │
                       │  (Auto/Manual)   │    │ (Auto Download) │
                       └──────────────────┘    └─────────────────┘
```

## File Structure

```
Whisper_docker/
├── 📄 README.md                    # This documentation
├── 📄 DATABASE_SETUP.md           # Database migration guide
├── 📄 CLAUDE.md                   # Development guidelines
├── 🐳 Dockerfile.server-cpu       # CPU whisper server
├── 🐳 Dockerfile.server-gpu       # GPU whisper server  
├── 🐳 Dockerfile.app              # Meeting app
├── 🐳 docker-compose.yml          # Service orchestration
├── 🛠️ build-docker.sh             # Build management
├── 🛠️ run-docker.sh               # Runtime management  
├── 🛠️ setup-db.sh                 # Database setup
├── 📁 app/                        # Meeting app source
│   ├── main.py                    # FastAPI application
│   ├── db.py                      # Database manager
│   ├── transcript_processor.py    # AI processing
│   └── requirements.txt           # Python dependencies
├── 📁 data/                       # Database storage (created)
│   └── meeting_minutes.db         # SQLite database
├── 📁 docker/                     # Docker utilities
│   └── entrypoint.sh              # Container startup
├── 📁 models/                     # Whisper models (created)
└── 📁 whisper.cpp/                # Whisper.cpp submodule
```

## System Requirements

### Minimum (CPU-only)
- Docker 20.10+ or Docker Desktop
- Git and sqlite3 installed
- **4GB RAM minimum (8GB+ strongly recommended to prevent audio drops)**
- **Allocate at least 2GB RAM to Docker containers**
- 3GB+ disk space (models: 39MB-1.5GB each, plus app dependencies)
- Any CPU architecture (ARM64, AMD64)

**⚠️ Audio Processing Note**: With minimum resources, you may experience audio drops during heavy processing. Monitor container logs for "Dropped old audio chunk" warnings.

### Recommended (GPU-accelerated + AI features)
- Docker 20.10+ with NVIDIA runtime (`nvidia-docker2`)
- NVIDIA GPU with 4GB+ VRAM
- **8GB+ RAM (for both services + AI processing + audio buffering)**
- **Configure Docker with adequate resource limits**
- 5GB+ disk space
- CUDA-compatible GPU drivers
- API keys for AI services (OpenAI, Claude, Groq)

**⚠️ Resource Allocation**: Insufficient resources will cause audio chunk drops. Ensure containers have adequate memory and CPU allocation.

### Tested Platforms
- ✅ **macOS** (ARM64 - Apple Silicon, AMD64 - Intel)
- ✅ **Linux** (AMD64, ARM64, with/without GPU)
- ✅ **Windows** (AMD64, with Docker Desktop)
- ✅ **Cloud** (AWS, GCP, Azure container services)

## License

This project follows the same license as whisper.cpp (MIT License).