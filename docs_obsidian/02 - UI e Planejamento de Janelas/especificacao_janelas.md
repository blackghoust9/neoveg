---
tags: [neoveg, ui, janelas]
---
# Especificação das janelas

Complementa o mapa visual em `layout_janelas.canvas`. A fonte normativa é a seção 4 do `AGENTS.md`; o código está em [[neoveg_ui]].

## Regras comuns
- Os painéis primários (Timeline, Visualização, Trimmer / Mídia do Projeto, Barramento Master) podem virar janelas flutuantes; fechar uma janela flutuante a devolve ao seu lugar no dock.
- Abas agrupadas usam rodapé compacto: faixa de abas embaixo, fonte 11, altura 22 px, setas de rolagem só quando as abas não cabem, aba selecionada sempre visível.
- Janelas podem ser mostradas ou ocultadas por atalho (`Alt+0`, `Alt+2` … `Alt+5`, `Ctrl+Alt+2`, `Ctrl+Alt+7`, `Ctrl+Alt+9`); layouts 1–9 são salvos em **Exibir → Layouts** (apenas na sessão).
- A UI nunca bloqueia: o trabalho pesado roda em workers.

## Linha do Tempo (inclui Cabeçalhos de Faixa)
- Ocupa a parte inferior (≈48% da altura). Cabeçalhos e lanes compartilham uma única rolagem vertical; a rolagem horizontal é uma só (régua, eventos e playhead).
- Zoom de 15 a 480 000 px/s; a roda aplica zoom ancorado no ponteiro.
- O ponto no número da faixa pisca (branco/preto, 500 ms) só na faixa selecionada.
- Playhead: linha de exatamente 1 pixel físico; cabeça azul com caixa de timecode.
- Redimensionar: a divisória entre a Visualização e a Timeline é arrastável; a barra de ferramentas principal fica logo abaixo da timeline.

## Visualização (Video Preview)
- Toolbar compacta só com ícones. Um único seletor de qualidade (Draft = 1/4, Preview = 1/2, Good e Best = cheio, Auto = ajusta à janela).
- Sobreposições (HUD SMPTE, áreas seguras, grade, gizmo `F`, bypass lado a lado) começam desligadas.
- Tela cheia com `Esc` para sair; pop-out para monitor externo.

## Mídia do Projeto · Trimmer · Legendas
- Painel à esquerda (≈28% da largura), três abas de rodapé. O Trimmer edita a mídia antes de colocá-la na timeline.
- Abaixo dele fica **Efeitos de Áudio**, com a cadeia da faixa selecionada.

## Barramento Master (Audio Mixer)
- Mostra apenas medidores de pico L e R em dBFS (escala 0, −6, −12, −24, −48, −∞), leituras numéricas e o fader master. Nunca contém scopes de vídeo.
- Parado ou sem áudio ativo, os medidores ficam zerados e estáticos. Detalhes em [[neoveg_audio]].
- Logo abaixo ficam as abas **Loudness** (M, S, I, LRA, TP) e **Scopes de Vídeo**.

## Scopes
- Aba de **Scopes de Vídeo** ao lado de Loudness, no lado direito.

## Redimensionamento
- Proporções iniciais: Timeline 48% abaixo; Mídia 28% à esquerda; Master 18% à direita. Todas as divisórias são arrastáveis e o layout é restaurado em **Exibir → Layouts → padrão**.

Voltar ao [[Index]]. Atalhos: [[atalhos_teclado]].
