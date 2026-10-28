# Auditoria da interface contra os manuais (AGENTS.md seção 33)

Fonte única de UI/UX/funcionalidades: os PDFs em `manuais/` (reserva: `G:\Meu Drive\vp-rev-eng\teste\teste2\manuais`).
Este documento lista o que, em `crates/neoveg_ui`, **não tem respaldo localizado nos manuais** ou é decisão própria do Neoveg.
Método: busca de termos nos textos extraídos dos manuais (v8, v10, v13, v14, v18, v21) e revisão do que foi implementado.
Não é uma leitura página a página dos PDFs; itens marcados "a confirmar" precisam de conferência manual antes de qualquer remoção.

## Respaldo encontrado

| Recurso | Evidência nos manuais |
| :--- | :--- |
| Loudness (LUFS, LRA, true peak) | centenas de ocorrências (v10 a v21) |
| Project Notes | v18 e v21 |
| Slideshow / apresentação de slides | todas as versões |
| Esquemas de cores / temas de interface | todas as versões (`color scheme`) |
| Tela inicial / boas-vindas | v8, v10, v21 |

## Decisões próprias do Neoveg (sem respaldo localizado)

| Item | Onde | Situação |
| :--- | :--- | :--- |
| Temas **Charcoal Lilac** e **High Contrast** | `neoveg_common/src/theme.rs` | Os manuais citam esquemas de cores e uma opção de alto contraste, mas não estas paletas. São escolhas visuais do projeto. Mantidos como temas opcionais; **a confirmar**. |
| Ordenação de imagens do slideshow (opções além das do manual) | `neoveg_engine/src/slideshow.rs` | A conferir contra o diálogo descrito nos manuais. |
| Layout do dock de áudio (`audio_dock`) | `neoveg_ui/src/app.rs` | Organização própria dos painéis. |
| Janelas destacadas em viewports nativos (seção 32) | `neoveg_ui/src/app/detached.rs` | Pedido explícito do projeto, não decorre dos manuais. |
| Cartão da tela de abertura (splash) com dados da máquina | `neoveg_ui/src/splash.rs` | Pedido explícito do projeto (seção 30). |
| Ícones coloridos opcionais em BMP (`assets/icons_color/`) | `neoveg_ui/src/icons.rs` | Pedido explícito do projeto. Vêm do conjunto Flat Color Icons (MIT), via Iconify. Os sites Icons8, FlatIcon, IconScout, Streamline e UXWing foram **avaliados e não usados diretamente**: as licenças deles exigem atribuição, cobram ou restringem a redistribuição dos arquivos em repositório público. |
| Efeitos de vídeo: Nitidez com intensidade fixa, Máscara de nitidez com raio fixo (0,6 % da altura) e escolha dos canais do desfoque gaussiano ainda sem controle | `neoveg_video/src/vfx.rs` | Os manuais descrevem o desfoque (alcance horizontal e vertical), a nitidez (sem parâmetros), a máscara de nitidez (proporção) e a vinheta (raios, aspecto, centro). O raio da máscara, a faixa dos valores, a unidade em fração da altura do quadro e a ordem do sinal são decisões do projeto; **a confirmar** com os manuais. |

## Regra aplicada daqui em diante

Funcionalidades novas só entram quando descritas nos manuais; o que for decisão própria do projeto deve ser registrado aqui.
Nenhum recurso foi removido nesta auditoria: não há, no que foi revisado, nada que contradiga os manuais, apenas itens a confirmar.
