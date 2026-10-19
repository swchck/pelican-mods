using System;
using System.Linq;
using System.Reflection;
using Newtonsoft.Json.Linq;
using StardewModdingAPI;

namespace PelicanCompanion;

/// <summary>Reloads a mod's config object from its config.json while the game runs.</summary>
/// <remarks>
/// Most mods read <c>this.Config.Something</c> whenever they need it, so swapping the
/// values inside that object is enough. Values a mod copied elsewhere at startup stay old
/// until the game restarts; nothing outside the mod can fix that.
/// </remarks>
internal static class LiveConfig
{
    private const BindingFlags AnyField = BindingFlags.Instance | BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic;

    /// <summary>Re-reads mod <paramref name="modId"/>'s config.json into its live config object.</summary>
    /// <returns>Whether a config object was found and refreshed.</returns>
    public static bool Reload(IModRegistry registry, string modId, JObject values)
    {
        if (Gmcm.ModInstance(registry, modId) is not IMod mod)
            return false;
        (FieldInfo Field, object Target)? found = FindConfig(mod, values);
        if (found is null)
            return false;
        object current = found.Value.Field.GetValue(found.Value.Field.IsStatic ? null : mod)!;
        Type type = current.GetType();
        // the mod's own helper reads with SMAPI's converters (keybinds, colours, points)
        MethodInfo read = typeof(IModHelper).GetMethod(nameof(IModHelper.ReadConfig))!.MakeGenericMethod(type);
        object fresh = read.Invoke(mod.Helper, null)!;
        Copy(fresh, current);
        return true;
    }

    // the config is whichever mod-defined object shares the most member names with config.json
    private static (FieldInfo, object)? FindConfig(IMod mod, JObject values)
    {
        var keys = values.Properties().Select(p => p.Name).ToHashSet(StringComparer.OrdinalIgnoreCase);
        if (keys.Count == 0)
            return null;
        Assembly assembly = mod.GetType().Assembly;
        (FieldInfo, object)? best = null;
        double bestScore = 0.6;
        for (Type? t = mod.GetType(); t is not null && t.Assembly == assembly; t = t.BaseType)
        {
            foreach (FieldInfo field in t.GetFields(AnyField | BindingFlags.DeclaredOnly))
            {
                if (field.FieldType.Assembly != assembly || !field.FieldType.IsClass || field.IsInitOnly && field.IsStatic)
                    continue;
                if (field.GetValue(field.IsStatic ? null : mod) is not { } value)
                    continue;
                int hits = Members(value.GetType()).Count(keys.Contains);
                double score = (double)hits / keys.Count;
                if (score > bestScore)
                {
                    bestScore = score;
                    best = (field, value);
                }
            }
        }
        return best;
    }

    private static string[] Members(Type type) =>
        type.GetProperties(BindingFlags.Instance | BindingFlags.Public).Select(p => p.Name)
            .Concat(type.GetFields(BindingFlags.Instance | BindingFlags.Public).Select(f => f.Name))
            .ToArray();

    // copy into the existing object: other parts of the mod may hold a reference to it
    private static void Copy(object from, object to)
    {
        Type type = to.GetType();
        foreach (PropertyInfo p in type.GetProperties(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
        {
            if (p.CanRead && p.CanWrite && p.GetIndexParameters().Length == 0)
                p.SetValue(to, p.GetValue(from));
        }
        foreach (FieldInfo f in type.GetFields(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
        {
            // auto-property backing fields were handled above; init-only ones can't be set
            if (!f.IsInitOnly && !f.Name.Contains('<'))
                f.SetValue(to, f.GetValue(from));
        }
    }
}
