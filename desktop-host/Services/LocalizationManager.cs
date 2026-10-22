using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Xml.Linq;
using Avalonia;

namespace Neoveg.Services;

public sealed record LanguageOption(string Code, string DisplayName, string EnglishName)
{
    public string DisplayLabel => DisplayName;
    public override string ToString() => DisplayName;
}

/// <summary>
/// Dynamic Multi-Language Internationalization Manager (i18n) for Neoveg.
/// Supports 8 official VEGAS Pro + Global languages:
/// - pt-BR (Português do Brasil — Default)
/// - en-US (English)
/// - es-ES (Español)
/// - fr-FR (Français)
/// - de-DE (Deutsch)
/// - zh-CN (简体中文)
/// - ja-JP (日本語)
/// - ru-RU (Русский)
/// Reads `.axaml` ResourceDictionaries and `.json` dictionaries from `Assets/i18n/`,
/// updates `Application.Current.Resources` in real time, and persists the user's preference in `settings.json`.
/// </summary>
public sealed class LocalizationManager
{
    private static LocalizationManager? _instance;
    public static LocalizationManager Instance
    {
        get => _instance ??= new LocalizationManager(AppContext.BaseDirectory);
        private set => _instance = value;
    }

    public static readonly IReadOnlyList<LanguageOption> Languages = new[]
    {
        new LanguageOption("pt-BR", "Português do Brasil (pt-BR)", "Brazilian Portuguese"),
        new LanguageOption("en-US", "English (en-US)", "English (United States)"),
        new LanguageOption("es-ES", "Español (es-ES)", "Spanish (Spain)"),
        new LanguageOption("fr-FR", "Français (fr-FR)", "French (France)"),
        new LanguageOption("de-DE", "Deutsch (de-DE)", "German (Germany)"),
        new LanguageOption("zh-CN", "简体中文 (zh-CN)", "Simplified Chinese"),
        new LanguageOption("ja-JP", "日本語 (ja-JP)", "Japanese"),
        new LanguageOption("ru-RU", "Русский (ru-RU)", "Russian")
    };

    public static IReadOnlyList<LanguageOption> SupportedLanguages => Languages;

    private readonly string _rootDir;
    private readonly Dictionary<string, Dictionary<string, string>> _catalog = new(StringComparer.OrdinalIgnoreCase);

    public string CurrentLanguageCode { get; private set; } = "pt-BR";
    public string SelectedThemeId { get; set; } = "era3_magix_dark";
    public bool ShowWelcomeOnStartup { get; set; } = true;

    public LanguageOption CurrentLanguage =>
        Languages.FirstOrDefault(l => l.Code.Equals(CurrentLanguageCode, StringComparison.OrdinalIgnoreCase)) ?? Languages[0];

    public LanguageOption GetLanguageMetadata(string langCode) =>
        Languages.FirstOrDefault(l => l.Code.Equals(langCode, StringComparison.OrdinalIgnoreCase)) ?? Languages[0];

    public event Action<string>? OnLanguageChanged;

    public LocalizationManager(string rootDir)
    {
        _rootDir = rootDir;
        Instance = this;
        LoadAllLanguageDictionaries();
        string savedLang = LoadLanguageFromSettingsJson();
        SetLanguage(savedLang, persistToSettings: false);
    }

    public string SettingsFilePath => Path.Combine(_rootDir, "settings.json");

    private void LoadAllLanguageDictionaries()
    {
        var searchDirs = new[]
        {
            Path.Combine(_rootDir, "assets", "i18n"),
            Path.Combine(_rootDir, "desktop-host", "Assets", "i18n"),
            Path.Combine(AppContext.BaseDirectory, "assets", "i18n"),
            Path.Combine(AppContext.BaseDirectory, "Assets", "i18n")
        };

        foreach (var lang in Languages)
        {
            var dict = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

            foreach (string dir in searchDirs)
            {
                if (!Directory.Exists(dir)) continue;

                // 1. Parse Avalonia .axaml ResourceDictionary (<sys:String x:Key="...">Value</sys:String>)
                string axamlPath = Path.Combine(dir, $"{lang.Code}.axaml");
                if (File.Exists(axamlPath))
                {
                    try
                    {
                        var xdoc = XDocument.Load(axamlPath);
                        XNamespace xNs = "http://schemas.microsoft.com/winfx/2006/xaml";
                        foreach (var elem in xdoc.Descendants())
                        {
                            var keyAttr = elem.Attribute(xNs + "Key");
                            if (keyAttr != null && !string.IsNullOrWhiteSpace(keyAttr.Value))
                            {
                                dict[keyAttr.Value] = elem.Value;
                            }
                        }
                    }
                    catch
                    {
                        // Ignore malformed XML and fallback to JSON
                    }
                }

                // 2. Parse .json dictionary if present
                string jsonPath = Path.Combine(dir, $"{lang.Code}.json");
                if (File.Exists(jsonPath))
                {
                    try
                    {
                        using var jdoc = JsonDocument.Parse(File.ReadAllText(jsonPath));
                        var root = jdoc.RootElement;
                        if (root.TryGetProperty("strings", out var stringsObj) && stringsObj.ValueKind == JsonValueKind.Object)
                        {
                            foreach (var prop in stringsObj.EnumerateObject())
                            {
                                if (prop.Value.ValueKind == JsonValueKind.String)
                                    dict[prop.Name] = prop.Value.GetString() ?? "";
                            }
                        }
                        else if (root.ValueKind == JsonValueKind.Object)
                        {
                            foreach (var prop in root.EnumerateObject())
                            {
                                if (prop.Value.ValueKind == JsonValueKind.String)
                                    dict[prop.Name] = prop.Value.GetString() ?? "";
                            }
                        }
                    }
                    catch
                    {
                        // Ignore malformed JSON
                    }
                }

                if (dict.Count > 0)
                    break;
            }

            _catalog[lang.Code] = dict;
        }
    }

    public string LoadLanguageFromSettingsJson()
    {
        try
        {
            if (File.Exists(SettingsFilePath))
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(SettingsFilePath));
                if (doc.RootElement.TryGetProperty("language", out var langProp))
                {
                    string? code = langProp.GetString();
                    if (!string.IsNullOrWhiteSpace(code) &&
                        Languages.Any(l => l.Code.Equals(code, StringComparison.OrdinalIgnoreCase)))
                    {
                        return Languages.First(l => l.Code.Equals(code, StringComparison.OrdinalIgnoreCase)).Code;
                    }
                }
            }
        }
        catch
        {
            // Default to pt-BR
        }
        return "pt-BR";
    }

    public void SaveLanguageToSettingsJson(string langCode)
    {
        try
        {
            var settingsObj = new Dictionary<string, object>
            {
                ["language"] = langCode,
                ["themeEra"] = 3,
                ["showWelcomeOnStartup"] = true,
                ["supportedLanguages"] = Languages.Select(l => l.Code).ToArray(),
                ["updatedAt"] = DateTime.UtcNow.ToString("O")
            };

            if (File.Exists(SettingsFilePath))
            {
                try
                {
                    var existing = JsonSerializer.Deserialize<Dictionary<string, object>>(File.ReadAllText(SettingsFilePath));
                    if (existing != null)
                    {
                        existing["language"] = langCode;
                        existing["updatedAt"] = DateTime.UtcNow.ToString("O");
                        settingsObj = existing;
                    }
                }
                catch
                {
                    // Overwrite with clean settings object
                }
            }

            string jsonOut = JsonSerializer.Serialize(settingsObj, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(SettingsFilePath, jsonOut);
        }
        catch
        {
            // Non-fatal if read-only media
        }
    }

    public void SaveSettings() => SaveLanguageToSettingsJson(CurrentLanguageCode);

    public void ApplyToAvaloniaApplicationResources(Dictionary<string, string> dict)
    {
        if (Application.Current != null)
        {
            foreach (var kv in dict)
            {
                Application.Current.Resources[kv.Key] = kv.Value;
            }
        }
    }

    public void SetLanguage(string langCode, bool persistToSettings = true, bool saveToSettings = true)
    {
        var matched = Languages.FirstOrDefault(l => l.Code.Equals(langCode, StringComparison.OrdinalIgnoreCase))
                      ?? Languages[0];

        CurrentLanguageCode = matched.Code;

        // Push all active keys into Avalonia Application.Current.Resources for live XAML/DynamicResource binding
        if (_catalog.TryGetValue(CurrentLanguageCode, out var dict))
        {
            ApplyToAvaloniaApplicationResources(dict);
        }

        if (persistToSettings && saveToSettings)
        {
            SaveLanguageToSettingsJson(CurrentLanguageCode);
        }

        OnLanguageChanged?.Invoke(CurrentLanguageCode);
    }

    /// <summary>
    /// Translates `key` into the active language (`CurrentLanguageCode`), falling back to `pt-BR` or `fallback`.
    /// </summary>
    public string T(string key, string fallback = "")
    {
        if (_catalog.TryGetValue(CurrentLanguageCode, out var activeDict) &&
            activeDict.TryGetValue(key, out string? val) &&
            !string.IsNullOrEmpty(val))
        {
            return val;
        }

        if (_catalog.TryGetValue("pt-BR", out var ptDict) &&
            ptDict.TryGetValue(key, out string? ptVal) &&
            !string.IsNullOrEmpty(ptVal))
        {
            return ptVal;
        }

        return string.IsNullOrEmpty(fallback) ? key : fallback;
    }

    public static string Tr(string key, string fallback = "") => Instance.T(key, fallback);
}
