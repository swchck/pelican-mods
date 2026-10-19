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
        var shipping = new List<CatalogItem>();
        var fish = new List<CatalogItem>();
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
        }

        return new CatalogMessage(
            Game: Game.Version,
            Language: content.GetProperty("CurrentLanguageCode", Static)?.GetValue(null)?.ToString() ?? "",
            Shipping: shipping,
            Fish: fish,
            Cooking: Recipes(recipe, "cookingRecipes", cooking: true),
            Crafting: Recipes(recipe, "craftingRecipes", cooking: false).Where(r => r.Key != "Wedding Ring").ToList(),
            Villagers: Villagers(game1),
            Monsters: Monsters(game1)
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
        MethodInfo immutablyFalse = Find("StardewValley.GameStateQuery")!.GetMethod("IsImmutablyFalse", Static)!;
        var list = new List<CatalogVillager>();
        foreach (DictionaryEntry entry in data)
        {
            dynamic c = entry.Value!;
            if (!(bool)c.PerfectionScore || (bool)immutablyFalse.Invoke(null, new object?[] { (string?)c.CanSocialize })!)
                continue;
            string key = (string)entry.Key;
            list.Add(new CatalogVillager(key, Parse((string?)c.DisplayName) ?? key, (bool)c.CanBeRomanced));
        }
        return list;
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

    private static string? Parse(string? text)
    {
        if (text is null)
            return null;
        MethodInfo parse = Find("StardewValley.TokenizableStrings.TokenParser")!.GetMethod("ParseText", Static)!;
        object?[] args = parse.GetParameters().Select((p, i) => i == 0 ? text : Type.Missing).ToArray();
        return parse.Invoke(null, BindingFlags.OptionalParamBinding, null, args, null) as string;
    }
}
