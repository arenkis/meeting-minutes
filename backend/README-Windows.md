# Meetily Backend - Windows Setup Guide

Complete guide for setting up Meetily Backend on Windows with **enhanced Docker experience**.

## ⚠️ Audio Processing Warning

**CRITICAL: Windows Docker Resource Configuration**

On Windows systems, Docker Desktop resource limitations can severely impact audio processing, leading to frequent audio drops. The Tauri frontend audio system (lib.rs:54, 330) will drop audio chunks when Docker containers cannot keep up with real-time processing.

**Windows-Specific Issues:**
- WSL2 memory limitations affect Docker performance
- Default Docker Desktop allocations may be insufficient
- Audio processing is more resource-intensive on Windows

**Required Windows Docker Configuration:**
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
- Use Resource Monitor to check Docker resource usage
- Consider using smaller Whisper models on resource-constrained systems

---

## 🐳 Docker Setup (Recommended)

**The easiest way to run Meetily Backend on Windows with interactive features:**

### Prerequisites

- **Docker Desktop** with WSL2 backend enabled
- **PowerShell 5.1** or **PowerShell 7+** (PowerShell 7+ recommended)
- **Git** (optional, for version tagging)

### Quick Start

```powershell
# Clone the repository
git clone https://github.com/Zackriya-Solutions/meeting-minutes.git
cd meeting-minutes/backend

# 🎯 Complete Interactive Setup (Recommended for new users)
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

### 🚀 Interactive Setup Features

The interactive setup (`-Interactive`) guides you through:

1. **🎯 Model Selection**: Choose from 20+ Whisper models with size/accuracy info
2. **🌐 Language Selection**: Select from 25+ languages or use auto-detection
3. **🔌 Port Configuration**: Smart port selection with conflict resolution
4. **🗄️ Database Setup**: Automatic detection and migration of existing databases
5. **🖥️ GPU Configuration**: Automatic GPU detection with user confirmation
6. **⚙️ Advanced Options**: Translation, speaker diarization, and more

### 🚀 Enhanced Windows Features

- **🎯 Complete Interactive Setup**: Guided configuration for all components
- **🗄️ Database Migration**: Seamless upgrade from existing Meetily installations
- **🔌 Port Management**: Automatic conflict detection and resolution
- **📊 Real-time Progress Tracking**: Visual download progress with time estimates
- **🔄 Interactive Log Management**: Press Ctrl+C during log viewing for service controls
- **⚡ Health Monitoring**: Automatic service health checks with connectivity tests
- **💡 Context-aware Prompts**: Intelligent assistance and troubleshooting guidance

## 🎯 Complete Interactive Setup Workflow

**New users should follow this recommended workflow for the best experience:**

### Step 1: Database Preparation (Optional)
```powershell
# If upgrading from existing Meetily installation
.\setup-db.ps1 -Interactive    # Guided database discovery and migration

# For fresh installations, skip this step
```

### Step 2: Interactive Configuration & Launch
```powershell
# Complete guided setup with preferences system
.\run-docker.ps1 start -Interactive
```

**This interactive setup will guide you through:**

1. **🔄 Previous Settings Recovery** (if available)
   - Use previous settings
   - Customize settings 
   - Use defaults

2. **🎯 Model Selection Menu**
   - Visual selection from 20+ Whisper models
   - Size estimates and accuracy ratings
   - Performance recommendations

3. **🌐 Language Configuration**
   - Auto-detection or manual selection
   - 25+ supported languages
   - Translation and optimization options

4. **🔌 Port Configuration**
   - Automatic conflict detection
   - Smart port selection
   - Custom port options

5. **🖥️ GPU Detection & Configuration**
   - Automatic hardware detection
   - User confirmation for GPU usage
   - Fallback to CPU if needed

6. **⚙️ Advanced Features**
   - Translation to English
   - Speaker diarization
   - Custom audio processing options

7. **💾 Preference Saving**
   - Automatically saves your choices
   - Reusable for future runs
   - Easy to modify or reset

### Step 3: Verification & Management
```powershell
# Check that everything is running correctly
.\run-docker.ps1 status

# View logs for troubleshooting
.\run-docker.ps1 logs -Follow
```

### Optional Dependencies
- **sqlite3** (for database setup script)
- **NVIDIA Docker** (for GPU support)

## 🛠️ Enhanced PowerShell Scripts

### .\run-docker.ps1 - Enhanced Interactive Management (⭐ Recommended)
Comprehensive container deployment and management with advanced interactive features and user preferences.

```powershell
# 🎯 Interactive Setup - Complete guided configuration
.\run-docker.ps1 start -Interactive

# ⚡ Quick Commands with saved preferences support
.\run-docker.ps1 start -Detach                    # Uses saved or default settings
.\run-docker.ps1 start -Model large-v3 -Detach   # Specific model configuration
.\run-docker.ps1 start -Gpu -Language es -Detach # GPU + Spanish with translation support
.\run-docker.ps1 start -Translate -Diarize -Detach # Advanced audio processing features

# 🔧 Enhanced Management & Monitoring
.\run-docker.ps1 status                           # Detailed health checks + connectivity tests
.\run-docker.ps1 logs -Follow                     # Interactive log viewing with service controls
.\run-docker.ps1 logs -Service whisper -Follow    # Whisper-specific logs
.\run-docker.ps1 logs -Service app -Follow        # Meeting app logs
.\run-docker.ps1 models list                      # Show downloaded models with sizes
.\run-docker.ps1 models download large-v3         # Pre-download with progress tracking
.\run-docker.ps1 gpu-test                         # Comprehensive GPU testing
.\run-docker.ps1 shell -Service whisper          # Open shell in whisper container
.\run-docker.ps1 stop                             # Graceful service shutdown
```

**🚀 Enhanced Interactive Features:**
- **Smart Preferences System**: Automatically saves and reuses your configuration choices
- **Previous Settings Recovery**: Choose to reuse, customize, or reset your saved preferences
- **Advanced Model Selection**: Visual menu with 20+ models, size estimates, and accuracy ratings
- **Intelligent Language Detection**: Auto-detect or choose from 25+ languages with optimization tips
- **Port Conflict Resolution**: Smart port selection with automatic conflict detection
- **Database Migration Assistant**: Seamless upgrade from existing Meetily installations
- **Real-time Progress Tracking**: Visual download progress with time estimates and validation
- **Interactive Log Management**: Advanced log viewing with service control options
- **Comprehensive Health Monitoring**: Automatic connectivity tests and detailed service status reporting

### .\build-docker.ps1 - Enhanced Multi-platform Builder
Advanced Docker build script with automatic platform detection, macOS optimization, and comprehensive build management.

```powershell
# Build CPU version (universal compatibility + automatic platform detection)
.\build-docker.ps1 cpu

# Build GPU version (NVIDIA GPU acceleration with CUDA support)
.\build-docker.ps1 gpu

# Build macOS-optimized version (Apple Silicon compatibility)
.\build-docker.ps1 macos

# Build all versions (CPU + GPU + Meeting App)
.\build-docker.ps1 both

# Multi-platform builds for distribution
.\build-docker.ps1 gpu -Registry "ghcr.io/username" -Push -Platforms "linux/amd64,linux/arm64"

# Advanced build options
.\build-docker.ps1 cpu -BuildArgs "CUDA_VERSION=12.1.1" -NoCache
.\build-docker.ps1 gpu -Tag "custom-build" -DryRun

# Show comprehensive help
.\build-docker.ps1 -Help
```

**Enhanced Parameters:**
- `BuildType`: `cpu`, `gpu`, `macos`, or `both` (default: `cpu`, auto-detects macOS)
- `-Registry, -r`: Docker registry prefix for distribution
- `-Push, -p`: Push images to registry (required for multi-platform builds)
- `-Tag, -t`: Custom tag (default: auto-generated with timestamp and git hash)
- `-Platforms`: Target platforms (default: current platform, supports multi-platform)
- `-BuildArgs`: Additional build arguments (e.g., CUDA versions)
- `-NoCache`: Build without cache for clean builds
- `-DryRun`: Show commands without executing (perfect for testing)

**🚀 New Features:**
- **Automatic macOS Detection**: Switches to macOS-optimized builds on Apple Silicon
- **Intelligent Platform Selection**: Auto-detects current platform for local builds
- **Multi-stage Build Optimization**: Separate builder and runtime stages for minimal image size
- **Git Integration**: Automatic tagging with git commit hashes for version tracking
- **Comprehensive Error Handling**: Detailed error messages and build validation
- **Cross-platform Compatibility**: Supports Windows, Linux, and macOS development environments

### Quick Reference Commands
Core container management with intelligent defaults and user preference support.

```powershell
# ⭐ Smart Interactive Setup (Recommended for first run)
.\run-docker.ps1 start -Interactive

# Quick start options with preference system
.\run-docker.ps1 start                            # Uses saved preferences or prompts
.\run-docker.ps1 start -Model large-v3 -Port 8081 -Detach
.\run-docker.ps1 start -Gpu -Language es -Translate -Detach
.\run-docker.ps1 start -Cpu -Diarize -AppPort 5168 -Detach

# Advanced service management
.\run-docker.ps1 logs -Follow -Service whisper    # Whisper server logs
.\run-docker.ps1 logs -Follow -Service app        # Meeting app logs
.\run-docker.ps1 logs -Lines 50                   # Last 50 lines from both services
.\run-docker.ps1 status                           # Comprehensive health check
.\run-docker.ps1 restart                          # Graceful restart
.\run-docker.ps1 stop                             # Stop all services

# Container shell access
.\run-docker.ps1 shell -Service whisper          # Open shell in whisper container
.\run-docker.ps1 shell -Service app              # Open shell in meeting app container

# Cleanup and maintenance
.\run-docker.ps1 clean                            # Remove containers
.\run-docker.ps1 clean --images                   # Remove containers and images
```

**Available Commands:**
- `start`: Start both whisper server and meeting app with intelligent configuration
- `stop`: Gracefully stop all running services
- `restart`: Stop and restart services with saved preferences
- `logs`: Enhanced log viewing with service filtering and follow options
- `status`: Detailed service status with connectivity tests
- `shell`: Interactive shell access to running containers
- `clean`: Container and image cleanup with confirmation
- `build`: Proxy to build-docker.ps1 for image building
- `models`: Advanced model management (list, download, cache info)
- `gpu-test`: Comprehensive GPU detection and testing
- `setup-db`: Database setup and migration assistant
- `compose`: Direct docker-compose command passthrough

### .\setup-db.ps1 - Enhanced Database Migration Assistant
Intelligent database setup and migration script with comprehensive discovery and validation.

```powershell
# 🎯 Interactive Setup (Recommended) - Guided database discovery and migration
.\setup-db.ps1

# ⚡ Automated Migration - Auto-detect existing databases and migrate seamlessly
.\setup-db.ps1 -Auto

# 🆕 Fresh Installation - Create new database for first-time setup
.\setup-db.ps1 -Fresh

# 📂 Custom Path Migration - Migrate from specific database location
.\setup-db.ps1 -DbPath "C:\Users\username\Documents\meeting_minutes.db"
```

**Enhanced Parameters:**
- `-DbPath`: Custom database path to migrate from (supports full Windows paths)
- `-Fresh`: Skip existing database search, create fresh database for new installations
- `-Auto`: Auto-detect and migrate without prompts (perfect for automated setups)
- `-Help, -h`: Show comprehensive help with usage examples

**🚀 Enhanced Features:**
- **Intelligent Database Discovery**: Automatically searches common Windows locations and HomeBrew paths
- **Multi-location Search**: Checks user documents, desktop, and application directories
- **Database Validation**: Verifies SQLite database integrity before migration
- **Detailed Database Info**: Shows database size, modification date, and record counts
- **Interactive Selection**: Choose from multiple found databases with detailed information
- **Cross-platform Path Support**: Handles Windows, WSL, and cross-platform file paths
- **Safe Migration**: Creates backups and validates successful database copying
- **Progress Reporting**: Clear status updates throughout the migration process

## Windows-Specific Notes

### Docker Desktop Configuration
1. Ensure Docker Desktop is running with WSL2 backend
2. Enable GPU support in Docker Desktop settings (for NVIDIA GPUs)
3. Verify Docker Buildx is available: `docker buildx version`

### PowerShell Execution Policy
If you get execution policy errors, run this in an elevated PowerShell:
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### GPU Support
For NVIDIA GPU support on Windows:
1. Install [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/install-guide.html#docker)
2. Restart Docker Desktop
3. Test with: `.\run-docker.ps1 gpu-test`

### Path Differences
- Windows paths use backslashes (`\`) but scripts handle both formats
- Database paths default to Windows-appropriate locations
- Models directory created at `.\models\` relative to script location

### Performance Tips
1. Use PowerShell 7+ for better performance
2. Enable Windows features: Hyper-V, Containers, WSL2
3. Place project on the WSL2 filesystem for better Docker performance
4. Use SSD storage for model caching

## Troubleshooting

### Common Issues

**Docker not found:**
```powershell
# Ensure Docker Desktop is running and restart PowerShell
docker version
```

**Permission denied:**
```powershell
# Check execution policy
Get-ExecutionPolicy
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

**Build failures:**
```powershell
# Check Docker Buildx
docker buildx version
docker buildx ls

# Create builder if needed
docker buildx create --name whisper-builder --use
```

**Multi-platform build issues:**
- Multi-platform builds require `--push` flag
- For local builds, use single platform: `-Platforms "linux/amd64"`

### Getting Help
Each script supports the `-Help` parameter:
```powershell
.\build-docker.ps1 -Help
.\run-docker.ps1 -Help
.\setup-db.ps1 -Help
```

## Integration with Existing Workflow

These PowerShell scripts are functionally equivalent to their bash counterparts:

| Bash Script | PowerShell Script | Purpose |
|-------------|-------------------|---------|
| `build-docker.sh` | `build-docker.ps1` | Build Docker images |
| `run-docker.sh` | `run-docker.ps1` | Deploy and manage containers |
| `setup-db.sh` | `setup-db.ps1` | Database setup and migration |

You can use the same `docker-compose.yml` and Docker configurations with either set of scripts.