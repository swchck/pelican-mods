# What's new

## 0.3.0

### Progress

- A season plan opens the report: crops of the save's season to grow and ship for polyculture, seasonal Community Center items still missing, the season's fish, and birthdays with loved gifts.

- A detailed save report in the spirit of Stardew Checkup: achievements, skills and mastery, Community Center bundles item by item, the museum with what's found but not donated, friendship with every villager, collections and polyculture, quests and special orders, Monster Slayer, the island and its upgrades, books, powers and secret notes, Grandpa's evaluation, money.
- Names come from your modded game in its language; the rules come from the 1.6 game code.
- Before the game has run through the launcher, the report counts against the base game's lists built into the launcher, in the game's language, instead of showing empty sections.
- Each section explains what it counts.

### Helpers

- A calendar of birthdays and festivals for each season, today marked from the running game or the last save, with each villager's loved gifts.
- A fish guide: what bites now by season, weather and time while the game runs, or across the season or year, where, and which fish the save hasn't caught.

- Forecasts in the spirit of Stardew Predictor: night events for the next four weeks (meteorite, fairy, witch, owl, capsule, earthquake and more), the bookseller's days this season, the traveling cart's stock on cart days, and the next mystery boxes alongside the geodes. All of it comes from the game's own code with your mods loaded.

### Mod settings

- The settings editor knows each mod's defaults: a content pack's from its schema, a C# mod's from what a fresh copy writes, seen by the companion in the game. Changed fields are marked with their default value and a reset, and everything resets at once.

### Console

- A Report tab sums up the last session's SMAPI log: mods SMAPI skipped and why, save locations and villagers the game dropped because their mod didn't load, and each mod's errors and warnings with repeats folded. Skipped incompatible mods show the update that fixes them, and one button updates them all; the mods waiting on them then load by themselves. After a session that skipped mods or lost save data, the app points to it.

### Updates

- Profiles export to a mod list file and import back: the import shows which mods you have, installs the missing ones the way dependencies install, and makes a profile with the list's mods. Lists hold names and sources only, no files.
- A mod that ships translations but none into the game's language says so in its card, with a button that searches Nexus for a translation.
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
