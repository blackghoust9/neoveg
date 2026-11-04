---
tags: [neoveg, referencia, ui, medidas]
---
# Medidas da interface encontradas nas três fontes

Hub: [[fontes_de_referencia_de_ui]]. Só números e descrições (AGENTS.md §28): nenhuma imagem foi copiada.

## 1. Imagens extraídas (`results_assets/`)
384 arquivos: 368 PNG e 16 BMP, todos nomeados pelo número do recurso do executável.

| Tamanho (px) | Quantidade | Leitura |
| :--- | :--- | :--- |
| 16×16 | 28 | célula de ícone a 100 % de escala |
| 20×20 | 20 | a mesma célula a 125 % |
| 32×32 | 26 | a 200 % |
| 64×64 | 26 | a 400 % |
| 40×20, 80×40, 160×80 | 18 cada | o dobro da largura da célula, nas escalas 1, 2 e 4 (provavelmente dois estados lado a lado; uso não confirmado) |
| 504×49, 616×36, 448×32, 224×16, 336×16 | 20, 4, 4, 4, 3 | tiras de vários ícones numa só imagem |
| 100×60, 200×120, 400×240 | 5, 4, 4 | imagens de 100×60 e as versões 2× e 4× (uso não identificado) |

Conclusão: a referência desenha o ícone de barra de ferramentas numa célula de **16 px** e a escala por inteiros e 1,25 para telas de alta densidade (20, 32, 64).

## 2. Bitmaps de recurso na varredura (`VegasScan/01_Interface`)
Há 20 recursos na versão antiga e 25 na recente. Os que importam para o desenho: uma tira de 336×16 (21 células de 16 px), tiras de 280×18 e 18×280 (horizontal e vertical), bitmaps de 17×23 e 19×23, imagens de 256×256 e de 100×60 (e 200×120 e 400×240 nas telas densas). A varredura lista o tamanho, mas não diz para que cada um serve.

## 3. Diálogos (`VegasScan/04_Windows`), em unidades de diálogo
Fonte de 8 pt. Em 96 DPI uma unidade vale cerca de 1,5 px na horizontal e 1,6 px na vertical.

| Diálogo | Tamanho (unidades) | Aproximado em px | Controles |
| :--- | :--- | :--- | :--- |
| Propriedades do evento de áudio | 242 × 231 | 363 × 375 | 33 |
| Propriedades da mídia | 222 × 257 | 333 × 418 | 34 |
| CD de áudio | 270 × 71 | 405 × 115 | 5 |
| Transições de áudio | 171 × 44 | 257 × 72 | 2 |

Só estes quatro têm título; os outros 250 e poucos modelos da varredura não têm nome, então não dá para ligá-los a uma janela. Os controles que se repetem são campos numéricos com setas (classe de "spinner") e caixas de seleção com atalho sublinhado.

## 4. O que o Neoveg faz hoje
| Item | Neoveg | Resultado |
| :--- | :--- | :--- |
| Botão de barra de ferramentas | 26 px com ícone encolhido em 18 % de cada lado, ou seja, glifo de cerca de 16,6 px | **bate** com a célula de 16 px; nada a mudar |
| Ícone em SVG | desenhado em 24×24 e escalado | escala sem perda para 20, 32 e 64 px |
| Escalas de tela | o egui usa pixels por ponto | cobre 125 %, 200 % e 400 % sem arquivos extras |
| Diálogo de propriedades do evento | janela redimensionável, sem tamanho fixo | a referência é fixa em 363 × 375 px; manter redimensionável |

## 5. O que a varredura não responde
Altura da régua, largura do cabeçalho de faixa, margens entre painéis, tamanho do evento e cores exatas. O README da varredura confirma que essas medidas exigem inspeção manual do programa aberto. Sem fonte, o Neoveg mantém suas próprias decisões (registradas em `docs/research/ui_audit.md`).
