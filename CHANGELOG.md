# What's new

## 0.4.2

### Library and saves

- Saves have a History chart: total earned, money, perfection and skills by in-game day, noted on every look at a save and filled in from its snapshots.
- After each session a notice offers what changed on the farm since the game started, compared against the copy taken before launch.
- A profile can keep mod settings of its own: switching away saves every mod's config.json for it, and coming back puts them back, so "SVE hard" and "SVE cozy" can set the same mods differently.
- When a search ends on a mod, "Check what it breaks with" runs it alone, then halves the mods the search cleared with it kept on, so a problem that needs two mods names both.
- Undoing an install is itself a removal in the history, so it can be taken back too, and removing a mod sets it aside before its copy in the game's Mods folder goes to the Trash.
- The library shows each Nexus mod's picture: a thumbnail in its row and a larger one atop its details, fetched in one batch and cached for a week.

### Settings

- What the game keeps hidden until it happens — night events, geode and mystery box contents, the traveling cart's stock — now waits for a "Spoilers and predictions" setting, off by default. Luck and tomorrow's weather, which the TV tells anyway, stay.

### Helpers

- The Chests tab reads the running game live, and from saves it now finds Junimo chests and gives auto-grabbers their tile.
- A Crops tab ranks what to plant today by profit per tile until the season ends, with growth times and prices from the game, Speed-Gro, Agriculturist, Tiller and a greenhouse switch.
- With spoilers on, a Walnuts tab lists every golden walnut source on Ginger Island by area — each bush and buried spot with its tile, puzzles, Field Office donations and the capped drops with how many are left — and marks what the save has already found.

### Console

- "Check the profile" in the play menu starts the game, waits until SMAPI has loaded every mod, closes it and opens the log report: skipped mods, lost save data and errors before you sit down to play.

## 0.4.1 — 2026-10-20

### Helpers

- A Chests tab finds anything the save keeps: every chest, fridge, auto-grabber and the backpack, with how many of each and the chest's tile, read from the save file so the game needn't run. Places carry the game's own names.
- Long festival names wrap in the calendar instead of being cut off.

### Console

- A Load tab times every mod's SMAPI event handlers for 30 seconds while you play, in the running game, and shows who costs how many milliseconds a second and their worst spikes. SMAPI 4 no longer has a performance monitor of its own.
- The same tab lists game methods several mods patch with Harmony, the usual place C# mods break each other, with transpilers and prefixes that can cancel the original marked first.

### Library and saves

- A mod that doesn't speak your language can be translated in the app: its lines sit next to fields for yours, the translation goes into the mod's i18n folder, and the app keeps a copy that it puts back after the mod updates.
- Finding the mod behind a problem by halving: each round runs half the suspects, with the mods they need, in a profile of its own; you launch, say whether the problem showed, and the suspects halve until one is left. A crash in a round suggests the answer, and the search ends back in your profile. It starts from the profile menu or a crash report.
- "What changed" on the Saves screen compares two states of a farm, the day before, any copy or now: money, playtime and perfection, skill levels, hearts, firsts shipped, caught, cooked, crafted and donated, new achievements, and what came into or left the chests.

### Game link

- A morning briefing in the game: at the start of each day the HUD names whose birthday it is and which of their loved gifts you already have, a festival, the traveling cart, the bookseller and a notable luck. It can be turned off in Settings.

### Updates

- Nexus collections import from their link or code: the app shows which of the collection's mods you have, downloads the missing ones with Premium, or opens their file pages one after another without it, and installs what Nexus sends back.

## 0.4.0 — 2026-10-20

### Progress

- Community Center rooms, skills and farm buildings in the report carry the names the game itself uses in its language, read from the game; the built-in lists carry them too.

### Console

- When the game crashes, the app names the error it died on and the mod whose code is in its stack, with the stack in the report.
- The log report folds SMAPI's multi-line notices into one entry, shows the compatibility list's advice next to a skipped mod, and offers to turn off an incompatible mod that has no update, so SMAPI stops skipping it on every launch.

### Updates

- The Nexus catalog marks mods SMAPI's compatibility list calls broken, abandoned, obsolete or needing a workaround or unofficial version, with its note in the mod's page, before you install them.
- While the app is open it rechecks for mod updates every few hours and sends a macOS notification when new ones appear.

### Library and saves

- The library filters by state (problems, updates, turned off) and by your own tags: label packages in their details, then click a tag to see just those. The text filter matches tags too.
- Each save remembers the profile it was played with; when the last played save's profile isn't the active one, launching offers to switch to it first. The Saves screen shows each save's profile.
- The library keeps a history of installs, updates and removals, and any of them can be undone: a removed mod is set aside instead of deleted and comes back as it was, an update rolls back to its backup.
- A disk space card shows what the app keeps (earlier mod versions, removed mods, save copies, downloads) and cleans each up: the latest mod version and five automatic copies per save stay, and copies made by hand or before a restore are left alone.

### Settings

- The launcher remembers every game folder it has seen, found or added by hand (a Steam and a GOG copy, a beta), and Settings switches between them in a click. Mods, profiles and saves stay shared; SMAPI goes into each install.
- The app speaks every language the game ships: German, Spanish, French, Portuguese, Italian, Japanese, Korean, Chinese, Turkish and Hungarian join Russian and English, and it follows the game's language out of the box. The new translations haven't been reviewed by native speakers yet.

### Security

- Installing missing mods from a shared mod list takes them from the sources smapi.io knows; a list's own repo or update keys are used only for mods smapi.io doesn't know, and the list shows them.
- A mod file nested thousands of levels deep, a content pack including a file outside itself, or a download over 4 GB no longer crash or exhaust the app.
- The Nexus key never follows a redirect off the Nexus API, and an nxm link's one-time key stays out of error messages.

## 0.3.0 — 2026-10-20

### Progress

- A season plan opens the report: crops of the save's season to grow and ship for polyculture, seasonal Community Center items still missing, the season's fish, and birthdays with loved gifts.

- A detailed save report in the spirit of Stardew Checkup: achievements, skills and mastery, Community Center bundles item by item, the museum with what's found but not donated, friendship with every villager, collections and polyculture, quests and special orders, Monster Slayer, the island and its upgrades, books, powers and secret notes, Grandpa's evaluation, money.
- Names come from your modded game in its language; the rules come from the 1.6 game code.
- Before the game has run through the launcher, the report counts against the base game's lists built into the launcher, in the game's language, instead of showing empty sections.
- Each section explains what it counts.

### Helpers

- A calendar of birthdays and festivals for each season, today marked from the running game or the last save, with each villager's loved gifts.
- The calendar lists the week's birthdays and, with the farm loaded in the game, which of each villager's loved gifts you already have and in which chest.
- A museum helper lists the pieces the save hasn't donated and, with the farm loaded in the game, the chest each found one sits in, ready-to-carry ones first.
- A fish guide: what bites now by season, weather and time while the game runs, or across the season or year, where, and which fish the save hasn't caught.

- Forecasts in the spirit of Stardew Predictor: night events for the next four weeks (meteorite, fairy, witch, owl, capsule, earthquake and more), the bookseller's days this season, the traveling cart's stock on cart days, and the next mystery boxes alongside the geodes. All of it comes from the game's own code with your mods loaded.

### Mod settings

- The settings editor knows each mod's defaults: a content pack's from its schema, a C# mod's from what a fresh copy writes, seen by the companion in the game. Changed fields are marked with their default value and a reset, and everything resets at once.

### Console

- The report also times the game's startup, names the mods it paused on, and lists the mods loading and editing the most game assets.
- A Report tab sums up the last session's SMAPI log: mods SMAPI skipped and why, save locations and villagers the game dropped because their mod didn't load, and each mod's errors and warnings with repeats folded. Skipped incompatible mods show the update that fixes them, and one button updates them all; the mods waiting on them then load by themselves. After a session that skipped mods or lost save data, the app points to it.

### Updates

- Profiles export to a mod list file and import back: the import shows which mods you have, installs the missing ones the way dependencies install, and makes a profile with the list's mods. Lists hold names and sources only, no files.
- A co-op partner's mod list compares against yours: what you lack, versions that differ either way, and mods only you have.
- A mod that ships translations but none into the game's language says so in its card, with a button that searches Nexus for a translation.
- An update shows what's new before you install it: the GitHub release notes, or the Nexus changelog for every version since yours.
- A missing or outdated dependency installs from the mod's card in one click: the app finds it on smapi.io by its UniqueID and fetches it the way it fetches updates.

- Mods that list a GitHub repo update from its releases first, even when the newest version was found on Nexus, so they update in one click without a Nexus Premium account.

- Mods that share a Nexus page, such as Grandpa's Farm on the Stardew Valley Expanded page, update with their own file instead of the page's main one.

- A mod's card takes a GitHub repo of your own; updates are then checked and fetched from its releases first.

### Library and saves

- Before the game starts, the last played save is checked against the mods that will load: if it uses mods that are off in the profile or that SMAPI skipped last time, the app says what the save would lose and offers to turn them on first.
- Before a mod is removed, the app checks which saves still use it, by its items and data or by the locations it adds, warns what the game would drop, and backs those saves up first unless told not to.
- The library warns about content packs that replace the same asset exclusively, which makes Content Patcher load neither, before the game finds out.
- Turning a mod off asks first when saves still use it, the same way removing it does.
- Removing a mod also sends the copy in the game's Mods folder to the Trash, so it doesn't come back as importable or load when the game starts without the launcher.
- Saves can be copied to a folder of your choice, such as iCloud Drive, after each session; the 10 newest copies of each save are kept and import back as saves.
- Saves can be deleted from the Saves screen; the folder goes to the Trash.

- Item, villager and quest names in Progress and Helpers link to their wiki page; pick the official wiki, Fandom or no links in Settings.

### Settings

- The app's language follows the game's (Russian or English) unless you pick one; other game languages fall back to the system's.

## 0.2.0 — 2026-10-19

### Game link

- A SMAPI companion mod ships with the app and shows the farm, date, time, money and where the player is.
- Settings of mods with a GMCM menu open with the same names, hints, limits and sections as in the game, and apply without a restart.

### Perfection

- Progress to 100% by the 1.6 rules: shipping, obelisks, the Gold Clock, Monster Slayer, friends, level, stardrops, cooking, crafting, fish and walnuts, with what's left in each.
- Fish, recipe and item lists come from your modded game, in its language.

### Helpers

- Forecast: today's luck, tomorrow's weather and Clint's next geodes.
- Fair: the best nine items from your backpack and chests for the grange, with the score.

### Mod updates

- Nexus mods update from the app: right away with Premium, through the site's button without it.
- ModDrop mods update in one click.

### Reliability and security

- Download links from websites install a mod only after you confirm.
- Installing, removing and updating mods waits until the game is closed.
- Automatic save backups no longer push out the ones you made by hand.
- The game console costs the app less; sections load as you open them.

## 0.1.0 — 2026-10-19

The first Pelican Mods release.

### Game and SMAPI

- Finds Stardew Valley by itself and checks whether SMAPI is installed and which version.
- Installs and updates SMAPI with its official installer and offers updates on launch.
- Points out when Rosetta 2 is needed on Apple silicon Macs.
- Launches the game through SMAPI with a console that shows message levels.

### Mod library

- Mods live outside the game folder; profiles switch sets without copying files.
- Installs from .zip, .7z and .rar, by drag and drop or from the game's own folder.
- Dependency check: what's missing, turned off or outdated, with "Turn on" and "Find" shortcuts.
- Mod settings editor: a form built from config.json or a content pack's ConfigSchema, including defaults.

### Mod updates

- Update checks through smapi.io with compatibility warnings.
- One-click updates for mods hosted on GitHub, and rollback to the previous version.

### Nexus Mods

- Search with category, update date and endorsement filters.
- Mod pages with description, screenshot gallery, files and requirements.
- Downloads straight into the app; without Premium, through the site's "Mod Manager Download" button.

### Saves

- Farm list with progress: Community Center, museum, fish, recipes, friendships, skills.
- A backup before every game launch, plus restore, export and import as .zip.

### The app

- Updates itself and shows what's new in each version.
- Keyboard shortcuts for sections, the game and lists; the cheat sheet is ⌘/ or "?".
