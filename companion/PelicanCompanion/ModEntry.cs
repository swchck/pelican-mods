using System;
using System.IO;
using System.Linq;
using System.Reflection;
using Newtonsoft.Json.Linq;
using StardewModdingAPI;
using StardewModdingAPI.Events;

namespace PelicanCompanion;

/// <summary>Links a running game to the Pelican Mods launcher.</summary>
/// <remarks>
/// The launcher passes <c>PELICAN_COMPANION_URL</c> when it starts SMAPI; without it (the
/// game was started some other way) the companion stays idle.
/// </remarks>
internal sealed class ModEntry : Mod
{
    private const string UrlVariable = "PELICAN_COMPANION_URL";

    // the launcher's tests have no save to load; the title screen's placeholder player
    // is enough to check that the reflection still binds
    private static readonly bool TestMode = Environment.GetEnvironmentVariable("PELICAN_COMPANION_TEST") == "1";

    private Bridge? Bridge;
    private Gmcm? Gmcm;
    private GameSnapshot? LastState;
    private volatile bool Resend;
    private bool Launched;
    private bool LocaleDirty;
    /// <summary>Ticks until today's briefing shows; 0 when there's none to show.</summary>
    private int BriefingIn;
    private CatalogMessage? CachedCatalog;
    private ConfigDefaultsMessage? CachedDefaults;
    private string? CachedLanguage;

    public override void Entry(IModHelper helper)
    {
        string? url = Environment.GetEnvironmentVariable(UrlVariable);
        if (string.IsNullOrEmpty(url) || !Uri.TryCreate(url, UriKind.Absolute, out Uri? uri))
            return;
        this.Gmcm = new Gmcm(helper.ModRegistry, this.ReadModConfig);
        this.Bridge = new Bridge(uri, msg => this.Monitor.Log(msg, LogLevel.Warn));
        this.Bridge.Connected += () => this.Resend = true;
        this.Bridge.Start();
        helper.Events.GameLoop.GameLaunched += (_, _) => this.Launched = true;
        helper.Events.GameLoop.UpdateTicked += this.OnUpdateTicked;
        helper.Events.GameLoop.ReturnedToTitle += (_, _) => this.SendState(force: true);
        // the HUD drops messages during the fade-in; two seconds in they're seen
        helper.Events.GameLoop.DayStarted += (_, _) => this.BriefingIn = Briefing.Enabled ? 120 : 0;
        // the game applies the player's language after GameLaunched; names follow it. The
        // event fires before the game reloads its achievements, so the catalog waits a tick
        helper.Events.Content.LocaleChanged += (_, _) => this.LocaleDirty = true;
    }

    protected override void Dispose(bool disposing)
    {
        this.Bridge?.Dispose();
        base.Dispose(disposing);
    }

    private void OnUpdateTicked(object? sender, UpdateTickedEventArgs e)
    {
        Bridge bridge = this.Bridge!;
        if (this.BriefingIn > 0 && --this.BriefingIn == 0)
        {
            try
            {
                foreach (string line in Briefing.Lines(this.Helper.Translation))
                    Game.Notify(line);
            }
            catch (Exception ex)
            {
                this.Monitor.Log($"couldn't put together today's briefing: {ex}", LogLevel.Trace);
            }
        }
        if (this.LocaleDirty && this.Launched)
        {
            this.LocaleDirty = false;
            this.SendCatalog();
        }
        // GMCM menus are registered during GameLaunched, so describe them after it
        if (this.Resend && this.Launched)
        {
            this.Resend = false;
            try
            {
                bridge.Send(new Hello(
                    Constants.ApiVersion.ToString(),
                    Game.Version,
                    this.Helper.ModRegistry.GetAll().Select(m => m.Manifest.UniqueID).ToList()
                ));
            }
            catch (Exception ex)
            {
                this.Monitor.Log($"couldn't greet the launcher: {ex}", LogLevel.Warn);
            }
            this.Describe();
            this.SendCatalog();
            this.SendConfigDefaults();
            this.SendState(force: true);
        }
        while (bridge.Inbox.TryDequeue(out Command? command))
        {
            try
            {
                this.Handle(command);
            }
            catch (Exception ex)
            {
                this.Monitor.Log($"couldn't handle the launcher's '{command.Kind}': {ex}", LogLevel.Warn);
            }
        }
        // once a second is plenty: the clock moves every 10 in-game minutes (~7s)
        if (e.IsMultipleOf(60))
            this.SendState(force: false);
    }

    private void Handle(Command command)
    {
        switch (command.Kind)
        {
            case "applyConfig" when command.ModId is not null && command.Config is not null:
                this.Bridge!.Send(this.ApplyConfig(command.Id, command.ModId, command.Config));
                break;
            case "notify" when command.Text is not null:
                Game.Notify(command.Text);
                break;
            case "describe":
                this.Describe();
                break;
            case "forecast":
                this.Answer(command.Id, () => Helpers.Forecast(count: 10));
                break;
            case "items":
                this.Answer(command.Id, Helpers.Items);
                break;
            // timing and patches don't need a farm: the title screen runs handlers too
            case "profileStart":
                this.Answer(command.Id, () => Profiler.Start(this.ModManifest.UniqueID), needsWorld: false);
                break;
            case "profileStop":
                this.Answer(command.Id, Profiler.Stop, needsWorld: false);
                break;
            case "briefing":
                this.Answer(command.Id, () => Briefing.Lines(this.Helper.Translation));
                break;
            case "patches":
                this.Answer(command.Id, Profiler.SharedPatches, needsWorld: false);
                break;
            // tests only: opens an existing save for the forecast checks; nothing is written
            // unless a day ends, and the tests stop the game long before that
            case "testLoad" when TestMode && command.Text is not null && !Context.IsWorldReady:
                // as LoadGameMenu's slot does: start loading, then close the title menu
                Type.GetType("StardewValley.SaveGame, Stardew Valley", throwOnError: true)!
                    .GetMethod("Load", new[] { typeof(string) })!
                    .Invoke(null, new object[] { command.Text });
                Type.GetType("StardewValley.Game1, Stardew Valley", throwOnError: true)!
                    .GetMethod("exitActiveMenu", BindingFlags.Public | BindingFlags.Static)!
                    .Invoke(null, null);
                break;
            // tests only: switches the language so the launcher can record each language's
            // base game lists; LocaleChanged then sends the catalog again
            case "testLocale" when TestMode && command.Text is not null:
                Type content = Type.GetType("StardewValley.LocalizedContentManager, Stardew Valley", throwOnError: true)!;
                content.GetProperty("CurrentLanguageCode", BindingFlags.Public | BindingFlags.Static)!
                    .SetValue(null, Enum.Parse(content.GetNestedType("LanguageCode")!, command.Text));
                break;
        }
    }

    private void Answer(long id, Func<object> build, bool needsWorld = true)
    {
        if (needsWorld && !Context.IsWorldReady && !TestMode)
        {
            this.Bridge!.Send(new Reply(id, null, Error: "no_world"));
            return;
        }
        try
        {
            this.Bridge!.Send(new Reply(id, build()));
        }
        catch (Exception ex)
        {
            this.Monitor.Log($"couldn't answer the launcher: {ex}", LogLevel.Warn);
            this.Bridge!.Send(new Reply(id, null, Error: ex.GetBaseException().Message));
        }
    }

    private Result ApplyConfig(long id, string modId, JObject config)
    {
        try
        {
            bool reloaded = LiveConfig.Reload(this.Helper.ModRegistry, modId, config);
            bool gmcm = this.Gmcm!.Apply(modId, config, reloaded);
            if (!reloaded && !gmcm)
                return new Result(id, false, Error: "unsupported");
            this.Monitor.Log($"applied new settings for {modId} ({(gmcm ? "gmcm" : "reload")})", LogLevel.Info);
            return new Result(id, true, Method: gmcm ? "gmcm" : "reload");
        }
        catch (Exception ex)
        {
            this.Monitor.Log($"couldn't apply settings for {modId}: {ex}", LogLevel.Warn);
            return new Result(id, false, Error: ex.GetBaseException().Message);
        }
    }

    private void Describe()
    {
        try
        {
            this.Bridge!.Send(new GmcmMessage(this.Gmcm!.Describe()));
        }
        catch (Exception ex)
        {
            this.Monitor.Log($"couldn't read GMCM menus: {ex}", LogLevel.Trace);
        }
    }

    private void SendConfigDefaults()
    {
        try
        {
            // the mods' assemblies don't change during a session
            this.CachedDefaults ??= new ConfigDefaultsMessage(ConfigDefaults.Collect(this.Helper, this.Monitor));
            this.Bridge!.Send(this.CachedDefaults);
        }
        catch (Exception ex)
        {
            this.Monitor.Log($"couldn't work out config defaults: {ex}", LogLevel.Trace);
        }
    }

    private void SendCatalog()
    {
        try
        {
            // building walks every item and recipe; a reconnect in the same language reuses it
            string language = this.Helper.Translation.Locale;
            if (this.CachedCatalog is null || this.CachedLanguage != language)
            {
                this.CachedCatalog = Catalog.Build();
                this.CachedLanguage = language;
            }
            this.Bridge!.Send(this.CachedCatalog);
        }
        catch (Exception ex)
        {
            this.Monitor.Log($"couldn't list perfection items: {ex}", LogLevel.Trace);
        }
    }

    private void SendState(bool force)
    {
        GameSnapshot? state;
        try
        {
            state = Game.Snapshot();
        }
        catch (Exception ex)
        {
            this.Monitor.LogOnce($"couldn't read the game state: {ex}", LogLevel.Trace);
            state = null;
        }
        if (!force && state == this.LastState)
            return;
        this.LastState = state;
        this.Bridge?.Send(new StateMessage(state));
    }

    private JObject? ReadModConfig(string modId)
    {
        IModInfo? info = this.Helper.ModRegistry.Get(modId);
        string? dir = info?.GetType().GetProperty("DirectoryPath")?.GetValue(info) as string;
        string? file = dir is null ? null : Path.Combine(dir, "config.json");
        if (file is null || !File.Exists(file))
            return null;
        try
        {
            return JObject.Parse(File.ReadAllText(file));
        }
        catch (Exception)
        {
            return null;
        }
    }
}
