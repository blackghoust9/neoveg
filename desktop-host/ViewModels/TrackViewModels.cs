using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using Avalonia.Media;
using Neoveg.Models;
using Neoveg.Services;

namespace Neoveg.ViewModels;

/// <summary>
/// Base MVVM ViewModel for Timeline Tracks (`VideoTrackViewModel` and `AudioTrackViewModel`).
/// Automatically assigns `TrackAccentColor` via `TrackColorPaletteService.GetNextTrackColor(trackIndex)`
/// and propagates soft Clip Header / Border tints to child events.
/// </summary>
public abstract class TrackViewModelBase : INotifyPropertyChanged
{
    private string _trackAccentColor;
    private bool _propagateTintToClips = true;

    public TimelineTrack Model { get; }
    public int TrackIndex { get; }
    public event PropertyChangedEventHandler? PropertyChanged;

    public string TrackName
    {
        get => Model.Name;
        set
        {
            if (Model.Name != value)
            {
                Model.Name = value;
                OnPropertyChanged();
            }
        }
    }

    public string TrackAccentColor
    {
        get => _trackAccentColor;
        set
        {
            if (!string.IsNullOrWhiteSpace(value) && _trackAccentColor != value)
            {
                _trackAccentColor = value;
                Model.Color = value;
                Model.TrackAccentColor = value;
                if (_propagateTintToClips)
                {
                    PropagateAccentColorToTrackClips();
                }
                OnPropertyChanged();
                OnPropertyChanged(nameof(TrackAccentBrush));
            }
        }
    }

    public IBrush TrackAccentBrush =>
        new SolidColorBrush(TrackColorPaletteService.ParseSafeColor(_trackAccentColor));

    private bool _isSelected;

    /// <summary>
    /// Active (selected) track on the timeline. Only the selected track's Track Number Box indicator blinks;
    /// unselected tracks keep the dot static/hidden.
    /// </summary>
    public bool IsSelected
    {
        get => _isSelected;
        set
        {
            if (_isSelected != value)
            {
                _isSelected = value;
                OnPropertyChanged();
            }
        }
    }

    public bool PropagateTintToClips
    {
        get => _propagateTintToClips;
        set
        {
            if (_propagateTintToClips != value)
            {
                _propagateTintToClips = value;
                Model.PropagateTintToClips = value;
                OnPropertyChanged();
            }
        }
    }

    protected TrackViewModelBase(
        TimelineTrack model,
        int trackIndex,
        string? previousTrackColor = null,
        TrackColorPaletteService? paletteService = null)
    {
        Model = model;
        TrackIndex = trackIndex;
        var service = paletteService ?? TrackColorPaletteService.Instance;

        string assigned = !string.IsNullOrWhiteSpace(model.TrackAccentColor)
            ? model.TrackAccentColor!
            : service.GetNextTrackColor(trackIndex, previousTrackColor);

        _trackAccentColor = assigned;
        Model.Color = assigned;
        Model.TrackAccentColor = assigned;
        if (_propagateTintToClips)
        {
            PropagateAccentColorToTrackClips();
        }
    }

    public void PropagateAccentColorToTrackClips()
    {
        if (Model.Clips == null)
            return;

        foreach (var clip in Model.Clips)
        {
            clip.Color = _trackAccentColor;
            clip.TrackAccentColor = _trackAccentColor;
        }
    }

    public TimelineTrack ToTimelineTrack() => Model;

    protected void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}

public sealed class VideoTrackViewModel : TrackViewModelBase
{
    public VideoTrackViewModel(
        TimelineTrack model,
        int trackIndex,
        string? previousTrackColor = null,
        TrackColorPaletteService? paletteService = null)
        : base(model, trackIndex, previousTrackColor, paletteService)
    {
        model.Type = "video";
    }
}

public sealed class AudioTrackViewModel : TrackViewModelBase
{
    public AudioTrackViewModel(
        TimelineTrack model,
        int trackIndex,
        string? previousTrackColor = null,
        TrackColorPaletteService? paletteService = null)
        : base(model, trackIndex, previousTrackColor, paletteService)
    {
        model.Type = "audio";
    }
}
