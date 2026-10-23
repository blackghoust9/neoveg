# Neoveg 1.0.0

## Estado por plataforma

| Plataforma | Estado | Motor nativo (Rust `neoveg_core` + C++ `neoveg_cpp_engine`) |
| --- | --- | --- |
| **Windows x64** (`win-x64`) | **Funcional** | Binários nativos compilados e carregados (`neoveg_core.dll`, `neoveg_cpp_engine.dll`). |
| Linux x64 (`linux-x64`) | **Experimental** | Ainda sem binário nativo real: o pacote roda no **Fallback C# Engine** (motor gerenciado de contingência). |
| macOS Intel / Apple Silicon (`osx-x64`, `osx-arm64`) | **Experimental** | Ainda sem binário nativo real: o pacote roda no **Fallback C# Engine**. |

Os pacotes portáteis de Linux e macOS gerados até aqui continham bibliotecas nativas de placeholder; eles não devem ser distribuídos como estáveis. O binário nativo dessas plataformas precisa ser compilado em ambiente nativo ou no pipeline Docker (`Dockerfile`, estágio `native-builder`) antes de uma release estável.

## Funcionalidades (Windows x64)

- Timeline multifaixa com régua SMPTE (drop-frame / non-drop-frame), grade de frames adaptativa ao zoom e playhead de 1 px.
- Forma de onda de áudio exata (motor Rust `waveform_source`): mínimo/máximo reais por coluna de pixel e amostras individuais no zoom máximo.
- Painéis acopláveis e destacáveis (Timeline, Video Preview, Trimmer, Master Bus, Project Media), abas compactas no rodapé.
- Master Bus com medidores ligados à telemetria real de áudio (sem animação simulada).
- Event Pan/Crop em janela própria, com prévia lateral opcional.
- Atalhos de teclado de edição profissional (por exemplo `S` dividir, `Ctrl+Q` faixa de áudio, `Ctrl+Shift+Q` faixa de vídeo, `F8` snapping).
- Sistema centralizado de logs (`logs/neoveg.log`, `logs/crash.log`) e AppCache local para execução a partir de unidades sincronizadas.

## Limitações conhecidas

- A prévia ainda não decodifica vídeo (clipes de vídeo aparecem como gráfico sintético) e a reprodução não tem saída de áudio.
- Efeitos e integrações listados no catálogo (por exemplo LUTs, CorridorKey, Whisper.cpp, LADSPA) ainda são, em grande parte, representações visuais ou pontes sem processamento completo.
- O resultado de QA válido é o gerado por `scripts/run_qa_suite.ps1` / `.sh` em `coverage/qa/qa_report.json`.

## Distribuição

- Windows: publicação .NET 8 Release self-contained em `dist/Neoveg-Portable-win-x64/`, iniciada por `Executar_Neoveg.cmd`.
- Inclui `AUTHORS.md`, `LICENSE` e os avisos de licença de terceiros (`third_party/README.md`).
