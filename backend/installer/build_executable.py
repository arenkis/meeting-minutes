"""
PyInstaller build script for Meeting Minutes Backend
Creates a standalone Windows executable from the Python FastAPI application
"""

import os
import sys
import shutil
import subprocess
from pathlib import Path

def build_executable():
    """Build the Python backend into a standalone executable"""
    
    # Get the backend directory
    backend_dir = Path(__file__).parent.parent
    app_dir = backend_dir / "app"
    
    # PyInstaller spec configuration
    spec_content = f'''
# -*- mode: python ; coding: utf-8 -*-

a = Analysis(
    ['{app_dir / "main.py"}'],
    pathex=['{backend_dir}', '{app_dir}'],
    binaries=[],
    datas=[
        ('{backend_dir / "requirements.txt"}', '.'),
        ('{backend_dir / "temp.env"}', '.'),
    ],
    hiddenimports=[
        'uvicorn.logging',
        'uvicorn.loops',
        'uvicorn.loops.auto',
        'uvicorn.protocols',
        'uvicorn.protocols.http',
        'uvicorn.protocols.http.auto',
        'uvicorn.protocols.websockets',
        'uvicorn.protocols.websockets.auto',
        'uvicorn.lifespan',
        'uvicorn.lifespan.on',
        'fastapi',
        'pydantic',
        'pydantic_ai',
        'pandas',
        'aiosqlite',
        'ollama',
        'devtools',
        'dotenv',
        'multipart',
    ],
    hookspath=[],
    hooksconfig={{}},
    runtime_hooks=[],
    excludes=[
        'tkinter',
        'matplotlib',
        'notebook',
        'jupyter',
        'pytest',
    ],
    noarchive=False,
    optimize=2,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='meeting-minutes-backend',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=None,
    version_file=None,
)
'''
    
    # Write spec file
    spec_file = backend_dir / "meeting-minutes-backend.spec"
    with open(spec_file, 'w') as f:
        f.write(spec_content)
    
    print("Building executable with PyInstaller...")
    print(f"Spec file: {spec_file}")
    
    # Run PyInstaller
    try:
        subprocess.run([
            sys.executable, "-m", "PyInstaller",
            "--clean",
            "--noconfirm",
            str(spec_file)
        ], check=True, cwd=backend_dir)
        
        # Find the output executable
        dist_dir = backend_dir / "dist"
        exe_file = dist_dir / "meeting-minutes-backend.exe"
        
        if exe_file.exists():
            print(f"✓ Executable built successfully: {exe_file}")
            
            # Copy to installer directory
            installer_dir = backend_dir / "installer"
            installer_dir.mkdir(exist_ok=True)
            shutil.copy2(exe_file, installer_dir / "meeting-minutes-backend.exe")
            print(f"✓ Copied to installer directory")
            
            return True
        else:
            print("✗ Executable not found after build")
            return False
            
    except subprocess.CalledProcessError as e:
        print(f"✗ Build failed: {e}")
        return False
    finally:
        # Cleanup
        if spec_file.exists():
            os.remove(spec_file)
        
        # Clean build artifacts
        for dir_name in ["build", "dist", "__pycache__"]:
            dir_path = backend_dir / dir_name
            if dir_path.exists():
                shutil.rmtree(dir_path)

if __name__ == "__main__":
    # Install PyInstaller if not present
    try:
        __import__('PyInstaller')
    except ImportError:
        print("Installing PyInstaller...")
        subprocess.run([sys.executable, "-m", "pip", "install", "pyinstaller"], check=True)
    
    success = build_executable()
    sys.exit(0 if success else 1)