# macrofactor-cli

A command-line interface for [MacroFactor](https://macrofactorapp.com/) nutrition tracking, built on top of [macro-factor-api](https://github.com/benthecarman/macro-factor-rs).

## Install

```bash
# From source
git clone https://github.com/kelaode-dev/macrofactor-cli.git
cd macrofactor-cli
cargo install --path .
```

## Usage

### Login

```bash
macrofactor-cli login --email you@example.com --password yourpassword
```

Saves a refresh token to `~/.config/macrofactor-cli/config.json`.

### View Data

```bash
# User profile
macrofactor-cli profile

# Current goals & TDEE
macrofactor-cli goals

# Nutrition summaries (default: today)
macrofactor-cli nutrition
macrofactor-cli nutrition --start 2025-01-01 --end 2025-01-07

# Food log for a day (default: today, shows entry IDs for delete)
macrofactor-cli food-log
macrofactor-cli food-log --date 2025-01-15

# Weight entries (default: last 7 days)
macrofactor-cli weight
macrofactor-cli weight --start 2025-01-01 --end 2025-01-31

# Step counts (default: last 7 days)
macrofactor-cli steps
```

### Search & Log Foods

```bash
# Search the food database
macrofactor-cli search-food "chicken breast"

# Log a food from search results (by index number)
macrofactor-cli log-searched-food --date 2025-01-15 --food-index 3
macrofactor-cli log-searched-food --date 2025-01-15 --food-index 3 --quantity 2.0
macrofactor-cli log-searched-food --date 2025-01-15 --food-index 3 --serving 2 --time 12:30
```

Search results are cached locally so you can reference them by index with `log-searched-food`.

### Log Data

```bash
# Quick-add a food entry
macrofactor-cli log-food --date 2025-01-15 --name "Chicken Breast" \
  --calories 165 --protein 31 --carbs 0 --fat 3.6
macrofactor-cli log-food --date 2025-01-15 --name "Snack" \
  --calories 200 --protein 10 --carbs 20 --fat 8 --time 14:30

# Log weight (kg)
macrofactor-cli log-weight --date 2025-01-15 --weight 80.5 --body-fat 15.0

# Log nutrition summary (manual import)
macrofactor-cli log-nutrition --date 2025-01-15 \
  --calories 2000 --protein 150 --carbs 200 --fat 70
```

### Delete Entries

```bash
# Delete a food entry (get entry_id from food-log output)
macrofactor-cli delete-food --date 2025-01-15 --entry-id 1705312800000000

# Delete a weight entry for a date
macrofactor-cli delete-weight --date 2025-01-15
```

### Sync Daily Totals

After adding or deleting food entries, sync the daily micro/macro summary:

```bash
macrofactor-cli sync-day --date 2025-01-15
```

### JSON Output

Add `--json` to any command for machine-readable output:

```bash
macrofactor-cli --json goals
macrofactor-cli --json nutrition --start 2025-01-01 --end 2025-01-07
macrofactor-cli --json search-food "oatmeal"
```

## Credits

Built on [macro-factor-api](https://github.com/benthecarman/macro-factor-rs) by [@benthecarman](https://github.com/benthecarman).

## License

MIT
