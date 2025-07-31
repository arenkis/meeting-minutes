# Easy deployment script for Whisper Server and Meeting App Docker containers
# Handles model downloads, GPU detection, and container management
#
# ⚠️  AUDIO PROCESSING WARNING:
# Insufficient Docker resources cause audio drops! The audio processing system
# drops chunks when queue is full (MAX_AUDIO_QUEUE_SIZE=10, lib.rs:54).
# Symptoms: "Dropped old audio chunk" in logs (lib.rs:330-333).
# Solution: Allocate 8GB+ RAM and adequate CPU to Docker containers.

param(
    [Parameter(Position=0)]
    [ValidateSet("start", "stop", "restart", "logs", "status", "shell", "clean", "build", "models", "gpu-test", "setup-db", "compose", "help")]
    [string]$Command = "start",
    
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$RemainingArgs = @(),
    
    [switch]$DryRun,
    
    [Alias("h")]
    [switch]$Help,
    
    [Alias("i")]
    [switch]$Interactive
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Configuration
$ScriptDir = $PSScriptRoot
$WhisperProjectName = "whisper-server"
$WhisperContainerName = "whisper-server"
$AppProjectName = "meeting-app"
$AppContainerName = "meeting-app"
$DefaultPort = 8178
$DefaultAppPort = 5167
$DefaultModel = "base.en"
$PreferencesFile = Join-Path $ScriptDir ".docker-preferences"

# Available whisper models
$AvailableModels = @(
    "tiny", "tiny.en", "tiny-q5_1",
    "base", "base.en", "base-q5_1",
    "small", "small.en", "small-q5_1",
    "medium", "medium.en", "medium-q5_1",
    "large-v1", "large-v2", "large-v3",
    "large-v1-q5_1", "large-v2-q5_1", "large-v3-q5_1",
    "large-v1-turbo", "large-v2-turbo", "large-v3-turbo"
)

# Color functions
function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

function Show-Help {
    @"
Whisper Server and Meeting App Docker Deployment Script

Usage: run-docker.ps1 [COMMAND] [OPTIONS]

COMMANDS:
  start         Start both whisper server and meeting app
  stop          Stop running services
  restart       Restart services
  logs          Show service logs (use -Service to specify)
  status        Show service status
  shell         Open shell in running container (use -Service to specify)
  clean         Remove containers and images
  build         Build Docker images
  models        Manage whisper models
  gpu-test      Test GPU availability
  setup-db      Setup/migrate database from existing installation
  compose       Pass commands directly to docker-compose

START OPTIONS:
  -Model, -m MODEL        Whisper model to use (default: base.en)
  -Port, -p PORT         Whisper port to expose (default: 8178)
  -AppPort PORT          Meeting app port to expose (default: 5167)
  -Gpu, -g               Force GPU mode for whisper
  -Cpu, -c               Force CPU mode for whisper
  -Language LANG         Language code (default: auto)
  -Translate             Enable translation to English
  -Diarize               Enable speaker diarization
  -Detach, -d            Run in background
  -Interactive, -i       Interactive setup with prompts
  -EnvFile FILE          Load environment from file

LOG/SHELL OPTIONS:
  -Service, -s SERVICE   Service to target (whisper|app) (default: both for logs)
  -Follow, -f            Follow log output
  -Lines, -n N           Number of lines to show (default: 100)

GLOBAL OPTIONS:
  -DryRun                Show commands without executing
  -Help, -h              Show this help

Examples:
  # Interactive setup with prompts for model, language, ports, database, etc.
  .\run-docker.ps1 start -Interactive
  
  # Start with default settings (may prompt for missing options)
  .\run-docker.ps1 start
  
  # Start with large model on port 8081
  .\run-docker.ps1 start -Model large-v3 -Port 8081 -Detach
  
  # Start with GPU and custom language  
  .\run-docker.ps1 start -Gpu -Language es -Detach
  
  # Start with translation enabled
  .\run-docker.ps1 start -Model base -Translate -Language auto -Detach
  
  # Build and start interactively
  .\build-docker.ps1 cpu; .\run-docker.ps1 start -Interactive
  
  # View whisper logs
  .\run-docker.ps1 logs -Service whisper -Follow
  
  # View meeting app logs
  .\run-docker.ps1 logs -Service app -Follow
  
  # Check status of both services
  .\run-docker.ps1 status
  
  # Database setup (run before first start)
  .\run-docker.ps1 setup-db                         # Interactive database setup
  .\run-docker.ps1 setup-db -Auto                   # Auto-detect existing database
  
  # Using docker-compose directly
  .\run-docker.ps1 compose up -d                    # Start both services in background
  .\run-docker.ps1 compose logs meeting-app         # View app logs
  .\run-docker.ps1 compose down                     # Stop all services

User Preferences:
  The script automatically saves your configuration choices and offers to reuse them
  on subsequent runs. Preferences are stored in: .docker-preferences
  
  When starting interactively, you'll be offered:
  1) Use previous settings - Reuse your last configuration
  2) Customize settings - Go through interactive setup again  
  3) Use defaults - Skip setup and use default values

Environment Variables:
  WHISPER_MODEL         Default whisper model
  WHISPER_PORT          Default whisper port
  APP_PORT              Default app port
  WHISPER_REGISTRY      Default registry
  WHISPER_GPU           Force GPU mode (true/false)
"@
}

# Function to detect system capabilities
function Get-SystemInfo {
    $gpuAvailable = $false
    $gpuType = "none"
    
    # Check for NVIDIA GPU
    try {
        nvidia-smi | Out-Null
        $gpuAvailable = $true
        $gpuType = "nvidia"
        Write-Info "NVIDIA GPU detected"
    } catch {
        if (Test-Path "/dev/nvidiactl") {
            $gpuAvailable = $true
            $gpuType = "nvidia"
            Write-Info "NVIDIA GPU drivers detected"
        }
    }
    
    # Check for AMD GPU
    try {
        rocm-smi | Out-Null
        $gpuAvailable = $true
        $gpuType = "amd"
        Write-Info "AMD GPU detected"
    } catch {
        # AMD GPU not available
    }
    
    # Check Docker
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        Write-Error "Docker is not installed"
        exit 1
    }
    
    # Check Docker Compose
    $composeAvailable = $false
    try {
        docker-compose version | Out-Null
        $composeAvailable = $true
    } catch {
        try {
            docker compose version | Out-Null
            $composeAvailable = $true
        } catch {
            # Docker Compose not available
        }
    }
    
    return @{
        GpuAvailable = $gpuAvailable
        GpuType = $gpuType
        ComposeAvailable = $composeAvailable
    }
}

# Function to choose image type
function Get-ImageInfo {
    param(
        [string]$ForceMode,
        [string]$Registry = ""
    )
    
    $systemInfo = Get-SystemInfo
    $imageTag = ""
    $dockerArgs = @()
    
    switch ($ForceMode) {
        "gpu" {
            if ($systemInfo.GpuAvailable) {
                $imageTag = "gpu"
                if ($systemInfo.GpuType -eq "nvidia") {
                    $dockerArgs += "--gpus", "all"
                }
                Write-Info "Using GPU image (forced)"
            } else {
                Write-Warn "GPU forced but no GPU detected, falling back to CPU"
                $imageTag = "cpu"
            }
        }
        "cpu" {
            $imageTag = "cpu"
            Write-Info "Using CPU image (forced)"
        }
        default {
            if ($systemInfo.GpuAvailable) {
                $imageTag = "gpu"
                if ($systemInfo.GpuType -eq "nvidia") {
                    $dockerArgs += "--gpus", "all"
                }
                Write-Info "Using GPU image (auto-detected)"
            } else {
                $imageTag = "cpu"
                Write-Info "Using CPU image (no GPU detected)"
            }
        }
    }
    
    $fullImage = if ($Registry) { "${Registry}/${WhisperProjectName}:${imageTag}" } else { "${WhisperProjectName}:${imageTag}" }
    
    return @{
        Image = $fullImage
        DockerArgs = $dockerArgs
    }
}

# Function to check if image exists and find best match
function Test-Image {
    param([string]$Image)
    
    # First, try exact match
    try {
        docker image inspect $Image | Out-Null
        return $Image
    } catch {
        # Exact match failed
    }
    
    # If exact match fails, try to find the latest timestamped version
    $imageBase = $Image -replace ':.*$', ''  # Remove tag part
    $tag = $Image -replace '^.*:', ''        # Get tag part
    
    # Look for any images with the same base and tag pattern
    try {
        $foundImage = docker images --format "{{.Repository}}:{{.Tag}}" | Select-String "^${imageBase}:${tag}-" | Select-Object -First 1
        if ($foundImage) {
            return $foundImage.ToString()
        }
    } catch {
        # No matching images found
    }
    
    # Return original image name (will fail when used)
    return $Image
}

# Function to ensure models directory exists
function Initialize-ModelsDir {
    $modelsDir = Join-Path $ScriptDir "models"
    
    if (-not (Test-Path $modelsDir -PathType Container)) {
        Write-Info "Creating models directory: $modelsDir"
        New-Item -ItemType Directory -Path $modelsDir -Force | Out-Null
    }
    
    return $modelsDir
}

# Function to start both services using docker-compose
function Start-Server {
    # Parse arguments
    $model = $env:WHISPER_MODEL ?? $DefaultModel
    $port = $env:WHISPER_PORT ?? $DefaultPort
    $appPort = $env:APP_PORT ?? $DefaultAppPort
    $forceMode = "auto"
    $detach = $false
    $envFile = ""
    $language = ""
    $translate = $false
    $diarize = $false
    $runInteractive = $false
    
    # Check if we should run interactive mode and handle preferences
    $setupMode = "interactive"
    $hasSavedPreferences = $false
    
    # Try to load saved preferences
    if (Load-Preferences) {
        $hasSavedPreferences = $true
    }
    
    for ($i = 0; $i -lt $RemainingArgs.Length; $i++) {
        switch -Regex ($RemainingArgs[$i]) {
            "^(-m|--model)$" {
                $model = $RemainingArgs[++$i]
            }
            "^(-p|--port)$" {
                $port = [int]$RemainingArgs[++$i]
            }
            "^--app-port$" {
                $appPort = [int]$RemainingArgs[++$i]
            }
            "^(-g|--gpu)$" {
                $forceMode = "gpu"
            }
            "^(-c|--cpu)$" {
                $forceMode = "cpu"
            }
            "^--language$" {
                $language = $RemainingArgs[++$i]
            }
            "^--translate$" {
                $translate = $true
            }
            "^--diarize$" {
                $diarize = $true
            }
            "^(-d|--detach)$" {
                $detach = $true
            }
            "^(-i|--interactive)$" {
                $runInteractive = $true
            }
            "^--env-file$" {
                $envFile = $RemainingArgs[++$i]
            }
        }
    }
    
    # Check if we should run interactive mode
    if ($Interactive -or ($runInteractive) -or ($model -eq $DefaultModel -and -not $language)) {
        $runInteractive = $true
        
        if ($hasSavedPreferences) {
            $setupMode = Show-PreviousSettings
        } else {
            $setupMode = "customize"
        }
    }
    
    # Interactive mode - prompt for settings
    if ($runInteractive) {
        $dbSelection = "fresh"
        
        switch ($setupMode) {
            "previous" {
                # Use saved preferences
                Write-Host "=== Using Previous Settings ===" -ForegroundColor Green
                $model = $Global:SAVED_MODEL ?? $model
                $port = $Global:SAVED_PORT ?? $port
                $appPort = $Global:SAVED_APP_PORT ?? $appPort
                $forceMode = $Global:SAVED_FORCE_MODE ?? $forceMode
                $language = $Global:SAVED_LANGUAGE ?? $language
                $translate = ($Global:SAVED_TRANSLATE -eq "true")
                $diarize = ($Global:SAVED_DIARIZE -eq "true")
                $dbSelection = $Global:SAVED_DB_SELECTION ?? "fresh"
                
                Write-Info "✓ Loaded previous configuration"
                Write-Host ""
            }
            "defaults" {
                # Use defaults, skip interactive setup
                Write-Host "=== Using Default Settings ===" -ForegroundColor Green
                Write-Info "✓ Using default configuration"
                Write-Host ""
            }
            "customize" {
                # Full interactive setup with saved preferences as defaults
                Write-Host "=== Interactive Setup ===" -ForegroundColor Green
                Write-Host ""
                
                # Model selection
                Write-Host "🎯 Model Selection" -ForegroundColor Blue
                $currentModel = $Global:SAVED_MODEL ?? $model
                $model = Select-Model $currentModel
                Write-Host "Selected model: $model" -ForegroundColor Green
                Write-Host ""
                
                # Language selection
                Write-Host "🌐 Language Selection" -ForegroundColor Blue
                $currentLanguage = $Global:SAVED_LANGUAGE ?? $language
                $language = Select-Language $currentLanguage
                Write-Host "Selected language: $language" -ForegroundColor Green
                Write-Host ""
                
                # Port selection (simplified for PowerShell)
                Write-Host "🔌 Port Selection" -ForegroundColor Blue
                $currentPort = $Global:SAVED_PORT ?? $port
                $portChoice = Read-Host "Whisper server port [default: $currentPort]"
                if ($portChoice) { $port = $portChoice } else { $port = $currentPort }
                
                $currentAppPort = $Global:SAVED_APP_PORT ?? $appPort
                $appPortChoice = Read-Host "Meeting app port [default: $currentAppPort]"
                if ($appPortChoice) { $appPort = $appPortChoice } else { $appPort = $currentAppPort }
                
                Write-Host "Selected Whisper port: $port" -ForegroundColor Green
                Write-Host "Selected Meeting app port: $appPort" -ForegroundColor Green
                Write-Host ""
                
                # GPU mode selection
                if ($forceMode -eq "auto") {
                    $systemInfo = Get-SystemInfo
                    if ($systemInfo.GpuAvailable) {
                        $savedGpuMode = $Global:SAVED_FORCE_MODE ?? "auto"
                        $gpuDefault = if ($savedGpuMode -eq "cpu") { "n" } else { "Y" }
                        $gpuChoice = Read-Host "GPU detected. Use GPU acceleration? (Y/n) [current: $savedGpuMode] [default: $gpuDefault]"
                        $gpuChoice = if ($gpuChoice) { $gpuChoice } else { $gpuDefault }
                        if ($gpuChoice -match '^[Nn]') {
                            $forceMode = "cpu"
                        } else {
                            $forceMode = "gpu"
                        }
                    } else {
                        Write-Info "No GPU detected, using CPU mode"
                        $forceMode = "cpu"
                    }
                }
                
                # Advanced options
                Write-Host ""
                $savedTranslate = $Global:SAVED_TRANSLATE ?? "false"
                $translateDefault = if ($savedTranslate -eq "true") { "y" } else { "N" }
                $translateChoice = Read-Host "Enable translation to English? (y/N) [current: $savedTranslate] [default: $translateDefault]"
                $translateChoice = if ($translateChoice) { $translateChoice } else { $translateDefault }
                if ($translateChoice -match '^[Yy]') {
                    $translate = $true
                }
                
                $savedDiarize = $Global:SAVED_DIARIZE ?? "false"
                $diarizeDefault = if ($savedDiarize -eq "true") { "y" } else { "N" }
                $diarizeChoice = Read-Host "Enable speaker diarization? (y/N) [current: $savedDiarize] [default: $diarizeDefault]"
                $diarizeChoice = if ($diarizeChoice) { $diarizeChoice } else { $diarizeDefault }
                if ($diarizeChoice -match '^[Yy]') {
                    $diarize = $true
                }
                
                # Save the new preferences
                Save-Preferences $model $port $appPort $forceMode $language $translate.ToString() $diarize.ToString() $dbSelection
                Write-Host ""
            }
        }
    }
    
    # Determine dockerfile based on force_mode
    $dockerfile = ""
    switch ($forceMode) {
        "gpu" {
            $dockerfile = "Dockerfile.server-gpu"
            Write-Info "Using GPU mode"
        }
        "cpu" {
            $dockerfile = "Dockerfile.server-cpu"
            Write-Info "Using CPU mode"
        }
        default {
            # Auto-detect GPU
            $systemInfo = Get-SystemInfo
            if ($systemInfo.GpuAvailable) {
                $dockerfile = "Dockerfile.server-gpu"
                Write-Info "GPU detected, using GPU mode"
            } else {
                $dockerfile = "Dockerfile.server-cpu"
                Write-Info "No GPU detected, using CPU mode"
            }
        }
    }
    
    # Convert model name to proper path format for whisper.cpp
    $whisperModelPath = if ($model -match "^models/") { $model } else { "models/ggml-${model}.bin" }
    
    # Build environment variables for docker-compose
    $env:DOCKERFILE = $dockerfile
    $env:WHISPER_MODEL = $whisperModelPath
    $env:WHISPER_PORT = $port.ToString()
    $env:APP_PORT = $appPort.ToString()
    $env:MODEL_NAME = $model  # For model-downloader compatibility
    
    if ($language) { $env:WHISPER_LANGUAGE = $language }
    if ($translate) { $env:WHISPER_TRANSLATE = "true" }
    if ($diarize) { $env:WHISPER_DIARIZE = "true" }
    
    # Check if images exist, build if needed
    $buildType = if ($dockerfile -match "gpu") { "gpu" } else { "cpu" }
    
    # Check if both images exist
    $whisperImageExists = $false
    $appImageExists = $false
    
    try {
        docker images --format "{{.Repository}}:{{.Tag}}" | Select-String "whisper-server:$buildType" | Out-Null
        $whisperImageExists = $true
    } catch {
        # Image doesn't exist
    }
    
    try {
        docker images --format "{{.Repository}}:{{.Tag}}" | Select-String "meetily-backend:" | Out-Null
        $appImageExists = $true
    } catch {
        # Image doesn't exist
    }
    
    # Build images if they don't exist
    if (-not $whisperImageExists -or -not $appImageExists) {
        Write-Info "Some images missing, building..."
        if (-not $DryRun) {
            & "$ScriptDir/build-docker.ps1" $buildType
        }
    }
    
    Write-Info "Starting Whisper Server + Meeting App..."
    Write-Info "Whisper Model: $whisperModelPath"
    Write-Info "Whisper Port: $port"
    Write-Info "Meeting App Port: $appPort"
    Write-Info "Docker mode: $dockerfile"
    
    if ($language) { Write-Info "Language: $language" }
    if ($translate) { Write-Info "Translation: enabled" }
    if ($diarize) { Write-Info "Diarization: enabled" }
    
    if ($DryRun) {
        Write-Info "DRY RUN - Command would be:"
        $composeCmd = "docker-compose up"
        if ($detach) { $composeCmd += " -d" }
        if ($envFile) { $composeCmd += " --env-file $envFile" }
        Write-Host $composeCmd
        return
    }
    
    # Execute docker-compose
    try {
        $composeArgs = @("up")
        if ($detach) { $composeArgs += "-d" }
        if ($envFile) { $composeArgs += "--env-file", $envFile }
        
        & docker-compose $composeArgs
        
        if ($detach) {
            Write-Info "✓ Services started in background"
            Write-Info "Whisper Server: http://localhost:$port"
            Write-Info "Meeting App: http://localhost:$appPort"
            Write-Info "View logs with: .\run-docker.ps1 logs -Follow"
        }
    } catch {
        Write-Error "✗ Failed to start services"
        exit 1
    }
}

# Function to stop services
function Stop-Server {
    Write-Info "Stopping services..."
    if ($DryRun) {
        Write-Info "DRY RUN - Would run: docker-compose down"
        return
    }
    
    try {
        $env:MODEL_NAME = "base.en"
        docker-compose down
        Write-Info "✓ Services stopped"
    } catch {
        Write-Error "✗ Failed to stop services"
        exit 1
    }
}

# Function to show logs
function Show-Logs {
    $follow = $false
    $lines = 100
    $service = ""
    
    for ($i = 0; $i -lt $RemainingArgs.Length; $i++) {
        switch -Regex ($RemainingArgs[$i]) {
            "^(-f|--follow)$" {
                $follow = $true
            }
            "^(-n|--lines)$" {
                $lines = [int]$RemainingArgs[++$i]
            }
            "^(--service|-s)$" {
                $service = $RemainingArgs[++$i]
            }
        }
    }
    
    $logCmd = @("docker-compose", "logs", "--tail=$lines")
    
    if ($follow) {
        $logCmd += "-f"
    }
    
    # Add service if specified
    switch ($service) {
        "whisper" {
            $logCmd += "whisper-server"
        }
        { $_ -in @("app", "backend") } {
            $logCmd += "meetily-backend"
        }
        "" {
            # Show logs from both services
        }
        default {
            $logCmd += $service
        }
    }
    
    if ($DryRun) {
        Write-Info "DRY RUN - Would run: $($logCmd -join ' ')"
        return
    }
    
    # Set MODEL_NAME to suppress warnings
    $env:MODEL_NAME = "base.en"
    & $logCmd[0] $logCmd[1..($logCmd.Length-1)]
}

# Function to show status
function Show-Status {
    Write-Info "=== Services Status ==="
    
    if ($DryRun) {
        Write-Info "DRY RUN - Would run: docker-compose ps"
        return
    }
    
    # Show docker-compose status
    $env:MODEL_NAME = "base.en"
    docker-compose ps
    
    # Check individual service health
    $whisperRunning = $false
    $appRunning = $false
    
    try {
        docker ps --format "{{.Names}}" | Select-String "whisper-server" | Out-Null
        $whisperRunning = $true
        $whisperPort = (docker port whisper-server "8178/tcp" 2>$null) -replace '.*:', ''
        if ($whisperPort) {
            Write-Info "Whisper Server: http://localhost:$whisperPort"
            # Test connectivity
            try {
                Invoke-WebRequest -Uri "http://localhost:$whisperPort/" -TimeoutSec 2 -UseBasicParsing | Out-Null
                Write-Info "✓ Whisper Server is responding"
            } catch {
                Write-Warn "✗ Whisper Server is not responding"
            }
        }
    } catch {
        # Container not running
    }
    
    try {
        docker ps --format "{{.Names}}" | Select-String "meetily-backend" | Out-Null
        $appRunning = $true
        $appPort = (docker port meetily-backend "5167/tcp" 2>$null) -replace '.*:', ''
        if ($appPort) {
            Write-Info "Meeting App: http://localhost:$appPort"
            # Test connectivity
            try {
                Invoke-WebRequest -Uri "http://localhost:$appPort/get-meetings" -TimeoutSec 2 -UseBasicParsing | Out-Null
                Write-Info "✓ Meeting App is responding"
            } catch {
                Write-Warn "✗ Meeting App is not responding"
            }
        }
    } catch {
        # Container not running
    }
    
    if (-not $whisperRunning -and -not $appRunning) {
        Write-Warn "✗ No services are running"
    }
}

# Function to open shell
function Open-Shell {
    $service = "whisper"
    
    for ($i = 0; $i -lt $RemainingArgs.Length; $i++) {
        switch -Regex ($RemainingArgs[$i]) {
            "^(--service|-s)$" {
                $service = $RemainingArgs[++$i]
            }
        }
    }
    
    $containerName = switch ($service) {
        "whisper" { "whisper-server" }
        { $_ -in @("app", "backend") } { "meetily-backend" }
        default { $service }
    }
    
    try {
        docker ps -q -f "name=$containerName" | Out-Null
        if ($?) {
            Write-Info "Opening shell in $containerName..."
            docker exec -it $containerName bash
        } else {
            Write-Error "Container $containerName is not running"
            exit 1
        }
    } catch {
        Write-Error "Container $containerName is not running"
        exit 1
    }
}

# Function to clean up
function Clean-Up {
    $removeImages = $false
    
    for ($i = 0; $i -lt $RemainingArgs.Length; $i++) {
        switch ($RemainingArgs[$i]) {
            "--images" {
                $removeImages = $true
            }
        }
    }
    
    Write-Info "Cleaning up services..."
    
    if ($DryRun) {
        Write-Info "DRY RUN - Would run:"
        Write-Info "  docker-compose down"
        if ($removeImages) {
            Write-Info "  docker-compose down --rmi all"
        }
        return
    }
    
    # Stop and remove containers
    Write-Info "Stopping and removing containers..."
    try {
        if ($removeImages) {
            docker-compose down --rmi all --volumes --remove-orphans
        } else {
            docker-compose down --volumes --remove-orphans
        }
        Write-Info "✓ Cleanup complete"
    } catch {
        Write-Error "✗ Failed to cleanup"
        exit 1
    }
}

# Function to show interactive model selection
function Select-Model {
    param([string]$DefaultModel = "base.en")
    
    Write-Host "=== Model Selection ===" -ForegroundColor Blue
    Write-Host "Available Whisper models:" -ForegroundColor Green
    Write-Host ""
    
    for ($i = 0; $i -lt $AvailableModels.Length; $i++) {
        $model = $AvailableModels[$i]
        if ($model -eq $DefaultModel) {
            Write-Host ("  {0,2}) {1} (current)" -f ($i + 1), $model) -ForegroundColor Green
        } else {
            Write-Host ("  {0,2}) {1}" -f ($i + 1), $model)
        }
    }
    
    Write-Host ""
    Write-Host "Model size guide:" -ForegroundColor Yellow
    Write-Host "  tiny    (~39 MB)  - Fastest, least accurate"
    Write-Host "  base    (~142 MB) - Good balance of speed/accuracy"
    Write-Host "  small   (~244 MB) - Better accuracy"
    Write-Host "  medium  (~769 MB) - High accuracy"
    Write-Host "  large   (~1550 MB)- Best accuracy, slowest"
    Write-Host ""
    
    do {
        $choice = Read-Host "Select model number (1-$($AvailableModels.Length)) or enter model name [default: $DefaultModel]"
        
        if (-not $choice) {
            return $DefaultModel
        }
        
        # Check if it's a number
        if ($choice -match '^\d+$') {
            $index = [int]$choice - 1
            if ($index -ge 0 -and $index -lt $AvailableModels.Length) {
                return $AvailableModels[$index]
            } else {
                Write-Host "Invalid selection. Please choose 1-$($AvailableModels.Length)" -ForegroundColor Red
                continue
            }
        } else {
            # Check if it's a valid model name
            if ($AvailableModels -contains $choice) {
                return $choice
            } else {
                Write-Host "Invalid model name. Please choose from available models." -ForegroundColor Red
            }
        }
    } while ($true)
}

# Function to show interactive language selection
function Select-Language {
    param([string]$DefaultLanguage = "auto")
    
    Write-Host "=== Language Selection ===" -ForegroundColor Blue
    Write-Host "Common languages:" -ForegroundColor Green
    
    $languages = @(
        @{"num"="1"; "code"="auto"; "name"="auto (automatic detection)"},
        @{"num"="2"; "code"="en"; "name"="en (English)"},
        @{"num"="3"; "code"="es"; "name"="es (Spanish)"},
        @{"num"="4"; "code"="fr"; "name"="fr (French)"},
        @{"num"="5"; "code"="de"; "name"="de (German)"},
        @{"num"="6"; "code"="it"; "name"="it (Italian)"},
        @{"num"="7"; "code"="pt"; "name"="pt (Portuguese)"},
        @{"num"="8"; "code"="ru"; "name"="ru (Russian)"},
        @{"num"="9"; "code"="ja"; "name"="ja (Japanese)"},
        @{"num"="10"; "code"="zh"; "name"="zh (Chinese)"}
    )
    
    foreach ($lang in $languages) {
        if ($lang.code -eq $DefaultLanguage) {
            Write-Host ("  {0}) {1} (current)" -f $lang.num, $lang.name) -ForegroundColor Green
        } else {
            Write-Host ("  {0}) {1}" -f $lang.num, $lang.name)
        }
    }
    Write-Host " 11) Other (enter language code)"
    Write-Host ""
    
    do {
        $choice = Read-Host "Select language [default: $DefaultLanguage]"
        
        if (-not $choice) {
            return $DefaultLanguage
        }
        
        switch ($choice) {
            "1" { return "auto" }
            "2" { return "en" }
            "3" { return "es" }
            "4" { return "fr" }
            "5" { return "de" }
            "6" { return "it" }
            "7" { return "pt" }
            "8" { return "ru" }
            "9" { return "ja" }
            "10" { return "zh" }
            "11" {
                $langCode = Read-Host "Enter language code (e.g., ko, ar, hi)"
                if ($langCode) {
                    return $langCode
                } else {
                    return $DefaultLanguage
                }
            }
            default {
                # Check if it's a direct language code
                if ($choice -match '^[a-z]{2}$') {
                    return $choice
                } else {
                    Write-Host "Invalid selection. Please choose 1-11 or enter a valid language code." -ForegroundColor Red
                }
            }
        }
    } while ($true)
}

# Function to manage models
function Manage-Models {
    $action = if ($RemainingArgs.Length -gt 0) { $RemainingArgs[0] } else { "list" }
    
    switch ($action) {
        "list" {
            Write-Info "=== Available Models ==="
            $modelsDir = Initialize-ModelsDir
            
            if (Test-Path $modelsDir -PathType Container) {
                $models = Get-ChildItem -Path $modelsDir -Filter "*.bin" | Sort-Object Name
                if ($models) {
                    foreach ($model in $models) {
                        $size = [math]::Round($model.Length / 1MB, 1)
                        Write-Info "  $($model.Name) ($size MB)"
                    }
                } else {
                    Write-Warn "No models found in $modelsDir"
                    Write-Info "Models will be automatically downloaded when needed"
                }
            } else {
                Write-Warn "No models found in $modelsDir"
                Write-Info "Models will be automatically downloaded when needed"
            }
        }
        "download" {
            $modelName = if ($RemainingArgs.Length -gt 1) { $RemainingArgs[1] } else { "base.en" }
            $modelsDir = Initialize-ModelsDir
            $modelFile = Join-Path $modelsDir "ggml-${modelName}.bin"
            
            if (Test-Path $modelFile) {
                Write-Info "Model already exists: $modelFile"
                return
            }
            
            Write-Info "Downloading model: $modelName"
            $downloadUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${modelName}.bin"
            
            try {
                Invoke-WebRequest -Uri $downloadUrl -OutFile $modelFile -UseBasicParsing
                Write-Info "✓ Model downloaded: $modelFile"
            } catch {
                Write-Error "✗ Failed to download model"
                Remove-Item -Path $modelFile -Force -ErrorAction SilentlyContinue
                exit 1
            }
        }
        default {
            Write-Error "Unknown models action: $action"
            Write-Info "Available actions: list, download"
            exit 1
        }
    }
}

# Function to test GPU
function Test-Gpu {
    Write-Info "=== GPU Test ==="
    
    $systemInfo = Get-SystemInfo
    
    Write-Info "GPU Available: $($systemInfo.GpuAvailable)"
    Write-Info "GPU Type: $($systemInfo.GpuType)"
    
    if ($systemInfo.GpuAvailable) {
        if ($systemInfo.GpuType -eq "nvidia") {
            Write-Info "NVIDIA GPU Details:"
            try {
                nvidia-smi
            } catch {
                Write-Warn "nvidia-smi not available"
            }
        }
        
        # Test with container
        Write-Info "Testing GPU in container..."
        $imageInfo = Get-ImageInfo "gpu" ""
        $image = $imageInfo.Image
        
        try {
            docker image inspect $image | Out-Null
            docker run --rm --gpus all $image gpu-test
        } catch {
            Write-Warn "GPU image not built, run: .\run-docker.ps1 build gpu"
        }
    } else {
        Write-Info "No GPU detected"
    }
}

# Main function
function Main {
    if ($Help) {
        Show-Help
        exit 0
    }
    
    Set-Location $ScriptDir
    
    switch ($Command) {
        "start" { Start-Server }
        "stop" { Stop-Server }
        "restart" {
            Stop-Server
            Start-Sleep -Seconds 2
            Start-Server
        }
        "logs" { Show-Logs }
        "status" { Show-Status }
        "shell" { Open-Shell }
        "clean" { Clean-Up }
        "build" {
            & "$ScriptDir/build-docker.ps1" @RemainingArgs
        }
        "models" { Manage-Models }
        "gpu-test" { Test-Gpu }
        "setup-db" {
            & "$ScriptDir/setup-db.ps1" @RemainingArgs
        }
        "compose" {
            docker-compose @RemainingArgs
        }
        "help" { Show-Help }
        default {
            Write-Error "Unknown command: $Command"
            Show-Help
            exit 1
        }
    }
}

# Execute main function
Main