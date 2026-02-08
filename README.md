# llm-check

[![CI](https://github.com/kaushal1717/llm-check/workflows/CI/badge.svg)](https://github.com/kaushal1717/llm-check/actions)

Check if LLMs fit your hardware — with native support for Apple Silicon and NVIDIA GPUs.

## Features

- **Hardware Detection** — CPU, RAM, GPU, and memory bandwidth scanning
- **Model Compatibility** — Check if specific HuggingFace models fit your VRAM
- **Performance Prediction** — Estimate tokens/second based on your hardware
- **MoE Support** — Accurate VRAM and speed estimates for Mixture of Experts models
- **Multi-GPU Topology** — PCIe and NVLink configuration inspection
- **Energy & Carbon Tracking** — Environmental impact of local inference
- **Bandwidth Benchmark** — Measure real memory bandwidth for accurate predictions
- **Shell Completions** — Tab completion for bash, zsh, fish
- **Self-Update** — One-command updates from GitHub releases

## Installation

### Pre-built Binaries

Download from [Releases](https://github.com/kaushal1717/llm-check/releases/latest):

```bash
# macOS (Apple Silicon)
curl -L https://github.com/kaushal1717/llm-check/releases/latest/download/llm-check-macos-aarch64.tar.gz | tar xz
sudo mv llm-check /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/kaushal1717/llm-check/releases/latest/download/llm-check-macos-x86_64.tar.gz | tar xz
sudo mv llm-check /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/kaushal1717/llm-check/releases/latest/download/llm-check-linux-x86_64.tar.gz | tar xz
sudo mv llm-check /usr/local/bin/
```

### Build from Source

```bash
cargo install --git https://github.com/kaushal1717/llm-check
```

## Usage

### Scan Hardware
```bash
llm-check scan
llm-check --format json scan    # machine-readable output
```

### Check Model Compatibility
```bash
llm-check check meta-llama/Llama-3.1-8B
llm-check check meta-llama/Llama-3.1-8B --quant fp16
llm-check check mistralai/Mixtral-8x7B-v0.1    # MoE model
llm-check check meta-llama/Llama-3.1-8B --eco   # include carbon impact
```

### Suggest Compatible Models
```bash
llm-check suggest
```

### Multi-GPU Topology
```bash
llm-check topology
```

### Benchmark Memory Bandwidth
```bash
llm-check bench
llm-check bench --iterations 20
```

### Configuration
```bash
llm-check config set-token hf_xxxxx       # HuggingFace token
llm-check config set-region EU             # carbon intensity region
llm-check config status                    # show current config
```

### Shell Completions
```bash
# Bash
llm-check completions bash > /etc/bash_completion.d/llm-check

# Zsh
llm-check completions zsh > ~/.zsh/completion/_llm-check

# Fish
llm-check completions fish > ~/.config/fish/completions/llm-check.fish
```

### Self-Update
```bash
llm-check update                # install latest version
llm-check update --check-only   # check without installing
```

## Platform Support

| Platform | GPU | Status |
|----------|-----|--------|
| macOS (Apple Silicon) | Unified Memory | Full support |
| macOS (Intel) | - | CPU-only |
| Linux (x86_64) | NVIDIA CUDA | Full support |
| Windows (x86_64) | NVIDIA CUDA | Full support |

## License

MIT
