# Neoveg (fork do Kdenlive)

## Visão geral
Este fork adiciona um **perfil visual e de comportamento inspirado no Vegas Pro** mantendo as funcionalidades do Kdenlive (proxy, keyframes, efeitos, ferramentas etc.).

## Temas Vegas implementados (seleção em Configurações > Tema Vegas)
- **UltraWhite** (Vegas 8)
- **ClassicCinza** (Vegas 10–13)
- **JorgeTheUser** (Vegas 14)
- **ActualBlack** (Vegas 15+)

Os temas foram recriados via paleta + QSS próprios em `data/themes/` (sem copiar assets proprietários).

## O que foi implementado neste passo
- Registro do menu **Tema Vegas** em Configurações
- Persistência em configuração (`vegasThemeProfile`)
- Aplicação dinâmica de paleta + QSS + cores de timeline
- Layout adicional **Vegas** (`data/layouts/vegas.json`)
- Ordem padrão de layouts iniciando por `vegas`
- Ajustes de atalho estilo Vegas: `S` (cut/split clip), `D` (select tool)
- Esquema de atalhos `data/shortcuts/Vegas`

## Build Linux Mint (script)
```bash
cd /home/ubuntu/kdenlive_vegas
./scripts/build_linux_mint.sh
```

## Build Windows 10/11 (automático com .bat)
1. Instalar ferramentas mínimas:
   - Visual Studio 2022 Build Tools (workload **Desktop development with C++**)
   - CMake
   - Ninja
   - Qt >= 6.10, KDE Frameworks >= 6.21, Extra CMake Modules (ECM) >= 6.21, MLT >= 7.38 e FFmpeg, todos compatíveis com o compilador escolhido
2. Definir `CMAKE_PREFIX_PATH` com os prefixes onde Qt, ECM, KDE Frameworks e as demais dependências foram instalados.
   Exemplo no Prompt de Comando:
```bat
set CMAKE_PREFIX_PATH=C:\kde\usr;C:\Qt\6.10.0\msvc2022_64
```
Se o CMake ainda não localizar o ECM, defina `ECM_DIR` para o diretório que contém `ECMConfig.cmake`.
3. Executar o script:
```bat
cd scripts
build_windows_10_11.bat
```
4. Para também instalar os artefatos em `dist-win`:
```bat
build_windows_10_11.bat install
```

### Build Windows manual (alternativa)
```powershell
cmake -S . -B build-win-release -G Ninja -DCMAKE_BUILD_TYPE=Release -DKDE_INSTALL_USE_QT_SYS_PATHS=ON
cmake --build build-win-release --config Release
```

## Roadmap/checklist
### Temas
- [x] UltraWhite
- [x] ClassicCinza
- [x] JorgeTheUser
- [x] ActualBlack

### Subsistemas
- [x] Timeline (cores principais e contraste geral)
- [x] Cabeçalhos de faixa (paleta/realce)
- [x] Barras de ferramentas (QSS/paleta)
- [x] Docks (QSS/paleta)
- [x] Atalhos (comportamento base estilo Vegas)
- [x] Layout inicial estilo Vegas (arquivo de layout adicional)

### Pendências técnicas
- [ ] Calibração fina por comparação visual com capturas reais em execução local
- [ ] Ajustes milimétricos de densidade e microtipografia
- [ ] Testes completos de build em Linux Mint e Windows

## Nota sobre “JorgeTheUser”
A grafia solicitada foi mantida **exatamente** como `JorgeTheUser` no produto. Historicamente, a referência mais comum para o tema do Vegas 14 é “JustTheUser”.

## Honestidade de validação
Neste ambiente, as alterações de código e arquivos foram aplicadas e organizadas, mas a compilação completa do Kdenlive pode depender de toolchain/dependências locais. Não declarar build bem-sucedido sem evidência local.
