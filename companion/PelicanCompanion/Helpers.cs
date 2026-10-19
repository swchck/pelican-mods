using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using StardewModdingAPI;

namespace PelicanCompanion;

/// <summary>Answers the launcher's helper pages from the loaded save, with the game's own code.</summary>
/// <remarks>Call from the game thread with a world loaded.</remarks>
internal static class Helpers
{
    private const BindingFlags Static = BindingFlags.Public | BindingFlags.Static;

    /// <summary>Geodes Clint can open, in the order the launcher shows them.</summary>
    private static readonly string[] Geodes = { "(O)535", "(O)536", "(O)537", "(O)749", "(O)275", "(O)791" };

    private static Type Find(string name) => Type.GetType($"{name}, Stardew Valley", throwOnError: true)!;

    public static object Forecast(int count)
    {
        Type game1 = Find("StardewValley.Game1");
        dynamic player = Member(game1, "player")!;
        dynamic stats = Member(game1, "stats")!;
        object? island = Safe(() =>
        {
            dynamic world = ((dynamic)Member(game1, "netWorldState")!).Value;
            return (object?)(string?)world.GetWeatherForLocation("Island").WeatherForTomorrow;
        });
        return new
        {
            luck = (double)player.DailyLuck,
            weatherTomorrow = (string?)Member(game1, "weatherForTomorrow"),
            islandWeatherTomorrow = island,
            geodesCracked = (uint)stats.GeodesCracked,
            geodes = PredictGeodes(stats, count),
        };
    }

    // getTreasureFromGeode seeds its RNG from GeodesCracked, which the geode menu bumps
    // before asking; set it to each future count, ask, and put the real value back
    private static List<object> PredictGeodes(dynamic stats, int count)
    {
        MethodInfo treasure = Find("StardewValley.Utility").GetMethod("getTreasureFromGeode", Static)!;
        MethodInfo create = Find("StardewValley.ItemRegistry").GetMethods(Static)
            .First(m => m.Name == "Create" && !m.IsGenericMethod && m.GetParameters()[0].ParameterType == typeof(string));
        uint cracked = stats.GeodesCracked;
        var result = new List<object>();
        try
        {
            foreach (string id in Geodes)
            {
                dynamic geode = create.Invoke(null, CreateArgs(create, id))!;
                var next = new List<object>();
                for (uint i = 1; i <= count; i++)
                {
                    stats.GeodesCracked = cracked + i;
                    dynamic? item = treasure.Invoke(null, new object[] { geode });
                    if (item is not null)
                        next.Add(new { id = (string)item.QualifiedItemId, name = (string)item.DisplayName, stack = (int)item.Stack });
                }
                result.Add(new { id, name = (string)geode.DisplayName, next });
            }
        }
        finally
        {
            stats.GeodesCracked = cracked;
        }
        return result;
    }

    /// <summary>Every object the player has on hand or in a chest or fridge, for the grange.</summary>
    public static List<object> Items()
    {
        Type game1 = Find("StardewValley.Game1");
        Type obj = Find("StardewValley.Object");
        Type chest = Find("StardewValley.Objects.Chest");
        var items = new List<object>();

        void Add(IEnumerable? source, string where)
        {
            if (source is null)
                return;
            foreach (object? raw in source)
            {
                if (raw is null || !obj.IsInstanceOfType(raw) || chest.IsInstanceOfType(raw))
                    continue;
                dynamic o = raw;
                items.Add(new
                {
                    id = (string)o.QualifiedItemId,
                    name = (string)o.DisplayName,
                    category = (int)o.Category,
                    quality = (int)o.Quality,
                    price = (int)o.sellToStorePrice(-1L),
                    stack = (int)o.Stack,
                    where,
                });
            }
        }

        dynamic player = Member(game1, "player")!;
        Add((IEnumerable)player.Items, "inventory");
        var locations = new List<object>();
        // Func<object, bool> passes for Func<GameLocation, bool> through delegate variance
        Func<object, bool> collect = l =>
        {
            locations.Add(l);
            return true;
        };
        Find("StardewValley.Utility").GetMethod("ForEachLocation", Static)!.Invoke(null, new object[] { collect, true, false });
        foreach (dynamic location in locations)
        {
            string name = location.DisplayName ?? location.Name;
            foreach (object placed in (IEnumerable)location.objects.Values)
            {
                if (chest.IsInstanceOfType(placed))
                    Add((IEnumerable)((dynamic)placed).Items, name);
            }
            // the farmhouse fridge is a chest that isn't placed anywhere
            object? fridge = Safe(() => (object?)location.fridge?.Value);
            if (fridge is not null)
                Add((IEnumerable)((dynamic)fridge).Items, name);
        }
        return items;
    }

    // Game1 mixes static fields and properties, and 1.6 moved some from one to the other
    private static object? Member(Type type, string name) =>
        type.GetField(name, Static)?.GetValue(null) ?? type.GetProperty(name, Static)?.GetValue(null);

    private static object?[] CreateArgs(MethodInfo create, string id) =>
        create.GetParameters().Select((p, i) => i == 0 ? id : p.HasDefaultValue ? p.DefaultValue : Type.Missing).ToArray();

    private static T? Safe<T>(Func<T?> f)
    {
        try
        {
            return f();
        }
        catch (Exception)
        {
            return default;
        }
    }
}
