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
            mysteryBoxesOpened = (uint)stats.Get("MysteryBoxesOpened"),
            mysteryBoxes = PredictMysteryBoxes(player, stats, count * 2),
            nights = PredictNights(game1, player, stats, nights: 28),
            bookseller = Safe(() => (object?)((IEnumerable<int>)Find("StardewValley.Utility").GetMethod("getDaysOfBooksellerThisSeason", Static)!.Invoke(null, null)!).ToList()),
            cart = Safe(() => CartToday(game1)),
        };
    }

    // same stepping as geodes, on "MysteryBoxesOpened". The book branch adds mail the first
    // time it rolls, so that flag is put back as well
    private static List<object> PredictMysteryBoxes(dynamic player, dynamic stats, int count)
    {
        MethodInfo treasure = Find("StardewValley.Utility").GetMethod("getTreasureFromGeode", Static)!;
        MethodInfo create = Find("StardewValley.ItemRegistry").GetMethods(Static)
            .First(m => m.Name == "Create" && !m.IsGenericMethod && m.GetParameters()[0].ParameterType == typeof(string));
        uint opened = stats.Get("MysteryBoxesOpened");
        bool hadBook = player.mailReceived.Contains("GotMysteryBook");
        var result = new List<object>();
        try
        {
            foreach (string id in new[] { "(O)MysteryBox", "(O)GoldenMysteryBox" })
            {
                dynamic box = create.Invoke(null, CreateArgs(create, id))!;
                var next = new List<object>();
                for (uint i = 1; i <= count; i++)
                {
                    stats.Set("MysteryBoxesOpened", opened + i);
                    dynamic? item = treasure.Invoke(null, new object[] { box });
                    if (!hadBook)
                        player.mailReceived.Remove("GotMysteryBook");
                    if (item is not null)
                        next.Add(new { id = (string)item.QualifiedItemId, name = (string)item.DisplayName, stack = (int)item.Stack });
                }
                result.Add(new { id, name = (string)box.DisplayName, next });
            }
        }
        finally
        {
            stats.Set("MysteryBoxesOpened", opened);
            if (!hadBook)
                player.mailReceived.Remove("GotMysteryBook");
        }
        return result;
    }

    /// <summary>The random part of <c>Utility.pickFarmEvent</c> for the coming nights.</summary>
    /// <remarks>
    /// The game rolls at the start of the next day, so each night uses the next morning's
    /// DaysPlayed and date. Calling pickFarmEvent itself isn't an option: it reads today's
    /// state and resets the fairy rose flag. Scripted nights (weddings, Community Center
    /// rooms finishing) take precedence in the game and aren't predicted.
    /// </remarks>
    private static List<object> PredictNights(Type game1, dynamic player, dynamic stats, int nights)
    {
        MethodInfo createRandom = Find("StardewValley.Utility").GetMethod("CreateRandom", Static)!;
        dynamic master = Member(game1, "MasterPlayer")!;
        uint days = stats.DaysPlayed;
        ulong id = (ulong)Member(game1, "uniqueIDForThisGame")!;
        int day = (int)Member(game1, "dayOfMonth")!;
        int season = (int)Member(game1, "seasonIndex")!;
        int year = (int)Member(game1, "year")!;
        bool pantry = player.mailReceived.Contains("ccPantry");
        bool treeFallen = master.mailReceived.Contains("raccoonTreeFallen");
        bool capsuleSeen = master.mailReceived.Contains("Got_Capsule");
        bool qiPlaneSeen = player.mailReceived.Contains("sawQiPlane");
        bool billboardTicket = player.mailReceived.Contains("gotFirstBillboardPrizeTicket");
        bool rose = Safe(() => (bool)((dynamic)game1.GetMethod("getFarm", Static)!.Invoke(null, null)!).hasMatureFairyRoseTonight);
        var result = new List<object>();
        for (int k = 0; k < nights; k++)
        {
            if (++day > 28)
            {
                day = 1;
                if (++season > 3)
                {
                    season = 0;
                    year++;
                }
            }
            uint daysPlayed = days + (uint)k + 1;
            var random = (Random)createRandom.Invoke(null, new object[] { (double)daysPlayed, (double)(id / 2), 0.0, 0.0, 0.0 })!;
            for (int i = 0; i < 10; i++)
                random.NextDouble();
            string? ev = null;
            if (daysPlayed == 31)
                ev = "earthquake";
            else if (pantry && random.NextDouble() < 0.1 && !treeFallen)
            {
                ev = "raccoonTree";
                treeFallen = true;
            }
            else if (!qiPlaneSeen && (billboardTicket || daysPlayed > 50))
            {
                ev = "qiPlane";
                qiPlaneSeen = true;
            }
            else if (random.NextDouble() < 0.01 + (k == 0 && rose ? 0.007 : 0) && season != 3 && day != 1)
                ev = "fairy";
            else if (random.NextDouble() < 0.01 && daysPlayed > 20)
                ev = "witch";
            else if (random.NextDouble() < 0.01 && daysPlayed > 5)
                ev = "meteor";
            else if (random.NextDouble() < 0.005)
                ev = "owl";
            else if (random.NextDouble() < 0.008 && year > 1 && !capsuleSeen)
            {
                ev = "capsule";
                capsuleSeen = true;
            }
            if (ev is not null)
                result.Add(new { day, season, year, @event = ev });
        }
        return result;
    }

    // the cart's stock depends on the day in too many places to forecast; on a cart day
    // the game's own shop builder gives today's exactly
    private static object? CartToday(Type game1)
    {
        int day = (int)Member(game1, "dayOfMonth")!;
        if (day % 7 != 0 && day % 7 != 5)
            return null;
        MethodInfo stock = Find("StardewValley.Internal.ShopBuilder").GetMethods(Static)
            .First(m => m.Name == "GetShopStock" && m.GetParameters().Length == 1);
        var items = new List<object>();
        foreach (DictionaryEntry entry in (IDictionary)stock.Invoke(null, new object[] { "Traveler" })!)
        {
            dynamic item = entry.Key;
            dynamic info = entry.Value!;
            items.Add(new { id = (string)item.QualifiedItemId, name = (string)item.DisplayName, price = (int)info.Price, stock = (int)info.Stock });
        }
        return items;
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
