---
tags: [neoveg, arquitetura, rust]
crate: neoveg_video
---
# neoveg_video

Vídeo e gráficos. Depende só de [[neoveg_common]].

## Responsabilidades
- Quadros RGBA, geradores de mídia (barras SMPTE, cor sólida, padrão de teste, títulos).
- Envio de texturas para `wgpu`.
- Decodificação FFmpeg via `ffmpeg-next`, atrás da feature opcional `ffmpeg` (encaminhada como `neoveg_ui/ffmpeg`), com um worker de quadros que processa só a requisição mais nova durante o scrub.

## Regras
- Sem a feature `ffmpeg` o aplicativo abre e avisa na barra de status que a decodificação de vídeo está desligada.
- FFmpeg 7.1.x compartilhado (LGPL), ligado dinamicamente.

## Ligações
Usada por [[neoveg_ui]] (painel Visualização e Scopes, ver [[especificacao_janelas]]). Voltar ao [[Index]].
