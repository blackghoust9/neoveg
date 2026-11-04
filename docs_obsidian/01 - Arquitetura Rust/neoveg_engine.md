---
tags: [neoveg, arquitetura, rust]
crate: neoveg_engine
---
# neoveg_engine

O modelo de edição, testável sem janela nem placa de som. Depende só de [[neoveg_common]].

## Responsabilidades
- `Timeline`, faixas e eventos (`TrackEvent`), marcadores e regiões.
- `History::execute(EditCommand)`: cada gesto do usuário é exatamente um passo de desfazer; `History::revision()` indica alterações não salvas.
- Regras puras: geometria, snapping, trim, crossfade automático (`crossfade`), ripple, relink — ver [[regras_de_timeline]].
- Arquivos `.neoveg` (ZIP com `project.json`, `media_pool.json`, `history.json`), gravação atômica, autosave e recuperação.
- `ProjectManager`: várias abas e subprojetos aninhados — ver [[subprojetos_e_abas]].
- Interoperabilidade: EDL CMX 3600, FCP7 XML, FCPXML, OTIO; legendas SubRip e WebVTT.

## Regras
- Não pode depender de áudio, vídeo ou UI.
- Todo comportamento novo vem com teste unitário; formatos com teste de ida e volta, incluindo arquivos antigos.

## Ligações
Consumida por [[neoveg_ui]]. Voltar ao [[Index]].
