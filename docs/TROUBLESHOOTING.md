# Troubleshooting Guide

Common issues and solutions for GameDev packages.

## Table of Contents

- [CUDA Out of Memory](#cuda-out-of-memory)
- [Model Download Fails](#model-download-fails)
- [Import Errors](#import-errors)
- [PyTorch/CUDA Issues](#pytorchcuda-issues)
- [Installation Problems](#installation-problems)
- [CLI Errors](#cli-errors)

---

## CUDA Out of Memory

**Symptom:** `CUDA out of memory` error when running generation.

**Solutions:**

### 1. Reduce batch size
```bash
# Text3D with smaller batches
text3d generate "prompt" --batch-size 1

# Part3D with fewer parts per batch
part3d generate input.glb --max-parts-per-batch 2
```

### 2. Use low VRAM mode
```bash
text3d generate "prompt" --low-vram
part3d generate input.glb --low-vram
```

### 3. Enable CPU offload
```bash
text3d generate "prompt" --cpu-offload
```

### 4. Clear GPU memory
```python
import torch
torch.cuda.empty_cache()
```

### 5. Close other GPU applications
```bash
# Check GPU usage
nvidia-smi

# Kill other processes if needed
sudo kill -9 <PID>
```

---

## Model Download Fails

**Symptom:** `HTTPError` or `ConnectionError` when downloading models.

### 1. Set HuggingFace token
```bash
export HF_TOKEN=your_token_here

# Or in ~/.bashrc for persistence
echo 'export HF_TOKEN=your_token_here' >> ~/.bashrc
```

### 2. Login to HuggingFace
```bash
pip install huggingface_hub
huggingface-cli login
```

### 3. Use offline mode (if models already downloaded)
```bash
export HF_HUB_OFFLINE=1
text3d generate "prompt"
```

### 4. Check network connectivity
```bash
curl -I https://huggingface.co
```

### 5. Retry with longer timeout
```python
from huggingface_hub import hf_hub_download
hf_hub_download(..., timeout=300)  # 5 minutes
```

---

## Import Errors

**Symptom:** `ModuleNotFoundError` or `ImportError` when importing a package.

### 1. Activate virtual environment
```bash
source Text2D/.venv/bin/activate
```

### 2. Reinstall package
```bash
pip uninstall gamedev-text2d
pip install -e Text2D/'[dev]'
```

### 3. Check Python version
```bash
python --version  # Should be 3.10+
```

### 4. Install missing dependencies
```bash
pip install -r Text2D/requirements.txt
```

---

## PyTorch/CUDA Issues

**Symptom:** PyTorch can't find CUDA or GPU.

### 1. Check CUDA installation
```bash
nvidia-smi  # Should show GPU info
python -c "import torch; print(torch.cuda.is_available())"
```

### 2. Install PyTorch with CUDA
```bash
pip install torch torchvision --index-url https://download.pytorch.org/whl/cu121
```

### 3. Verify GPU detection
```python
import torch
print(f"CUDA available: {torch.cuda.is_available()}")
print(f"GPU count: {torch.cuda.device_count()}")
print(f"GPU name: {torch.cuda.get_device_name(0)}")
```

### 4. For NVIDIA drivers
```bash
# Check driver version
nvidia-smi | head -3

# Update drivers if needed
sudo apt update && sudo apt upgrade nvidia-driver-535
```

---

## Installation Problems

**Symptom:** `pip install -e .` fails.

### 1. Create fresh virtual environment
```bash
rm -rf Text2D/.venv
python -m venv Text2D/.venv
source Text2D/.venv/bin/activate
pip install -U pip
pip install -e Text2D/'[dev]'
```

### 2. Check system dependencies
```bash
# Ubuntu/Debian
sudo apt install python3-dev build-essential

# Fedora
sudo dnf install python3-devel gcc-c++
```

### 3. For packages with Rust (Materialize, Rigging3D)
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 4. Clean pip cache
```bash
pip cache purge
pip install -e .
```

---

## CLI Errors

### "Command not found" after install

```bash
# Refresh shell
hash -r

# Or reinstall the package
pip install -e Text2D/ --force-reinstall
```

### "Error: Invalid value for '--output'" 

```bash
# Check output directory exists
mkdir -p outputs
text3d generate "prompt" --output outputs/result.glb
```

### "Permission denied" errors

```bash
# Don't use sudo with pip install --user
pip install -e . --user

# Or fix permissions
chmod +x ~/.local/bin/text3d
```

---

## Getting Help

If issues persist:

1. Check GitHub Issues: https://github.com/maikramer/GameDev/issues
2. Run with verbose logging:
   ```bash
   export GAMEDEV_LOG_LEVEL=DEBUG
   text3d generate "prompt" 2>&1 | tee debug.log
   ```
3. Include in your issue:
   - OS and Python version
   - GPU model and driver version
   - Full error traceback
   - Steps to reproduce
