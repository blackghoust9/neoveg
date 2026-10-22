// ============================================================================
// Neoveg — Official C# Roslyn Script Sample
// File: scripts/Batch_Split_and_Datamosh.cs
// Engine: Microsoft.CodeAnalysis.CSharp.Scripting (Roslyn) + Neoveg.Scripting
// Description:
//   Percorre a faixa de vídeo selecionada (ou a primeira faixa de vídeo),
//   divide os eventos (TrackEvent.Split) em intervalos regulares e aplica o
//   efeito de interpolação/glitch Datamosh (I-Frame Drop + P-Vector Bloom),
//   ajustando velocidade (PlaybackRate) e inserindo marcadores na Timeline.
// ============================================================================

using System;
using System.Linq;
using Neoveg.Scripting;

public class EntryPoint
{
    public void FromVegas(Vegas myVegas)
    {
        Timecode splitInterval = Timecode.FromSeconds(0.85);
        int totalSplits = 0;
        int totalGlitched = 0;

        foreach (Track track in myVegas.Project.Tracks)
        {
            if (!track.IsVideo)
                continue;

            // Processa a faixa selecionada ou a faixa principal de vídeo (Index == 0)
            if (!track.Selected && track.Index != 0)
                continue;

            var eventsSnapshot = track.Events.ToList();
            foreach (TrackEvent ev in eventsSnapshot)
            {
                if (ev is VideoEvent videoEv)
                {
                    if (videoEv.Length > Timecode.FromSeconds(1.20))
                    {
                        TrackEvent? rightHalf = videoEv.Split(splitInterval);
                        if (rightHalf is VideoEvent glitchedRight)
                        {
                            glitchedRight.ApplyDatamoshGlitch(0.90, "P-Frame Motion Vector Bloom");
                            glitchedRight.PlaybackRate = 1.15;
                            glitchedRight.FadeIn.Length = Timecode.FromSeconds(0.12);
                            glitchedRight.FadeIn.Curve = CurveType.Sharp;
                            myVegas.Project.Markers.Add(new Marker(glitchedRight.Start, $"Datamosh Glitch #{totalSplits + 1}"));
                            totalSplits++;
                            totalGlitched++;
                        }
                    }

                    videoEv.ApplyDatamoshGlitch(0.75, "I-Frame Drop + Optical Flow");
                    totalGlitched++;
                }
            }
        }

        myVegas.Log($"[Batch_Split_and_Datamosh.cs] Concluido: {totalSplits} corte(s) sub-quadro e {totalGlitched} evento(s) com Datamosh Glitch aplicado.");
        myVegas.UpdateUI();
    }
}
