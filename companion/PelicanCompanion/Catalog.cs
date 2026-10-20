using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

namespace PelicanCompanion;

/// <summary>Lists what perfection counts in this game, with mods, in the game's language.</summary>
/// <remarks>
/// The launcher checks saves offline against this; each list follows the matching
/// <c>Utility.get*Percent</c> in 1.6, so mod-added fish and recipes count the way the
/// game counts them. Call from the game thread after GameLaunched.
/// </remarks>
internal static class Catalog
{
    private const BindingFlags Static = BindingFlags.Public | BindingFlags.Static;

    private static Type? Find(string name) => Type.GetType($"{name}, Stardew Valley");

    public static CatalogMessage Build()
    {
        Type itemRegistry = Find("StardewValley.ItemRegistry")!;
        Type obj = Find("StardewValley.Object")!;
        Type recipe = Find("StardewValley.CraftingRecipe")!;
        Type game1 = Find("StardewValley.Game1")!;
        Type content = Find("StardewValley.LocalizedContentManager")!;

        // GetAllData() is an extension method dynamic can't bind; the definition's own
        // GetAllIds and GetData give the same ParsedItemData
        dynamic objects = itemRegistry.GetMethod("GetObjectTypeDefinition", Static)!.Invoke(null, null)!;
        MethodInfo basicShipped = obj.GetMethod("isPotentialBasicShipped", Static)!;
        MethodInfo donatable = Find("StardewValley.Locations.LibraryMuseum")!.GetMethod("IsItemSuitableForDonation", Static)!;
        var shipping = new List<CatalogItem>();
        var fish = new List<CatalogItem>();
        var items = new List<CatalogItem>();
        var museum = new List<CatalogMuseumItem>();
        foreach (string itemId in (IEnumerable<string>)objects.GetAllIds())
        {
            dynamic? data = objects.GetData(itemId);
            if (data is null)
                continue;
            string id = data.ItemId;
            int category = data.Category;
            string type = data.ObjectType;
            if (category != -7 && category != -2 && (bool)basicShipped.Invoke(null, new object[] { id, category, type })!)
                shipping.Add(new CatalogItem(id, data.DisplayName));
            if (type == "Fish" && !ExcludedFromFishing(data.RawData))
                fish.Add(new CatalogItem((string)data.QualifiedItemId, data.DisplayName));
            items.Add(new CatalogItem(id, data.DisplayName));
            if ((bool)donatable.Invoke(null, new object[] { id, false })!)
                museum.Add(new CatalogMuseumItem(id, data.DisplayName, type == "Arch" ? "artifact" : "mineral"));
        }

        return new CatalogMessage(
            Game: Game.Version,
            Language: content.GetProperty("CurrentLanguageCode", Static)?.GetValue(null)?.ToString() ?? "",
            Shipping: shipping,
            Fish: fish,
            Cooking: Recipes(recipe, "cookingRecipes", cooking: true),
            Crafting: Recipes(recipe, "craftingRecipes", cooking: false).Where(r => r.Key != "Wedding Ring").ToList(),
            Villagers: Villagers(game1),
            Monsters: Monsters(game1),
            Items: items,
            Museum: museum,
            Achievements: Achievements(game1),
            SpecialOrders: SpecialOrders(game1),
            Crops: Crops(game1),
            Festivals: Festivals(),
            FishInfo: FishInfo(fish),
            Terms: Terms(game1)
        );
    }

    private static bool ExcludedFromFishing(object? raw) =>
        raw?.GetType().GetField("ExcludeFromFishingCollection")?.GetValue(raw) is true;

    private static List<CatalogRecipe> Recipes(Type recipe, string field, bool cooking)
    {
        var all = (Dictionary<string, string>)recipe.GetField(field, Static)!.GetValue(null)!;
        var list = new List<CatalogRecipe>();
        foreach ((string key, string value) in all)
        {
            // field 2 is "<output id> [count]", the id recipesCooked is keyed by
            string[] fields = value.Split('/');
            string output = fields.Length > 2 ? fields[2].Split(' ', StringSplitOptions.RemoveEmptyEntries).FirstOrDefault() ?? "" : "";
            string name = key;
            try
            {
                dynamic instance = Activator.CreateInstance(recipe, key, cooking)!;
                name = instance.DisplayName ?? key;
            }
            catch (Exception)
            {
                // a broken mod recipe still counts toward perfection, it just lacks a nice name
            }
            list.Add(new CatalogRecipe(key, name, output));
        }
        return list;
    }

    private static List<CatalogVillager> Villagers(Type game1)
    {
        var data = (IDictionary)game1.GetField("characterData", Static)!.GetValue(null)!;
        var tastes = (IDictionary<string, string>)game1.GetField("NPCGiftTastes", Static)!.GetValue(null)!;
        MethodInfo immutablyFalse = Find("StardewValley.GameStateQuery")!.GetMethod("IsImmutablyFalse", Static)!;
        var list = new List<CatalogVillager>();
        foreach (DictionaryEntry entry in data)
        {
            dynamic c = entry.Value!;
            if (!(bool)c.PerfectionScore || (bool)immutablyFalse.Invoke(null, new object?[] { (string?)c.CanSocialize })!)
                continue;
            string key = (string)entry.Key;
            object? season = c.BirthSeason;
            list.Add(new CatalogVillager(
                key,
                Parse((string?)c.DisplayName) ?? key,
                (bool)c.CanBeRomanced,
                season?.ToString()?.ToLowerInvariant(),
                (int)c.BirthDay,
                Loves(tastes, key)
            ));
        }
        return list;
    }

    /// <summary>
    /// Names the launcher would otherwise translate itself, keyed "room.Pantry",
    /// "skill.farming", "building.Gold Clock", "location.Town".
    /// </summary>
    private static Dictionary<string, string> Terms(Type game1)
    {
        var terms = new Dictionary<string, string>();
        // a missing term only costs the launcher's own translation, never the whole catalog
        void Add(string key, Func<string?> name)
        {
            try
            {
                if (name() is { Length: > 0 } value)
                    terms[key] = value;
            }
            catch (Exception)
            {
            }
        }
        Type cc = Find("StardewValley.Locations.CommunityCenter")!;
        MethodInfo areaKey = cc.GetMethod("getAreaNameFromNumber", Static)!;
        MethodInfo areaName = cc.GetMethod("getAreaDisplayNameFromNumber", Static)!;
        // 0..5 are the rooms, 6 the Joja warehouse
        for (int area = 0; area <= 6; area++)
        {
            object[] arg = { area };
            string key = ((string?)areaKey.Invoke(null, arg) ?? "").Replace(" ", "");
            if (key != "")
                Add("room." + key, () => (string?)areaName.Invoke(null, arg));
        }
        MethodInfo skill = Find("StardewValley.Farmer")!.GetMethod("getSkillDisplayNameFromIndex", Static)!;
        string[] skills = { "farming", "fishing", "foraging", "mining", "combat" };
        for (int i = 0; i < skills.Length; i++)
        {
            object[] arg = { i };
            Add("skill." + skills[i], () => (string?)skill.Invoke(null, arg));
        }
        foreach (DictionaryEntry entry in (IDictionary)game1.GetField("buildingData", Static)!.GetValue(null)!)
            Add("building." + entry.Key, () => Parse((string?)((dynamic)entry.Value!).Name));
        foreach (DictionaryEntry entry in (IDictionary)DataLoader("Locations"))
            Add("location." + entry.Key, () => Parse((string?)((dynamic)entry.Value!).DisplayName));
        return terms;
    }

    private static List<CatalogGoal> Monsters(Type game1)
    {
        object content = game1.GetField("content", Static)!.GetValue(null)!;
        MethodInfo load = Find("StardewValley.DataLoader")!.GetMethod("MonsterSlayerQuests", Static)!;
        var quests = (IDictionary)load.Invoke(null, new[] { content })!;
        var list = new List<CatalogGoal>();
        foreach (DictionaryEntry entry in quests)
        {
            dynamic q = entry.Value!;
            var targets = ((IEnumerable<string>?)q.Targets)?.ToList() ?? new List<string>();
            string key = (string)entry.Key;
            list.Add(new CatalogGoal(key, Parse((string?)q.DisplayName) ?? key, (int)q.Count, targets));
        }
        return list;
    }

    // "name^description^shown on collections tab^prerequisite^hat"
    private static List<CatalogAchievement> Achievements(Type game1)
    {
        var all = (IDictionary)game1.GetField("achievements", Static)!.GetValue(null)!;
        var list = new List<CatalogAchievement>();
        foreach (DictionaryEntry entry in all)
        {
            string[] fields = ((string)entry.Value!).Split('^');
            list.Add(new CatalogAchievement((int)entry.Key, fields[0], fields.Length > 1 ? fields[1] : ""));
        }
        return list.OrderBy(a => a.Id).ToList();
    }

    private static List<CatalogOrder> SpecialOrders(Type game1)
    {
        object content = game1.GetField("content", Static)!.GetValue(null)!;
        MethodInfo load = Find("StardewValley.DataLoader")!.GetMethod("SpecialOrders", Static)!;
        var orders = (IDictionary)load.Invoke(null, new[] { content })!;
        // names are "[Willy_Name]" keys into Strings/SpecialOrderStrings, resolved the game's way
        MethodInfo localize = Find("StardewValley.SpecialOrders.SpecialOrder")!.GetMethod("MakeLocalizationReplacements", Static)!;
        var list = new List<CatalogOrder>();
        foreach (DictionaryEntry entry in orders)
        {
            dynamic o = entry.Value!;
            string key = (string)entry.Key;
            string name = (string?)o.Name is { } raw ? (string)localize.Invoke(null, new object[] { raw })! : key;
            list.Add(new CatalogOrder(key, name, (string?)o.Requester ?? "", (string?)o.OrderType == "Qi"));
        }
        return list;
    }

    private static List<CatalogCrop> Crops(Type game1)
    {
        var crops = (IEnumerable)(game1.GetField("cropData", Static)!.GetValue(null)!);
        var list = new List<CatalogCrop>();
        foreach (object raw in crops)
        {
            dynamic c = ((dynamic)raw).Value;
            if (!(bool)c.CountForPolyculture && !(bool)c.CountForMonoculture)
                continue;
            var seasons = new List<string>();
            foreach (object season in (IEnumerable)c.Seasons)
                seasons.Add(season.ToString()!.ToLowerInvariant());
            list.Add(new CatalogCrop(
                (string)c.HarvestItemId,
                (bool)c.CountForPolyculture,
                (bool)c.CountForMonoculture,
                (string)((dynamic)raw).Key,
                seasons
            ));
        }
        return list;
    }

    private static string? Parse(string? text)
    {
        if (text is null)
            return null;
        MethodInfo parse = Find("StardewValley.TokenizableStrings.TokenParser")!.GetMethod("ParseText", Static)!;
        object?[] args = parse.GetParameters().Select((p, i) => i == 0 ? text : Type.Missing).ToArray();
        return parse.Invoke(null, BindingFlags.OptionalParamBinding, null, args, null) as string;
    }

    // NPCGiftTastes: "love text/love ids/like text/like ids/…"; negative ids are categories
    private static List<string> Loves(IDictionary<string, string> tastes, string npc)
    {
        if (!tastes.TryGetValue(npc, out string? raw))
            return new List<string>();
        string[] fields = raw.Split('/');
        return fields.Length > 1
            ? fields[1].Split(' ', StringSplitOptions.RemoveEmptyEntries).Where(id => !id.StartsWith('-')).ToList()
            : new List<string>();
    }

    private static object DataLoader(string method)
    {
        object content = Find("StardewValley.Game1")!.GetField("content", Static)!.GetValue(null)!;
        return Find("StardewValley.DataLoader")!.GetMethod(method, Static)!.Invoke(null, new[] { content })!;
    }

    private static List<CatalogFestival> Festivals()
    {
        var list = new List<CatalogFestival>();
        // "spring13" → "Egg Festival"
        foreach ((string key, string name) in (Dictionary<string, string>)DataLoader("Festivals_FestivalDates"))
        {
            int digits = key.IndexOfAny("0123456789".ToCharArray());
            if (digits > 0 && int.TryParse(key[digits..], out int day))
                list.Add(new CatalogFestival(key[..digits], day, day, Parse(name) ?? name));
        }
        foreach (DictionaryEntry entry in (IDictionary)DataLoader("PassiveFestivals"))
        {
            dynamic f = entry.Value!;
            if (!(bool)f.ShowOnCalendar)
                continue;
            list.Add(new CatalogFestival(
                ((object)f.Season).ToString()!.ToLowerInvariant(),
                (int)f.StartDay,
                (int)f.EndDay,
                Parse((string?)f.DisplayName) ?? (string)entry.Key
            ));
        }
        return list;
    }

    // Data/Fish: "name/difficulty/behavior/min/max/times/seasons/weather/…", or
    // "name/trap/…" for crab pot catches; where they bite comes from Data/Locations
    private static List<CatalogFish> FishInfo(List<CatalogItem> fish)
    {
        var where = new Dictionary<string, SortedSet<string>>();
        foreach (DictionaryEntry entry in (IDictionary)DataLoader("Locations"))
        {
            string key = (string)entry.Key;
            if (key == "Default")
                continue;
            dynamic loc = entry.Value!;
            string name = Parse((string?)loc.DisplayName) ?? key;
            if (loc.Fish is not IEnumerable spawns)
                continue;
            foreach (dynamic spawn in spawns)
            {
                string? id = spawn.ItemId;
                if (id is null)
                    continue;
                if (!where.TryGetValue(id, out SortedSet<string>? set))
                    where[id] = set = new SortedSet<string>();
                set.Add(name);
            }
        }
        var data = (Dictionary<string, string>)DataLoader("Fish");
        var list = new List<CatalogFish>();
        foreach (CatalogItem f in fish)
        {
            string id = f.Id.StartsWith("(O)") ? f.Id[3..] : f.Id;
            if (!data.TryGetValue(id, out string? raw))
                continue;
            string[] fields = raw.Split('/');
            bool trap = fields.Length > 1 && fields[1] == "trap";
            var locations = where.TryGetValue(f.Id, out SortedSet<string>? set) ? set.ToList() : new List<string>();
            if (trap)
            {
                list.Add(new CatalogFish(f.Id, f.Name, new List<string>(), "any", new List<int[]>(), locations, true));
                continue;
            }
            if (fields.Length < 8)
                continue;
            int[] clock = fields[5].Split(' ', StringSplitOptions.RemoveEmptyEntries).Select(int.Parse).ToArray();
            var times = new List<int[]>();
            for (int i = 0; i + 1 < clock.Length; i += 2)
                times.Add(new[] { clock[i], clock[i + 1] });
            var seasons = fields[6].Split(' ', StringSplitOptions.RemoveEmptyEntries).Select(s => s.ToLowerInvariant()).ToList();
            list.Add(new CatalogFish(f.Id, f.Name, seasons, fields[7], times, locations, false));
        }
        return list;
    }
}
