---
tags: [neoveg, referencia, ui, atalhos]
---
# Validação dos atalhos de painel nos manuais

Hub: [[fontes_de_referencia_de_ui]]. Tabela completa do Neoveg: [[atalhos_teclado]].

Fonte: lista de atalhos da versão 18 do editor de referência (15 páginas, páginas 3 e 4 para exibição e janelas) e a entrada 17 da pesquisa (`docs/research/nle_features_research.txt`). A varredura não traz atalhos padrão (o mapa vive dentro do executável), então os manuais são a única fonte aqui.

## Exibição e janelas
| Função (manual) | Atalho (manual) | Neoveg |
| :--- | :--- | :--- |
| Mostrar ou ocultar trilhas de barramento de áudio | `B` | `sem plano` ainda |
| Mostrar ou ocultar trilhas de barramento de vídeo | `Ctrl+Shift+B` | `sem plano` ainda |
| Marcadores de mídia no evento | `Ctrl+Shift+K` | `sem plano` ainda |
| Informação do take ativo | `Ctrl+Shift+I` | `sem plano` ainda |
| Formas de onda nos eventos | `Ctrl+Shift+W` | `sem plano` ainda |
| Botões do evento | `Ctrl+Shift+C` | `sem plano` ainda |
| Cabeçalho do evento | `Ctrl+Alt+M` | **conflito**: no Neoveg é "Mudo do evento" (atalho próprio) |
| Comprimento do fade entre eventos | `Ctrl+Shift+T` | `sem plano` ainda |
| Chamar layout de janela 0 a 9 | `Alt+D` e depois o número | `previsto` (hoje por menu) |
| Salvar layout | `Ctrl+Alt+D` e depois o número | `previsto` |
| Layouts de tarefa (padrão, mixagem de áudio, correção de cor) | `Alt+D` e depois `D`, `A` ou `C` | `parcial`: padrão e áudio por menu |
| Painel "dashboard" | `Ctrl+Alt+8` | o Neoveg não tem o painel e usou `Ctrl+Alt+8` para **Efeitos de vídeo** (atalho próprio) |
| Alternar foco entre lista de trilhas e linha do tempo | `Tab` | `sem plano` ainda |
| Trilha anterior e próxima | `Alt+Shift+Cima` e `Alt+Shift+Baixo` | `feito` |

## Zoom e altura de trilha
| Função (manual) | Atalho (manual) | Neoveg |
| :--- | :--- | :--- |
| Zoom horizontal em passos pequenos | `Cima` e `Baixo` | `sem plano` ainda (hoje roda do mouse e botões) |
| Zoom horizontal em passos grandes ou até a seleção | `Ctrl+Cima` e `Ctrl+Baixo` | `sem plano` ainda |
| Zoom até um quadro por miniatura | `Alt+Cima` | `sem plano` ainda |
| Diminuir ou aumentar a altura de todas as trilhas | `Ctrl+Shift+Baixo` e `Ctrl+Shift+Cima` | `sem plano` ainda |
| Minimizar ou restaurar altura | `` ` `` (acento grave) | `sem plano` ainda |
| Altura padrão das trilhas | `Ctrl+` e `` ` `` | `sem plano` ainda |
| Minimizar ou restaurar a área de janelas acopladas | `F11` ou `Alt+` e acento grave | o Neoveg usa `F11` para tela cheia de uma janela destacada |
| Maximizar a linha do tempo (esconde as janelas e a lista de trilhas) | `Ctrl+F11` ou `Ctrl+Alt+` e acento grave | `sem plano` ainda |
| Minimizar ou restaurar a lista de trilhas | `Shift+F11` ou `Shift+Alt+` e acento grave | `sem plano` ainda |

## Observações
- Dois atalhos próprios do Neoveg colidem com a referência: `Ctrl+Alt+M` e `Ctrl+Alt+8`. Os dois ficam, porque o AGENTS.md §4.5 já declara os interruptores de evento como atalhos próprios; qualquer usuário pode trocar em **Editar → Personalizar teclado**.
- Qualquer atalho novo deve entrar na tabela do mapa de teclas (`keymap` em `preferences.json`) antes de ser usado (AGENTS.md §4.5).
