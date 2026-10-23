# =============================================================================
# Neoveg — Official Python Script Sample
# File: scripts/VPFlow_AutoCrossfade_Helper.py
# Engine: python-backend/neoveg_scripting.py (NeovegProjectWrapper)
# Description:
#   Alinha automaticamente clipes sequenciais/sobrepostos, aplica curvas de
#   fade suaves (Smooth S-Curve de 0.75s) estilo VPFlow, configura interpolação
#   suave (Bezier Smoothness 88%) nos quadros Pan/Crop e normaliza o áudio.
# =============================================================================

overlap_seconds = 0.75
crossfade_count = neoveg.apply_vpflow_easing_and_crossfades(overlap_sec=overlap_seconds, curve="smooth")
normalized_audio = neoveg.normalize_audio_gains(1.0)

for idx, track in enumerate(neoveg.tracks):
    if track.get("type") == "video":
        for clip in track.get("clips", []):
            clip["fadeInCurve"] = "smooth"
            clip["fadeOutCurve"] = "smooth"

neoveg.add_marker(neoveg.cursor, "VPFlow Auto-Crossfade Sync", "#38bdf8")
neoveg.log(
    f"[VPFlow_AutoCrossfade_Helper.py] Aplicados {crossfade_count} auto-crossfades suaves ({overlap_seconds}s Smooth S-Curve) "
    f"e {normalized_audio} eventos de audio normalizados."
)
