---
tags: [neoveg, referencia, video, efeitos]
---
# Catálogo de efeitos de vídeo

Funções de efeito de vídeo que o editor de referência traz, agrupadas por finalidade. A versão antiga tem cerca de 65 efeitos; a recente passa de 90 e acrescenta o grupo de IA e correções de lente. Hub: [[segundo_cerebro]]. Estado no Neoveg: [[neoveg_video]], [[lacunas_funcionais]].

## Cor e tom
Brilho e contraste, níveis, balanço de cor, corretor primário e secundário, curvas, ajuste HSL e LAB, saturação, balanço de branco, restaurar pretos, preto e branco, sépia, inverter, limiar, mapa de gradiente, contraste suave, luz de preenchimento, cores de transmissão, correspondência de cor, filtro de LUT, looks automáticos, gradação de cor.
- Neoveg: `feito` correção primária (exposição, temperatura, matiz, lift, contraste, gama, saturação) e LUT `.cube` 1D e 3D do projeto. `previsto`: curvas, secundário, correspondência de cor, OCIO.

## Desfoque, nitidez e ruído
Desfoque gaussiano, linear, radial e rápido, desfoque de croma, desfoque de lente, desfoque de movimento, nitidez, máscara de nitidez, mediana, mínimo e máximo, núcleo de convolução, redução de ruído, controle de cintilação.
- Neoveg: `sem plano` ainda; depende da cadeia de efeitos por evento na GPU.

## Distorção e geometria
Deformação, malha de distorção, esfera, pinça e protrusão, redemoinho, onda, espelho, pixelização e pixelização radial, deslocar e repetir, recorte, imagem dentro de imagem, pequeno planeta, correção de lente, dimensionalidade de camada, recorte em forma.
- Neoveg: `previsto` recorte e Pan/Crop do evento; o resto `sem plano`.

## Luz e estilo
Brilho (glow), cintilação, reflexo de lente, raios de luz, raios, estrela, vinheta, efeitos de filme, grão de filme, adicionar ruído, jornal, simulador de TV, borda, mapas de relevo, deslocamento e altura.
- Neoveg: `previsto` junto com os efeitos na GPU.

## Chave e máscara
Chave de croma, gerador de máscara, máscara Bézier, preenchimento de barras pretas.
- Neoveg: `feito` chave de croma em Rust puro (YCbCr Rec. 709). `previsto`: máscaras (retângulo, elipse, Bézier) e máscara de efeito.

## Análise, movimento e utilitários
Histograma, código de tempo na imagem, rastreador de movimento, detecção de cena, estabilização (inclusive 360°), câmera lenta, ajuste estereoscópico 3D, rotação de cena 360° e costura de dupla olho de peixe.
- Neoveg: `feito` escopos de vídeo (forma de onda, vectorscópio, histograma, parada RGB). `previsto`: rastreamento de movimento.

## IA (só na versão recente)
Reenquadramento automático, colorização, remoção de névoa, mapa de profundidade, nitidez, máscara inteligente (duas gerações), suavização, transferência de estilo, ampliação de resolução.
- Neoveg: `previsto` transcrição, máscara e ampliação (fase de IA do roadmap).

## Próximos passos
1. Cadeia de efeitos por evento, faixa, mídia e projeto na GPU (`wgpu`), base de todos os grupos acima.
2. Começar pelos efeitos de maior valor: desfoques, nitidez, curvas de cor, vinheta e recorte.
