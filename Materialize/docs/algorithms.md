# Algoritmos de Processamento

## Visão Geral

O Materialize CLI implementa seis algoritmos principais, cada um executado como compute shader WGSL:

1. **Height from Diffuse** - Extrai informação de altura da imagem colorida
2. **Normal from Height** - Calcula vetores normais a partir do height map
3. **Metallic from Diffuse** - Detecta metalicidade por análise de cor
4. **Smoothness** - Base + contribuição do metallic (difusa + metallic como entrada)
5. **Edge from Normal** - Gradiente da normal (X/Y) para detecção de bordas
6. **AO from Height** - Cavity-style: amostras em 8 direções, oclusão quando vizinho > centro

## 1. Height Map Generation

### Objetivo

Converter uma imagem RGB em um mapa de altura em escala de cinza onde:
- Branco (1.0) = pontos altos
- Preto (0.0) = pontos baixos
- Valores intermediários = gradiente de altura

### Algoritmo Completo

#### Passo 1: Luminance Conversion

Converte RGB para luminância usando pesos perceptuais:

```wgsl
fn rgb_to_luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
}
```

**Rationale:** Pesos baseados na sensibilidade do olho humano (verde mais perceptível).

#### Passo 2: Multi-Level Gaussian Blur

Aplica Gaussian blur em múltiplas escalas para capturar detalhes em diferentes frequências:

```
Level 0: σ = 1.0  (detalhes finos)
Level 1: σ = 2.0  (detalhes médios)
Level 2: σ = 4.0  (formas grandes)
Level 3: σ = 8.0  (estrutura geral)
Level 4: σ = 16.0 (contornos grandes)
Level 5: σ = 32.0 (formas principais)
Level 6: σ = 64.0 (estrutura macro)
```

**Pesos de combinação:**
```
weights = [0.5, 0.3, 0.15, 0.03, 0.015, 0.003, 0.002]
```

**Kernel Gaussiano 1D (para separação):**
```
G(x) = (1.0 / (sqrt(2π) * σ)) * exp(-x² / (2σ²))
```

**Implementação separável:**
- Passo horizontal: amostra linha, aplica kernel 1D
- Passo vertical: amostra coluna, aplica kernel 1D
- Mais eficiente que kernel 2D (O(n) vs O(n²))

#### Passo 3: Contrast Enhancement

Aplica sigmoid para aumentar contraste:

```wgsl
fn enhance_contrast(value: f32, contrast: f32) -> f32 {
    // Map [0,1] to [-1,1]
    let centered = value * 2.0 - 1.0;
    // Sigmoid function
    let enhanced = centered / (1.0 + exp(-contrast * centered));
    // Map back to [0,1]
    return (enhanced + 1.0) * 0.5;
}
```

**Parâmetros:**
- `contrast = 1.0`: Linear
- `contrast > 1.0`: Mais contraste
- `contrast < 1.0`: Menos contraste

### Pseudocódigo WGSL

```wgsl
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let coords = vec2<i32>(gid.xy);
    let dims = textureDimensions(input_texture);
    
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }
    
    // Sample and convert to luminance
    let color = textureLoad(input_texture, coords, 0).rgb;
    let luminance = rgb_to_luminance(color);
    
    // Multi-level blur
    var height = 0.0;
    let weights = array<f32, 7>(0.5, 0.3, 0.15, 0.03, 0.015, 0.003, 0.002);
    
    for (var level = 0; level < 7; level++) {
        let sigma = f32(1 << level);  // 1, 2, 4, 8, 16, 32, 64
        let blurred = gaussian_blur(luminance, coords, sigma, dims);
        height += weights[level] * blurred;
    }
    
    // Contrast enhancement
    height = enhance_contrast(height, 1.5);
    
    textureStore(output_texture, coords, vec4<f32>(height, 0.0, 0.0, 1.0));
}
```

### Parâmetros (MVP Defaults)

| Parâmetro | Valor | Descrição |
|-----------|-------|-----------|
| Blur levels | 7 | Número de níveis de blur |
| Max sigma | 64.0 | Blur mais amplo |
| Contrast | 1.5 | Fator de contraste |

## 2. Normal Map Generation

### Objetivo

Calcular vetores normais da superfície a partir do gradiente do height map.

### Teoria

Uma normal é perpendicular à superfície. Calculamos via:

```
normal = normalize(cross(dY, dX))
```

Onde:
- `dX` = vetor tangente na direção X (1, 0, ∂height/∂x)
- `dY` = vetor tangente na direção Y (0, 1, ∂height/∂y)

### Operador Sobel

Calcula gradientes usando kernels 3x3:

**Sobel X (horizontal):**
```
[-1, 0, +1]
[-2, 0, +2]
[-1, 0, +1]
```

**Sobel Y (vertical):**
```
[-1, -2, -1]
[ 0,  0,  0]
[+1, +2, +1]
```

### Pseudocódigo WGSL

```wgsl
fn sample_height(coords: vec2<i32>) -> f32 {
    return textureLoad(height_texture, coords, 0).r;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let coords = vec2<i32>(gid.xy);
    let dims = textureDimensions(height_texture);
    
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }
    
    // Sobel operator
    let gx = sample_height(coords + vec2<i32>(-1, -1)) * -1.0
           + sample_height(coords + vec2<i32>(-1,  0)) * -2.0
           + sample_height(coords + vec2<i32>(-1,  1)) * -1.0
           + sample_height(coords + vec2<i32>( 1, -1)) *  1.0
           + sample_height(coords + vec2<i32>( 1,  0)) *  2.0
           + sample_height(coords + vec2<i32>( 1,  1)) *  1.0;
           
    let gy = sample_height(coords + vec2<i32>(-1, -1)) * -1.0
           + sample_height(coords + vec2<i32>( 0, -1)) * -2.0
           + sample_height(coords + vec2<i32>( 1, -1)) * -1.0
           + sample_height(coords + vec2<i32>(-1,  1)) *  1.0
           + sample_height(coords + vec2<i32>( 0,  1)) *  2.0
           + sample_height(coords + vec2<i32>( 1,  1)) *  1.0;
    
    // Normal vector (pointing up, gradient down)
    var normal = vec3<f32>(-gx, -gy, 1.0);
    normal = normalize(normal);
    
    // Encode to [0,1] for storage
    let encoded = normal * 0.5 + 0.5;
    
    textureStore(output_texture, coords, vec4<f32>(encoded, 1.0));
}
```

### Parâmetros

| Parâmetro | Valor | Descrição |
|-----------|-------|-----------|
| Intensity | 1.0 | Escala dos gradientes (multiplica gx, gy) |
| Flip Y | false | Inverte eixo Y da normal |

**Intensidade > 1.0:** Normais mais "fortes", superfície parece mais rugosa
**Intensidade < 1.0:** Normais mais "fracas", superfície mais plana

### Formatos de Normal Map

#### DirectX (Minecraft, Unity)
- Y-up: `normal.y = -normal.y` (ou não flip)
- Canais: RGB

#### OpenGL (Blender, Godot)
- Y-up: `normal.y = normal.y` (flip em relação ao DirectX)
- Canais: RGB

O MVP usa formato DirectX (Y down na textura).

## 3. Metallic Map Generation

### Objetivo

Detectar áreas metálicas na imagem difusa e gerar uma máscara em escala de cinza.

### Heurísticas de Metal

Metais puros (ouro, prata, cobre, ferro) têm características distintas:

1. **Saturação baixa:** Metais são cinzentos (exceto ouro/cobre)
2. **Luminância alta:** Metais refletem bem
3. **Matiz específico:** Ouro é amarelo, cobre é laranja

### Espaço HSL

Converte RGB para Hue/Saturation/Luminance para análise:

```wgsl
fn rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
    let max_val = max(max(rgb.r, rgb.g), rgb.b);
    let min_val = min(min(rgb.r, rgb.g), rgb.b);
    let delta = max_val - min_val;
    
    // Luminance
    let l = (max_val + min_val) * 0.5;
    
    // Saturation
    var s = 0.0;
    if (delta > 0.0) {
        s = delta / (1.0 - abs(2.0 * l - 1.0));
    }
    
    // Hue
    var h = 0.0;
    if (delta > 0.0) {
        if (max_val == rgb.r) {
            h = (rgb.g - rgb.b) / delta;
        } else if (max_val == rgb.g) {
            h = 2.0 + (rgb.b - rgb.r) / delta;
        } else {
            h = 4.0 + (rgb.r - rgb.g) / delta;
        }
        h = h / 6.0;
        if (h < 0.0) { h += 1.0; }
    }
    
    return vec3<f32>(h, s, l);
}
```

### Algoritmo de Detecção

```wgsl
fn detect_metallic(rgb: vec3<f32>) -> f32 {
    let hsl = rgb_to_hsl(rgb);
    let h = hsl.x;  // Hue [0,1]
    let s = hsl.y;  // Saturation [0,1]
    let l = hsl.z;  // Luminance [0,1]
    
    // Heurística: metal = baixa saturação + alta luminância
    // ou matiz específico (ouro/cobre)
    
    var metallic = 0.0;
    
    // Metais cinzentos (prata, aço, alumínio)
    if (s < 0.15 && l > 0.4) {
        metallic = smoothstep(0.4, 0.8, l) * (1.0 - s * 6.0);
    }
    
    // Ouro (matiz ~0.08-0.15)
    if (h > 0.08 && h < 0.15 && s > 0.3 && l > 0.3) {
        metallic = smoothstep(0.3, 0.6, l) * smoothstep(0.3, 0.7, s);
    }
    
    // Cobre (matiz ~0.02-0.08)
    if (h > 0.02 && h < 0.08 && s > 0.4 && l > 0.25) {
        metallic = smoothstep(0.25, 0.5, l) * smoothstep(0.4, 0.8, s);
    }
    
    return saturate(metallic);
}
```

### Pseudocódigo WGSL Completo

```wgsl
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let coords = vec2<i32>(gid.xy);
    let dims = textureDimensions(input_texture);
    
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }
    
    let color = textureLoad(input_texture, coords, 0).rgb;
    let metallic = detect_metallic(color);
    
    textureStore(output_texture, coords, vec4<f32>(metallic, 0.0, 0.0, 1.0));
}
```

### Parâmetros

| Parâmetro | Valor | Descrição |
|-----------|-------|-----------|
| Saturation threshold | 0.15 | Max saturação para metais cinzentos |
| Luminance threshold | 0.4 | Min luminância para metais cinzentos |
| Gold hue range | 0.08-0.15 | Range de matiz para ouro |
| Copper hue range | 0.02-0.08 | Range de matiz para cobre |

### Limitações

**Algoritmo atual:**
- Funciona bem para materiais puros
- Pode falhar para:
  - Metais sujos/oxidadas
  - Metais pintados
  - Materiais misturados

**Melhorias futuras:**
- Machine learning (classificação por ML)
- Amostras de cor definidas pelo usuário
- Análise de contexto (vizinhança)

## Otimizações

### Performance

1. **Separable convolution:** Gaussian blur em 2 passos (H + V) em vez de kernel 2D
2. **Shared memory:** Carregar blocos para `workgroup` memory para acessos coalesced
3. **Texture cache:** Reutilizar texturas intermediárias
4. **Early exit:** Workgroups fora dos bounds retornam imediatamente

### Precisão

1. **R32Float para height:** Preservar precisão intermediária
2. **RGBA8Unorm para normal:** Suficiente para visualização
3. **R8Unorm para metallic:** Um canal, valores 0-1

## Referências

- [Sobel Operator](https://en.wikipedia.org/wiki/Sobel_operator)
- [Gaussian Blur](https://en.wikipedia.org/wiki/Gaussian_blur)
- [HSL Color Space](https://en.wikipedia.org/wiki/HSL_and_HSV)
- [Physically Based Rendering](https://pbr-book.org/)
- Materialize original shaders (HLSL/CG)
