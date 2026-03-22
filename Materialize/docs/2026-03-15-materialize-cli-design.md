# Design Document: Materialize CLI

**Data:** 2026-03-15  
**Versão:** 1.0 (MVP)  
**Status:** Aprovado para implementação

---

## Resumo Executivo

Materialize CLI é uma ferramenta de linha de comando em Rust que converte imagens difusas em materiais PBR (Height, Normal, Metallic) usando compute shaders GPU via wgpu. Baseada no Materialize original (Unity), esta versão prioriza simplicidade: um comando, múltiplas saídas.

### Diferenciais

- **Minimalista:** Sem UI, sem Unity, apenas CLI
- **Performática:** Processamento GPU nativo via compute shaders
- **Cross-platform:** Linux, Windows, macOS
- **Direta:** Fluxo simples e intuitivo

---

## Requisitos

### Funcionais

| ID | Requisito | Prioridade |
|----|-----------|------------|
| F1 | Gerar Height map a partir de imagem difusa | Must |
| F2 | Gerar Normal map a partir do Height | Must |
| F3 | Gerar Metallic map a partir da imagem difusa | Must |
| F4 | Suportar formatos PNG, JPG, TGA, EXR | Must |
| F5 | Interface CLI simples (um comando) | Must |
| F6 | Preview de progresso (verbose mode) | Should |
| F7 | Configurar diretório de saída | Must |
| F8 | Configurar formato de saída | Should |

### Não-funcionais

| ID | Requisito | Meta |
|----|-----------|------|
| NF1 | Tempo de processamento | < 1s para imagem 2K |
| NF2 | Uso de memória GPU | < 500MB |
| NF3 | Tamanho do binário | < 10MB |
| NF4 | Dependências runtime | Zero (binário standalone) |
| NF5 | Documentação | Completa (docs/) |

---

## Arquitetura

### Diagrama de Componentes

```
┌────────────────────────────────────────────────────────────────┐
│                         CLI (main.rs)                         │
│                    clap para argumentos                        │
└────────────────────────┬───────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────────┐
│                      Pipeline (pipeline.rs)                    │
│  Orquestra: Diffuse → Height → Normal → Metallic              │
│  Gerencia dependências entre mapas (Height necessário p/ Normal) │
└────────────────────────┬───────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────────┐
│                      GPU Context (gpu.rs)                      │
│  - Instance/Adapter/Device/Queue (wgpu)                         │
│  - Texture management (input/output buffers)                    │
│  - Compute pipeline setup                                       │
└────────────────────────┬───────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────────┐
│                    Compute Shaders (WGSL)                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │  height     │  │   normal    │  │  metallic   │             │
│  │  .wgsl      │  │   .wgsl     │  │  .wgsl      │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└────────────────────────┬───────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────────┐
│                      I/O (io.rs)                               │
│  - image crate: PNG/JPG/TGA/BMP/EXR                             │
│  - GPU↔CPU texture transfer                                     │
└────────────────────────────────────────────────────────────────┘
```

### Descrição dos Componentes

#### 1. CLI (`main.rs` + `cli.rs`)

**Responsabilidade:** Entry point e parsing de argumentos

**Implementação:**
```rust
// cli.rs
use clap::Parser;

#[derive(Parser)]
#[command(name = "materialize")]
#[command(about = "Generate PBR maps from diffuse texture")]
pub struct Cli {
    #[arg(help = "Input image path")]
    pub input: String,
    
    #[arg(short, long, help = "Output directory", default_value = "./")]
    pub output: String,
    
    #[arg(short, long, help = "Output format", default_value = "png")]
    pub format: OutputFormat,
    
    #[arg(short, long, help = "Quality for JPEG (0-100)", default_value = "95")]
    pub quality: u8,
    
    #[arg(short, long, help = "Verbose output")]
    pub verbose: bool,
}
```

#### 2. Pipeline (`pipeline.rs`)

**Responsabilidade:** Coordenar execução sequencial dos shaders

**Fluxo:**
```rust
pub struct Pipeline {
    gpu: GpuContext,
}

impl Pipeline {
    pub async fn process(&self, input: &Image) -> Result<PbrMaps> {
        // 1. Upload input texture
        let diffuse = self.gpu.create_texture_from_image(input);
        
        // 2. Generate height
        let height = self.gpu.create_texture(input.size(), R32Float);
        self.run_shader(&self.height_pipeline, &diffuse, &height);
        
        // 3. Generate normal from height
        let normal = self.gpu.create_texture(input.size(), RGBA8Unorm);
        self.run_shader(&self.normal_pipeline, &height, &normal);
        
        // 4. Generate metallic from diffuse
        let metallic = self.gpu.create_texture(input.size(), R8Unorm);
        self.run_shader(&self.metallic_pipeline, &diffuse, &metallic);
        
        // 5. Download results
        Ok(PbrMaps {
            height: self.gpu.read_texture(&height).await,
            normal: self.gpu.read_texture(&normal).await,
            metallic: self.gpu.read_texture(&metallic).await,
        })
    }
}
```

#### 3. GPU Context (`gpu.rs`)

**Responsabilidade:** Abstrair todas interações com wgpu

**API Pública:**
```rust
pub struct GpuContext {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuContext {
    pub async fn new() -> Result<Self>;
    pub fn create_texture(&self, size: Extent3d, format: TextureFormat) -> Texture;
    pub fn load_shader(&self, wgsl: &str) -> ShaderModule;
    pub fn create_compute_pipeline(&self, shader: &ShaderModule, entry: &str) -> ComputePipeline;
    pub fn dispatch(&self, pipeline: &ComputePipeline, bind_group: &BindGroup, workgroups: (u32, u32, u32));
    pub async fn read_texture(&self, texture: &Texture) -> Vec<u8>;
}
```

#### 4. Shaders WGSL

**Responsabilidade:** Processamento de imagem em GPU

**Estratégia:** Cada shader é independente, compilado em runtime

**Embutimento:**
```rust
// Shaders são strings literais incluídas no binário
const HEIGHT_SHADER: &str = include_str!("shaders/height.wgsl");
const NORMAL_SHADER: &str = include_str!("shaders/normal.wgsl");
const METALLIC_SHADER: &str = include_str!("shaders/metallic.wgsl");
```

#### 5. I/O (`io.rs`)

**Responsabilidade:** Ler e escrever arquivos de imagem

**Implementação:**
```rust
use image::{DynamicImage, ImageFormat};

pub fn load_image(path: &str) -> Result<DynamicImage>;
pub fn save_image(image: &DynamicImage, path: &str, format: ImageFormat, quality: u8) -> Result<()>;
```

---

## Algoritmos

### Height Map Generation

**Entrada:** Imagem RGBA  
**Saída:** Height map (grayscale, R32Float)

**Passos:**
1. Converter para luminância (pesos RGB: 0.299, 0.587, 0.114)
2. Aplicar Gaussian blur em 7 níveis (σ = 1, 2, 4, 8, 16, 32, 64)
3. Combinar com pesos: [0.5, 0.3, 0.15, 0.03, 0.015, 0.003, 0.002]
4. Aplicar sigmoid para contraste (contrast = 1.5)

**Complexidade:** O(n × k) onde n = pixels, k = blur kernel size

### Normal Map Generation

**Entrada:** Height map (R32Float)  
**Saída:** Normal map (RGB, RGBA8Unorm)

**Passos:**
1. Calcular gradientes via operador Sobel:
   - Sobel X: [-1, 0, +1; -2, 0, +2; -1, 0, +1]
   - Sobel Y: [-1, -2, -1; 0, 0, 0; +1, +2, +1]
2. Construir vetor normal: (-gx, -gy, 1.0)
3. Normalizar e codificar para [0, 255]

**Formato:** DirectX (Y down in texture)

### Metallic Map Generation

**Entrada:** Imagem RGBA (diffuse)  
**Saída:** Metallic map (grayscale, R8Unorm)

**Passos:**
1. Converter RGB para HSL
2. Detectar metais por heurísticas:
   - Metais cinzentos: saturação < 0.15, luminância > 0.4
   - Ouro: matiz 0.08-0.15, saturação > 0.3, luminância > 0.3
   - Cobre: matiz 0.02-0.08, saturação > 0.4, luminância > 0.25
3. Aplicar smoothstep para transições suaves

---

## Interface CLI

### Comando

```
materialize [OPTIONS] <INPUT>
```

### Argumentos

| Argumento | Tipo | Obrigatório | Descrição |
|-----------|------|-------------|-----------|
| `INPUT` | String | Sim | Caminho para imagem de entrada |

### Options

| Option | Curta | Tipo | Padrão | Descrição |
|--------|-------|------|--------|-----------|
| `--output` | `-o` | String | `./` | Diretório de saída |
| `--format` | `-f` | Enum | `png` | Formato (png, jpg, tga, exr) |
| `--quality` | `-q` | Int | `95` | Qualidade JPEG (0-100) |
| `--verbose` | `-v` | Bool | `false` | Modo verbose |
| `--help` | `-h` | | | Ajuda |
| `--version` | `-V` | | | Versão |

### Outputs Gerados

Para input `texture.png`:
- `texture_height.png` - Height map
- `texture_normal.png` - Normal map
- `texture_metallic.png` - Metallic map

### Exemplos

```bash
# Básico
materialize texture.png

# Diretório de saída
materialize texture.png -o ./materials/

# Formato diferente
materialize texture.png -f exr

# Qualidade JPEG
materialize texture.jpg -f jpg -q 85

# Verbose
materialize texture.png -v
```

---

## Dependências

### Cargo.toml

```toml
[package]
name = "materialize-cli"
version = "1.0.0"
edition = "2021"

[dependencies]
wgpu = "0.19"
pollster = "0.3"
image = { version = "0.24", default-features = false, features = ["png", "jpeg", "tga", "exr"] }
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"

[dev-dependencies]
tempfile = "3.10"
```

### Justificativas

- **wgpu 0.19:** Versão estável com compute shaders completos
- **pollster:** Runtime async blocking mais simples que tokio para esse caso
- **image:** Crate padrão de processamento de imagem em Rust
- **clap:** CLI parsing mais popular e bem documentado
- **anyhow:** Error handling ergonomico para aplicações

---

## Estrutura de Diretórios

```
materialize-cli/
├── Cargo.toml
├── README.md
├── docs/
│   ├── README.md
│   ├── architecture.md
│   ├── features.md
│   ├── cli-api.md
│   ├── algorithms.md
│   ├── shaders.md
│   └── roadmap.md
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── cli.rs
│   ├── pipeline.rs
│   ├── gpu.rs
│   ├── io.rs
│   └── shaders/
│       ├── height.wgsl
│       ├── normal.wgsl
│       └── metallic.wgsl
├── tests/
│   └── integration_tests.rs
└── examples/
    └── batch_convert.rs
```

---

## Casos de Uso

### Game Development Pipeline

```bash
#!/bin/bash
# Pipeline de assets

INPUT_DIR="./raw_textures"
OUTPUT_DIR="./processed"

mkdir -p "$OUTPUT_DIR"

for texture in "$INPUT_DIR"/*.png; do
    name=$(basename "$texture" .png)
    echo "Processing $name..."
    materialize "$texture" -o "$OUTPUT_DIR/$name/" -p "$name"
done
```

### Blender Workflow

```bash
# Preparar textura importada
materialize photo.jpg -f exr -o ~/blender_project/textures/

# Usar em Blender:
# - height.exr → Displacement modifier
# - normal.exr → Bump node
# - metallic.exr → Metallic socket do Principled BSDF
```

### Web (Three.js)

```bash
# Otimizar para web
materialize texture.png -f jpg -q 85 -o ./public/textures/
```

---

## Limitações do MVP

1. **Parâmetros fixos:** Não é possível ajustar blur, contraste, etc.
2. **Um formato:** Todos outputs no mesmo formato
3. **Sem config file:** Não suporta arquivos de configuração
4. **GPU memory bound:** Tamanho máximo depende da GPU
5. **Sem alpha handling:** Canal alpha ignorado
6. **Batch manual:** Sem processamento automático de diretórios

---

## Roadmap

| Versão | Features | Timeline |
|--------|----------|----------|
| 1.0 (MVP) | Height, Normal, Metallic, CLI básica | Atual |
| 1.1 | Parâmetros inline, Smoothness | +2 semanas |
| 1.2 | Batch processing, progress bar | +1 mês |
| 2.0 | AO (Ambient Occlusion) | +2 meses |
| 2.1 | Config files (TOML), profiles | +3 meses |
| 3.0 | Preview window (SDL2) | +4 meses |
| 3.1 | Seamless texture maker | +5 meses |
| 4.0 | ML-based detection, super-resolution | +6 meses |

---

## Decisões de Design

### Por que wgpu?

- **Cross-platform:** Vulkan/Metal/DX12 com uma API
- **Moderno:** WebGPU é o futuro
- **Compute:** Projetado para GPGPU, não só gráficos
- **Rust-native:** Melhor integração

### Por que compute shaders?

- **Direto:** Sem overhead de render pipeline
- **Simples:** Um shader = uma função
- **Eficiente:** Sem rasterização

### Por que Rust?

- **Performance:** Zero-cost abstractions
- **Segurança:** Ownership previne bugs
- **Ecosystem:** wgpu é Rust-first
- **Deploy:** Binário standalone

### Por que não reimplementar tudo do Materialize original?

- **Foco:** 3 mapas essenciais cobrem 80% dos casos de uso
- **Complexidade:** AO, Edge requerem ray marching (significativamente mais complexo)
- **MVP:** Menor scope = entrega mais rápida

---

## Critérios de Sucesso

### Funcionais

- [ ] Processa imagem 2K em < 1s
- [ ] Gera 3 mapas corretamente
- [ ] CLI intuitiva e bem documentada
- [ ] Binário < 10MB

### Qualidade

- [ ] Height map captura formas principais
- [ ] Normal map é matematicamente correto
- [ ] Metallic detecta metais comuns
- [ ] Code review aprovado

### User Experience

- [ ] Mensagens de erro claras
- [ ] Documentação completa
- [ ] Exemplos funcionais

---

## Aprovação

**Design aprovado para implementação.**

Próximo passo: Criar plano de implementação detalhado via skill `writing-plans`.
