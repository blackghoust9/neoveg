---
tags: [neoveg, manual, subprojetos]
---
# Subprojetos e abas

Vários projetos podem ficar abertos ao mesmo tempo, uma aba cada, e um projeto pode tocar outro como um único clipe (sequência aninhada).

## Como usar
- **Nova aba**: `Ctrl+N`. **Abrir em nova aba**: Arquivo → *Abrir em nova aba…*.
- **Criar subprojeto**: selecione eventos e use `Ctrl+Shift+N` (ou o menu de contexto do evento). Os eventos são copiados para um novo projeto salvo em `subprojetos/`, e na timeline original a seleção vira um único evento.
- **Editar o subprojeto**: dê duplo clique no evento que o toca; abre a aba dele (ou a existente).
- **Fechar uma aba suja** pergunta se deseja salvar, descartar ou cancelar.
- Um aninhamento circular, ou mais fundo que 8 níveis, é recusado.

Detalhes do motor em [[neoveg_engine]]. Voltar ao [[Index]].

<!-- auto:begin -->
Resumo técnico (fonte: AGENTS.md, seção 24):

- **Engine (`neoveg_engine::project_manager`).** `ProjectManager` keeps every open `Project` (`ProjectId`, `ProjectInfo`, `Timeline`, `History`, path, `saved_revision`) in tab order plus the active id: `register`, `open_project(path)` (returns the tab of a file that is already open), `close_project(id)` (the neighbour tab becomes active), `switch_active_project(id)`, `find_by_path`, `take` / `put`, and `create_subproject_from_events(...)`. The application edits the active project in its own state (`AppState.project`, `timeline`, `history`, ...) and the manager holds the others: a tab switch stashes the live project with `put` and brings the other back with `take`, so every editing function keeps working on `self.timeline` unchanged. Pure rules and unit tests live in the engine; the UI only calls them.
- **Media kinds (`neoveg_common::project`).** A media pool entry is `MediaSource::File(path)` or `MediaSource::SubProject(path)`; `MediaRef::source()` reads it from the persisted `subproject: bool` flag (`#[serde(default)]`, not written for plain files, so older projects load). A sub-project is a normal `.neoveg` file; paths are absolute and relative ones are taken from the parent's folder.
- **Circular dependencies.** `validate_no_cycle(parent, child)` follows the `SubProject` entries of the project files (depth-first, canonical paths) and refuses a nesting that reaches the parent again or goes deeper than `MAX_NESTING` (8 levels); `create_subproject_from_events` checks it before writing, and `open_project` refuses a project that reaches itself. Playback expansion also stops at `MAX_NESTING`, so a broken file can never loop.
- **Creating a sub-project** (`Ctrl+Shift+N`, command `file.create_subproject`, Edit/event context menu "Criar Sub-projeto a partir da seleção"): the selected events are copied (media pool entries included) into a new project, moved so the earliest one starts at zero, saved under `<project folder>/subprojetos/` (the app data folder for an unsaved project) and opened in a tab; in the parent the selection is replaced, as one undo step (`EditCommand::ReplaceEvents`), by one event that plays it (`assign_event_ids` gives it its id). `Ctrl+Shift+N` replaced the unbound "new empty project without dialog".
- **Playback (`flatten_subprojects`).** Before audio or video is built, every event that plays a sub-project is expanded: the child's events inside the window the event shows (`trim_in` .. `trim_in + duration`) are copied onto new tracks of a flattened timeline at the parent's time (child fades and trims kept; the child's track effects, fader and pan are not applied), and the event itself is dropped. The media of the child get stable ids from the hash of their path (`nested_media_id`, top bit set, so they never collide with a pool id) and are decoded on the worker like any media (`AppState::flattened()` caches the result until the project, a tab or a saved file changes). The audio mixer (`build_clips`) and the video preview (`request_preview_frame`) read the flattened view; a sub-project open in another tab is played as it is being edited.
- **Interface (`neoveg_ui/src/app/tabs.rs`).** A tab strip under the menu shows the open projects (`name`, `*` when dirty, `x` to close, `+` for a new one); closing a dirty tab asks to save, discard or cancel; the last tab is replaced by an empty project. File > New (`Ctrl+N`) opens an empty project in a new tab, File > "Abrir em nova aba…" opens a file in one. **Double-clicking an event that plays a sub-project** (or its context-menu entry) opens that project in a tab (the existing tab if it is already open).
- **Known limits.** Closing the whole window only checks the active project for unsaved changes; the layout of the dock (`egui_dock`) is shared by all tabs; autosave and recovery cover the active project only.
<!-- auto:end -->
