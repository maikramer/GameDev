# Inference Server

Serviço HTTP (FastAPI) para enfileirar inferência nos pacotes **Text2D**, **Text3D**, **Skymap2D** e **Texture2D** numa máquina com GPU (ex.: servidor no porão). O cliente envia parâmetros em JSON e descarrega os artefactos (PNG, GLB, etc.).

## Instalação (servidor)

Na raiz do monorepo ou dentro de `InferenceServer/`:

```bash
pip install -e ".[pipelines]"
```

O extra `pipelines` instala as dependências locais (`text2d`, `text3d`, `skymap2d`, `texture2d`). Sem ele, o servidor arranca mas os jobs falham ao importar os geradores.

## Arranque

```bash
# Variáveis (opcional)
set INFERENCE_SERVER_API_KEY=uma-chave-secreta
set INFERENCE_SERVER_HOST=0.0.0.0
set INFERENCE_SERVER_PORT=8765
set INFERENCE_SERVER_DATA_DIR=%USERPROFILE%\.cache\inference_server

inference-server
# ou: python -m inference_server
```

- **`INFERENCE_SERVER_API_KEY`**: se definida, todos os endpoints (exceto `GET /health`) exigem cabeçalho `Authorization: Bearer <chave>`. Em LAN confiável ainda assim é recomendado definir uma chave.
- **`INFERENCE_SERVER_DATA_DIR`**: SQLite (`jobs.sqlite`) e pasta `artifacts/<job_id>/`.
- **`INFERENCE_SERVER_JOB_TTL_SECONDS`**: após conclusão, jobs e pastas antigas são removidos (defeito: 7 dias).
- **`INFERENCE_SERVER_CLEANUP_INTERVAL_SECONDS`**: intervalo do garbage collector (defeito: 3600).

## Rede e segurança

1. **LAN**: escuta em `0.0.0.0` só na rede em que confias; restringe no router/firewall a clientes conhecidos.
2. **Windows Firewall**: permite entrada TCP na porta escolhida (ex.: 8765) apenas para a sub-rede local.
3. **Fora de casa**: não exponhas o serviço diretamente à Internet; usa VPN (Tailscale, WireGuard) e mantém a API key.
4. **`GET /health`** não usa autenticação (útil para probes); o resto exige Bearer se a chave estiver configurada.

## API (resumo)

| Método | Caminho | Descrição |
|--------|---------|-----------|
| GET | `/health` | Estado do processo |
| GET | `/version` | Versão e pacotes opcionais importáveis |
| POST | `/jobs` | Corpo `{"type":"text2d"\|"text3d"\|"skymap2d"\|"texture2d","params":{...}}` |
| GET | `/jobs/{id}` | Estado: `queued`, `running`, `succeeded`, `failed` |
| GET | `/jobs/{id}/artifacts` | Lista de ficheiros no job |
| GET | `/jobs/{id}/download/{filename}` | Download (só após `succeeded`) |

## Cliente CLI

```bash
set INFERENCE_CLIENT_BASE_URL=http://192.168.1.50:8765
set INFERENCE_CLIENT_API_KEY=uma-chave-secreta

inference-client run text2d "um gato estilizado" -o ./out
inference-client run text3d --prompt "robô" --preset hq -o ./meshes
inference-client run text3d --from-image ref.png -o ./meshes
inference-client run skymap2d "céu ao pôr do sol" -o ./sky
inference-client run texture2d "pedra musgosa" -o ./tex

inference-client submit job.json
inference-client status <job_id>
inference-client fetch <job_id> -o ./out
```

`job.json` deve ter o formato `{"type":"text2d","params":{...}}` (o mesmo que `POST /jobs`).

## Variáveis Hugging Face

No servidor, configura como nos pacotes individuais: `HF_TOKEN`, `HF_HOME`, `TEXT2D_MODEL_ID`, etc.
