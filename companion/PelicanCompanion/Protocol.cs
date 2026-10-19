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
    string? Location
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

internal record CatalogMessage(
    string Game,
    string Language,
    List<CatalogItem> Shipping,
    List<CatalogItem> Fish,
    List<CatalogRecipe> Cooking,
    List<CatalogRecipe> Crafting,
    List<CatalogVillager> Villagers,
    List<CatalogGoal> Monsters
)
{
    public string Type => "catalog";
}

/// <summary>An item; shipping ids are unqualified ("24"), fish ids qualified ("(O)128").</summary>
internal record CatalogItem(string Id, string Name);

/// <summary>A recipe by its data key, with the id of the item it makes.</summary>
internal record CatalogRecipe(string Key, string Name, string Output);

internal record CatalogVillager(string Key, string Name, bool Romance);

/// <summary>A Monster Slayer goal: <see cref="Count"/> kills among <see cref="Targets"/>.</summary>
internal record CatalogGoal(string Key, string Name, int Count, List<string> Targets);

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
