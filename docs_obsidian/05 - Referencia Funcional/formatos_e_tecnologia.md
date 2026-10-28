---
tags: [neoveg, referencia, formatos, codecs, tecnologia]
---
# Formatos, codecs e tecnologia do editor de referência

Hub: [[segundo_cerebro]]. Implementação: [[neoveg_video]], [[neoveg_audio]], [[neoveg_engine]].

## Entrada e saída de arquivos (um módulo por formato)
Cerca de 30 módulos de leitura e escrita. Agrupados:
- **Áudio:** WAV, AIFF, FLAC, MP3, OGG, AC-3 (estéreo e surround), ATRAC, AAC (versão recente), Broadcast Wave e arquivos de pico.
- **Vídeo de consumo:** AVI, MP4, MOV (QuickTime), GIF, Windows Media, imagens estáticas.
- **Câmeras e produção:** MXF (vários perfis, incluindo XAVC, HDCAM SR, P2), AVC-Intra, HEVC, RAW de câmera de cinema (duas famílias), vídeo estereoscópico (MVC).
- **Legendas e texto:** SCC (closed caption), STL e variantes.
- **Contêiner composto:** um arquivo que embute vídeo e áudio em fluxos separados.

| Área | Neoveg |
| :--- | :--- |
| Áudio comum (WAV, FLAC, MP3, OGG, AAC) | `feito` via `symphonia` |
| Vídeo comum (MP4, MOV, WebM, AVI) | `feito` via FFmpeg opcional |
| Imagens estáticas e sequências | `feito` |
| Legendas SRT e WebVTT | `feito`; SCC e STL `sem plano` |
| MXF profissional, RAW de câmera, MVC | `sem plano` (dependem do FFmpeg e de licenças) |
| AAF | `previsto` |

## Aceleração e IA
- **Versão antiga:** aceleração por OpenCL (com cache de kernels) e codificação por hardware da Intel.
- **Versão recente:** acrescenta execução de modelos de IA por runtime ONNX com DirectML, biblioteca de visão computacional (OpenCV 4.9) para efeitos de IA e detecção de hardware de codificação (Intel, AMD, NVIDIA).
- Neoveg: `previsto` GPU por `wgpu` para efeitos; hoje a cor e o croma rodam na CPU. IA é a fase final do roadmap.

## Arquitetura (resumo)
- Núcleo nativo em C++ com interface Win32 própria e docking próprio.
- Camada gerenciada .NET para scripts do usuário (compila C#, VB e JavaScript de uma pasta de menu) e janelas novas.
- Versão recente: WPF e WebView2, banco de mídia em SQLite, serviço online com SignalR, autenticação por conta.
- Plug-ins: OFX 1.x para vídeo; VST2, VST3 e DirectX para áudio; servidores de varredura isolados em processo próprio.
- Neoveg: **tudo em Rust** (AGENTS.md §2.2); scripts previstos em Rust; OFX e VST3 `previstos` com processo isolado.

## Próximos passos
1. Decidir quais formatos profissionais valem a pena (AAF primeiro, por interoperabilidade).
2. Prototipar um host OFX mínimo em processo isolado.
3. Medir a GPU com o benchmark de reprodução antes de escolher a primeira família de efeitos ([[desempenho_resolucoes]]).
