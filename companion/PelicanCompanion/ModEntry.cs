using System;
using System.IO;
using System.Linq;
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

    private Bridge? Bridge;
    private Gmcm? Gmcm;
    private GameSnapshot? LastState;
    private volatile bool Resend;
    private bool Launched;

    public override void Entry(IModHelper helper)
    {
        string? url = Environment.GetEnvironmentVariable(UrlVariable);
        if (string.IsNullOrEmpty(url) || !Uri.TryCreate(url, UriKind.Absolute, out Uri? uri))
            return;
        this.Gmcm = new Gmcm(helper.ModRegistry, this.ReadModConfig);
        this.Bridge = new Bridge(uri, msg => this.Monitor.Log(msg, LogLevel.Warn));
        this.Bridge.Connected += () => this.Resend = true;
        helper.Events.GameLoop.GameLaunched += (_, _) => this.Launched = true;
        helper.Events.GameLoop.UpdateTicked += this.OnUpdateTicked;
        helper.Events.GameLoop.ReturnedToTitle += (_, _) => this.SendState(force: true);
    }

    protected override void Dispose(bool disposing)
    {
        this.Bridge?.Dispose();
        base.Dispose(disposing);
    }

    private void OnUpdateTicked(object? sender, UpdateTickedEventArgs e)
    {
        Bridge bridge = this.Bridge!;
        // GMCM menus are registered during GameLaunched, so describe them after it
        if (this.Resend && this.Launched)
        {
            this.Resend = false;
            bridge.Send(new Hello(
                Constants.ApiVersion.ToString(),
                Game.Version,
                this.Helper.ModRegistry.GetAll().Select(m => m.Manifest.UniqueID).ToList()
            ));
            this.Describe();
            this.SendState(force: true);
        }
        while (bridge.Inbox.TryDequeue(out Command? command))
            this.Handle(command);
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
