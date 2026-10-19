using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Newtonsoft.Json;
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
    /// <remarks>
    /// Reads the file, not <paramref name="values"/>, which only helps find the object: the
    /// launcher writes config.json before asking, so both agree.
    /// </remarks>
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

    // copy into the existing object: other parts of the mod may hold a reference to it.
    // Only what config.json can carry (public, not [JsonIgnore]) moves, so caches and derived
    // state the mod keeps on the same class survive; nested sections are copied into in
    // place for the same reason. Every value is read first and a failing setter rolls back.
    private static void Copy(object from, object to)
    {
        var undo = new List<Action>();
        try
        {
            CopyInto(from, to, undo, depth: 0);
        }
        catch
        {
            for (int i = undo.Count - 1; i >= 0; i--)
                undo[i]();
            throw;
        }
    }

    private static void CopyInto(object from, object to, List<Action> undo, int depth)
    {
        Type type = to.GetType();
        Assembly assembly = type.Assembly;
        foreach (MemberInfo member in SerializedMembers(type))
        {
            object? next = Get(member, from);
            object? current = Get(member, to);
            if (depth < 4 && current is not null && next is not null && current.GetType() == next.GetType()
                && current.GetType().IsClass && current.GetType().Assembly == assembly)
            {
                CopyInto(next, current, undo, depth + 1);
                continue;
            }
            Set(member, to, next);
            undo.Add(() => Set(member, to, current));
        }
    }

    private static IEnumerable<MemberInfo> SerializedMembers(Type type)
    {
        bool ignored(MemberInfo m) => m.IsDefined(typeof(JsonIgnoreAttribute), inherit: true);
        foreach (PropertyInfo p in type.GetProperties(BindingFlags.Instance | BindingFlags.Public))
        {
            if (p.GetGetMethod() is not null && p.GetSetMethod() is not null && p.GetIndexParameters().Length == 0 && !ignored(p))
                yield return p;
        }
        foreach (FieldInfo f in type.GetFields(BindingFlags.Instance | BindingFlags.Public))
        {
            if (!f.IsInitOnly && !ignored(f))
                yield return f;
        }
    }

    private static object? Get(MemberInfo m, object target) =>
        m is PropertyInfo p ? p.GetValue(target) : ((FieldInfo)m).GetValue(target);

    private static void Set(MemberInfo m, object target, object? value)
    {
        if (m is PropertyInfo p)
            p.SetValue(target, value);
        else
            ((FieldInfo)m).SetValue(target, value);
    }
}
