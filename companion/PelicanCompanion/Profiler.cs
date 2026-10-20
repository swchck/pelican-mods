using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Reflection;
using HarmonyLib;
using StardewModdingAPI;

namespace PelicanCompanion;

/// <summary>Times every mod's SMAPI event handlers for a while, to find what slows the game.</summary>
/// <remarks>
/// SMAPI 4 dropped its own performance monitor. While a measurement runs, each handler method
/// gets a Harmony prefix and postfix that add up its time; stopping removes them again, so
/// the game pays nothing outside a measurement. Call from the game thread.
/// </remarks>
internal static class Profiler
{
    private const string HarmonyId = "swchck.PelicanCompanion.profiler";
    private const BindingFlags Any = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static;

    private static Harmony? Patcher;
    private static readonly Dictionary<MethodBase, Timing> Timings = new();
    private static long StartedAt;

    private sealed class Timing
    {
        public readonly string Mod;
        public readonly string Event;
        public long Ticks;
        public long MaxTicks;
        public int Calls;

        public Timing(string mod, string eventName)
        {
            this.Mod = mod;
            this.Event = eventName;
        }
    }

    public static bool Running => Patcher is not null;

    /// <summary>Patches every handler other mods registered; returns how many.</summary>
    public static int Start(string ownId)
    {
        Stop();
        Timings.Clear();
        Patcher = new Harmony(HarmonyId);
        var before = new HarmonyMethod(typeof(Profiler).GetMethod(nameof(Before), Any));
        var after = new HarmonyMethod(typeof(Profiler).GetMethod(nameof(After), Any));
        foreach ((string eventName, string mod, string modId, MethodInfo method) in Handlers())
        {
            if (modId.Equals(ownId, StringComparison.OrdinalIgnoreCase) || Timings.ContainsKey(method))
                continue;
            try
            {
                Patcher.Patch(method, prefix: before, postfix: after);
                Timings[method] = new Timing(mod, eventName);
            }
            catch (Exception)
            {
                // abstract, generic or already-inlined handlers can't be patched; they go unmeasured
            }
        }
        StartedAt = Stopwatch.GetTimestamp();
        return Timings.Count;
    }

    /// <summary>Removes the patches and returns each mod's time, slowest first.</summary>
    public static ProfileResult Stop()
    {
        Patcher?.UnpatchAll(HarmonyId);
        Patcher = null;
        double seconds = StartedAt == 0 ? 0 : (Stopwatch.GetTimestamp() - StartedAt) / (double)Stopwatch.Frequency;
        double Ms(long ticks) => ticks * 1000.0 / Stopwatch.Frequency;
        var mods = Timings.Values
            .Where(t => t.Calls > 0)
            .GroupBy(t => t.Mod)
            .Select(g => new ProfileMod(
                g.Key,
                Ms(g.Sum(t => t.Ticks)),
                Ms(g.Max(t => t.MaxTicks)),
                g.Sum(t => t.Calls),
                g.OrderByDescending(t => t.Ticks).Take(3)
                    .Select(t => new ProfileEvent(t.Event, Ms(t.Ticks), t.Calls)).ToList()
            ))
            .OrderByDescending(m => m.TotalMs)
            .ToList();
        StartedAt = 0;
        return new ProfileResult(seconds, mods);
    }

    private static void Before(out long __state) => __state = Stopwatch.GetTimestamp();

    private static void After(long __state, MethodBase __originalMethod)
    {
        long spent = Stopwatch.GetTimestamp() - __state;
        if (Timings.TryGetValue(__originalMethod, out Timing? t))
        {
            t.Ticks += spent;
            t.Calls++;
            if (spent > t.MaxTicks)
                t.MaxTicks = spent;
        }
    }

    // SCore.Instance.EventManager holds a ManagedEvent<T> field per event, each with a
    // Handlers list of ManagedEventHandler<T> { Handler, SourceMod }; all internal to SMAPI
    private static IEnumerable<(string Event, string Mod, string ModId, MethodInfo Method)> Handlers()
    {
        Type core = Type.GetType("StardewModdingAPI.Framework.SCore, StardewModdingAPI", throwOnError: true)!;
        object instance = core.GetProperty("Instance", Any)!.GetValue(null)!;
        object events = core.GetField("EventManager", Any)!.GetValue(instance)!;
        foreach (FieldInfo field in events.GetType().GetFields(Any))
        {
            object? managed = field.GetValue(events);
            if (managed?.GetType().GetField("Handlers", Any)?.GetValue(managed) is not IEnumerable handlers)
                continue;
            foreach (object handler in handlers)
            {
                var callback = handler.GetType().GetProperty("Handler", Any)?.GetValue(handler) as Delegate;
                // SMAPI's mod metadata is internal, which dynamic can't see into; IModInfo is public
                var mod = handler.GetType().GetProperty("SourceMod", Any)?.GetValue(handler) as IModInfo;
                if (callback is null || mod is null)
                    continue;
                yield return (field.Name, mod.Manifest.Name, mod.Manifest.UniqueID, callback.Method);
            }
        }
    }

    /// <summary>Game methods more than one mod patches with Harmony, which is where they collide.</summary>
    public static PatchReport SharedPatches()
    {
        var shared = new List<PatchedMethod>();
        int patched = 0;
        foreach (MethodBase method in Harmony.GetAllPatchedMethods())
        {
            patched++;
            Patches? info = Harmony.GetPatchInfo(method);
            if (info is null)
                continue;
            var patches = Kinds(info)
                .Where(p => p.Patch.owner != HarmonyId)
                .Select(p => new PatchOwner(p.Patch.owner, p.Kind,
                    // a bool prefix can skip the original and every later prefix
                    p.Kind == "prefix" && p.Patch.PatchMethod.ReturnType == typeof(bool)))
                .ToList();
            if (patches.Select(p => p.Owner).Distinct(StringComparer.OrdinalIgnoreCase).Count() < 2)
                continue;
            shared.Add(new PatchedMethod($"{method.DeclaringType?.FullName}.{method.Name}", patches));
        }
        return new PatchReport(patched, shared.OrderBy(m => m.Method).ToList());
    }

    private static IEnumerable<(string Kind, Patch Patch)> Kinds(Patches info) =>
        info.Prefixes.Select(p => ("prefix", p))
            .Concat(info.Postfixes.Select(p => ("postfix", p)))
            .Concat(info.Transpilers.Select(p => ("transpiler", p)))
            .Concat(info.Finalizers.Select(p => ("finalizer", p)));
}

internal record ProfileResult(double Seconds, List<ProfileMod> Mods);

/// <summary>One mod's handler time over the measurement, in milliseconds.</summary>
internal record ProfileMod(string Mod, double TotalMs, double MaxMs, int Calls, List<ProfileEvent> Events);

internal record ProfileEvent(string Event, double TotalMs, int Calls);

/// <summary>A game method and every Harmony patch on it, by owner (usually a mod's UniqueID).</summary>
/// <summary>How many game methods carry Harmony patches, and those more than one mod patches.</summary>
internal record PatchReport(int Patched, List<PatchedMethod> Shared);

internal record PatchedMethod(string Method, List<PatchOwner> Patches);

internal record PatchOwner(string Owner, string Kind, bool CanSkip);
