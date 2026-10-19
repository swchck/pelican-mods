using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using Newtonsoft.Json.Linq;
using StardewModdingAPI;

namespace PelicanCompanion;

/// <summary>Reads the menus mods registered with Generic Mod Config Menu, and drives them.</summary>
/// <remarks>
/// Everything here reaches into GMCM internals (Mod.ConfigManager, ModConfig, the option
/// classes) by name. A GMCM release that renames them makes this report nothing rather
/// than throw; the launcher then falls back to its own form.
/// </remarks>
internal sealed class Gmcm
{
    private const string GmcmId = "spacechase0.GenericModConfigMenu";
    // by name: SMAPI's assembly is x64-only, and naming its types here would load it in arm64 tests
    private const string ButtonType = "StardewModdingAPI.SButton";
    private const string KeybindType = "StardewModdingAPI.Utilities.KeybindList";
    private const BindingFlags Instance = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;

    private readonly IModRegistry Registry;
    private readonly Func<string, JObject?> ReadConfig;

    public Gmcm(IModRegistry registry, Func<string, JObject?> readConfig)
    {
        this.Registry = registry;
        this.ReadConfig = readConfig;
    }

    /// <summary>Describes every registered menu.</summary>
    public List<GmcmMod> Describe() =>
        this.Configs().Select(c => DescribeOne(c.Id, c.Config, this.ReadConfig(c.Id) ?? new JObject())).ToList();

    /// <summary>Describes one GMCM <c>ModConfig</c>, matching options to keys in <paramref name="json"/>.</summary>
    internal static GmcmMod DescribeOne(string id, object config, JObject json)
    {
        var options = new List<GmcmOption>();
        string? section = null;
        foreach (object option in Options(config))
        {
            string? name = Text(option, "Name");
            if (option.GetType().Name.StartsWith("SectionTitleModOption"))
            {
                section = name;
                continue;
            }
            Type? valueType = ValueType(option.GetType());
            if (valueType is null || name is null)
                continue;
            options.Add(new GmcmOption(
                Path: PathOf(option, json, checkValue: true),
                Name: name,
                Tooltip: Text(option, "Tooltip"),
                Kind: KindOf(option, valueType),
                Section: section,
                Min: Number(option, "Minimum"),
                Max: Number(option, "Maximum"),
                Interval: Number(option, "Interval"),
                Choices: Choices(option)
            ));
        }
        bool titleOnly = Get(config, "DefaultTitleScreenOnly") is true && Get(config, "AnyEditableInGame") is not true;
        return new GmcmMod(id, titleOnly, options);
    }

    /// <summary>Runs mod <paramref name="modId"/>'s GMCM save flow with <paramref name="values"/>.</summary>
    /// <param name="modId">The mod's UniqueID.</param>
    /// <param name="values">The new config.json content.</param>
    /// <param name="reloaded">The mod's config object already holds the new values.</param>
    /// <returns>Whether the mod has a GMCM menu this could drive.</returns>
    /// <remarks>
    /// Follows what GMCM's own Save button does: refresh every option from the mod, set the
    /// changed ones, then BeforeSave, the mod's save callback and AfterSave, so mods that
    /// react to their config changing get to. Options whose key the getter didn't reveal
    /// keep the mod's current value, so without <paramref name="reloaded"/> those stay stale.
    /// </remarks>
    public bool Apply(string modId, JObject values, bool reloaded)
    {
        object? config = this.Configs().FirstOrDefault(c => string.Equals(c.Id, modId, StringComparison.OrdinalIgnoreCase)).Config;
        if (config is null)
            return false;
        ApplyTo(config, values, reloaded);
        return true;
    }

    /// <summary>Runs one GMCM <c>ModConfig</c>'s save flow; see <see cref="Apply"/>.</summary>
    internal static void ApplyTo(object config, JObject values, bool reloaded)
    {
        List<object> options = Options(config).ToList();
        Call(options, "BeforeMenuOpened");
        foreach (object option in reloaded ? new List<object>() : options)
        {
            Type? valueType = ValueType(option.GetType());
            string[]? path = valueType is null ? null : PathOf(option, values, checkValue: false);
            JToken? token = path?.Aggregate((JToken?)values, (t, key) => t is JObject o ? o[key] : null);
            if (token is null)
                continue;
            try
            {
                option.GetType().GetProperty("Value", Instance)?.SetValue(option, Convert(token, valueType!));
            }
            catch (Exception)
            {
                // one value the option rejects (a null its setter can't compare, a bad
                // keybind) shouldn't stop the others; it keeps the mod's current value
            }
        }
        Call(options, "BeforeSave");
        (Get(config, "Save") as Action)?.Invoke();
        Call(options, "AfterSave");
    }

    private IEnumerable<(string Id, object Config)> Configs()
    {
        object? gmcm = ModInstance(this.Registry, GmcmId);
        object? manager = gmcm is null ? null : Get(gmcm, "ConfigManager");
        if (manager?.GetType().GetField("Configs", Instance)?.GetValue(manager) is not IDictionary configs)
            yield break;
        // GMCM mutates the dictionary as mods register; copy under its own lock. Keys, not
        // Cast<DictionaryEntry>: a generic Dictionary enumerates as KeyValuePair
        List<(string, object)> entries = new();
        lock (configs)
        {
            foreach (object key in configs.Keys)
            {
                if (key is string id && configs[key] is { } config)
                    entries.Add((id, config));
            }
        }
        foreach ((string, object) entry in entries)
            yield return entry;
    }

    private static IEnumerable<object> Options(object config)
    {
        if (Get(config, "Pages") is not IDictionary pages)
            yield break;
        foreach (object? page in pages.Values)
        {
            if (page is not null && Get(page, "Options") is IEnumerable options)
            {
                foreach (object option in options)
                    yield return option;
            }
        }
    }

    // checkValue: json is what the mod loaded, so the key found must hold the option's
    // current value; that catches a renamed section making `Sub.Enabled` match a top-level
    // `Enabled`. Off when matching against new values, which differ by design.
    private static string[]? PathOf(object option, JObject json, bool checkValue)
    {
        if (option.GetType().GetField("GetValue", Instance)?.GetValue(option) is not Delegate getter)
            return null;
        string? fieldId = Get(option, "FieldId") as string;
        if (fieldId is not null && Safe(() => json.SelectToken(fieldId)) is { } byId && byId is not JObject)
            return byId.Path.Split('.');
        string[]? path = MatchDelegate(getter, json, depth: 3);
        if (path is null || !checkValue)
            return path;
        JToken? token = path.Aggregate((JToken?)json, (t, key) => t is JObject o ? o[key] : null);
        return token is not null && Agrees(token, Safe(() => Get(option, "Value"))) ? path : null;
    }

    private static bool Agrees(JToken token, object? value) => value switch
    {
        bool b => token.Type == JTokenType.Boolean && token.Value<bool>() == b,
        int or long or float or double or decimal => token.Type is JTokenType.Integer or JTokenType.Float
            && Math.Abs(token.Value<double>() - ((IConvertible)value).ToDouble(null)) < 1e-6,
        string or Enum => string.Equals(token.ToString(), value.ToString(), StringComparison.OrdinalIgnoreCase),
        // keybinds, colours and other objects: no cheap comparison, trust the IL
        _ => true,
    };

    // helper libraries wrap the mod's lambda, `() => get(this.GetConfig())`, so when the
    // outer body names no key, look into delegates the closure captured
    private static string[]? MatchDelegate(Delegate d, JObject json, int depth)
    {
        List<string> members = IlPath.Members(d.Method);
        string[]? path = IlPath.Match(members, json);
        // `Config.Limit.Value` on a nullable reads the key, then Nullable<T>.Value
        if (path is null && members.Count > 1 && members[^1] is "Value" or "HasValue")
            path = IlPath.Match(members.Take(members.Count - 1).ToList(), json);
        if (path is not null || depth == 0 || d.Target is null)
            return path;
        foreach (FieldInfo field in d.Target.GetType().GetFields(Instance))
        {
            // <>c caches every non-capturing lambda of the class; those belong to other options
            if (field.Name.StartsWith("<>"))
                continue;
            if (field.GetValue(d.Target) is Delegate inner && MatchDelegate(inner, json, depth - 1) is { } found)
                return found;
        }
        return null;
    }

    // SimpleModOption<T> and its subclasses; anything else is a label or an image
    private static Type? ValueType(Type type)
    {
        for (Type? t = type; t is not null; t = t.BaseType)
        {
            if (t.IsGenericType && t.GetGenericTypeDefinition().Name.StartsWith("SimpleModOption"))
                return t.GetGenericArguments()[0];
        }
        return null;
    }

    private static string KindOf(object option, Type valueType)
    {
        if (option.GetType().Name.StartsWith("ChoiceModOption"))
            return "choice";
        if (valueType == typeof(bool))
            return "bool";
        if (valueType == typeof(int) || valueType == typeof(long))
            return "int";
        if (valueType == typeof(float) || valueType == typeof(double))
            return "float";
        if (valueType.FullName is ButtonType or KeybindType)
            return "keybind";
        return "string";
    }

    private static List<GmcmChoice>? Choices(object option)
    {
        if (Get(option, "Choices") is not Array values)
            return null;
        var format = Get(option, "FormatChoice") as Func<string, string>;
        return values.Cast<object>()
            .Select(v => v.ToString() ?? "")
            .Select(v => new GmcmChoice(v, Safe(() => format?.Invoke(v)) ?? v))
            .ToList();
    }

    private static object? Convert(JToken token, Type type)
    {
        if (type.FullName == KeybindType)
            return type.GetMethod("Parse", new[] { typeof(string) })!.Invoke(null, new object[] { token.ToString() });
        if (type == typeof(string))
            return token.Type == JTokenType.Null ? null : token.ToString();
        if (type.IsEnum)
            return Enum.Parse(type, token.ToString(), ignoreCase: true);
        return token.ToObject(type);
    }

    private static void Call(IEnumerable<object> options, string method)
    {
        foreach (object option in options)
            option.GetType().GetMethod(method, Instance, Type.EmptyTypes)?.Invoke(option, null);
    }

    private static string? Text(object target, string name) => Safe(() => (Get(target, name) as Delegate)?.DynamicInvoke() as string);

    private static double? Number(object target, string name) => Get(target, name) is IConvertible c ? c.ToDouble(null) : null;

    private static object? Get(object target, string name)
    {
        Type type = target.GetType();
        return type.GetProperty(name, Instance)?.GetValue(target) ?? type.GetField(name, Instance)?.GetValue(target);
    }

    private static T? Safe<T>(Func<T?> f)
    {
        try
        {
            return f();
        }
        catch (Exception)
        {
            // a mod's label callback may need a loaded world or its own state
            return default;
        }
    }

    /// <summary>Returns the entry object of a loaded mod; SMAPI only exposes it internally.</summary>
    public static object? ModInstance(IModRegistry registry, string id)
    {
        IModInfo? info = registry.Get(id);
        return info?.GetType().GetProperty("Mod", Instance)?.GetValue(info);
    }
}
