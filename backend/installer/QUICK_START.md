# Quick Start Guide - Windows Installer

## For Users: 3 Simple Steps

### 1️⃣ Download
Get the latest installer from [Releases](https://github.com/zackriya-solutions/meeting-minutes/releases)

### 2️⃣ Install
- Right-click → "Run as Administrator"
- Choose "Small" model (recommended)
- Click Next → Install

### 3️⃣ Launch
- Use Desktop shortcut
- Or Start Menu → Meeting Minutes Backend

**That's it!** Access at: http://localhost:8000/docs

---

## For Developers: Build Your Own Installer

### Quick Build (Existing Setup)
```cmd
cd backend
python installer/build_executable.py
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer/installer.iss
```

### Automated Build (GitHub Actions)
```bash
git tag v1.0.0
git push origin v1.0.0
# Installer appears in Releases automatically
```

---

## Common Tasks

### Check if Running
```cmd
meeting-minutes-launcher.cmd --status
```

### Stop Services
```cmd
meeting-minutes-launcher.cmd --stop
```

### View Logs
```cmd
cd "C:\Program Files\MeetingMinutes\logs"
type backend.log
```

---

## Need Help?

- 📖 [Full Documentation](WINDOWS_INSTALLER_GUIDE.md)
- 🐛 [Report Issues](https://github.com/zackriya-solutions/meeting-minutes/issues)
- 💬 [Discussions](https://github.com/zackriya-solutions/meeting-minutes/discussions)