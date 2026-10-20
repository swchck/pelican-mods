using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using StardewModdingAPI;

namespace PelicanCompanion;

/// <summary>A few HUD lines at the start of each day: what's on today and worth a detour.</summary>
/// <remarks>
/// Birthdays with loved gifts the player already owns, a festival, the traveling cart, the
/// bookseller and the day's luck. The launcher turns it off with
/// <c>PELICAN_BRIEFING=0</c> in the game's environment. Call from the game thread.
/// </remarks>
internal static class Briefing
{
    private const BindingFlags Static = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static;
    /// <summary>Loved gifts named per birthday; the HUD line runs out of room past that.</summary>
    private const int GiftsShown = 3;

    public static bool Enabled { get; } = Environment.GetEnvironmentVariable("PELICAN_BRIEFING") != "0";

    private static Type Find(string name) => Type.GetType($"StardewValley.{name}, Stardew Valley", throwOnError: true)!;

    /// <summary>Today's lines, in the game's language.</summary>
    public static List<string> Lines(ITranslationHelper i18n)
    {
        Type game1 = Find("Game1");
        object? Member(string name) =>
            game1.GetField(name, Static)?.GetValue(null) ?? game1.GetProperty(name, Static)?.GetValue(null);
        string season = Member("currentSeason")?.ToString()?.ToLowerInvariant() ?? "";
        int day = (int)Member("dayOfMonth")!;
        var lines = new List<string>();

        foreach (CatalogFestival f in Catalog.Festivals().Where(f => f.Season == season && f.Day <= day && day <= f.EndDay))
            lines.Add(i18n.Get("briefing.festival", new { name = f.Name }));

        var owned = new HashSet<string>(Helpers.Items().Select(i => (string)((dynamic)i).id));
        foreach (CatalogVillager v in Catalog.Villagers(game1).Where(v => v.BirthSeason == season && v.BirthDay == day))
        {
            // what's already in a chest or the backpack beats what still has to be found
            var have = v.Loves.Where(id => owned.Contains(Qualified(id))).Take(GiftsShown).Select(Name).ToList();
            lines.Add(have.Count > 0
                ? i18n.Get("briefing.birthdayHave", new { name = v.Name, gifts = string.Join(", ", have) })
                : i18n.Get("briefing.birthday", new { name = v.Name, gifts = string.Join(", ", v.Loves.Take(GiftsShown).Select(Name)) }));
        }

        if (day % 7 == 0 || day % 7 == 5)
            lines.Add(i18n.Get("briefing.cart"));
        var bookseller = Find("Utility").GetMethod("getDaysOfBooksellerThisSeason", Static)?.Invoke(null, null) as IEnumerable<int>;
        if (bookseller?.Contains(day) == true)
            lines.Add(i18n.Get("briefing.bookseller"));

        double luck = (double)((dynamic)Member("player")!).DailyLuck;
        // the TV fortune teller's bands, from TV.getFortuneForecast
        string? mood = luck switch
        {
            > 0.07 => "great",
            > 0.02 => "good",
            < -0.07 => "awful",
            < -0.02 => "bad",
            _ => null,
        };
        if (mood is not null)
            lines.Add(i18n.Get($"briefing.luck.{mood}"));
        return lines;
    }

    private static string Qualified(string id) => id.StartsWith('(') ? id : $"(O){id}";

    private static string Name(string id)
    {
        MethodInfo data = Find("ItemRegistry").GetMethod("GetDataOrErrorItem", Static)!;
        return (string)((dynamic)data.Invoke(null, new object[] { Qualified(id) })!).DisplayName;
    }
}
