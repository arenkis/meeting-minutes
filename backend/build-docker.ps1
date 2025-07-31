# Multi-platform Docker build script for Whisper Server and Meeting App
# Supports both CPU-only and GPU-enabled builds across multiple architectures
#
# ⚠️  AUDIO PROCESSING WARNING:
# Docker containers with insufficient resources will drop audio chunks when
# the processing queue becomes full (MAX_AUDIO_QUEUE_SIZE=10, lib.rs:54).
# Ensure containers have adequate memory (8GB+) and CPU allocation.
# Monitor logs for "Dropped old audio chunk" messages (lib.rs:330).

param(
    [Parameter(Position=0)]
    [ValidateSet("cpu", "gpu", "macos", "both")]
    [string]$BuildType = "cpu",
    
    [Alias("r")]
    [string]$Registry = $env:REGISTRY,
    
    [Alias("p")]
    [switch]$Push,
    
    [Alias("t")]
    [string]$Tag,
    
    [string]$Platforms,
    
    [string]$BuildArgs = $env:BUILD_ARGS,
    
    [switch]$NoCache,
    
    [switch]$DryRun,
    
    [Alias("h")]
    [switch]$Help
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Configuration
$ScriptDir = $PSScriptRoot
$WhisperProjectName = "whisper-server"
$AppProjectName = "meetily-backend"

# Platform detection for cross-platform compatibility
$DetectedOS = [System.Environment]::OSVersion.Platform
$IsWindows = $IsWindows -or ($env:OS -eq "Windows_NT")
$IsLinux = $IsLinux -or ($DetectedOS -eq [System.PlatformID]::Unix -and (Test-Path "/proc/version"))
$IsMacOS = $IsMacOS -or ($DetectedOS -eq [System.PlatformID]Unix -and -not (Test-Path "/proc/version"))

if ($IsMacOS) {
    Write-Info "macOS detected via PowerShell - will support macOS-optimized configurations"
}

# Default to current platform for local builds, multi-platform for registry pushes
if (-not $Platforms) {
    $arch = if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq [System.Runtime.InteropServices.Architecture]::X64) { "amd64" } else { "arm64" }
    $os = if ($IsLinux) { "linux" } else { "linux" }  # Default to linux for Docker
    $Platforms = "$os/$arch"
}

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

# Error handling
function Handle-Error {
    param([string]$Message)
    Write-Error $Message
    exit 1
}

function Show-Help {
    @"
Multi-platform Whisper Server and Meeting App Docker Builder

Usage: build-docker.ps1 [OPTIONS] [BUILD_TYPE]

BUILD_TYPE:
  cpu           Build whisper server CPU-only + meeting app (default)
  gpu           Build whisper server GPU-enabled + meeting app
  macos         Build whisper server macOS-optimized + meeting app (cross-platform compatibility)
  both          Build both whisper server versions + meeting app
  
OPTIONS:
  -Registry, -r REGISTRY    Docker registry (e.g., ghcr.io/user)
  -Push, -p                 Push images to registry
  -Tag, -t TAG              Custom tag (default: auto-generated)
  -Platforms PLATFORMS      Target platforms (default: current platform)
  -BuildArgs ARGS           Additional build arguments
  -NoCache                  Build without cache
  -DryRun                   Show commands without executing
  -Help, -h                 Show this help

Examples:
  # Build whisper CPU version + meeting app for current platform
  .\build-docker.ps1 cpu
  
  # Build whisper GPU version + meeting app
  .\build-docker.ps1 gpu
  
  # Build whisper macOS-optimized version + meeting app
  .\build-docker.ps1 macos
  
  # Build both whisper versions + meeting app
  .\build-docker.ps1 both
  
  # Build GPU version for multiple platforms (requires -Push)
  .\build-docker.ps1 gpu -Platforms "linux/amd64,linux/arm64" -Push
  
  # Build both versions and push to registry
  .\build-docker.ps1 both -Registry "ghcr.io/myuser" -Push
  
  # Build with custom CUDA version
  .\build-docker.ps1 gpu -BuildArgs "CUDA_VERSION=12.1.1"

Note: The meeting app is always built alongside the whisper server as they work as a package.

Environment Variables:
  REGISTRY      Docker registry prefix
  PUSH          Push to registry (true/false)
  PLATFORMS     Target platforms
  BUILD_ARGS    Additional build arguments
"@
}

# Function to check prerequisites
function Test-Prerequisites {
    Write-Info "Checking prerequisites..."
    
    # Check Docker
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
        Handle-Error "Docker is not installed or not in PATH"
    }
    
    # Check Docker Buildx
    try {
        docker buildx version | Out-Null
    } catch {
        Handle-Error "Docker Buildx is not available. Please install Docker Desktop or enable Buildx"
    }
    
    # Check if buildx builder exists
    $builderExists = docker buildx ls | Select-String "whisper-builder"
    if (-not $builderExists) {
        Write-Info "Creating multi-platform builder..."
        docker buildx create --name whisper-builder --platform $Platforms --use
    } else {
        Write-Info "Using existing whisper-builder"
        docker buildx use whisper-builder
    }
    
    # Check whisper.cpp directory
    if (-not (Test-Path "$ScriptDir/whisper.cpp" -PathType Container)) {
        Handle-Error "whisper.cpp directory not found. Please ensure whisper.cpp is cloned in the current directory"
    }
    
    Write-Info "Prerequisites check passed"
}

# Prepare whisper.cpp custom files
Write-Info "Changing to whisper.cpp directory..."
try {
    Set-Location "$ScriptDir/whisper.cpp"
} catch {
    Handle-Error "Failed to change to whisper.cpp directory"
}

Write-Info "Checking for custom server directory..."
if (-not (Test-Path "../whisper-custom/server" -PathType Container)) {
    Handle-Error "Directory '../whisper-custom/server' not found. Please make sure the custom server files exist"
}

Write-Info "Copying custom server files..."
try {
    Copy-Item -Path "../whisper-custom/server/*" -Destination "examples/server/" -Recurse -Force
    Write-Info "Custom server files copied successfully"
} catch {
    Handle-Error "Failed to copy custom server files"
}

Write-Info "Verifying server files..."
Get-ChildItem "examples/server/" | Out-Null

Write-Info "Returning to original directory..."
Set-Location $ScriptDir

# Function to generate image tag
function New-Tag {
    param(
        [string]$BuildType,
        [string]$CustomTag
    )
    
    if ($CustomTag) {
        return $CustomTag
    }
    
    $timestamp = Get-Date -Format "yyyyMMdd"
    
    # Get git commit hash if available
    $gitHash = ""
    try {
        $gitHash = "-$(git rev-parse --short HEAD 2>$null)"
    } catch {
        # Git not available or not in repo
    }
    
    switch ($BuildType) {
        "cpu" { return "cpu-${timestamp}${gitHash}" }
        "gpu" { return "gpu-${timestamp}${gitHash}" }
        "macos" { return "macos-${timestamp}${gitHash}" }
        default { return "${BuildType}-${timestamp}${gitHash}" }
    }
}

# Function to build Docker image
function Build-Image {
    param(
        [string]$BuildType,
        [string]$Tag
    )
    
    $dockerfile = ""
    $projectName = ""
    
    # Determine dockerfile and project name
    switch ($BuildType) {
        "cpu" {
            $dockerfile = "Dockerfile.server-cpu"
            $projectName = $WhisperProjectName
        }
        "gpu" {
            $dockerfile = "Dockerfile.server-gpu"
            $projectName = $WhisperProjectName
        }
        "macos" {
            $dockerfile = "Dockerfile.server-macos"
            $projectName = $WhisperProjectName
        }
        "app" {
            $dockerfile = "Dockerfile.app"
            $projectName = $AppProjectName
        }
        default {
            Write-Error "Unknown build type: $BuildType"
            return $false
        }
    }
    
    # Construct full tag
    $fullTag = if ($Registry) { "${Registry}/${projectName}:${Tag}" } else { "${projectName}:${Tag}" }
    
    # Build command
    $buildCmd = @("docker", "buildx", "build", "--platform", $Platforms, "--file", $dockerfile, "--tag", $fullTag)
    
    # Parse build arguments
    if ($BuildArgs) {
        $buildArgsArray = $BuildArgs -split '\s+'
        foreach ($arg in $buildArgsArray) {
            $buildCmd += "--build-arg"
            $buildCmd += $arg
        }
    }
    
    # Add cache options
    if ($NoCache) {
        $buildCmd += "--no-cache"
    }
    
    # Add push/load option - only use --load for single platform builds
    if ($Push) {
        $buildCmd += "--push"
    } else {
        # Check if building for multiple platforms
        if ($Platforms -contains ",") {
            Write-Warn "Multi-platform build detected without -Push"
            Write-Warn "Multi-platform builds cannot be loaded locally"
            Write-Warn "Either use -Push or specify single platform with -Platforms"
            return $false
        } else {
            $buildCmd += "--load"
        }
    }
    
    # Add context
    $buildCmd += "."
    
    Write-Info "Building $BuildType image: $fullTag"
    Write-Info "Platforms: $Platforms"
    Write-Info "Dockerfile: $dockerfile"
    
    if ($DryRun) {
        Write-Info "DRY RUN - Command would be:"
        Write-Host ($buildCmd -join " ")
        return $true
    }
    
    # Execute build
    try {
        & $buildCmd[0] $buildCmd[1..($buildCmd.Length-1)]
        Write-Info "✓ Successfully built: $fullTag"
        
        # Also tag as latest for this build type
        $latestTag = if ($Registry) { "${Registry}/${projectName}:${BuildType}" } else { "${projectName}:${BuildType}" }
        
        if ($Push) {
            Write-Info "Tagging as latest: $latestTag"
            $latestCmd = @("docker", "buildx", "build", "--platform", $Platforms, "--file", $dockerfile, "--tag", $latestTag, "--push", ".")
            if ($BuildArgs) {
                $buildArgsArray = $BuildArgs -split '\s+'
                foreach ($arg in $buildArgsArray) {
                    $latestCmd = $latestCmd[0..4] + "--build-arg" + $arg + $latestCmd[5..($latestCmd.Length-1)]
                }
            }
            & $latestCmd[0] $latestCmd[1..($latestCmd.Length-1)]
        } else {
            # For local builds, create a simple tag without timestamp
            Write-Info "Tagging locally: $latestTag"
            docker tag $fullTag $latestTag
        }
        
        return $true
    } catch {
        Write-Error "✗ Failed to build: $fullTag"
        return $false
    }
}

# Main function
function Main {
    if ($Help) {
        Show-Help
        exit 0
    }
    
    Write-Info "=== Whisper Server Docker Builder ==="
    Write-Info "Build type: $BuildType"
    Write-Info "Registry: $(if ($Registry) { $Registry } else { '<none>' })"
    Write-Info "Platforms: $Platforms"
    Write-Info "Push: $Push"
    
    # Auto-detect macOS and adjust build type if needed
    if ($IsMacOS -and $BuildType -eq "cpu") {
        Write-Info "macOS detected - switching from CPU to macOS-optimized build"
        $BuildType = "macos"
    } elseif ($IsMacOS -and $BuildType -eq "gpu") {
        Write-Warn "GPU build requested on macOS - switching to macOS-optimized (CPU-only) build"
        $BuildType = "macos"
    }
    
    # Check prerequisites
    Test-Prerequisites
    
    # Build images - always build meeting app alongside whisper server
    switch ($BuildType) {
        "cpu" {
            $whisperTag = New-Tag "cpu" $Tag
            $appTag = New-Tag "app" $Tag
            
            Write-Info "Building whisper server (CPU) + meeting app..."
            $success1 = Build-Image "cpu" $whisperTag
            $success2 = Build-Image "app" $appTag
            
            if (-not ($success1 -and $success2)) {
                exit 1
            }
        }
        "gpu" {
            $whisperTag = New-Tag "gpu" $Tag
            $appTag = New-Tag "app" $Tag
            
            Write-Info "Building whisper server (GPU) + meeting app..."
            $success1 = Build-Image "gpu" $whisperTag
            $success2 = Build-Image "app" $appTag
            
            if (-not ($success1 -and $success2)) {
                exit 1
            }
        }
        "macos" {
            $whisperTag = New-Tag "macos" $Tag
            $appTag = New-Tag "app" $Tag
            
            Write-Info "Building whisper server (macOS-optimized) + meeting app..."
            $success1 = Build-Image "macos" $whisperTag
            $success2 = Build-Image "app" $appTag
            
            if (-not ($success1 -and $success2)) {
                exit 1
            }
        }
        "both" {
            $cpuTag = New-Tag "cpu" $Tag
            $gpuTag = New-Tag "gpu" $Tag
            $appTag = New-Tag "app" $Tag
            
            Write-Info "Building both whisper server versions + meeting app..."
            $success1 = Build-Image "cpu" $cpuTag
            $success2 = Build-Image "gpu" $gpuTag
            $success3 = Build-Image "app" $appTag
            
            if (-not ($success1 -and $success2 -and $success3)) {
                exit 1
            }
        }
        default {
            Handle-Error "Invalid build type: $BuildType"
        }
    }
    
    Write-Info "=== Build Complete ==="
    
    # Show built images
    if (-not $DryRun -and -not $Push) {
        Write-Info "Built images:"
        try {
            docker images $WhisperProjectName --format "table {{.Repository}}:{{.Tag}}`t{{.Size}}`t{{.CreatedAt}}"
            docker images $AppProjectName --format "table {{.Repository}}:{{.Tag}}`t{{.Size}}`t{{.CreatedAt}}"
        } catch {
            # Ignore errors if images command fails
        }
    }
}

# Execute main function
Main