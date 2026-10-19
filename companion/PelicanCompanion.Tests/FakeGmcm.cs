using System;
using System.Collections.Generic;

namespace PelicanCompanion.Tests;

// Stand-ins shaped like GMCM's internal classes; Gmcm finds members by these names.

internal class ModConfig
{
    public Dictionary<string, ModConfigPage> Pages { get; } = new() { [""] = new ModConfigPage() };
    public Action Save { get; init; } = () => { };
    public bool DefaultTitleScreenOnly { get; init; }
    public bool AnyEditableInGame { get; init; } = true;
    public void Add(object option) => this.Pages[""].Options.Add(option);
}

internal class ModConfigPage
{
    public List<object> Options { get; } = new();
}

internal class SimpleModOption<T>
{
    protected readonly Func<T> GetValue;
    protected readonly Action<T> SetValue;

    public SimpleModOption(string name, Func<T> getValue, Action<T> setValue, string? tooltip = null)
    {
        this.Name = () => name;
        this.Tooltip = () => tooltip;
        this.GetValue = getValue;
        this.SetValue = setValue;
        this.Value = getValue();
    }

    public string FieldId { get; } = Guid.NewGuid().ToString("N");
    public Func<string> Name { get; }
    public Func<string?> Tooltip { get; }
    public virtual T Value { get; set; }

    public void BeforeMenuOpened() => this.Value = this.GetValue();
    public void BeforeSave() => this.SetValue(this.Value);
    public void AfterSave() { }
}

internal class NumericModOption<T> : SimpleModOption<T>
    where T : struct
{
    public NumericModOption(string name, Func<T> getValue, Action<T> setValue, T? min, T? max, T? interval)
        : base(name, getValue, setValue)
    {
        this.Minimum = min;
        this.Maximum = max;
        this.Interval = interval;
    }

    public T? Minimum { get; }
    public T? Maximum { get; }
    public T? Interval { get; }
}

internal class ChoiceModOption<T> : SimpleModOption<T>
{
    public ChoiceModOption(string name, Func<T> getValue, Action<T> setValue, T[] choices, Func<string, string>? format)
        : base(name, getValue, setValue)
    {
        this.Choices = choices;
        this.FormatChoice = format;
    }

    public T[] Choices { get; }
    public Func<string, string>? FormatChoice { get; }
}

internal class SectionTitleModOption
{
    public SectionTitleModOption(string text) => this.Name = () => text;

    public Func<string> Name { get; }
}
