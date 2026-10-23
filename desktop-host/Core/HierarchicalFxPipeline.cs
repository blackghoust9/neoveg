using System;
using System.Collections.Generic;
using System.Linq;
using Neoveg.Models;

namespace Neoveg.Core;

public enum FxHierarchyStage
{
    MediaFx = 1,
    TakeFx = 2,
    EventFx = 3,
    TrackFx = 4,
    MasterFx = 5
}

public sealed record ResolvedFxStageNode(
    FxHierarchyStage Stage,
    string StageLabel,
    VideoFxInstance EffectInstance);

/// <summary>
/// Implements the official Neoveg 5-stage Hierarchical Audio & Video FX Signal Routing Pipeline:
/// Media FX -> Take FX -> Event FX -> Track FX -> Master FX.
/// Referenced in `AGENTS.md` Section 10 (Neoveg 10, 13, 18, and 21 Reference Manuals).
/// </summary>
public static class HierarchicalFxPipeline
{
    public const string CanonicalPipelineSignature =
        "Media FX -> Take FX -> Event FX -> Track FX -> Master FX";

    public static string GetStageDisplayName(FxHierarchyStage stage) => stage switch
    {
        FxHierarchyStage.MediaFx => "Media FX",
        FxHierarchyStage.TakeFx => "Take FX",
        FxHierarchyStage.EventFx => "Event FX",
        FxHierarchyStage.TrackFx => "Track FX",
        FxHierarchyStage.MasterFx => "Master FX",
        _ => "Event FX"
    };

    /// <summary>
    /// Resolves the ordered 5-stage effect execution list for a given timeline event:
    /// 1. Media FX (Project Media asset + clip MediaFx)
    /// 2. Take FX (Active Take effect chain)
    /// 3. Event FX (Timeline event effect chain)
    /// 4. Track FX (Parent track effect chain)
    /// 5. Master FX (Project Master Video/Audio bus chain)
    /// </summary>
    public static IReadOnlyList<ResolvedFxStageNode> ResolveOrderedPipeline(
        NeovegProject? project,
        TimelineTrack? track,
        TimelineClip? clip)
    {
        var ordered = new List<ResolvedFxStageNode>();

        // Stage 1: Media FX
        if (project?.MediaPool != null && clip != null && !string.IsNullOrEmpty(clip.MediaId))
        {
            var poolAsset = project.MediaPool.FirstOrDefault(m => m.Id == clip.MediaId);
            if (poolAsset?.MediaFx != null)
            {
                foreach (var fx in poolAsset.MediaFx.Where(f => f.Enabled))
                {
                    ordered.Add(new ResolvedFxStageNode(FxHierarchyStage.MediaFx, "Media FX", fx));
                }
            }
        }
        if (clip?.MediaFx != null)
        {
            foreach (var fx in clip.MediaFx.Where(f => f.Enabled))
            {
                ordered.Add(new ResolvedFxStageNode(FxHierarchyStage.MediaFx, "Media FX", fx));
            }
        }

        // Stage 2: Take FX
        if (clip?.TakeFx != null)
        {
            foreach (var fx in clip.TakeFx.Where(f => f.Enabled))
            {
                ordered.Add(new ResolvedFxStageNode(FxHierarchyStage.TakeFx, "Take FX", fx));
            }
        }

        // Stage 3: Event FX
        if (clip?.EventFx != null)
        {
            foreach (var fx in clip.EventFx.Where(f => f.Enabled))
            {
                ordered.Add(new ResolvedFxStageNode(FxHierarchyStage.EventFx, "Event FX", fx));
            }
        }

        // Stage 4: Track FX
        if (track?.TrackFx != null)
        {
            foreach (var fx in track.TrackFx.Where(f => f.Enabled))
            {
                ordered.Add(new ResolvedFxStageNode(FxHierarchyStage.TrackFx, "Track FX", fx));
            }
        }

        // Stage 5: Master FX
        if (project?.MasterFx != null)
        {
            foreach (var fx in project.MasterFx.Where(f => f.Enabled))
            {
                ordered.Add(new ResolvedFxStageNode(FxHierarchyStage.MasterFx, "Master FX", fx));
            }
        }

        return ordered;
    }

    public static List<VideoFxInstance> GetMutableListForStage(
        NeovegProject project,
        TimelineTrack track,
        TimelineClip clip,
        FxHierarchyStage stage)
    {
        return stage switch
        {
            FxHierarchyStage.MediaFx => clip.MediaFx,
            FxHierarchyStage.TakeFx => clip.TakeFx,
            FxHierarchyStage.EventFx => clip.Fx,
            FxHierarchyStage.TrackFx => track.Fx,
            FxHierarchyStage.MasterFx => project.MasterFx,
            _ => clip.Fx
        };
    }
}
