using System;
using System.Reflection;
using StardewModdingAPI;

namespace PelicanCompanion;

/// <summary>Reads and pokes the running game through reflection.</summary>
/// <remarks>
/// The companion doesn't reference Stardew Valley.dll so it can build without a game
/// install; <c>dynamic</c> binds to the same public members a direct reference would.
/// Call only from the game thread.
/// </remarks>
internal static class Game
{
    private static readonly Type? Game1 = Type.GetType("StardewValley.Game1, Stardew Valley");
    private static readonly Type? HudMessage = Type.GetType("StardewValley.HUDMessage, Stardew Valley");

    /// <summary>The game version, such as "1.6.15".</summary>
    public static string Version =>
        Game1?.GetField("version", BindingFlags.Public | BindingFlags.Static)?.GetValue(null) as string ?? "";

    public static GameSnapshot? Snapshot()
    {
        if (!Context.IsWorldReady || Game1 is null)
            return null;
        dynamic player = Static("player")!;
        dynamic? location = Static("currentLocation");
        return new GameSnapshot(
            Farmer: (string)player.Name,
            Farm: (string)player.farmName.Value,
            Season: (string)Static("currentSeason")!,
            Day: (int)Static("dayOfMonth")!,
            Year: (int)Static("year")!,
            Time: (int)Static("timeOfDay")!,
            Money: (long)(int)player.Money,
            Location: location is null ? null : (string)location.Name
        );
    }

    /// <summary>Shows a message in the game's HUD, bottom left.</summary>
    public static void Notify(string text)
    {
        if (Game1 is null || HudMessage is null || !Context.IsWorldReady)
            return;
        object? message = Activator.CreateInstance(HudMessage, text);
        Game1.GetMethod("addHUDMessage", BindingFlags.Public | BindingFlags.Static)?.Invoke(null, new[] { message });
    }

    private static object? Static(string name)
    {
        const BindingFlags flags = BindingFlags.Public | BindingFlags.Static;
        return Game1!.GetField(name, flags)?.GetValue(null) ?? Game1.GetProperty(name, flags)?.GetValue(null);
    }
}
