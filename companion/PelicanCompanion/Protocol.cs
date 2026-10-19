using System.Collections.Generic;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using Newtonsoft.Json.Serialization;

namespace PelicanCompanion;

/// <summary>Messages exchanged with the launcher, one JSON object per WebSocket text frame.</summary>
/// <remarks>Changing a shape needs the matching change in src-tauri/src/companion.rs.</remarks>
internal static class Protocol
{
    public const int Version = 1;

    public static readonly JsonSerializerSettings Json = new()
    {
        ContractResolver = new CamelCasePropertyNamesContractResolver(),
        NullValueHandling = NullValueHandling.Ignore,
    };

    public static string Write(object message) => JsonConvert.SerializeObject(message, Json);
}

internal record Hello(string Smapi, string Game, List<string> Mods)
{
    public string Type => "hello";
    public int Protocol => PelicanCompanion.Protocol.Version;
}

internal record StateMessage(GameSnapshot? State)
{
    public string Type => "state";
}

/// <summary>What the player is doing right now; null while on the title screen.</summary>
internal record GameSnapshot(
    string Farmer,
    string Farm,
    string Season,
    int Day,
    int Year,
    int Time,
    long Money,
    string? Location,
    /// <summary>"sun", "rain", "storm" or "snow".</summary>
    string Weather
);

internal record GmcmMessage(List<GmcmMod> Mods)
{
    public string Type => "gmcm";
}

internal record GmcmMod(string Id, bool TitleScreenOnly, List<GmcmOption> Options);

/// <summary>One editable GMCM option, tied to its config.json key when the getter reveals it.</summary>
internal record GmcmOption(
    string[]? Path,
    string Name,
    string? Tooltip,
    string Kind,
    string? Section,
    double? Min,
    double? Max,
    double? Interval,
    List<GmcmChoice>? Choices
);

internal record GmcmChoice(string Value, string Label);

/// <summary>Each C# mod's config as a fresh instance writes it, by UniqueID.</summary>
internal record ConfigDefaultsMessage(Dictionary<string, JToken> Mods)
{
    public string Type => "configDefaults";
}

internal record CatalogMessage(
    string Game,
    string Language,
    List<CatalogItem> Shipping,
    List<CatalogItem> Fish,
    List<CatalogRecipe> Cooking,
    List<CatalogRecipe> Crafting,
    List<CatalogVillager> Villagers,
    List<CatalogGoal> Monsters,
    List<CatalogItem> Items,
    List<CatalogMuseumItem> Museum,
    List<CatalogAchievement> Achievements,
    List<CatalogOrder> SpecialOrders,
    List<CatalogCrop> Crops,
    List<CatalogFestival> Festivals,
    List<CatalogFish> FishInfo
)
{
    public string Type => "catalog";
}

/// <summary>An item; shipping ids are unqualified ("24"), fish ids qualified ("(O)128").</summary>
internal record CatalogItem(string Id, string Name);

/// <summary>A recipe by its data key, with the id of the item it makes.</summary>
internal record CatalogRecipe(string Key, string Name, string Output);

/// <summary>Something the museum takes; <see cref="Kind"/> is "artifact" or "mineral".</summary>
internal record CatalogMuseumItem(string Id, string Name, string Kind);

internal record CatalogAchievement(int Id, string Name, string Description);

/// <summary>A special order; Qi's walnut-room orders have <see cref="Qi"/> set.</summary>
internal record CatalogOrder(string Key, string Name, string Requester, bool Qi);

/// <summary>A crop the Polyculture or Monoculture achievements count, by harvest id.</summary>
internal record CatalogCrop(string Id, bool Polyculture, bool Monoculture, string Seed, List<string> Seasons);

/// <summary>A festival or a passive one (Night Market) over <c>Day</c>..<c>EndDay</c>.</summary>
internal record CatalogFestival(string Season, int Day, int EndDay, string Name);

/// <summary>When and where a fish bites; crab pot catches have <c>Trap</c> and no clock.</summary>
internal record CatalogFish(
    string Id,
    string Name,
    List<string> Seasons,
    string Weather,
    List<int[]> Times,
    List<string> Locations,
    bool Trap
);

internal record CatalogVillager(
    string Key,
    string Name,
    bool Romance,
    string? BirthSeason,
    int BirthDay,
    List<string> Loves
);

/// <summary>A Monster Slayer goal: <see cref="Count"/> kills among <see cref="Targets"/>.</summary>
internal record CatalogGoal(string Key, string Name, int Count, List<string> Targets);

/// <summary>The answer to a launcher request that asked for data.</summary>
internal record Reply(long Id, object? Data, string? Error = null)
{
    public string Type => "reply";
}

internal record Result(long Id, bool Ok, string? Method = null, string? Error = null)
{
    public string Type => "result";
}

/// <summary>A launcher request; <see cref="Kind"/> picks which other fields are set.</summary>
internal class Command
{
    [JsonProperty("type")]
    public string Kind { get; set; } = "";
    public long Id { get; set; }
    public string? ModId { get; set; }
    public JObject? Config { get; set; }
    public string? Text { get; set; }
}
