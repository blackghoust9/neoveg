# Neoveg 2.1.0

Editor não linear de vídeo e áudio escrito em Rust puro. O que está pronto e o que falta, feature por feature, fica no roteiro do `README.md`.

## Estado por plataforma

| Plataforma | Estado |
| --- | --- |
| **Windows x64** (`win-x64`, Windows 10 e 11) | **Pacote portátil e executável avulso.** É a plataforma de desenvolvimento e de teste. |
| Linux x64 (`linux-x64`) | Ainda não gerado nem testado. O porte está planejado; o ambiente Docker existe (`docker/`), mas está pausado. |
| macOS Intel e Apple Silicon (`osx-x64`, `osx-arm64`) | Ainda não gerado nem testado (precisa de um Mac ou do SDK da Apple). |

## O que o programa faz hoje

- Linha do tempo multifaixa com régua SMPTE (com e sem drop-frame), grade de quadros que acompanha o zoom, snapping, ripple, seleção múltipla, grupos, marcadores e regiões.
- Edição não destrutiva com desfazer e refazer, projetos em abas, sub-projetos aninhados e autosave com recuperação.
- Painéis acopláveis, destacáveis em janelas próprias e com abas no rodapé; layouts salvos em disco.
- Áudio: forma de onda amostra a amostra, mixer com barramentos e sends, cadeia de efeitos por faixa e no master, plug-ins VST3 e CLAP, medidores de pico e de loudness (EBU R128).
- Vídeo: composição em várias faixas, efeitos e transições (CPU e GPU), Pan/Crop e movimento de faixa com quadros-chave, envelope de velocidade, correção de cor, LUTs e chaveamento de croma.
- Importação e exportação de AAF, XML do Final Cut Pro 7, FCPXML, OpenTimelineIO, EDL e legendas (SRT e WebVTT); transcrição de fala para legendas com Whisper.
- Renderização com codificador H.264 + AAC interno (sem FFmpeg) ou com o FFmpeg instalado; relatório de loudness; legendas queimadas no vídeo.
- Interface em português do Brasil, inglês, alemão, espanhol, francês, russo, japonês e chinês.
- Logs em `logs/neoveg.log` e `logs/crash.log`, ao lado do executável.

## Limitações conhecidas

- A decodificação de vídeo precisa das bibliotecas do FFmpeg. O pacote só as traz quando foi gerado com `--ffmpeg`.
- ProRes e VP9 dependem de um FFmpeg que tenha esses codificadores; o FFmpeg LGPL do `winget` não tem.
- Japonês e chinês usam a fonte do sistema e as traduções ainda não foram revisadas por falantes nativos.
- Scripting para o usuário ainda não existe (item em decisão no roteiro).
- Linux e macOS não foram testados.

## Distribuição

- `Neoveg_v<versão>_win-x64_Portable.zip`: pasta portátil com o executável, as tabelas de cor (`assets/luts/`), as licenças e, quando gerado com FFmpeg, as DLLs dele. Basta extrair e abrir `neoveg.exe`.
- `neoveg_v<versão>_win-x64.exe`: executável avulso, construído com o runtime C estático (não precisa do Visual C++ Redistributable).
- `SHA256SUMS.txt` e `build_manifest.json` com os tamanhos e as somas de verificação.
- As licenças de terceiros ficam em `AUTHORS.md`, `third_party/` (inclusive as das fontes embutidas e a do FFmpeg, quando presente) e `LICENSE`, `LICENSE-MIT` e `LICENSE-APACHE`.
