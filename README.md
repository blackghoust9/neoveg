# Neoveg

Editor não linear de vídeo e áudio multiplataforma, desenvolvido com C# / Avalonia, Rust FFI, C++ e ferramentas Python para mídia.

Consulte [AGENTS.md](./AGENTS.md) para os protocolos de arquitetura e desenvolvimento, [RELEASE_NOTES.md](./RELEASE_NOTES.md) para o resumo da release 1.0.0 e [REQUIREMENTS.md](./REQUIREMENTS.md) para preparar o ambiente de desenvolvimento.

## Executar a versão portátil no Windows

O pacote de referência é gerado em `dist\Neoveg-Portable-win-x64\`. Para executá-lo:

```powershell
.\dist\Neoveg-Portable-win-x64\Neoveg.exe
```

Também é possível iniciar pelo `Executar_Neoveg.cmd` na raiz ou na pasta do pacote. Ao detectar execução em unidade sincronizada ou de rede, o bootstrap pode copiar a aplicação para `%LOCALAPPDATA%\Neoveg\AppCache` e iniciá-la dali para evitar problemas de I/O e carregamento de bibliotecas.

## Gerar o pacote portátil

No Windows, com as ferramentas indicadas em [REQUIREMENTS.md](./REQUIREMENTS.md), gere o alvo Windows:

```powershell
python .\build_portable.py --targets windows
```

O pipeline cria o pacote em `dist\Neoveg-Portable-win-x64\`, publica o executável .NET em Release, monta os recursos portáteis e atualiza `dist\build_manifest.json`. No Windows, o pipeline também executa `scripts\sync_gdrive.ps1` para sincronizar o workspace e os pacotes de distribuição com o destino configurado no script. Para outros alvos, o builder aceita `linux`, `macos` ou `all`; a geração de binários nativos de cada plataforma deve ser feita em ambiente compatível.

Para uma publicação .NET self-contained separada do pacote portátil completo:

```powershell
dotnet publish desktop-host\Neoveg.csproj -c Release -r win-x64 --self-contained true -o dist\Neoveg_v1.0.0_win-x64_Portable
```

O projeto copia as DLLs nativas Windows para a saída de build/publicação. Na inicialização, o resolvedor procura primeiro junto ao executável e depois no AppCache; a validação de startup registra os caminhos encontrados. Os pacotes também carregam `Assets/`, scripts, runtimes e arquivos legais necessários ao formato portátil.

## Arquitetura

| Subsistema | Implementação | Responsabilidade |
| --- | --- | --- |
| Interface desktop | C# / Avalonia UI | Workspace de edição, timeline multifaixa, cabeçalho de eventos, régua de renderização, painéis e localização. |
| Núcleo NLE e DSP | Rust (`neoveg_core`) | Operações de timeline, timecode SMPTE, fades, crossfades, análise de áudio e cache de forma de onda. |
| Extensões nativas | C++17 / CMake (`neoveg_cpp_engine`) | Pontes para processamento de efeitos e áudio, interoperabilidade nativa e transferência de frames. |
| Mídia e automação | Python | Integração com FFmpeg/ffprobe, análise de mídia, automação e ferramentas de QA. |

## QA e estado de validação

As verificações realizadas nesta máquina Windows encontraram os seguintes limites; portanto, não representam uma aprovação completa da suíte multiplataforma:

- `cargo test --release` passou com 2 testes no ciclo de QA anterior.
- `dotnet test Neoveg.sln` terminou com sucesso, mas a solução não contém um projeto de testes .NET.
- `ctest` não encontrou testes C++ configurados.
- A coleta completa do `pytest` foi bloqueada por dependências ausentes e conflitos de importação. Testes direcionados de intercâmbio XML passaram; a auditoria Python específica validou operações de mídia/cache e um stress test de 5.000 operações, mas não completou as verificações que exigem bibliotecas nativas Linux/macOS.
- A publicação self-contained Windows e o smoke test do pacote foram executados. A aplicação abriu uma janela responsiva, carregou as DLLs nativas, foi transferida para o AppCache quando executada do Google Drive e encerrou normalmente.

**Limitação importante do build atual:** Cargo não estava disponível no ambiente usado para gerar o pacote portátil no Google Drive. O builder reportou `native-x86_64-c-abi-synthesized` para o Rust FFI. Assim, o carregamento das DLLs no smoke test foi confirmado, mas isso não substitui a compilação e validação do núcleo Rust real com Cargo. Para uma release de produção, instale o toolchain Rust e repita a compilação e os testes nativos antes da distribuição.

## Créditos e licenciamento

O código do projeto é distribuído sob a licença [MIT](./LICENSE). Consulte [AUTHORS.md](./AUTHORS.md) e [third_party/README.md](./third_party/README.md) para atribuições e licenças das dependências. Preserve esses arquivos ao redistribuir os pacotes.
