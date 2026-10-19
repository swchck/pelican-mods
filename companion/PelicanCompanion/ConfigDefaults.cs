using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using StardewModdingAPI;

namespace PelicanCompanion;

/// <summary>Works out what each C# mod's <c>config.json</c> holds before anyone changes it.</summary>
/// <remarks>
/// SMAPI doesn't record which type a mod reads its config as, so this looks in the mod's
/// assembly for the class whose public members best match the keys of its current
/// <c>config.json</c>, creates one, and serializes it with SMAPI's own settings so keybinds
/// and colours come out exactly as the mod would write them.
/// </remarks>
internal static class ConfigDefaults
{
    // a config class rarely holds every key a mod ever wrote; most of them is enough
    private const double MinShare = 0.6;

    public static Dictionary<string, JToken> Collect(IModHelper helper, IMonitor monitor)
    {
        var found = new Dictionary<string, JToken>();
        foreach (IModInfo info in helper.ModRegistry.GetAll())
        {
            if (info.IsContentPack)
                continue;
            try
            {
                JToken? defaults = ForMod(helper, info);
                if (defaults is not null)
                    found[info.Manifest.UniqueID] = defaults;
            }
            catch (Exception ex)
            {
                monitor.Log($"no config defaults for {info.Manifest.UniqueID}: {ex.Message}", LogLevel.Trace);
            }
        }
        return found;
    }

    private static JToken? ForMod(IModHelper helper, IModInfo info)
    {
        // ModMetadata's members are public on an internal type; reflection still reaches them
        string? dir = info.GetType().GetProperty("DirectoryPath")?.GetValue(info) as string;
        object? mod = info.GetType().GetProperty("Mod")?.GetValue(info);
        if (dir is null || mod is null)
            return null;
        string file = Path.Combine(dir, "config.json");
        if (!File.Exists(file))
            return null;
        if (JToken.Parse(File.ReadAllText(file)) is not JObject current || current.Count == 0)
            return null;

        Type? type = BestMatch(mod.GetType().Assembly, current.Properties().Select(p => p.Name).ToList());
        if (type is null)
            return null;
        return Serialize(helper, Activator.CreateInstance(type)!);
    }

    private static Type? BestMatch(Assembly assembly, List<string> keys)
    {
        Type[] types;
        try
        {
            types = assembly.GetTypes();
        }
        catch (ReflectionTypeLoadException ex)
        {
            types = ex.Types.Where(t => t is not null).ToArray()!;
        }
        var wanted = new HashSet<string>(keys, StringComparer.OrdinalIgnoreCase);
        Type? best = null;
        int bestHits = 0;
        foreach (Type t in types)
        {
            if (!t.IsClass || t.IsAbstract || t.IsGenericTypeDefinition || t.GetConstructor(Type.EmptyTypes) is null)
                continue;
            int hits = t.GetProperties(BindingFlags.Public | BindingFlags.Instance)
                .Where(p => p.CanWrite)
                .Select(p => p.Name)
                .Concat(t.GetFields(BindingFlags.Public | BindingFlags.Instance).Select(f => f.Name))
                .Distinct()
                .Count(wanted.Contains);
            if (hits > bestHits)
                (best, bestHits) = (t, hits);
        }
        return bestHits > 0 && bestHits >= MinShare * keys.Count ? best : null;
    }

    private static JToken Serialize(IModHelper helper, object instance)
    {
        // SMAPI's own settings carry its converters (Keybind, Color…), so the result reads
        // like the file the mod writes; they sit on DataHelper's private JsonHelper
        object? json = helper.Data.GetType()
            .GetField("JsonHelper", BindingFlags.NonPublic | BindingFlags.Instance)
            ?.GetValue(helper.Data);
        var settings = json?.GetType().GetProperty("JsonSettings")?.GetValue(json) as JsonSerializerSettings;
        return settings is null
            ? JToken.FromObject(instance)
            : JToken.Parse(JsonConvert.SerializeObject(instance, settings));
    }
}
