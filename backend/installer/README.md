# Meeting Minutes Backend Installer

This directory contains the Windows installer build system for the Meeting Minutes Backend, designed to create a single-click installation experience for non-technical users.

## Overview

The installer bundles:
- **Whisper.cpp server** (pre-compiled binary)
- **Python backend API** (packaged as standalone executable)
- **Whisper AI models** (user-selectable during install)
- **Visual C++ Redistributables**
- **Launcher and management scripts**

## Files

- `build_executable.py` - PyInstaller script to package Python backend
- `installer.iss` - Inno Setup script for creating Windows installer
- `meeting-minutes-launcher.cmd` - Service launcher and manager
- `README.md` - This documentation

## Building the Installer

### Prerequisites (Development Machine Only)

1. **Python 3.11+** with pip
2. **Visual Studio Build Tools** or full Visual Studio
3. **CMake** for building whisper.cpp
4. **Inno Setup 6** for creating installer
5. **PyInstaller** (`pip install pyinstaller`)

### Build Steps

#### 1. Build Whisper.cpp
```cmd
cd backend
build_whisper.cmd small
```

#### 2. Package Python Backend
```cmd
cd backend
python installer/build_executable.py
```

#### 3. Create Installer
```cmd
cd backend/installer
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer.iss
```

The installer will be created in `installer/output/MeetingMinutesSetup-1.0.0.exe`

## Automated Builds

Push a git tag to trigger automated installer build:
```bash
git tag v1.0.0
git push origin v1.0.0
```

The GitHub Actions workflow will:
1. Build whisper.cpp for Windows
2. Package Python backend
3. Create installer
4. Upload to GitHub Releases

## User Experience

### For End Users (No Technical Knowledge Required)

1. **Download** the installer from Releases
2. **Run** the installer (may need "Run as Administrator")
3. **Select** model size during installation
4. **Launch** from Start Menu or Desktop

The installer handles everything:
- No Python installation needed
- No Visual Studio needed
- No command line required
- Automatic dependency installation
- Service management included

### Post-Installation

Users can:
- Start/stop services from the launcher
- Access web interface at http://localhost:8000/docs
- Configure settings through the UI
- Uninstall cleanly from Windows Settings

## Customization

### Adding Models

Edit `installer.iss` to add more model options:
```pascal
Source: "..\models\ggml-large-v3-turbo.bin"; DestDir: "{app}\models"; Check: IsModelSelected('turbo')
```

### Changing Ports

Default ports are configured in the installer and can be changed during installation.

### Branding

Replace `icon.ico` with your application icon and update the installer metadata in `installer.iss`.

## Troubleshooting

### Build Issues

1. **PyInstaller fails**: Ensure all Python dependencies are installed
2. **Whisper.cpp build fails**: Check Visual Studio Build Tools installation
3. **Installer creation fails**: Verify Inno Setup is installed correctly

### Runtime Issues

The launcher creates logs in the `logs/` directory:
- `whisper-server.log` - Whisper server output
- `backend.log` - Python backend output

## License

See main project LICENSE file.