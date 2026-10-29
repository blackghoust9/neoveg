# CLAUDE.md

As regras de arquitetura, qualidade e documentação do Neoveg estão em `AGENTS.md`. Este arquivo guarda só as preferências de trabalho que valem em toda sessão.

## Working preferences

- Rode em segundo plano (`run_in_background`) os testes que abrem janelas ou desenham a interface: a captura de tela offscreen, os testes de janelas destacadas e de layouts e o `cargo test` de `neoveg_ui`. Testes de regras puras podem rodar em primeiro plano.
- Depois de terminar as builds e os testes de uma tarefa (e de commitar), apague a pasta `target/` (`cargo clean`): ela chegou a 31 GB. Não apague no meio de uma sequência de compilações nem com testes rodando em segundo plano.
