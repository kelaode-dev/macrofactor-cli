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

# Food log for a day (default: today)
macrofactor-cli food-log
macrofactor-cli food-log --date 2025-01-15

# Weight entries (default: last 7 days)
macrofactor-cli weight
macrofactor-cli weight --start 2025-01-01 --end 2025-01-31

# Step counts (default: last 7 days)
macrofactor-cli steps
```

### Log Data

```bash
# Log a food entry
macrofactor-cli log-food --date 2025-01-15 --name "Chicken Breast" \
  --calories 165 --protein 31 --carbs 0 --fat 3.6

# Log weight (kg)
macrofactor-cli log-weight --date 2025-01-15 --weight 80.5 --body-fat 15.0

# Log nutrition summary
macrofactor-cli log-nutrition --date 2025-01-15 \
  --calories 2000 --protein 150 --carbs 200 --fat 70
```

### JSON Output

Add `--json` to any command for machine-readable output:

```bash
macrofactor-cli --json goals
macrofactor-cli --json nutrition --start 2025-01-01 --end 2025-01-07
```

## Credits

Built on [macro-factor-api](https://github.com/benthecarman/macro-factor-rs) by [@benthecarman](https://github.com/benthecarman).

## License

MIT
