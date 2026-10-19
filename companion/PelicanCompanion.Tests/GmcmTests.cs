using System.Linq;
using Newtonsoft.Json.Linq;
using Xunit;

namespace PelicanCompanion.Tests;

public class GmcmTests
{
    private class Inner
    {
        public int Count { get; set; } = 3;
    }

    private class Config
    {
        public bool Enabled { get; set; } = true;
        public Inner Section { get; set; } = new();
        public string Mode { get; set; } = "Fast";
    }

    // a mod registering its menu the way real ones do: closures over this.Config
    private class FakeMod
    {
        public Config Config = new();
        public int Saves;

        public ModConfig Register()
        {
            var menu = new ModConfig { Save = () => this.Saves++ };
            menu.Add(new SimpleModOption<bool>("Enabled", () => this.Config.Enabled, v => this.Config.Enabled = v, "Turns it on"));
            menu.Add(new SectionTitleModOption("Advanced"));
            menu.Add(new NumericModOption<int>("Count", () => this.Config.Section.Count, v => this.Config.Section.Count = v, 1, 10, 1));
            menu.Add(new ChoiceModOption<string>("Mode", () => this.Config.Mode, v => this.Config.Mode = v, new[] { "Fast", "Slow" }, v => $"{v}!"));
            return menu;
        }

        // how Pathoschild's shared GMCM helper registers options
        public ModConfig RegisterWrapped()
        {
            var menu = new ModConfig();
            this.AddCheckbox(menu, "Enabled", c => c.Enabled, (c, v) => c.Enabled = v);
            this.AddNumber(menu, "Count", c => c.Section.Count, (c, v) => c.Section.Count = v);
            return menu;
        }

        private Config GetConfig() => this.Config;

        private void AddCheckbox(ModConfig menu, string name, System.Func<Config, bool> get, System.Action<Config, bool> set) =>
            menu.Add(new SimpleModOption<bool>(name, () => get(this.GetConfig()), v => set(this.GetConfig(), v)));

        private void AddNumber(ModConfig menu, string name, System.Func<Config, int> get, System.Action<Config, int> set) =>
            menu.Add(new NumericModOption<int>(name, () => get(this.GetConfig()), v => set(this.GetConfig(), v), null, null, null));
    }

    private static readonly JObject Json = JObject.Parse(@"{""Enabled"": true, ""Section"": {""Count"": 3}, ""Mode"": ""Fast""}");

    [Fact]
    public void ReadsPathsFromGetters()
    {
        var mod = new FakeMod();
        var option = (SimpleModOption<int>)mod.Register().Pages[""].Options[2];
        var getter = (System.Delegate)typeof(SimpleModOption<int>)
            .GetField("GetValue", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!
            .GetValue(option)!;
        Assert.Equal(new[] { "Config", "Section", "Count" }, IlPath.Members(getter.Method));
        Assert.Equal(new[] { "Section", "Count" }, IlPath.Match(new[] { "Config", "section", "COUNT" }, Json));
        Assert.Null(IlPath.Match(new[] { "Config", "Section" }, Json));
        Assert.Null(IlPath.Match(new[] { "Missing" }, Json));
    }

    [Fact]
    public void DescribesMenus()
    {
        GmcmMod mod = Gmcm.DescribeOne("A.B", new FakeMod().Register(), Json);
        Assert.False(mod.TitleScreenOnly);
        Assert.Equal(3, mod.Options.Count);

        GmcmOption enabled = mod.Options[0];
        Assert.Equal(new[] { "Enabled" }, enabled.Path);
        Assert.Equal("bool", enabled.Kind);
        Assert.Equal("Turns it on", enabled.Tooltip);
        Assert.Null(enabled.Section);

        GmcmOption count = mod.Options[1];
        Assert.Equal(new[] { "Section", "Count" }, count.Path);
        Assert.Equal(("int", 1.0, 10.0, 1.0), (count.Kind, count.Min, count.Max, count.Interval));
        Assert.Equal("Advanced", count.Section);

        GmcmOption mode = mod.Options[2];
        Assert.Equal("choice", mode.Kind);
        Assert.Equal(new[] { "Fast!", "Slow!" }, mode.Choices!.Select(c => c.Label));
    }

    [Fact]
    public void FollowsWrappedGetters()
    {
        GmcmMod mod = Gmcm.DescribeOne("A.B", new FakeMod().RegisterWrapped(), Json);
        Assert.Equal(new[] { "Enabled" }, mod.Options[0].Path);
        Assert.Equal(new[] { "Section", "Count" }, mod.Options[1].Path);
    }

    [Fact]
    public void AppliesThroughTheSaveFlow()
    {
        var mod = new FakeMod();
        ModConfig menu = mod.Register();
        var next = JObject.Parse(@"{""Enabled"": false, ""Section"": {""Count"": 7}, ""Mode"": ""Slow""}");
        Gmcm.ApplyTo(menu, next, reloaded: false);
        Assert.False(mod.Config.Enabled);
        Assert.Equal(7, mod.Config.Section.Count);
        Assert.Equal("Slow", mod.Config.Mode);
        Assert.Equal(1, mod.Saves);

        // already reloaded: options refresh from the mod and must not undo it
        mod.Config.Section.Count = 9;
        Gmcm.ApplyTo(menu, Json, reloaded: true);
        Assert.Equal(9, mod.Config.Section.Count);
        Assert.Equal(2, mod.Saves);
    }
}
