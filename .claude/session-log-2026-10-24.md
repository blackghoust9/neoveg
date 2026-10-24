# Log da sessão — 2026-10-24 (pesquisa dos manuais e roteiro)

Branch: `refactor/100-percent-rust`. Idioma de trabalho: pt-BR. Commits e push são feitos manualmente pelo usuário (repositório privado).

## 1. Pedido inicial
Resumir os trabalhos mais importantes já feitos, colocar o que falta no README com checkboxes, listar tudo que foi instalado para os contribuidores e usar os manuais VEGAS Pro 10/11/12/13/18/21 e `C:\Users\Paulo César\Documents\results.md` como referência.

## 2. Ações executadas (em ordem)
| # | Ação | Resultado |
| --- | --- | --- |
| 1 | Leitura de `README.md`, `REQUIREMENTS.md`, `AGENTS.md` e do índice do `results.md` (3 MB, seções 1–22 + apêndices) | Contexto levantado; README já tinha tabela de pré-requisitos |
| 2 | Verificação do ambiente (`rustc`, `cargo`, `rustup component list`, `cargo install --list`, `winget list`) | Rust 1.99.1, MSVC target, componentes clippy/rustfmt/llvm-tools/rust-src/rust-docs, `cargo-llvm-cov` 0.9.1, VS 2026 Build Tools, FFmpeg 7.1.5 LGPL shared, LLVM 19 |
| 3 | Tentativa de baixar manuais por `WebFetch` | VP12: HTTP 403; VP10/VP18: acima de 10 MB. Nada lido nessa etapa |
| 4 | Edição do `README.md` | Seção "Ambiente de referência" (instalado na máquina de dev), bloco "A fazer a partir da pesquisa dos manuais e do results.md" com checkboxes, bloco "Itens extras vindos dos manuais (VP10–VP21)"; corrigida a referência obsoleta à "seção 22" do AGENTS.md |
| 5 | Correção de um escape quebrado (`\b` virou backspace) no caminho `C:\Program Files\LLVM\bin` do README | Corrigido com `sed` |
| 6 | `git commit` do README | `d139f65` (a pedido do usuário) |
| 7 | `git push` (3 tentativas, uma pelo usuário) | Falhou: `Repository not found` em `https://github.com/blackghoust9/neoveg.git` (repositório privado/credencial). Usuário decidiu commitar e enviar manualmente |
| 8 | PDFs baixados pelo usuário em `manuais/` (6 arquivos, ~100 MB) | Extração com `pdftotext -layout`; versões compactadas em `scratchpad/c/` |
| 9 | `.gitignore` | Adicionado `manuais/` |
| 10 | Pesquisa nível 1 (sumários + "What's new") → entradas 7–16 em `docs/research/nle_features_research.txt` | 10 entradas |
| 11 | Pesquisa nível 2 (capítulos restantes) → entradas 17–25 no mesmo arquivo | 9 entradas; arquivo agora tem 25 entradas de recursos |

Commits feitos pelo usuário durante a sessão (visíveis em `git log`): `d139f65` (README) e `364ba6d` ("docs(roadmap): detalha itens dos manuais"). A pesquisa (`docs/research/nle_features_research.txt`) estava **sem commit** ao fim desta sessão.

## 3. O que foi lido dos manuais
Lido em detalhe (texto completo):
- **VP13**: Editing events on the timeline (cap. 6), markers/regions/commands (cap. 9), Track editing (cap. 10), Using automation (cap. 11), Applying effects (cap. 13), Compositing video (cap. 16), Previewing your project (cap. 25), Monitoring video with scopes (cap. 26), Rendering projects (cap. 30), Keyboard shortcuts (cap. 36), Multicamera editing (cap. 7), Mixing Console (cap. 15, até as bus strips), audio panning modes, loudness meters, buses/routing (cap. 14, parte), Working with projects (cap. 4: propriedades, aninhamento, salvar) e Adding/managing media (cap. 5: Project Media até a importação BWF).
- **VP11 (apostila pt-BR)**: linhas 1–971 de 1.274 (interface, Track Motion, Pan/Crop, máscaras, automação, modos de composição, textos/créditos, transições, início de Video FX).
- **VP12 Reviewer's Guide**: linhas 1–~400 de 522.
- **VP18**: "What's new" e o capítulo Motion tracking.
- **VP21**: "What's new", Adjustment events, AI Text to Speech, Speech to Text / Text-Based Editing, AI Colorization, AI Smart Mask 2.0, AI Upscale, início do AI Z-Depth.
- **results.md**: índice e seções 1, 15 (parcial) — usado como inventário; não foi lido de ponta a ponta (3 MB).

**Não lido (lacunas conhecidas):**
- **VP10** inteiro: só o sumário e a lista de capítulos (a maior parte repete o VP13).
- VP13: capítulos 1–3 (janela/docking), 8 (3D estereoscópico), 12 (animação: Track Motion/Pan-Crop em detalhe), 17–24 (proxy, HDV, XDCAM, RED, AVCHD, S-Log/ACES, 5.1, closed captions), 27–29, 31–35 (preferências em detalhe, hardware controllers, scripting, personalização), 37 (glossário); restante do cap. 5 (propriedades de mídia, gravação de áudio, captura) e 15 (strips de bus/FX/master).
- VP11 linhas 972–1274 (Video FX, Chroma Key, Audio FX, renderização).
- VP12 linhas ~400–522 (restante das novidades).
- VP18 e VP21: tudo que não está listado acima (por exemplo VP21 Motion Tracking, AI Z-Depth completo, Hub, Media Share).
- `results.md` §6–§14 (menus, diálogos, comandos) e §16–§22.

## 4. Entregáveis
- `README.md`: ambiente de referência + roteiro ampliado (commitado pelo usuário).
- `.gitignore`: ignora `manuais/`.
- `docs/research/nle_features_research.txt`: entradas 7–25 (seleção/cortar-colar, ripple/trim/expanded edit, snapping, envelopes/switches/takes/grupos, marcadores, Trimmer, faixas/buses/pan/loudness, automação, Track Motion/máscaras/modos, novidades v12/18/21, mapa de teclado, render, propriedades de projeto, FX/transições/geradores, composição, preview/scopes, Mixing Console, mídia/multicâmera, tracking e IA).
- Este log.

## 5. Decisões e observações para a próxima sessão
- A pesquisa usa o rótulo "Classic NLE" nas fontes, conforme a convenção do arquivo (AGENTS.md 9.1 permite citar o VEGAS por nome).
- Conflito a resolver antes de implementar slip: no Neoveg `Alt` ignora o snap ao arrastar (AGENTS.md 4.3); no manual `Alt` faz slip. Registrado na entrada 8.
- A tabela de atalhos (entrada 17) vem de um PDF com colunas misturadas pelo `pdftotext`; os pares comando→tecla de J/K/L devem ser conferidos no aplicativo.
- Sugestão de ordem do roadmap: seleção múltipla + split/cut/paste/ripple → marcadores e `M` → volume/pan/mute/solo por faixa → envelopes/keyframes → FX chains e composição na GPU → Trimmer (3/4 pontos) → buses/sends/loudness → render.
- Próximos passos possíveis: ler as lacunas acima; converter as entradas 7–25 em itens detalhados no README; atualizar a tabela de atalhos do AGENTS.md 4.5 conforme as features forem implementadas.
- `git push` pendente (precisa de credencial no GitHub Desktop ou URL nova do remoto).

---

# Adendo — segunda rodada (mesmo dia): entradas 7–16 em tarefas + leitura das lacunas

## A. Ações
| # | Ação | Resultado |
| --- | --- | --- |
| 12 | Conversão das entradas 7–16 em tarefas no `README.md` | Seção "Plano de implementação detalhado" com 10 fases (seleção, ripple/trim, snap, envelopes/propriedades, marcadores, Trimmer, faixas/mixer, automação, movimento/composição, fluxos de trabalho), cada tarefa em checkbox e ligada à entrada da pesquisa |
| 13 | Leitura do `results.md` §6 (menus), §7.1 (menus de contexto), §8.2/8.5/8.6/8.7 (diálogos de relink, evento/mídia, preferências, propriedades do projeto), §13 (catálogo de 782 comandos), §14 (atalhos), §12.19–12.20 | Índice compacto gerado em `scratchpad/c/` (menus.txt, ctx.txt); confirmado que o mapa de teclado padrão **não é recuperável** do binário |
| 14 | Leitura de capítulos do VP13: janela/docking (cap. 3), animação de eventos e faixas (cap. 12: Pan/Crop, máscaras, Track Motion, keyframes), gravação de áudio/metrônomo/proxy/propriedades de mídia (cap. 5), personalizar interface e formatos de régua (cap. 34), preferências General/Video/Preview/Editing (cap. 35), Edit Details e histórico de desfazer (cap. 6) | Base das entradas 29–34 |
| 15 | Leitura do VP10: introdução, requisitos, snapping (cap. 5) e capítulo 3 inteiro (Media Manager) | Entrada 35. O restante do VP10 repete o conteúdo do VP13 |
| 16 | Entradas 26–35 adicionadas a `docs/research/nle_features_research.txt` (arquivo agora com 35 entradas) | 26 menus, 27 menus de contexto, 28 catálogo de comandos e mapa de teclado, 29 preferências, 30 propriedades de projeto/evento/mídia e relink, 31 Pan/Crop+máscaras+keyframes, 32 espaço de trabalho, 33 gravação/metrônomo/proxy, 34 régua e drop-frame, 35 Media Manager |
| 17 | Fases 11 e 12 acrescentadas ao README (menus/comandos/preferências; propriedades/régua/espaço de trabalho) | README com 125 checkboxes pendentes |

## B. Lacunas que continuam sem leitura
- **VP10:** capítulos 1–2 (a partir do que não é introdução), 4, 5 (exceto snapping), 6–25 não foram lidos linha a linha. Verificado por amostragem que cobrem os mesmos assuntos do VP13.
- **VP13:** cap. 1–2 (introdução, janela principal em detalhe), 8 (3D estereoscópico), 14 e 15 (strips de bus/FX/master, partes finais), 17–24 (proxy-first, HDV, XDCAM, RED, AVCHD, S-Log/ACES, 5.1, legendas ocultas), 27–29 (timecode, gravação de discos, print to tape), 31–33 (Connect, controladores de hardware, scripting), 35 (abas Audio/Display/CD/Sync/External Control em prosa), 37 (glossário).
- **VP11:** linhas 972–1274 (Video FX, Chroma Key, Audio FX, renderização).
- **VP12:** linhas ~400–522.
- **VP18/VP21:** tudo que não está nos trechos já citados (por exemplo VP21 Motion Tracking, AI Z-Depth completo, Hub, Media Share, Auto Reframe, Assistant).
- **results.md:** §7.1 contextos 14–25, §7.2, §8.1/8.3/8.4/8.8/8.9 (listas de controles), §9–§12 além do que foi citado, §13.99 (IDs), §15–§22 e apêndices.

## C. Observações
- O `results.md` registra que as tabelas de atalho padrão estão compiladas no código; a entrada 17 vem do manual do VP13 e deve ser conferida no aplicativo.
- `git status` ao fim: `README.md`, `docs/research/nle_features_research.txt` modificados; `.claude/session-log-2026-10-24.md` novo. Nada commitado nesta rodada.

---

# Adendo 2 — terceira rodada: atalhos a partir do manual, fim do VP11, capítulos restantes do VP13

## A. Ações
| # | Ação | Resultado |
| --- | --- | --- |
| 18 | Reextração das páginas 639–653 do PDF do VP13 com `pdftotext -layout` e realinhamento comando→tecla pela ordem das duas colunas | A entrada 17 estava errada em vários pontos (por exemplo: reprodução, mute/solo, MIDI timecode, prerender/RAM preview, conversão corte↔transição, mapeamento das janelas `Alt+0`…`Alt+9`). Entrada 17 reescrita por inteiro |
| 19 | `AGENTS.md` §4.5: 30 linhas "planned" e uma nota de fonte e divergências aceitas (`←`/`→` = 1 quadro, `Alt` = ignorar snap) | Part I segue prevalecendo; atalhos novos devem constar em `keymap` |
| 20 | Leitura do VP11 até o fim (Video FX, Stereoscopic 3D, Chroma Key, plug-ins externos, Audio FX padrão, Pitch Shift, Reverb, renderização, YouTube, DVD, dicas) | Entrada 41 |
| 21 | Leitura do VP13: cap. 8 (3D estereoscópico), 17 (proxy-first), 18–21 (HDV, XDCAM, RED, AVCHD, só o essencial), 22 (S-Log/ACES), 23 (início do 5.1 e panning), 24 (closed captions), 27 (timecode MIDI), 28 (discos, início), 31 (Connect, início), 32 (controladores, início), 33 (scripting e linha de comando) | Entradas 36 a 40 |
| 22 | README: Fase 13 (atalhos, 9 tarefas por grupo) e Fase 14 (inventário de funcionalidades ausentes, 10 tarefas) | 145 checkboxes pendentes no total |

## B. Estado da pesquisa
`docs/research/nle_features_research.txt` tem 41 entradas. Lacunas restantes (leitura): VP13 caps. 1–2 e 14–15 (partes), 29 (print to tape), 35 (abas Audio/Display/CD/Sync/External Control em prosa), 37 (glossário) e o final dos caps. 23/28/31/32; VP12 (linhas ~400–522); VP18/VP21 fora dos trechos citados; `results.md` §7.2, §8.1/8.3/8.4/8.8/8.9, §15–§22; VP10 além do capítulo 3 e do snapping.

## C. Pontos que dependem de conferência no aplicativo
- Roda do mouse (rolagem vertical/horizontal) e atalhos do Mixing Console: o PDF tem colunas desalinhadas nessas tabelas.
- Nada foi commitado nesta rodada (`git status`: README.md, AGENTS.md, docs/research/nle_features_research.txt modificados; .claude/session-log-2026-10-24.md novo ou modificado).

---

# Adendo 3 — Fase 13 em código: tabela de comandos e keymap

## Ações
| # | Ação | Resultado |
| --- | --- | --- |
| 23 | `crates/neoveg_common/src/commands.rs`: enum `Command` gerado por macro (78 comandos, id pontuado estável, rótulo pt-BR, `Category`, flag `implemented()`), `Command::from_id` | 3 testes (ids únicos e reversíveis, formato dos ids, categorias não vazias) |
| 24 | `crates/neoveg_common/src/keymap.rs`: `Shortcut` (Ctrl/Shift/Alt + tecla, `FromStr`/`Display` canônicos, aliases `Esc`, `Page Up`…), `normalize_key`, atalhos padrão (`DEFAULT_BINDINGS`, do manual do VP13), `KeyMap` (`bind` desloca o dono anterior, `unbind`, `clear_command`, `reset_command`, `dispatch_order` por nº de modificadores, `from_overrides` com avisos, `to_overrides` só com diferenças) | 12 testes; os atalhos que já existiam (`Ctrl+Q`, `S`, `Space`, `←`/`→`, `Delete`, `F8`, `Ctrl+Z/Y`, `Ctrl+Shift+Z`, `Ctrl+O/S/Shift+S`) continuam com os mesmos comandos |
| 25 | `Preferences.keymap` (`BTreeMap<String, Vec<String>>`, `#[serde(default)]`) e `Preferences::key_map()` | Teste de arquivo antigo sem o campo e de ida e volta; `format_version` não mudou |
| 26 | `crates/neoveg_ui/src/shortcuts.rs`: `key_name`, `from_egui`, `to_egui` | 3 testes (todo atalho padrão tem tecla no egui; ida e volta; teclas inválidas) |
| 27 | `app.rs`: `handle_shortcuts` agora despacha pelo `KeyMap`; `run_command`, `step_frames`, `with_hint` (dica de atalho nos menus), `persist_keymap`; janela `keymap_window` (**Editar → Personalizar teclado…**: filtro, captura da próxima tecla, remover, restaurar um comando ou tudo, aviso de conflito, "em breve" para comandos não implementados) | Novo comportamento: `Ctrl+Shift+X` alterna o crossfade automático; `Enter` não foi ligado a play/pause (ativa botão focado no egui) |
| 28 | Verificação | `cargo fmt --all -- --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` sem avisos; `cargo test --workspace` todos verdes; o aplicativo abriu e ficou rodando 8 s sem erro (não foi testado interativamente) |
| 29 | Documentação: README (Fase 13 item 1 marcado, itens novos de próximos passos, linha em "Feito"), AGENTS.md §4.5 (parágrafo "How shortcuts are implemented") | — |

## Limites conhecidos
- Teclado numérico não tem atalho padrão (o egui não distingue as teclas); atalhos em sequência (`Alt+D`,`n`) e mapas nomeados ficaram para depois.
- Só 13 dos 78 comandos executam algo; os outros reservam o atalho e aparecem como "(em breve)".
- A janela de atalhos não foi exercitada com a interface aberta (só compilação, testes de lógica e inicialização).

---

# Adendo 4 — Fase 1, empacotamento e limpeza de scripts

## Fase 1 (seleção e cortar/copiar/colar)
- Engine: `selection.rs` (`Selection`, `TimeRange`), `ops.rs` (delete, copy, cut por faixa de tempo com ripple opcional, paste overlay/insert, paste repeat, trim à faixa, trim início/fim, split em várias faixas) e `EditCommand::ReplaceEvents` (um passo de desfazer para qualquer edição composta). 21 testes novos no engine.
- UI: `selection` substitui `selected_event`; clique, `Ctrl`+clique, `Shift`+clique; arrastar em área vazia cria a seleção de tempo; `I`/`O`; faixa azul sobre as faixas; menu Editar com Cortar, Copiar, Colar, Colar inserindo, Repetir colar…, Excluir, Dividir, Aparar…, Selecionar tudo/nada; menu de contexto do evento com Cortar/Copiar/Excluir/Selecionar até o fim; janela "Repetir colar". 10 testes novos de estado no `neoveg_ui`.
- Mudança de comportamento: `S` agora divide todos os eventos sob o cursor quando nada está selecionado (antes só a faixa selecionada).
- Abertos: caixas de início/fim/duração da seleção, seleção retangular na UI, mover vários eventos juntos, `Ctrl`+arrastar copia, menu do botão direito ao arrastar, `[`/`]`, inserir evento vazio e inserir tempo.

## `build_portable.py` (raiz) reescrito
- Só biblioteca padrão do Python. Subcomandos `build` (padrão), `qa`, `check`. `build`: portões de QA → `cargo build --release -p neoveg_ui` → pasta `dist/Neoveg-Portable-<rid>/` → `.zip` (Windows) ou `.tar.gz` → exe avulso `dist/neoveg_v<versão>_<rid>[.exe]` → `SHA256SUMS.txt` e `build_manifest.json`. `--ffmpeg` exige `FFMPEG_DIR`/`LIBCLANG_PATH` e copia as bibliotecas do FFmpeg; `--targets host|all|lista`; `--skip-qa`; `--no-archive`; `--sync` (usa `scripts/sync_gdrive.ps1`).
- Windows: `RUSTFLAGS=-C target-feature=+crt-static`; verificado que o exe não importa `VCRUNTIME140.dll`, então não precisa do redistribuível do Visual C++.
- Linux: launcher `neoveg.sh` com `LD_LIBRARY_PATH`; macOS: pacote `Neoveg.app` com `Info.plist`. Esses dois alvos **não foram testados** (só o Windows foi construído nesta máquina).
- `qa` grava `coverage/qa/qa_report.json` e `docs/qa_report.js` no formato que `docs/index.html` já lê (pilares ISO/IEC 25010 por etapa) e registra em `logs/neoveg.log`.
- Teste: `python build_portable.py qa` passou (rustfmt, clippy, testes, build release); `build --skip-qa` gerou `neoveg_v2.1.0_win-x64.exe` (14,7 MiB) e `Neoveg_v2.1.0_win-x64_Portable.zip` (6,1 MiB); o executável abriu e ficou rodando 6 s.

## Scripts removidos (19 arquivos em `scripts/`)
`Batch_Split_and_Datamosh.cs`, `VPFlow_AutoCrossfade_Helper.py` (exemplos de script C#/Python do host legado), `audit_trademarks.py` (política de marcas que o AGENTS 9.1 já não exige), `build-portable.*`, `build_all_platforms.*`, `build_portable.py` duplicado, `check_env.*` (agora `build_portable.py check`), `install_prerequisites.*`, `install_vcredist.bat` (o exe usa CRT estático), `neoveg.bat`/`neoveg.sh` (o empacotador gera o launcher), `run_qa_suite.*` (agora `build_portable.py qa`), `test_neoveg_engine.py` (testava o `python-backend`), `update_third_party.py` (clonava SDKs C++). Recuperáveis pelo Git.
- **Mantidos:** `build_portable.py` (raiz) e `scripts/sync_gdrive.ps1` (espelho para o Google Drive citado no AGENTS §11; não ajuda a migração, mas é ferramenta de backup do usuário).
- **Não tocados (legado de código, não script de ferramenta):** `python-backend/`, `tests/test_xml_interchange.py`, `rust-core/` (inclui `cargo.cmd`), `cpp-extensions/`, `desktop-host*/`, `Dockerfile`, `docker-compose.yml`, `.github/workflows/dotnet-desktop.yml`, `extensions/*.ext.json`, `DOCUMENTACAO_SCRIPTS.md` (descreve os exemplos removidos). Continuam como referência de porte (por exemplo `xml_interchange.py` para FCP7/FCPX/EDL).
- `Executar_Neoveg.cmd` aponta para `dist\Neoveg-Portable-win-x64\neoveg.exe`; `docs/index.html` e `AGENTS.md` (§3, §7, §10 e nota na Parte II) atualizados.

---

# Adendo 5 — ambiente Docker 100% Rust (commit a28ee0d)

- Criados: `docker/Dockerfile.rust-linux`, `docker/Dockerfile.rust-macos` (osxcross, SDK fornecido pelo usuário em `docker/sdk/`), `docker/docker-compose.yml` (serviços `test-rust-linux`, `build-rust-linux-release`, `build-rust-macos-release` no perfil `macos`), `docker/entrypoints/*.sh`, `scripts/test_rust_crossplatform.sh` e `.ps1`.
- `build_portable.py` ganhou `--dist` e respeita `CARGO_TARGET_DIR`; as imagens reutilizam esse empacotador.
- Documentação: AGENTS.md seção **12** (o pedido dizia 21, mas o arquivo só vai até a 11; a seção foi criada como 12 com o título pedido) e README (subseção "Testar e compilar em qualquer máquina (Docker)"). `.gitignore` ignora `docker/sdk/*.tar.*`.
- **Não executado:** esta máquina não tem Docker. Verificado apenas: `bash -n` nos scripts, parse do PowerShell, `pyright` limpo no `build_portable.py` e o `awk` que soma os testes sobre uma saída real do `cargo test` (142 passed, 0 failed). Imagens, compose e build cruzado de macOS nunca rodaram; o build para macOS (cpal/coreaudio-sys com osxcross) é a parte mais provável de precisar de ajuste.
- O commit foi montado só com os arquivos do ambiente (README, AGENTS e .gitignore entraram apenas com as linhas novas); o restante do trabalho (Fase 1, keymap, remoção de scripts, pesquisa) continua sem commit.

---

# Adendo 6 — Docker testado de verdade

- Docker Desktop 29.9.2 instalado pelo usuário; o daemon foi iniciado a partir da sessão (`Docker Desktop.exe`) e o `docker` entrou no PATH só em um shell novo (`C:\Program Files\Docker\Docker\resources\bin`).
- `scripts/test_rust_crossplatform.sh --tests-only` e depois completo, e `test_rust_crossplatform.ps1 -TestsOnly`: **fmt ok, clippy ok, 142 testes passados, 0 falhas**; build de release Linux gerou `dist/linux/neoveg_v2.1.0_linux-x64` (ELF, 18,7 MiB), `Neoveg_v2.1.0_linux-x64_Portable.tar.gz` (7,3 MiB, `neoveg` e `neoveg.sh` com bit de execução) e o relatório `dist/reports/test-linux.txt`.
- Defeitos encontrados e corrigidos (ainda sem commit): (1) `xvfb-run` trava como PID 1 do container (espera o SIGUSR1) → `test-linux.sh` agora sobe o Xvfb à mão e o compose usa `init: true`; (2) o executável Linux abortava com `libxkbcommon-x11.so could not be loaded` → `libxkbcommon-x11-dev` na imagem e nas listas de pacotes do README e do REQUIREMENTS.
- Teste de fumaça: o executável Linux abriu e ficou 8 s rodando sob Xvfb (software Vulkan) sem erro no log.
- macOS: o Dockerfile foi construído até o passo de verificação do SDK, que falhou com a mensagem esperada (apt, alvos do Rust e clone do osxcross funcionaram). Compilação cruzada em si NÃO testada, falta o SDK da Apple.
- Cuidado: um `taskkill //IM bash.exe` meu encerrou shells do Git Bash; não repetir.
