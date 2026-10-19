using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Reflection.Emit;
using Newtonsoft.Json.Linq;

namespace PelicanCompanion;

/// <summary>Recovers which config.json key a GMCM option edits from its getter's IL.</summary>
/// <remarks>
/// GMCM keeps only closures like <c>() =&gt; this.Config.Section.Value</c>; the property and
/// field reads in that body spell the path. Field ids would be nicer, but most mods leave
/// them unset and GMCM fills in a GUID.
/// </remarks>
internal static class IlPath
{
    private static readonly Dictionary<short, OpCode> OpCodesByValue = typeof(OpCodes)
        .GetFields(BindingFlags.Public | BindingFlags.Static)
        .Select(f => (OpCode)f.GetValue(null)!)
        .ToDictionary(o => o.Value);

    /// <summary>Returns the member names the method reads, in order.</summary>
    public static List<string> Members(MethodInfo method)
    {
        var names = new List<string>();
        byte[]? il = method.GetMethodBody()?.GetILAsByteArray();
        if (il is null)
            return names;
        Type[]? typeArgs = method.DeclaringType is { IsGenericType: true } t ? t.GetGenericArguments() : null;
        Type[]? methodArgs = method.IsGenericMethod ? method.GetGenericArguments() : null;

        for (int i = 0; i < il.Length;)
        {
            short value = il[i] == 0xFE && i + 1 < il.Length ? (short)(0xFE00 | il[i + 1]) : il[i];
            if (!OpCodesByValue.TryGetValue(value, out OpCode op))
                break;
            i += op.Size;
            int operand = op.OperandType switch
            {
                OperandType.InlineNone => 0,
                OperandType.ShortInlineBrTarget or OperandType.ShortInlineI or OperandType.ShortInlineVar => 1,
                OperandType.InlineVar => 2,
                OperandType.InlineI8 or OperandType.InlineR => 8,
                OperandType.InlineSwitch => 4 + 4 * BitConverter.ToInt32(il, i),
                _ => 4,
            };
            if (op.OperandType is OperandType.InlineMethod or OperandType.InlineField)
            {
                int token = BitConverter.ToInt32(il, i);
                string? name = Resolve(method.Module, op, token, typeArgs, methodArgs);
                if (name is not null)
                    names.Add(name);
            }
            i += operand;
        }
        return names;
    }

    private static string? Resolve(Module module, OpCode op, int token, Type[]? typeArgs, Type[]? methodArgs)
    {
        try
        {
            if (op == OpCodes.Ldfld || op == OpCodes.Ldsfld)
            {
                FieldInfo field = module.ResolveField(token, typeArgs, methodArgs)!;
                // closure captures (<>4__this, CS$<>8__locals1) are plumbing, not config keys
                return field.Name.Contains('<') || field.Name.StartsWith("CS$") ? null : field.Name;
            }
            if (op == OpCodes.Call || op == OpCodes.Callvirt)
            {
                MethodBase called = module.ResolveMethod(token, typeArgs, methodArgs)!;
                return called.IsSpecialName && called.Name.StartsWith("get_") ? called.Name[4..] : null;
            }
        }
        catch (ArgumentException)
        {
        }
        return null;
    }

    /// <summary>
    /// Finds the longest tail of <paramref name="members"/> that names a value in
    /// <paramref name="config"/>, and returns it with the JSON's own key casing.
    /// </summary>
    public static string[]? Match(IReadOnlyList<string> members, JObject config)
    {
        for (int start = 0; start < members.Count; start++)
        {
            string[]? path = Walk(config, members.Skip(start));
            if (path is not null)
                return path;
        }
        return null;
    }

    private static string[]? Walk(JObject root, IEnumerable<string> names)
    {
        var path = new List<string>();
        JToken current = root;
        foreach (string name in names)
        {
            if (current is not JObject obj)
                return null;
            JProperty? prop = obj.Properties().FirstOrDefault(p => string.Equals(p.Name, name, StringComparison.OrdinalIgnoreCase));
            if (prop is null)
                return null;
            path.Add(prop.Name);
            current = prop.Value;
        }
        return path.Count > 0 && current is not JObject ? path.ToArray() : null;
    }
}
