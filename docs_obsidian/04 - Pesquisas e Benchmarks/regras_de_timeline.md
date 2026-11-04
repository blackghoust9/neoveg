---
tags: [neoveg, pesquisa, timeline]
---
# Regras de timeline

Regras de edição extraídas dos manuais de referência (Auto-Crossfade, Snapping, Envelopes e correlatos) e como o Neoveg as adota. Implementação em [[neoveg_engine]]; atalhos em [[atalhos_teclado]].

## Regras já implementadas no Neoveg
- **Auto-crossfade**: eventos da mesma faixa que se sobrepõem *parcialmente* fazem crossfade na sobreposição; um evento inteiramente dentro de outro não faz. Ligado por padrão (`Ctrl+Shift+X` alterna).
- **Snapping** (`F8`): alinha à origem, ao playhead, às bordas da seleção de tempo e às bordas de eventos em até 8 px; `Ctrl+F8` soma a grade de quadros, `Shift+F8` marcadores e regiões e `Ctrl+Shift+F8` os eventos de outras faixas. Senão, quantiza a quadros (`Alt+F8` desliga a quantização). `Alt` ao arrastar ignora.
- **Fades**: cinco curvas — Linear, Fast, Slow, Smooth, Sharp.
- **Trim**: mantém a fonte alinhada, nunca revela mídia fora da fonte e mantém ao menos um quadro.
- **Escopos de ripple** (`RippleScope`): faixas afetadas (eventos, keyframes e envelopes), faixas afetadas mais marcadores e regiões, ou todas as faixas mais marcadores e regiões. `F`, `Ctrl+F` e `Ctrl+Shift+F` escolhem o escopo usado pelo Ripple automático e pela exclusão com ripple e fazem o ripple da última edição (exclusão simples, aparo ou esticar) enquanto ela for a última mudança do histórico; tempos dentro de um trecho removido colapsam no início dele e nada vai antes de zero. Eventos travados não se movem. Com o Ripple automático ligado, aparar ou esticar uma borda também move o que vem depois pela mudança de comprimento (a borda esquerda fecha para a esquerda e o evento mantém o início).
- **Aparo de grupo**: aparar a borda de um evento agrupado apara também os membros do grupo cuja mesma borda está no mesmo instante; a borda comum para no limite mais restritivo. `Shift` ao arrastar (ou Ignorar agrupamento) apara só o evento arrastado (cortes em J e L). Não vale com o Ripple automático ligado.
- **Aparo adjacente (rolling)** (`Ctrl`+`Alt`+arrastar a borda entre dois eventos que se tocam; sem vizinho colado vira aparo comum): a borda entre dois eventos que se tocam anda para os dois lados e o comprimento total não muda; o limite é o mais restrito entre as duas fontes.
- **Shuffle** (ferramenta Embaralhar no ciclo `D`; o botão direito e o item de menu ainda faltam): reordena uma sequência simples (sem sobreposição nem eventos travados) e reaplica a ordem mantendo a sequência original dos intervalos.

## Resumo das entradas de pesquisa

<!-- auto:begin -->
Resumo automático das entradas de pesquisa (texto completo em `docs/research/nle_features_research.txt`).

### 2. Snapping
- Options > Enable Snapping: affects the main timeline and the Trimmer. Events snap to
- Holding Shift temporarily DISABLES snapping when it is on, and temporarily ENABLES it
- Snap to Grid: Options menu or Ctrl+F8; snaps to the vertical grid lines. Grid density is
- Snap to Markers: Options menu or Shift+F8; applies to markers, regions, CD track regions,
- Snap to All Events: Options menu or Ctrl+Shift+F8; snaps to event edges on other tracks.

*Sources:*

### 6. Fades e crossfades automáticos
- NOT DOCUMENTED: the exact mathematical shape of the five curve types (Linear, Fast, Slow,
- Events have a fade-in and a fade-out handle at the top corners of the event; dragging a handle
- Overlapping events on a track crossfade when automatic crossfades are enabled; the overlap
- gain(x), x = fade progress in [0, 1], gain(0) = 0, gain(1) = 1, monotonic:
- Fade-out uses gain(remaining/length). Linear, Smooth and Sharp are symmetric

*Sources:*

### 9. Variantes de snapping e quantização
- Snap targets with snapping on: other events' edges, the cursor, time-selection edges, the first
- Shift temporarily inverts snapping (overrides when on, enables when off).
- Each event has a draggable "snap offset" flag (white triangle, lower left) that moves the snap
- Snap indicator: a line in the track when snapping to an event; full timeline height for
- Quantize to Frames (independent of snapping) forces frame boundaries for: moving video events,

*Sources: Classic NLE v13 manual pp. 137-140. See also entries 1 and 2.*

### 10. Envelopes de evento, switches, takes e grupos
- Audio event envelopes: fade-in, fade-out, overall level (gain shown in dB while dragging).
- Switches (Edit > Switches / Event Properties): Mute, Lock, Loop, Invert Phase (audio),
- Audio event channel/stream selection (non-destructive): Both / Left only / Right only /
- Time stretch / pitch shift: Ctrl+edge drag; keys "=" and "-" = +/- 1 semitone, Ctrl = 1 cent,
- Takes: several media files on one event; Add as Takes; T / Shift+T = next/previous take;

*Sources: Classic NLE v13 manual pp. 166-186.*

### 14. Automação, envelopes e keyframes
- Envelopes (audio: mute, volume, pan, FX/bus sends, plug-in parameters; video: mute, composite
- Envelope editing: double-click adds a point, right-click deletes; segments can be dragged;
- A mute envelope is on/off with no fade; mute overrides solo.
- Automation recording modes per track: Off (control = static trim), Read (follows the envelope,
- "Lock Envelopes to Events" makes envelope points and track keyframes move with their events.

*Sources: Classic NLE v13 manual pp. 255-274; v11 tutorial booklet (keyframes, automation).*

### 42. Conferência com a ajuda oficial atual
- Playback: Space start; Enter or Ctrl+F12 play/pause; Space or Esc stop; Shift+Space or Shift+F12 play from
- Split: S splits the events at the cursor; Alt+S "Smart Split" of the selected event.
- Markers and regions: M marker, C command marker, R region, N CD track region, Shift+N CD track index;
- Zoom: Up / Down small horizontal steps (timeline focus); Ctrl+Up / Ctrl+Down large steps or zoom to the
- Grouping: G group, U ungroup, Ctrl+Shift+U ignore grouping, Ctrl+U clear the group without deleting events,

*Sources: Classic_NLE 2026 help, Keyboard shortcuts (reference-nle-help-archive*

### 43. Conferência com os PDFs oficiais (v18)
- Window shortcuts (default configuration): Alt+0 focus the timeline; Alt+1 Explorer; Alt+2 Trimmer;
- View and zoom: Tab toggles focus between track list and timeline; Alt+Shift+Up/Down previous / next
- Cursor and selection: Ctrl+G go to; Ctrl+Shift+G set the end of the time selection; Home / End go to
- Editing: Ctrl+X / Shift+Delete cut; Ctrl+C / Ctrl+Insert copy; Ctrl+V / Shift+Insert paste; Ctrl+Shift+V
- Edge trim: Numpad 7 or [ enter edge-trim on the event start and move to the previous edge; Numpad 9 or ]

*Sources: Classic_NLE_18_EN.pdf (pp. 28-31, 122-126, 150-152, 246-247, 278, 287-289, 309, 567-575) and*

### 44. Estudo entre versões (JKL, preferências de edição)
- "Take FX" check: the strings "Take FX" / "take effects" occur in none of the manuals (v9 to v21). The audio
- Scrubbing with J, K and L (v13 manual pp. 21-22, unchanged in v18 / v21): J = reverse scrub; pressing J
- Controllers: F3 / F9 are the jog / edit keys; Enter is play / pause; while paused the knob jogs by frame;
- Preferences > Editing that change timeline rules (v13 pp. 626-628, same in v18 / v21):
- Newer features worth tracking (v21 "What's new", pp. 21-25): Adjustment events (effects on an event apply to

*Sources: the PDFs listed above (v13 pp. 21-22, 581-583, 626-628; v18 pp. 122-126; v21 pp. 21-25).*

### 45. Manuais v8 e v14
- Shortcut stability (v8 keyboard table): the track and editing shortcuts already in Neoveg's key map are
- Hover scrub in the Trimmer (v14, new in that version): with "Enable Hover Scrub" on in the
- Velocity range (v14): the event Velocity envelope top end was raised to 1,000 % (earlier versions: 300 %,
- High-frame-rate (HFR) clips (v14): "Add At Project Frame Rate" on a Project Media item creates an event
- Other v14 additions (for tracking only, no Neoveg rule): Smart Zoom / Smart Scale / adaptive deinterlacing

*Sources: Classic_NLE_8_Manual.pdf (keyboard shortcut tables), Classic_NLE_v140_Manual_EN.pdf (pp. 15-16 what's*
<!-- auto:end -->
